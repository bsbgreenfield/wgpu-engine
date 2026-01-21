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
- an entry is created in the asset registry with asset_handle: builder
2. user creates a resource backed entity by providing an asset handle 
    - an entry is created in the entity manager RBEs linking entity handle -> ResourceBacking(asset handle, index) 
3. the entity is added to the scene using the entity handle
4. the user activates the scene
5. the asset manager loads the asset associated with the RBE with a residency level of GPU
- the asset manager calls load(ResLevel::GPU) on the correct asset_buider 
- the asset builder writes vertex and index data into the appropriate CPUMeshPool
- the asset builder creates a LoadedAsset entry with vertex and index offsets





TODO: add models(?), meshes, primitives to gtlf_assets becauase its the only logical way to track the vertex / index offsets 
for the mesh collections

load_model() should attempt to load all possible components from gltf (for now) and needs to create a loaded asset entry that contains 
mesh collections, which in turn contain AT LEAST mesh id, local transforms, primitives per mesh(with vetex index offsets)
