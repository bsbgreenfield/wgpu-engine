# steps

1. Include animations in the LoadedGltfAsset
2. in update_render_state() -> get_entity_renderables() 
    - add needs_animation data to Query
    - add animation to renderables as instance_data
    - add animation to AnimationController in the instance manager
3. in prepare_render_frame() run the animation with delta time 
4. pass data to renderer to update 


## skeletal

A gltf asset can define one or more skins
Each skin contains a list of joints, and their associated IBMs
A skin can be applied to a mesh, in which case the vertices of that mesh are influenced by the joints defined in the skin.
The joints defined in skin refers to node ids, which may or may not share a node tree with the meshes that they influence
A "skinned" mesh will contain a primitive which in turn contains joint and weight data
The joint ids contained in the primitive vertex data refer to indices within the skin


so to get from vertex joint id -> gpu buffer id

GPU BUFFER IDX = JOINT_IDX + SKIN OFFSET


so if there is are two skin with nodes [a, b, c, d] and [e, f, g, h]
and one mesh which is used twice in a model, the first instance using the first skin s1 and the second using the s2


1. create the mesh. The primtive's joints_0 will be [0, 1, 2, 3] 
2. build node tree for asset, marking node a as Joint(0, 0), g as Joint(1, 2) etc.
3. in spawn -> get render data
    - traverse the node tree
    - if node is Joint(x, y) insert lt into joints[x][y]
    - for each mesh instance entry in the mesh instance vec, add a skin offset
        - skin offset = asset.skins[0..skin_idx - 1].sum_lens()
        - so the return val from collect mesh instances is [(0, (lt, 0)), (0, (lt, 4))]
    - return joints in meshrenderables add to instance_upload_data.joints
    - for each pnujw primitive while iterating mesh instances, add skin_offset to joints_map[i]
4. add Op::JointUpload and RenderConstants::DataOwned(joints[0] + joints[1])
5. update instance manager gpu_bindings with offset val for joints in gpu
6. in gen_draw_calls, add DrawItem.joint_offset as bindings.joint_offset + joints_map[i]

    







