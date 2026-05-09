use std::{collections::HashMap, fmt::Debug};

use crate::{
    asset_manager_new::AssetHandle,
    world::{
        components::{AnimationAccessor, MeshAcessor, RigidAnimationMode},
        world::RenderView,
    },
};

#[derive(Debug, Default)]
pub struct InstanceUploadQueryNew<'a> {
    pub requirements: HashMap<AssetHandle, Vec<DataRequirement<'a>>>,
}

impl<'a> InstanceUploadQueryNew<'a> {
    pub fn modify(&mut self, asset_handle: AssetHandle, reqs: Vec<DataRequirement<'a>>) {
        self.requirements
            .entry(asset_handle)
            .or_insert_with(Vec::new)
            .extend(reqs);
    }
}

use bitflags::bitflags;
bitflags! {
    #[repr(C)]
    #[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy )]
    pub struct AssetRequirements: u8 {
        const MeshData = 0;
        const LocalTransforms = 1;
        const Donor = 2;
    }

}

pub trait EntityDataRequirement<'a, const new: bool> {
    type Output;

    fn fufill(self) -> Self::Output;
}

pub struct MeshDataRequirement<'a> {
    accessor: &'a MeshAcessor,
}

impl<'a> EntityDataRequirement<'a, true> for MeshDataRequirement<'a> {
    type Output = RenderView;

    fn fufill(self) -> Self::Output {
        todo!()
    }
}

pub enum DataRequirement<'a> {
    MeshData(&'a MeshAcessor),
    LocalTransformData {
        accessor: &'a MeshAcessor,
        mode: &'a RigidAnimationMode,
    },
    AnimationData {
        anim_accessor: &'a AnimationAccessor,
    },
}

impl Debug for DataRequirement<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MeshData(arg0) => f.debug_tuple("MeshData").field(arg0).finish(),
            Self::LocalTransformData { accessor, mode } => f
                .debug_struct("LocalTransformData")
                .field("accessor", accessor)
                .field("mode", mode)
                .finish(),
            Self::AnimationData { anim_accessor } => f
                .debug_struct("accessor")
                .field("accessor", anim_accessor)
                .finish(),
        }
    }
}
