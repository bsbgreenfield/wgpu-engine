#[cfg(test)]
mod rigid_tests {
    use std::{collections::HashMap, sync::Arc, vec};

    use cgmath::{Matrix4, SquareMatrix};

    use crate::{
        animation::animation::{
            Animation, AnimationChannels, AnimationSample, AnimationSampler,
            AnimationTransformType, AnimationTransforms, InterpolationType,
        },
        asset_manager_new::gltf::{GltfAnimation, GltfNode, NodeTransforms, NodeType},
        util::types::Mat4F32,
        world::instance_manager::AnimationInstance,
    };

    // buffer is column-major [[f32;4];4]: translation is column 3, so [3][0]=x, [3][1]=y, [3][2]=z

    fn translation_trs(tx: f32, ty: f32, tz: f32) -> [NodeTransforms; 3] {
        [
            NodeTransforms::Translation(cgmath::Vector3::new(tx, ty, tz)),
            NodeTransforms::Rotation(cgmath::Quaternion::new(1.0, 0.0, 0.0, 0.0)),
            NodeTransforms::Scale(cgmath::Vector3::new(1.0, 1.0, 1.0)),
        ]
    }

    fn identity_trs() -> [NodeTransforms; 3] {
        translation_trs(0.0, 0.0, 0.0)
    }

    fn translation_sampler(times: Vec<f32>, keyframes: Vec<f32>) -> AnimationSampler {
        AnimationSampler::new(
            InterpolationType::Linear,
            times,
            AnimationTransforms(keyframes),
        )
    }

    fn mock_translation_animation() -> (GltfAnimation, AnimationInstance) {
        let times = vec![0.0f32, 1.0];
        let translations: Vec<f32> = vec![
            0.0, 0.0, 0.0, // keyframe 0: origin
            10.0, 0.0, 0.0, // keyframe 1: (10, 0, 0)
        ];

        let sampler = translation_sampler(times.clone(), translations);

        let mut channels = AnimationChannels::new();
        channels.insert(0usize, vec![(0usize, AnimationTransformType::Translation)]);

        let mut buffer_slot_map = HashMap::new();
        buffer_slot_map.insert(0usize, 0usize);

        let node = Arc::new(GltfNode::mock(NodeType::Mesh(0), 0, vec![], identity_trs()));

        let animation =
            GltfAnimation::new_for_test(vec![node], buffer_slot_map, vec![sampler], channels);

        let samples = animation.init_samples();

        (animation, AnimationInstance::new_for_test(samples, 1))
    }

    // ── single-node sanity tests ─────────────────────────────────────────────

    #[test]
    fn translation_at_start() {
        let (anim, mut inst) = mock_translation_animation();
        anim.get_animation_frame(0.0, &mut inst, &Matrix4::identity());
        let x = inst.buffer[0][3][0];
        assert!(x.abs() < 1e-5, "expected x=0, got {x}");
    }

    #[test]
    fn translation_at_midpoint() {
        let (anim, mut inst) = mock_translation_animation();
        anim.get_animation_frame(0.5, &mut inst, &Matrix4::identity());
        let [x, y, z, _] = inst.buffer[0][3];
        assert!((x - 5.0).abs() < 1e-5, "expected x=5, got {x}");
        assert!(y.abs() < 1e-5);
        assert!(z.abs() < 1e-5);
    }

    #[test]
    fn base_transform_is_applied() {
        let (anim, mut inst) = mock_translation_animation();
        let base = Matrix4::from_translation(cgmath::Vector3::new(100.0, 0.0, 0.0));
        anim.get_animation_frame(0.5, &mut inst, &base);
        let x = inst.buffer[0][3][0];
        assert!(
            (x - 105.0).abs() < 1e-5,
            "expected x=105 (base 100 + animated 5), got {x}"
        );
    }

    #[test]
    fn animation_marks_complete_after_end_time() {
        let (anim, mut inst) = mock_translation_animation();
        anim.get_animation_frame(0.5, &mut inst, &Matrix4::identity());
        assert!(!inst.samples[0].complete);
        anim.get_animation_frame(1.5, &mut inst, &Matrix4::identity());
        assert!(inst.samples[0].complete);
    }

