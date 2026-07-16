use crate::{
    renderer::{DrawItem, RenderPacket},
    world::instance_manager::instance_manager::InstanceManager,
};

impl<'frame> DrawCallGenerator<'frame> for InstanceManager {
    fn gen_draw_calls(&'frame self, packet: &mut RenderPacket) {
        // adjust as archetype tables are added
        let record_len = self.pos.positions.len();

        packet.reset(self.render_groups.len(), record_len);

        packet.draw_packet.count_sort(
            &self.pos.arena.handles,
            &self.pos.record_indices,
            &self.sparse_entity_group,
        );

        for (i, record_slot) in self.pos.record_indices.iter().enumerate() {
            packet.global_transforms[*record_slot as usize] = self.pos.positions[i].into();
        }

        for (group_idx, group) in self.render_groups.iter().enumerate() {
            if packet.draw_packet.instance_ranges[group_idx].start
                == packet.draw_packet.instance_ranges[group_idx].end
            {
                continue;
            }
            for view in group.views.iter() {
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
                            instances: (packet.draw_packet.instance_ranges[group_idx]).clone(),
                            primitives: prim_range.clone(),
                            indices: pnu.index_ranges.as_ref().map(|x| x[i].clone()),
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
                            instances: (packet.draw_packet.instance_ranges[group_idx]).clone(),
                            primitives: prim_range.clone(),
                            indices: pnujw.index_ranges.as_ref().map(|x| x[i].clone()),
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
