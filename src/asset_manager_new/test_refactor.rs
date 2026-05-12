use std::{collections::HashMap, marker::PhantomData};

use crate::asset_manager_new::{AssetHandle, AssetLoadError, asset_manager_new::AssetManagerNew};

struct GltfAssetNew {
    name: String,
}

trait LoadsMeshData: AssetNew {
    fn get_mesh_data(&self, component_data: String) -> MeshData;
}

enum UnloadedAssetData {
    Gltf(String),
}
trait AssetNew {
    fn new() -> UnloadedAssetData
    where
        Self: Sized;
    fn get_upload_job(&self) -> Result<String, AssetLoadError>;

    fn as_mesh_provider(&self) -> Option<&dyn LoadsMeshData>;
}

impl AssetNew for GltfAssetNew {
    fn get_upload_job(&self) -> Result<String, AssetLoadError> {
        Ok(String::from("hello"))
    }

    fn new() -> UnloadedAssetData
    where
        Self: Sized,
    {
        UnloadedAssetData::Gltf(String::from("hello"))
    }

    fn as_mesh_provider(&self) -> Option<&dyn LoadsMeshData> {
        Some(self)
    }
}

impl LoadsMeshData for GltfAssetNew {
    fn get_mesh_data(&self, component_data: String) -> MeshData {
        MeshData {
            data: self.name.clone(),
        }
    }
}

struct MeshData {
    data: String,
}

trait DummyComponent {
    type Output;
    type Asset: AssetNew;
    fn get_data(&self, asset: Self::Asset) -> Self::Output;
}

impl<T: LoadsMeshData> DummyComponent for TestMeshComponent<T> {
    type Output = MeshData;
    type Asset = T;
    fn get_data(&self, asset: T) -> Self::Output {
        asset.get_mesh_data(self.name.clone())
    }
}

struct TestMeshComponent<T: LoadsMeshData + ?Sized> {
    name: String,
    resource_backing: ResourceBackingNew<T>,
}

struct RegisterAssetNew<A: AssetNew + ?Sized> {
    data: UnloadedAssetData,
    _t: PhantomData<A>,
}

struct DummyAssetManager {
    loaded: Vec<Box<dyn AssetNew>>,
    registered: HashMap<AssetHandle, RegisterAssetNew<dyn AssetNew>>,
}

struct ResourceBackingNew<A: AssetNew + ?Sized> {
    handle: AssetHandle,
    _t: PhantomData<A>,
}

impl<A: AssetNew + LoadsMeshData> ResourceBackingNew<A> {
    fn as_mesh_backing(self) -> ResourceBackingNew<dyn LoadsMeshData> {
        ResourceBackingNew {
            handle: self.handle,
            _t: PhantomData,
        }
    }
}

impl DummyAssetManager {
    fn register<A: AssetNew>(&mut self) -> ResourceBackingNew<A> {
        self.registered.insert(
            AssetHandle(0),
            RegisterAssetNew {
                data: A::new(),
                _t: PhantomData,
            },
        );

        ResourceBackingNew {
            handle: AssetHandle(0),
            _t: PhantomData,
        }
    }

    fn load_asset(&mut self, handle: &AssetHandle) {
        let registered = self.registered.get(handle).unwrap();
        match &registered.data {
            UnloadedAssetData::Gltf(data) => {
                // load gltf code
            } // other types
        }
    }
}

struct DummyEntityManager {
    mesh_components: Vec<TestMeshComponent<dyn LoadsMeshData>>,
    // other components
}
impl DummyEntityManager {
    fn add_mesh_component(
        &mut self,
        name: String,
        resource: Box<ResourceBackingNew<dyn LoadsMeshData>>,
    ) {
        let component = TestMeshComponent {
            name,
            resource_backing: *resource,
        };
        self.mesh_components.push(component);
    }
}
fn dummy_setup() {
    let mut asset_manager = DummyAssetManager {
        registered: HashMap::new(),
        loaded: Vec::new(),
    };
    let mut entity_manager = DummyEntityManager {
        mesh_components: Vec::new(),
    };
    let resource = asset_manager.register::<GltfAssetNew>();

    entity_manager.add_mesh_component(String::from("hello"), Box::new(resource.as_mesh_backing()));

    // user requests to load up the entity
    let component = entity_manager.mesh_components.get(0).unwrap();

    let asset = asset_manager.loaded[0].as_mesh_provider().unwrap();
    let output = asset.get_mesh_data(component.name.clone());
}
