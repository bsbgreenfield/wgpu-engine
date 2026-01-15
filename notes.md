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
1. register box asset
    - This creates an asset builder in the registry, keyed by asset handle ID. Register returns asset handle
2. Asset is a gltf, so the builder is a GltfModelBuilder - call builder.with_gltf()
3.  
4. request a mesh collection component from the asset manager for the given asset handle
    - asset_manager.get_components<ExtractComponents: (MeshCollectionComponent,)>(asset_handle)
    - check the registry - if the builder for this asset has a resource (check for binary data?)
    - then 
