1. register asset in the asset manager
2. add asset to the scene
3. mark scene as active
4. renderer loops through the entities in the scene, looking for renderable components
5. renderer finds a mesh collection component with a new vertex/index type combo
6. renderer creates a new RenderGroup with the new pipeline and shoots a request to the asset manager to begin loading the relevant asset
7. each frame, the renderer polls the asset manager to see if the data is gpu resident yet, and if so, it grabs the offset data and inserts it into the relevant render group
8. the renderer begins issuing draw calls for the asset.


If the user wants to ready the data by making it CPU resident without actually adding it to the GPU buffer,
they can add a message to the event queue so that the asset manager can begin loading that data.

If for some reason the renderer requests GPU residency for an asset that is actively being loaded to the CPU, we need the asset manager to handle that

## rendering a box
1. user registers the asset
- an entry is created in the asset registry with asset_handle: builder, and a list of components
2. user creates a resource backed entity by providing an asset handle 
    - an entry is created in the entity manager RBEs linking entity handle -> ResourceBacking(asset handle, index) 
3. the entity is added to the scene using the entity handle
4. the user activates the scene
5. the asset manager loads the asset associated with the RBE with a residency level of GPU
    - the asset manager calls load(ResLevel::GPU) on the correct asset_buider 
    - the asset builder writes vertex and index data into the appropriate CPUMeshPool
    - the asset builder creates a LoadedAsset entry with {MeshCollectionComponent: Vec[entry] } 
6. the asset manager creates and set the GPU buffer from the CPU data (in future will be pool)
7. the entity manager adds data for the mesh collection component and links it to the entity
8. the renderer attempts to create a render view for the newly actived entity
9. the renderer sees that there is no associated group, so it creates one
    - it uses the GPU vertex and index buffers in the asset manager for the correct vertex/index type
10. the renderer creates the render view by looping through the mesh collection component associated with the entity
    - for each mesh in the mesh collection
    - for each primitive in the mesh
    - create draw item from mesh id, and buffer slice created from buffer ref in group 
    and offsets defined by the primitive





TODO: add models(?), meshes, primitives to gtlf_assets becauase its the only logical way to track the vertex / index offsets 
for the mesh collections

load_model() should attempt to load all possible components from gltf (for now) and needs to create a loaded asset entry that contains 
mesh collections, which in turn contain AT LEAST mesh id, local transforms, primitives per mesh(with vertex index offsets)


## Buffer Arenas
The issue that I am struggling with is how to manage adding new data to a render pass.

Use case: the player approaches a group of entites whose data has been loaded to the CPU. We need to get that data into a vertex buffer 
so that the render can start issuing draw calls.

There are a few things that are happening behind the scenes here.
1. The group of entites needs to be marked as "visible" or "active"
2. the render needs to make new "draw items" which represent the new entites. This means it needs a buffer slice for vertex and index, 
which in turn need offset values
3. In order to obtain offset values, we need to have written data into some free slot in the vertex buffer

The idea is to have a buffer arena. the vertex and index buffer for a certain vertex / index format will be initialized with a reasonable 
capacity to handle new asset loads. When an entity needs to be loaded, we use queue.write_buffer() using the cpu data in the asset manager
which slots the data into some available space in the arena. Once that operation is (succesfully) completed, we generate draw items for the renderer.

With this method, we wont actually know the offsets until the gpu upload is actually completed (or at least started) 
so there isnt any point to managing offset data within the cpu. All we need to add an asset is a Vec<Vertex> and Vec<Index>


## Event queues
### Timeline of loading an asset into the world

1. set scene load level gpu - set scene dirty
2. world update: see that the scene is dirty, loop through scene events, process "load entities" command
3. for each entity
    - get all assets associated with the entity
    - call asset manager.set_minumum_load_level() on each asset
4. for each asset
    - asset manager will immediately return the asset load state  
        - pending CPU
        - CPU
        - pending GPU
        - GPU
    - if PendingCPU is returned, add delta "AssetDidLoad"
    - generate bytecode instructions to load the asset to the GPU
5. In the GPU, generate a global allocation ID. Generally speaking, we want one global alloc id per Asset, until some sort of asset
composition is implemented
6. using the loaded asset ref, split up the data into discrete things that need to be uploaded
    - these "mesh jobs" are unique per pipeline, and they contain
    local mesh ids, as well as primitive ranges which correspond to the 

7. emit a render delta event "AssetGPULoaded" that contains the GPUAllocationHandle for the asset
    - register this asset as gpu resident in the asset manager
8. next frame, loop through the entities that were queued to be loaded
    - The queue will observe that the GPU resident asset is "done"
    - once all assets are done, emit an world update delta event "EntityDidLoad"
    - encode commands for renderer to generate draw calls for the entity


