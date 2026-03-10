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
    - call asset manager.set_minumum_load_level()
4. for each asset
    - asset manager will immediately return the asset load state  
        - pending CPU
        - CPU
        - pending GPU
        - GPU
    - if PendingCPU is returned, add 
