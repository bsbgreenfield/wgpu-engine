use std::range::Range;
use std::{collections::HashMap, fmt::Debug, marker::PhantomData};

use crate::renderer::gpu_allocator::allocation_table::AllocationTable;
use crate::{
    renderer::{
        GPUAllocationHandle,
        gpu_allocator::{
            AllocMetaData, CHUNK_SIZE, GPUAllocator, GPUChunk, MIMIMUM_INDEX_ALLOCATION_SIZE,
            MIMIMUM_VERTEX_ALLOCATION_SIZE, UploadIndexJob, UploadMeshJob, VertexArenaError,
            free_list::FreeListAllocator,
        },
    },
    util::types::{ModelVertex, VIndex},
};
//****************************************************************
//