    #[test]
    fn completed_animation_does_not_write_buffer() {
        let (anim, mut inst) = mock_translation_animation();
        anim.get_animation_frame(1.5, &mut inst, &Matrix4::identity());
        assert!(inst.samples[0].complete);

        let tester: Mat4F32 = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [99.0, 0.0, 0.0, 1.0],
        ];
        inst.buffer[0] = tester;

        anim.get_animation_frame(2.0, &mut inst, &Matrix4::identity());
        assert_eq!(
            inst.buffer[0], tester,
            "buffer should not be touched after animation completes"
        );
    }

    #[test]
    fn cursor_advances_to_next_segment() {
        let times = vec![0.0f32, 0.5, 1.0];
        let sampler = translation_sampler(
            times.clone(),
            vec![0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 10.0, 0.0, 0.0],
        );

        let mut channels = AnimationChannels::new();
        channels.insert(0usize, vec![(0usize, AnimationTransformType::Translation)]);

        let mut buffer_slot_map = HashMap::new();
        buffer_slot_map.insert(0usize, 0usize);

        let node = Arc::new(GltfNode::mock(NodeType::Mesh(0), 0, vec![], identity_trs()));
        let animation =
            GltfAnimation::new_for_test(vec![node], buffer_slot_map, vec![sampler], channels);

        let samples = animation.init_samples();
        let mut instance = AnimationInstance::new_for_test(samples, 1);

        // t=0.75 is in the second segment [0.5, 1.0]: cursor should advance to 1
        // ratio = (0.75 - 0.5) / (1.0 - 0.5) = 0.5, lerp(5, 10, 0.5) = 7.5
        animation.get_animation_frame(0.75, &mut instance, &Matrix4::identity());
        assert_eq!(instance.samples[0].cursor, 1);
        let x = instance.buffer[0][3][0];
        assert!((x - 7.5).abs() < 1e-5, "expected x=7.5, got {x}");
    }

    #[test]
    fn rest_pose_used_when_no_channel() {
        let node = Arc::new(GltfNode::mock(
            NodeType::Mesh(0),
            0,
            vec![],
            translation_trs(3.0, 0.0, 0.0),
        ));

        let mut buffer_slot_map = HashMap::new();
        buffer_slot_map.insert(0usize, 0usize);

        let animation = GltfAnimation::new_for_test(
            vec![node],
            buffer_slot_map,
            vec![],
            AnimationChannels::new(),
        );

        let mut instance = AnimationInstance::new_for_test(vec![], 1);
        animation.get_animation_frame(0.5, &mut instance, &Matrix4::identity());

        let x = instance.buffer[0][3][0];
        assert!((x - 3.0).abs() < 1e-5, "expected rest pose x=3, got {x}");
    }

    // ── multi-node tests ─────────────────────────────────────────────────────

    /// two sibling nodes, each animated with their own channel/sampler
    fn animation_two() -> (GltfAnimation, AnimationInstance) {
        let times = vec![0.0f32, 1.0];

        let sampler_0 = translation_sampler(times.clone(), vec![0.0, 0.0, 0.0, 6.0, 0.0, 0.0]);
        let sampler_1 = translation_sampler(times.clone(), vec![0.0, 0.0, 0.0, 0.0, 6.0, 0.0]);

        let mut channels = AnimationChannels::new();
        channels.insert(0usize, vec![(0usize, AnimationTransformType::Translation)]);
        channels.insert(1usize, vec![(1usize, AnimationTransformType::Translation)]);

        let mut buffer_slot_map = HashMap::new();
        buffer_slot_map.insert(0usize, 0usize);
        buffer_slot_map.insert(1usize, 1usize);

        let node_0 = Arc::new(GltfNode::mock(NodeType::Mesh(0), 0, vec![], identity_trs()));
        let node_1 = Arc::new(GltfNode::mock(NodeType::Mesh(1), 1, vec![], identity_trs()));

        let animation = GltfAnimation::new_for_test(
            vec![node_0, node_1],
            buffer_slot_map,
            vec![sampler_0, sampler_1],
            channels,
        );
        let samples = animation.init_samples();
        let instance = AnimationInstance::new_for_test(samples, 2);
        (animation, instance)
    }

    #[test]
    fn two_sibling_nodes_write_to_separate_buffer_slots() {
        let (animation, mut instance) = animation_two();

        animation.get_animation_frame(0.5, &mut instance, &Matrix4::identity());

        assert!(
            (instance.buffer[0][3][0] - 3.0).abs() < 1e-5,
            "node 0 x expected 3, got {}",
            instance.buffer[0][3][0]
        );
        assert!(
            (instance.buffer[1][3][1] - 3.0).abs() < 1e-5,
            "node 1 y expected 3, got {}",
            instance.buffer[1][3][1]
        );
    }

    /// An animation with one parent and one child, no samplers
    fn parent_child_animation() -> (GltfAnimation, AnimationInstance) {
        // Expected: buffer[0] = T(5,0,0) * T(0,3,0) = T(5,3,0)
        let child = GltfNode::mock(NodeType::Mesh(0), 1, vec![], translation_trs(0.0, 3.0, 0.0));
        let parent = Arc::new(GltfNode::mock(
            NodeType::Node,
            0,
            vec![child],
            translation_trs(5.0, 0.0, 0.0),
        ));

        let mut buffer_slot_map = HashMap::new();
        buffer_slot_map.insert(0usize, 0usize);

        let animation = GltfAnimation::new_for_test(
            vec![parent],
            buffer_slot_map,
            vec![],
            AnimationChannels::new(),
        );

        let instance = AnimationInstance::new_for_test(vec![], 1);
        (animation, instance)
    }

    #[test]
    fn child_mesh_inherits_static_parent_transform() {
        // Parent (Node, id=0): rest T=(5,0,0), no animation
        // Child  (Mesh(0), id=1): rest T=(0,3,0), no animation
        // Expected: buffer[0] = T(5,0,0) * T(0,3,0) = T(5,3,0)
        let child = GltfNode::mock(NodeType::Mesh(0), 1, vec![], translation_trs(0.0, 3.0, 0.0));
        let parent = Arc::new(GltfNode::mock(
            NodeType::Node,
            0,
            vec![child],
            translation_trs(5.0, 0.0, 0.0),
        ));

        let mut buffer_slot_map = HashMap::new();
        buffer_slot_map.insert(0usize, 0usize);

        let animation = GltfAnimation::new_for_test(
            vec![parent],
            buffer_slot_map,
            vec![],
            AnimationChannels::new(),
        );

        let mut instance = AnimationInstance::new_for_test(vec![], 1);
        animation.get_animation_frame(0.0, &mut instance, &Matrix4::identity());

        assert!(
            (instance.buffer[0][3][0] - 5.0).abs() < 1e-5,
            "x expected 5, got {}",
            instance.buffer[0][3][0]
        );
        assert!(
            (instance.buffer[0][3][1] - 3.0).abs() < 1e-5,
            "y expected 3, got {}",
            instance.buffer[0][3][1]
        );
    }

    /// an animation with a parent node with two mesh children
    /// the parent node and the first child are animated, but the second child is not
    fn parent_two_children() -> (GltfAnimation, AnimationInstance) {
        // Root   (Node,   id=0): animated, T at t=0 is (3,0,0)
        // Child1 (Mesh(0), id=1): animated, T at t=0 is (0,0,0) → writes to slot 0
        // Child2 (Mesh(1), id=2): rest T=(0,0,7), no channel   → writes to slot 1
        //
        // At t=0:
        //   buffer[0] = T(3,0,0) * T(0,0,0) = T(3,0,0)  → [3, 0, 0]
        //   buffer[1] = T(3,0,0) * T(0,0,7) = T(3,0,7)  → [3, 0, 7]
        let times = vec![0.0f32, 1.0];

        let sampler_root = translation_sampler(times.clone(), vec![3.0, 0.0, 0.0, 9.0, 0.0, 0.0]);
        let sampler_child1 = translation_sampler(times.clone(), vec![0.0, 0.0, 0.0, 0.0, 6.0, 0.0]);

        let mut channels = AnimationChannels::new();
        channels.insert(0usize, vec![(0usize, AnimationTransformType::Translation)]);
        channels.insert(1usize, vec![(1usize, AnimationTransformType::Translation)]);

        let child_1 = GltfNode::mock(NodeType::Mesh(0), 1, vec![], identity_trs());
        let child_2 = GltfNode::mock(NodeType::Mesh(1), 2, vec![], translation_trs(0.0, 0.0, 7.0));
        let root = Arc::new(GltfNode::mock(
            NodeType::Node,
            0,
            vec![child_1, child_2],
            identity_trs(),
        ));

        let mut buffer_slot_map = HashMap::new();
        buffer_slot_map.insert(0usize, 0usize); //mesh0 -> buffer slot 0
        buffer_slot_map.insert(1usize, 1usize); //mesh1 -> buffer slot 1

        let animation = GltfAnimation::new_for_test(
            vec![root],
            buffer_slot_map,
            vec![sampler_root, sampler_child1],
            channels,
        );

        let samples = animation.init_samples();
        let instance = AnimationInstance::new_for_test(samples, 2);
        (animation, instance)
    }

    #[test]
    fn animated_parent_transform_propagates_to_children() {
        let (animation, mut instance) = parent_two_children();
        animation.get_animation_frame(0.0, &mut instance, &Matrix4::identity());

        let [x0, y0, z0, _] = instance.buffer[0][3];
        assert!((x0 - 3.0).abs() < 1e-5, "child1 x expected 3, got {x0}");
        assert!(y0.abs() < 1e-5, "child1 y expected 0, got {y0}");
        assert!(z0.abs() < 1e-5, "child1 z expected 0, got {z0}");

        let [x1, y1, z1, _] = instance.buffer[1][3];
        assert!((x1 - 3.0).abs() < 1e-5, "child2 x expected 3, got {x1}");
        assert!(y1.abs() < 1e-5, "child2 y expected 0, got {y1}");
        assert!((z1 - 7.0).abs() < 1e-5, "child2 z expected 7, got {z1}");
    }

    #[test]
    fn parent_two_children_at_half_second() {
        // At t=0.5:
        //   root:   lerp((3,0,0), (9,0,0), 0.5) = (6,0,0)
        //   child1: lerp((0,0,0), (0,6,0), 0.5) = (0,3,0)
        //   child2: rest (0,0,7)
        //
        //   buffer[0] = T(6,0,0) * T(0,3,0) = T(6,3,0)  — root x + own y
        //   buffer[1] = T(6,0,0) * T(0,0,7) = T(6,0,7)  — root x only, own z from rest
        let (animation, mut instance) = parent_two_children();
        animation.get_animation_frame(0.5, &mut instance, &Matrix4::identity());

        let [x0, y0, z0, _] = instance.buffer[0][3];
        assert!(
            (x0 - 6.0).abs() < 1e-5,
            "mesh0 x: root contributes 6, got {x0}"
        );
        assert!(
            (y0 - 3.0).abs() < 1e-5,
            "mesh0 y: own animation contributes 3, got {y0}"
        );
        assert!(z0.abs() < 1e-5, "mesh0 z expected 0, got {z0}");

        let [x1, y1, z1, _] = instance.buffer[1][3];
        assert!(
            (x1 - 6.0).abs() < 1e-5,
            "mesh1 x: root contributes 6, got {x1}"
        );
        assert!(y1.abs() < 1e-5, "mesh1 y: no own animation, got {y1}");
        assert!(
            (z1 - 7.0).abs() < 1e-5,
            "mesh1 z: rest pose contributes 7, got {z1}"
        );
    }

    // ── shared sampler tests ─────────────────────────────────────────────────

    /// Two sibling mesh nodes each have a Scale channel that both reference sampler 0.
    /// There is only one AnimationSample, keyed by sampler index 0.
    fn shared_sampler_scale_animation() -> (GltfAnimation, AnimationInstance) {
        let times = vec![0.0f32, 1.0];
        // scale goes from (1,1,1) to (3,3,3)
        let sampler = AnimationSampler::new(
            InterpolationType::Linear,
            times.clone(),
            AnimationTransforms(vec![1.0, 1.0, 1.0, 3.0, 3.0, 3.0]),
        );

        let mut channels = AnimationChannels::new();
        channels.insert(0usize, vec![(0usize, AnimationTransformType::Scale)]);
        channels.insert(1usize, vec![(0usize, AnimationTransformType::Scale)]);

        let mut buffer_slot_map = HashMap::new();
        buffer_slot_map.insert(0usize, 0usize);
        buffer_slot_map.insert(1usize, 1usize);

        let node_0 = Arc::new(GltfNode::mock(NodeType::Mesh(0), 0, vec![], identity_trs()));
        let node_1 = Arc::new(GltfNode::mock(NodeType::Mesh(1), 1, vec![], identity_trs()));

        let animation = GltfAnimation::new_for_test(
            vec![node_0, node_1],
            buffer_slot_map,
            vec![sampler],
            channels,
        );

        let samples = vec![AnimationSample::init(&times)];
        let instance = AnimationInstance::new_for_test(samples, 2);
        (animation, instance)
    }

    #[test]
    fn shared_sampler_animates_both_nodes() {
        // At t=0.5: scale = lerp((1,1,1),(3,3,3), 0.5) = (2,2,2)
        // Both nodes reference sampler 0 and the same AnimationSample; both should
        // read the same interpolated scale without interfering with each other.
        let (animation, mut instance) = shared_sampler_scale_animation();
        animation.get_animation_frame(0.5, &mut instance, &Matrix4::identity());

        // scale_x lives at buffer[slot][col=0][row=0] in column-major layout
        let sx0 = instance.buffer[0][0][0];
        let sx1 = instance.buffer[1][0][0];
        assert!(
            (sx0 - 2.0).abs() < 1e-5,
            "node0 scale_x expected 2, got {sx0}"
        );
        assert!(
            (sx1 - 2.0).abs() < 1e-5,
            "node1 scale_x expected 2, got {sx1}"
        );
    }

    #[test]
    fn shared_sampler_stops_writing_after_end_frame() {
        // The first call past end_time should return End so nodes can write their final
        // pose. Every call after that should return Done and skip all writes.
        let (animation, mut instance) = shared_sampler_scale_animation();

        // First call past end_time: End fires, nodes write their final pose and the
        // sampler is marked complete.
        animation.get_animation_frame(1.5, &mut instance, &Matrix4::identity());
        assert!(
            instance.samples[0].complete,
            "sampler should be complete after end time"
        );

        // Overwrite buffers with sentinel values to detect any future writes.
        instance.buffer[0][0][0] = 55.0;
        instance.buffer[1][0][0] = 55.0;

        // Second call: complete=true -> Done for all nodes -> no buffer writes.
        animation.get_animation_frame(2.0, &mut instance, &Matrix4::identity());
        assert_eq!(
            instance.buffer[0][0][0], 55.0,
            "node0 buffer must not change after sampler is done"
        );
        assert_eq!(
            instance.buffer[1][0][0], 55.0,
            "node1 buffer must not change after sampler is done"
        );
    }

    #[test]
    fn multiple_channels_on_one_node_all_applied() {
        // Node 0 (Mesh 0): has both a translation channel and a scale channel.
        // At t=0: T=(4,0,0), S=(2,2,2).
        // Expected buffer[0]: scale first then translate → x column scaled by 2, w col = (4,0,0,1)
        let times = vec![0.0f32, 1.0];

        let sampler_t = translation_sampler(times.clone(), vec![4.0, 0.0, 0.0, 8.0, 0.0, 0.0]);
        let sampler_s = AnimationSampler::new(
            InterpolationType::Linear,
            times.clone(),
            AnimationTransforms(vec![2.0, 2.0, 2.0, 4.0, 4.0, 4.0]),
        );

        let mut channels = AnimationChannels::new();
        channels.insert(
            0usize,
            vec![
                (0usize, AnimationTransformType::Translation),
                (1usize, AnimationTransformType::Scale),
            ],
        );

        let mut buffer_slot_map = HashMap::new();
        buffer_slot_map.insert(0usize, 0usize);

        let node = Arc::new(GltfNode::mock(NodeType::Mesh(0), 0, vec![], identity_trs()));

        let animation = GltfAnimation::new_for_test(
            vec![node],
            buffer_slot_map,
            vec![sampler_t, sampler_s],
            channels,
        );

        let samples = animation.init_samples();
        let mut instance = AnimationInstance::new_for_test(samples, 1);

        animation.get_animation_frame(0.0, &mut instance, &Matrix4::identity());

        // Translation column should be (4,0,0,1)
        let [tx, ty, tz, _] = instance.buffer[0][3];
        assert!((tx - 4.0).abs() < 1e-5, "tx expected 4, got {tx}");
        assert!(ty.abs() < 1e-5);
        assert!(tz.abs() < 1e-5);

        // x basis column should be scaled by 2: (2,0,0,0)
        let [sx, _, _, _] = instance.buffer[0][0];
        assert!((sx - 2.0).abs() < 1e-5, "scale x expected 2, got {sx}");
    }
}