In order to find the correct set of draw calls that correspond to a mesh collection
- for each pipeline, use the global allocation id to get the set of all draw calls for the mesh collection
- then, for each draw call in the set, select only the draw calls that have a mesh id which matches
one of the mesh ids in the mesh collection

The data containing the indexes of the draw calls to use should be collected in a structure "RenderView"
The renderviews must be organized by pipeline/render category, so they can put into a "RenderGroup"

render groups will map allocation id to Vec<RenderView>

when rendering, for each pipeline 
- locate the correct render group for the current set pipeline (1) 
- for each global alloc id in the group (2)
- for each render view associated with the global alloc id (3)
- for each range of draw indices in the render view (4)
- for each draw item index in the range (5)
    - get the draw item
    - resolve lt index with GAI and local mesh id, set immediates
    - do material stuff
    - draw using local offsets for he allocation



## Allocating an asset in the GPU
The payload needed for a gpu allocation right now is the "LoadedAsset"
```rust
    pub struct XXLoadedAsset {
        pub handle: AssetHandle,
        pub gltf_mesh_data: GltfLoadResult,
    }
```
where
```rust
    pub struct GltfLoadResult {
        pub pnujw_vertices: Vec<PNUJWVertex>,
        pub pnu_vertices: Vec<PNUVertex>,
        pub indices: Vec<u16>,
        pub local_transforms: Vec<LocalTransform>,
        pub mesh_data: Vec<GltfMeshData>,
    }
```
Each Operation::AddAsset indicates a unique "Allocation".
An Allocation is basically a reference frame, or even namespace for asset data. Within this namespace,
allocations can define mesh ids, local transform indices, vertex offsets, and anything else that refers to "local" asset data.

Allocations are unique, meaning they can only be defined once for their lifespan. All references to local asset data, like mentioned above, must exist only for the lifetime of the allocation

To create an allocation, The renderer VM generates a unique ID, then "UploadJob"s are created as distinct units to upload the data.

We need to split up the data like this because an asset may need to store its data in many different locations in in GPU memory. For example, 
an asset may have a mesh, and the vertices on that mesh may reference uv coordinates on a texture, which is also defined (or referenced) on that asset.

In this case, the vertices and indices of the mesh must be stored in a vertex buffer, and the texture must be stored in a texture buffer, but the data that we insert into the vertex buffer and the texture buffer are still related; They belong to the same allocation. 

To accomplish this persistent association between disparate data and and the global concept of an "allocation", the above mentioned Global allocation id is supplied to the various "Upload Jobs", before the latter are actually sent off to be uploaded.

The data is routed to the proper allocator to actually store the data. The allocator is free to move this data around however it sees fit, so we cant rely on any static indices or offsets to reference the data inside of the arena. 

Instead, after uploading, the allocator provides the caller with an "Allocation Handle". This handle must be used to retrieve the data for a specific allocation. For each pipeline that must use the data located within the arena, we store this Allocation Handle in a "Draw Map" 

TODO!!!! see about removing the global alloc id from the allocator specific handles




## CHANGE TO UPLOAD MESH

upload mesh jobs should only be the raw data payload needed to allocate into that arena

when we go to "active" a mesh collection only THEN do we use the mesh_ids_and_prim_ranges_of() function to resolve the 
primitive ranges for the mesh ids specified in the MeshCollection

For a given entity that needs to be uploaded, we construct a RENDERVIEW that has

- Global allocation ID
- PNUJW mesh ids[N]    |  Vec<PNUJWDrawItems>
- PNUJW prim ranges[N] |

- PNU mesh ids[J]       | Vec<PNUDrawItems>
- PNU prim ranges[J]    |

- material, textures, etc

when you go to draw, its 
```rust
for view in render_views {
    let lt_index_range = local_transform_arena.resolve(view.alloc_handle);
    // resolve materials
    // resolve texures
    for category in pass.render_categories {
        match category {
            PNUJW => {
                set pnjuw pipeline;
                let (allocation_range, buffer) = static_arena.resolve(view.alloc_handle);
                for draw in view.pnujw_draw_items {
                    pass.set_immediates(0, lt_index_range.0.start + draw.mesh_id)
                    pass.draw(draw.within(allocation_range))
                } 
            }
            PNU => {
                // SAME AS ABOVE
            }
        } 
    }
}

```

In the future, if we want to drop the CPU loaded asset from memory, we can do that, but we would probably need to 
create a function which CONSUMES the loaded_asset and creates a bunch of render views




## loading an entity vs "spawning"

Loading an entity is the process of making all of its assets GPU resident. These assets contain exclusively STATIC data.

The vertices, indices, local transforms, textures, materials, that belong to an entity do not change, even if they are modified by things like animations, and shading 
this is not so for "instance" data. This is data that make sense only in the context of the game world. 

Therefore, while "loading" an entity refers to the loading of static asset data, "spawning" refers to actually instantiating it within the world

