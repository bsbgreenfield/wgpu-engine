# Asset Upload Path

How an asset travels from a source file to a GPU allocation the renderer can draw from, and how
materials and textures behave differently depending on whether they are **embedded** (data owned by
another asset's file) or **external** (their own file, their own identity).

## Two axes, four combinations

Materials and textures are independently embedded or external:

- an **embedded material** is one of the material records in a gltf's own `material_palette`
- an **external material** is a `MaterialAsset` registered from its own file
- an **embedded texture** is pixels inside some asset's binary — a gltf buffer view, or an image
  carried by a material file
- an **external texture** is a `TextureAsset` registered from its own image file

An external material may sample an embedded texture; an embedded material may sample an external one.
All four combinations are legal and travel the same path.

## Identities

| type | who has one | what it names |
| --- | --- | --- |
| `AssetHandle` | every *registered* asset | a thing with a residency and a load lifecycle |
| `GPUAllocationHandle` | every asset that reaches the GPU | that asset's allocation, one per arena |

Embedded resources have neither. An embedded texture has no `AssetHandle`, no residency, and no ack;
it lives and dies with the allocation of the asset that carries it. This is the single fact that
explains every difference below.

Because a `GPUAllocationHandle` is the key into each arena's `AllocationTable`, an asset has **at
most one allocation per arena**: one PNU range, one PNUJW range, one index range, one material
palette range.

## The invariant

> An asset enters `PendingGPU` only once every asset it depends on is `AssetResidency::GPU`.

Everything downstream follows from this. By the time an upload job is built, every external resource
it names is already resident and resolvable. No upload waits on another upload; an ack is bookkeeping
that flows back to the world, never a rendezvous between two uploads.

---

## Phase 0 — Registration

`world.register_asset::<A>(source)`:

1. `A::new(source)` parses headers only. For a gltf this yields
   `UnloadedAssetData::Gltf { gltf, sources: AssetSources { binary_sources, textures } }` — no binary
   is read yet.
2. The manager mints an `AssetHandle` and inserts `RegisteredAsset::Unloaded`.
3. It walks the asset's declared external references and interns each one:
   - `TextureSource::ExternalFile(path)` → canonicalize, then
     `texture_registry.registered_textures.entry(path).or_insert_with(|| register a TextureAsset)`.
     The edge is recorded in `dependencies[owner].push(texture)`.
   - `TextureSource::BinarySource(_)` contributes nothing — no handle, no registry entry, no edge.
     Those are bytes owned by the gltf.
   - A material file registers its own texture references by the same walk.
4. A `ResourceBacking<A>` comes back for the caller to hang on a component.

Registration is the **only** place the texture registry and the dependency table are mutated, and it
runs while every asset is still `Registered`. Two consequences:

- the full dependency DAG is known before anything loads, so the gate in Phase 4 can be a pure lookup
- `UnloadedAssetData::load` takes `&AssetManager` and only *reads* the registry — resolving an
  external reference during a load is never necessary and never possible

Registering the same texture path twice returns the same handle. That interning is what makes two
models sharing one texture share one upload.

## Phase 1 — Declaration

An entity declares which palette and which textures it wants:

```rust
MeshCollectionDescriptor::new(fox_gltf.into(), ComponentAccessor::All)
    .with_material(MaterialComponentDescriptor::Embedded {
        texture: Some(MaterialTextureSource::External(fox_texture.into())),
    })
```

- `MaterialComponentDescriptor::Embedded` — the `MaterialPalleteComponent`'s resource backing is the
  *owner's* handle; the palette is the gltf's own.
- `MaterialComponentDescriptor::External { resource_backing, .. }` — the backing is a separate
  material asset; the gltf's own palette is unused for those slots.
- `MaterialTextureSource::Embedded` — sample the texture the material's own asset carries.
- `MaterialTextureSource::External(rb)` — sample a standalone `TextureAsset`.

`rbcs_of(entity)` returns the transitive closure of the component handles through `dependencies`, so
textures and external material assets are first-class nodes in the dependency graph with demand
counters of their own.

## Phase 2 — Demand

`scene_manager.set_load_level(scene, GPU)` calls `dependency_graph.recompute_asset_levels`, which
bumps `AssetDemand.gpu` on every asset in the closure. Each asset currently below the requested level
is written into `asset_requests` and counted into `pending[scene]`. `world.update` drains
`asset_requests` into `LoadQueue::add_load_job`.

Demand is per asset, not per owner. A texture used by three entities across two scenes carries its
own demand and stays resident while any holder still wants it.

## Phase 3 — CPU load

`LoadQueue::poll_jobs` runs every frame and keeps each job until `current == target`.

`UnloadedAssetData::load` produces:

- **`TextureAsset`** — the image file decoded once into
  `GPUTextureData { width, height, srgb, pixels: Arc<[u8]> }` (rgba8). The decoded pixels are held
  behind an `Arc` and cloned into each upload job, so loading is non-destructive and an
  unload/reload cycle re-uploads without re-decoding.
- **`GltfAsset`** — binary read; vertices, indices, node tree, skins and animations built; and
  `material_palette: Arc<[GltfMaterial]>` built. Per material base-color texture:
  - `gltf::image::Source::View` → decoded from the binary into `GltfTexture::Embedded(Arc<GPUTextureData>)`
  - `gltf::image::Source::Uri` → `GltfTexture::External(AssetHandle)`, looked up in the texture
    registry by canonical path. The lookup always succeeds, because registration interned it.

  Each primitive records the index of its material within the palette.
- **`MaterialAsset`** — one or more `GPUMaterialData` records plus the same per-record
  `Embedded | External` texture binding. Structurally identical to a gltf's palette; only the owner
  differs.

## Phase 4 — The dependency gate

Before requesting `GPU` for an asset, `poll_jobs` checks `asset_manager.deps_gpu_ready(handle)` —
true when every entry of `dependencies[handle]` is `AssetResidency::GPU`. If it is not ready, the
queue requests `CPU` this frame and leaves the job in place. Jobs persist across frames, so the check
re-runs next frame at no cost and needs no separate wait list.

The result is a topological upload order with no explicit sort, and one that does not depend on the
iteration order of the job map:

| frame | reaches `PendingGPU` |
| --- | --- |
| N | leaves — standalone textures, which have no dependencies |
| N+1 | material assets whose textures are now resident |
| N+2 | assets that name those material assets in their own upload jobs |

An asset with no external references skips the wait entirely and reaches `PendingGPU` on its first
poll.

Note what the gate is scoped to: an asset waits only on what its **own upload job names**, not on
everything the entity needs. A gltf paired at declaration time with an external material asset does
not wait for it — the gltf's palette is empty, so there is nothing to resolve. The two upload
independently, and the scene's `pending` counter is what holds the spawn back until both are
resident.

## Phase 5 — Upload job construction

`world.update` sees `transition.new == PendingGPU`, calls `asset_manager.get_upload_job_for(handle)`,
and pushes `WorldUpdateDelta::AssetDidLoad(job)`.

Every palette in a job is one flat run of records plus one binding per record:

```rust
struct MaterialPaletteJob {
    records:  Vec<GPUMaterialData>,   // contiguous; uploaded as a single blob
    bindings: Vec<TextureBinding>,    // parallel to records
}

enum TextureBinding {
    None,                             // sample the default 1x1 white
    Embedded(Arc<GPUTextureData>),    // pixels ride along with the job
    Resolved(GPUAllocationHandle),    // external texture, already GPU-resident
}
```

`TextureBinding::Resolved` is where the gate pays off: `get_upload_job` turns
`GltfTexture::External(asset_handle)` into a `GPUAllocationHandle` via `alloc_handle_of`, which
cannot fail here precisely because the gate held.

The three job shapes:

| asset | job |
| --- | --- |
| standalone texture | `TextureData { asset_handle, data: Arc<GPUTextureData> }` |
| gltf | `ModelData { asset_handle, pnu_vertices, pnujw_vertices, indices, palette }` |
| external material | `MaterialData { asset_handle, palette }` |

A gltf whose entity uses only external materials carries an empty palette.

Embedded and external differ in exactly two places at this layer: **which `asset_handle` the palette
uploads under**, and **whether pixels or an alloc handle ride in the binding**. Everything downstream
is identical.

## Phase 6 — Bytecode

`gen_bytecode` walks the frame's deltas, emitting instructions plus constants that borrow from the
deltas for the frame.

`ModelData`:

```
AddAsset        <key const>       ; push asset key, push a fresh GPUAllocationHandle
PNUUpload       <data const>      ; pop alloc, upload, push alloc
PNUJWUpload     <data const>
IndexUpload     <data const>
MaterialUpload  <palette const>   ; emitted only when the palette is non-empty
EmitAssetUpload                   ; pop alloc, pop key -> AssetGPULoaded
```

`MaterialData` is the same shape without the mesh ops:

```
AddAsset        <key const>
MaterialUpload  <palette const>
EmitAssetUpload
```

`TextureData` needs nothing on the stack — the op mints its own allocation:

```
TextureUpload   <key const> <data const>   ; -> TextureGPULoaded
```

Two rules hold across every program:

- **Embedded resources emit no instruction of their own.** An embedded texture is uploaded inside
  `MaterialUpload`; an embedded palette is uploaded under its owner's allocation.
- **Exactly one `Emit*` per `AssetHandle`.** An asset acks once.

## Phase 7 — VM execution

`MaterialUpload` pops the asset's `GPUAllocationHandle`, then resolves every record's texture before
writing anything:

| binding | what the VM does | slot |
| --- | --- | --- |
| `None` | — | bucket 0, layer 0 — the default 1×1 white |
| `Embedded(data)` | uploads the pixels now, recording the layer as owned by this allocation | the fresh (bucket, layer) |
| `Resolved(alloc)` | `texture_arena.alloc_table.resolve(&alloc)` | the existing (bucket, layer) |

Each slot is packed into that record's `tex_modifier`, and the whole `records` run is then written
with a single `gpu_alloc` — one node, one slot in the material arena's `AllocationTable`, keyed by
the asset's alloc handle. The palette's base index is
`resolve_byte_offset(&alloc) / size_of::<GPUMaterialData>()`.

The palette must be a single contiguous upload: one handle maps to one allocation per arena, so N
separate uploads under one handle would leave only the last resolvable.

Texture writes go through `TextureArena`, which buckets by dimension and hands out array layers
within a bucket. The first upload into a new bucket triggers a rebuild of the material bind group,
which exposes every live bucket at once — so no draw ordering depends on texture size.

Layer ownership follows the binding kind:

- an **embedded** texture's layer is recorded against the material palette's allocation and freed
  with it
- an **external** texture's layer is recorded against the texture asset's own allocation and freed
  only when that asset unloads

`TextureUpload` mints an allocation, writes the layer, registers it in the texture arena's alloc
table, and pushes `RenderUpdateDelta::TextureGPULoaded`.

## Phase 8 — Acknowledgement

`post_frame_update` consumes the VM's deltas:

- `AssetGPULoaded { key, alloc_handle }` → `register_asset_gpu_residency`, moving
  `PendingGPU(la) → GPU(alloc, la)`. Emitted for mesh assets and external material assets.
- `TextureGPULoaded { key, alloc_handle }` → the same transition for a standalone texture asset.
- Embedded materials and embedded textures produce nothing. They have no `AssetHandle` and no
  residency; their owner's single ack covers them.

`register_asset_gpu_residency` accepts only an asset in `PendingGPU`, which is what makes
one-ack-per-handle an enforced rule rather than a convention — a duplicate ack is an error, not a
silent overwrite.

The recorded `GPUAllocationHandle` is the asset's permanent GPU identity for the rest of its
residency: it resolves vertex ranges, index ranges and the material palette base, and it is the key
`DespawnAsset` frees.

`on_asset_level_changed` then decrements the holder scene's `pending` count. At zero the scene flips
to its requested level and its queued spawns run.

## Phase 9 — Draw-time resolution

`get_entity_render_data` builds a `RenderView` per mesh source. Alongside the mesh alloc handle,
every primitive carries a material reference resolved from the component:

- **embedded palette** → `(owner_alloc_handle, primitive's index within the gltf palette)`
- **external material** → `(material_asset_alloc_handle, index within that asset's palette)`

`gen_draw_calls` copies it into `DrawItem.material`. At draw time the renderer computes
`resolve_byte_offset(&handle) / stride + local_index` and pushes it as an immediate beside `lt_idx`.
The fragment shader reads that record, unpacks `tex_modifier` into bucket and layer, and samples the
matching array.

Nothing here requires the palette to be contiguous with the mesh or with any other palette. Because
the index is absolute and per draw, an entity whose palette is assembled from several external
material assets draws exactly like one using a single embedded palette.

## Unload

Demand drops to zero → residency `GPU → PendingUnloadGPU` → `AssetUnload` delta → `DespawnAsset` op →
the arenas free everything keyed by that alloc handle (vertex range, index range, material palette,
and every texture layer owned by that allocation) → `AssetUnloaded` ack → residency `CPU`.

External textures unload on their own schedule: a texture stays resident while any holder's demand is
non-zero, so unloading one of three models that share it frees nothing. Embedded textures have no
independent demand and are freed with their owner's palette allocation.

---

## Summary of the four combinations

| material | texture | handles | upload order |
| --- | --- | --- | --- |
| embedded | embedded | 1 | one `ModelData` carrying the pixels; single frame |
| embedded | external | 2 | texture first; the gltf waits one frame for it |
| external | embedded | 2 | independent — both upload in the same frame |
| external | external | 3 | texture, then the material one frame later; the gltf is independent of both |

## Worked example: the fox

`Fox.gltf` declares `"Texture.png"` as an image uri. The entity declares an embedded material with an
external texture.

**Registration.** The gltf takes handle 0. Walking `sources.textures` interns
`res/textures/Texture.png` as handle 1 and records `dependencies[0] = [1]`.

**Scene set to GPU.** Both handles gain GPU demand; `pending[scene] = 2`.

**Frame 1.** Handle 1 has no dependencies, so it goes `Registered → CPU → PendingGPU` and emits a
`TextureData` job. Handle 0's gate fails, so it advances to `CPU` only, decoding its palette into one
record bound to `GltfTexture::External(1)`. The VM runs `TextureUpload`: 1024×1024 lands in the 1024
bucket at layer 0, the bucket's chunk is created, and the bind group is rebuilt. The ack moves
handle 1 to `GPU(alloc A)`.

**Frame 2.** Handle 0's gate now passes, so it moves to `PendingGPU` and emits `ModelData` with a
one-record palette bound to `Resolved(A)`. The VM runs `AddAsset` (alloc B), the three mesh uploads,
then `MaterialUpload`, which resolves A to (bucket 4, layer 0), packs it into `tex_modifier`, and
writes the single record under B. `EmitAssetUpload` acks handle 0 to `GPU(alloc B)`. `pending` hits
zero, the scene reaches GPU, and the queued spawn runs.

**Frame 3.** The entity draws. Its `RenderView` carries alloc B; each primitive's material resolves
to `base(B) + 0`, and the fragment shader samples bucket 4, layer 0.
