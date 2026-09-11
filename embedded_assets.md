## lifecycle of an asset with embedded

1. the user defines the required embedded for an entity by specifying what it wants and where the data lives
- for a material, this is .with_material(material_embedded(external texture rb))
- a mesh collection component and a material component is stored under this entity handle in the entity manager

2. any embedded components will share the same entity handle as its owner, so set_min_load_level only ever gets called for the owner
- in this case, thats the owning mesh_collection_component asset

3. 
a. when the asset is loaded, it unconditionally loads all of its data, regardless of components
- the gltf asset loads its embedded materials 

b. If there are any external references inside the asset, those references are replaced with asset handles
and those assets are loaded (not sure how to do this)
- at this point, the gltf is cpu loaded and the texture is cpu loaded, and the gltf has an asset handle for the texture

4. the load queue sees that the main asset dependency is pending gpu, so it calls get_upload job
- for the gltf, this results in a mesh collection upload job and a upload jobs for all of its materials


5. any embedded assets at this point will generate their own upload jobs
- the material palette at this point can include in the upload job its own data, and also fetch the cpu loaded data
for the textures using the assets handles they have stored.
- if the texture has already been uploaded at this point, its just an acquire, but otherwise, the data is actually sent as a part of the material upload job

6. the vm does the GPU uploading for the all the jobs it receives
- for the materials, this includes an upload for the texture data, and the vm can at this point store the texture layer 
in the actual material storage buffer

// TODO: at this point we need to figure out what the rendererer is sending back, and how that is transformed into a 
// render view of materials that may or may not be of the same allocation as its owner

7. the renderer sends back a render update delta for the main owning component, and then the embedded components,
but the embedded upload ack has the same asset handle key and the same gpu allocation

8. the asset is marked as GPU loaded 