The world needs a way to keep track of "spawned" entities, and differentiate them from entities that are simply registered but not spawned. 

The concept of "spawning" is also deeply tied to the concept of RenderGroups and RenderViews. A RenderGroup must exist for an entity if and only if that entity has been spawned into the world

The fact that these two things - a world space abstraction of a spawned entity, and the renderer's RenderGroup-  can (and must) exist simultaneously neccesitates a single source of truth for the actualy "state" of these spanwed entities

The obvious choice here is for the World to be the source of truth for this state. The Renderer should simply react to and try to represent the current state of the world.

When an entity is spawned, it must spawned at a world location, so there must be an associated transform. There also must be a unique ID for the new instance so that we can keep track over the frames. The combination
of InstanceHandle and world transform (along with other stuff, instance count, instance state?) is the Instance Data for a particular instance. This data must be kept in sync with the GPU representation of these values.

### lifecycle of an entity spawn

1. Add the entity handle to a list of active Instances on the World. An InstanceHandle is created. (this will later need to be changed to account for defragmentation of instance data as it moves around)

2. The world transform for the instance is inserted into an array which can be accessed using the InstanceHandle

3. Renderer VM bytecode is generated containing Renderable information (mesh collections, etc.) as well as InstanceData (handle and transform) 

4. The renderer creates a rendergroup which stores the InstanceHandle as well as the RenderViews (basically references to the static Renderable data of the entity). In the future we will probably want RenderGroups to be able to share RenderViews

5. The renderer uses the InstanceHandle to write the transform into the global transform buffer

6. When rendering, we obtain the instance index / range with a call to global_transform_arena.resolve(group.instance_handle) which is later used in render_pass.draw(). 

If a new instance is created of a new entity, the renderer creates a new render group. If the instance is destroyed, the renderer deletes the rendergroup and the global_transform_arena dealloates.
For each frame, when instances move, a staging buffer is written to, and the renderer swaps in the new buffer. Because the global_transform_arena is responsible for associating instance handles (which are static) with dynamic 
instance indices/ranges, this is just a matter of the arena resolve() algorithm




## life cycle of an entity
1. entity is created, components are added.
2. entity is added to a scene
3. the load level of the scene is set to GPU, marked dirty
4. on world.update(), world processes SceneEvent::LoadLevelChanged
5. for each entity in the scene, get all resource baceked assets for the entity
6. for each asset in the new entity load job, poll asset load 
    - call asset_manager.set_minumum_load_level() to get AssetLoadResult
    - if the load result is equal to or greater than the expected load level of the job, set asset job state to Done
    - if the asset load result is PendingGPU, add a world update delta to signal the GPU to allocated static asset data
7. for each assetDidLoad event, allocate on the GPU. Submit AssetGPULoaded event when done
8. in the post frame update, set the state to GPU loaded for the assets in the asset manager
9. Next frame, poll entity jobs. For each completed entity job, add entity handle of the completed entity job to completed queue, emit an EntityDidSpawnEvent
10. dequeue the completed entity load jobs, and call World::spawn(), emit EntityDidSpawn 
11. in the renderer,  create a render group wich represents the instnace of the entity



currently im when i process a gltf file I store the mesh data PER MODEL (root node), but when i store the data in the loaded asset, i flatten out the list of meshes into a single vec and then allocate based off of that.

This works if there is only a single model, but if there are multiple models, then the LOCAL TRANSFORMS thhat apply to the list of meshes will be unique to that model, even if the meshes themselves are not, or even if the order of the meshes is the same. 

Say we have two models in a single gltf file, and their meshes, as we traverse the node tree, end up  being

{1, 2, 3} and {1, 2, 3} respectively. and the local transforms for the meshes are

{x, y, z} {x, t, f}

So this means the the meshes are the same, but they are arranged differently, so maybe a guy holding a sword above his head and a guy holding a sword below his head. The meshes also may have different textures or materials applied alond with the different transforms.

When we allocated into the local transform buffer, we obviously need to allocate for at least every unique local transform. It would be an optimization for later to try and dedepulicate like transforms, but really its probably safe to assume that each model can have a unique allocation for the set of transforms because they will probably in fact be unique.


So in the local transform buffer we need 

[x, y, z, x, t, f]

for the allocation. Now the difficulty is that this is all one allocation , so it has a single gpu allocation handle. and as the mesh IDS themselves are the same between the two models, we cant use them to index into the buffer.

I want to keep the RenderView as a representation of a single allocation in a for a single pipeline, so the rendere view wouldlook like 
```rust

RenderView {
    gpu_handle: 0,
    pnujw_draws: DrawSet {
            mesh_ids: [ 1, 2, 3, 1, 2, 3 ],
            primitive_ranges: [r1, r2, r3, r1, r2, r3],
            index_ranges: [i1, i2, i3, i1, i2, i3],
    }
    ///pnu draws not included
}
```

