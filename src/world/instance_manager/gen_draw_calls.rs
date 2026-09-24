use std::range::Range;

use crate::{
    renderer::{DrawItem, RenderPacket},
    world::instance_manager::instance_manager::InstanceManager,
};

impl<'frame> DrawCallGenerator<'frame> for InstanceManager {
    fn gen_draw_calls(&'frame self, packet: &mut RenderPacket) {
        // adjust as archetype tables are added
        let record_len = self.pos.positions.len();

        packet.reset(record_len);

        packet.count_sort(&self.pos.record_indices, &self.pos.positions);

        for bucket in packet.draw_packet.draw_buckets.iter() {
            let instance_range = Range::from(bucket.start..(bucket.start + bucket.count));
            let Some(group) = self.render_groups[bucket.group_idx].as_ref() else {
                panic!("draw bucket referenced a freed render group slot");
            };
            for view in group.views().iter() {
                if let Some(pnu) = &view.pnu_draws {
                    for (i, prim_range) in pnu.primtitive_ranges.iter().enumerate() {
                        let entry = packet
                            .draw_packet
                            .pnu
                            .entry(view.alloc_handle.clone())
                            .or_insert_with(Vec::new);
                        entry.push(DrawItem {
                            lt_idx: pnu.mesh_map[i],
                            joint_offset: None,
                            instances: instance_range.clone(),
                            primitives: prim_range.clone(),
                            indices: pnu.index_ranges.as_ref().map(|x| x[i].clone()),
                            material: pnu.material_indices[i],
                        });
                    }
                }
                if let Some(pnujw) = &view.pnujw_draws {
                    for (i, prim_range) in pnujw.primtitive_ranges.iter().enumerate() {
                        let entry = packet
                            .draw_packet
                            .pnujw
                            .entry(view.alloc_handle.clone())
                            .or_insert_with(Vec::new);
                        entry.push(DrawItem {
                            lt_idx: pnujw.mesh_map[i],
                            joint_offset: Some(pnujw.joint_map[i]),
                            instances: instance_range.clone(),
                            primitives: prim_range.clone(),
                            indices: pnujw.index_ranges.as_ref().map(|x| x[i].clone()),
                            material: pnujw.material_indices[i],
                        });
                    }
                }
            }
        }
    }
}

pub trait DrawCallGenerator<'frame> {
    fn gen_draw_calls(&'frame self, packet: &mut RenderPacket);
}