So two problems here: 
1. primitive ranges and index ranges are unecessarily duplicated. There should be one entry PER mesh id.
SOLUTION: there should only be one primitive range per mesh index. We index into the primitive range with the mesh id
 so if we have meshes [1, 2, 3, 1, 4], prim ranges should be [r1, r2, r3, r4]

2. there are no local transform indices
SOLUTION: when rendering, we assume that each draw item is arranged in the same order that the local transforms were placed in. Therefore, all we need, is for each draw entry, we store a LT_OFFSET, which is the index of the first local transorm for the current allocation. Then

```rust
for each (i, draw ) in draw_entry.1.iter().enumurate() {
    render_pass.set_immediates(0, bytemuck::cast_slice(&[draw_entry.lt_offset + i]));
    render_pass.draw();
}

```


## New process

1. scene is added to the world
2. The assets of the scene are loaded by the asset manager, WorldUpdatgeDelta::AssetDidLoad emitted
3. GPU upload job is passed to the renderer, vertices and indices are uploaded to the proper GPU buffers
4. in post frame update, asset is marked as GPU loaded, next frame (or when all assets are GPU loaded), call World::spawn()
5.





1. make sure that instance handles instance idx are stable into the archetyp table
2. use the instance handle to ge the index of the instance for the draw calls
3. upload the local transform data using the instance handle, so that it can be resolved using the instance handle


## generating draw calls

The app hangs on to a DrawPacket into which the draws for each frame are written

for each vertex type (pipeline currently), the draw packet contains
- a list of Draw items, sorted by unique VertexBuffer / IndexBuffer combination
- A cache which maps GPUAllocHandle id -> vertex / index buffer allocation ranges and the index of the sorted group of draws


Every frame:
- for each render group (entity)
- for each render view within the render group (unique asset allocation per entity)
- grab the alloc ranges and index of draw group per its unique vertex buffer index buffer combo
- clear the draw data for each draw that belongs to that group
- for each instance in the render group
- use the instance handle to find the index of the instance within GPU buffers that have data for each instance
- for each mesh, write a DrawItem that contains the vertex range, index range, and instance id





### new system
Draw calls need to be organized per pipeline first, and per vertex/ index buffer second

Each mesh within an instance already knows its primitive offsets within the allocation, the only hard part is to resolve the correct allocation range within which these primitive offsets are valid. 


Proposal: each DrawItems are grouped by global alloction ID directly, the sorting for this happens in the gen draw calls function, and now the draw items only contain their relative primitive offsets. in the render function the renderer uses the global allocation ID of the group of draw items to query the proper vertex/ index arena to set an "offset" variable/ variables to add to the primitive offset ranges specified by each draw item.

The vertex arenas keep their own cache of alloc handle -> alloc range. If this has not changed, it immediately can return the range when the renderer asks for it, but if it has been invalidated, then it will create a new cahce entry and return after that


## possible new scene load setup

1. add scene to world
2. call scene.spawn(archetype data)
3. world update, sees that the scene is dirty, add scene load job to queue
    - this loads all the assets needed for a scene, polling every frame until complete
4. world.spawn() adds an instance of the entity to the instance manager
    - this populates the archetype table for that entity instance
5. instance manager creates  and stores a RenderGroup and RenderViews that holds the relative primitive ranges for every instance for this entity
6. instance manager generates bytecode for the renderer to update its gpu buffers
7. using the render groups and the archetype table data, instance manager populates the DrawPacket with draw calls





### What i was doing last time
To create new animation data for spawning instances we need to do a similar thing to waht we are already doing with 
the mesh data. If its a brand new instance we need the Vec<Arc<Animation>> and a buffer_slot_map to map the mesh ids to their offsets within the GPU local transform buffer.

If its not a new instance, there should be an entry in the AnimationController that contains the needed data for that entity already.
So when generating Renderables for an entity spawn, we need to figure out what data is actaully needed for the animation.

NOTE: the buffer_slot_map can only be known at SPAWN time because the entity will define a subset of all the root nodes in a gltf, so what order the local transforms will be in in the GPU will depend on that.

To this end i started to think of better ways to gather data requirements for entity uploads than the current system

basically they are this:

1. Mesh + Animation + new instance
needs:
    - mesh renderable data
    - local transform data
    - animation refs
    - buffer slot map

2. Mesh + animation + rigid shared mode + old instance
needs:
    - nothing

3. Mesh + animation + independant mode + old instance
needs: 
    - local transform data (can we just copy this in the gpu?)

4. Mesh new instance
needs: 
    - mesh renderable data
    - local transform data

5. mesh old instance shared mode
needs: 
    - nothing

6. mesh old instance independant mode
needs:
    - local transform data 
