# steps

1. Include animations in the LoadedGltfAsset
2. in update_render_state() -> get_entity_renderables() 
    - add needs_animation data to Query
    - add animation to renderables as instance_data
    - add animation to AnimationController in the instance manager
3. in prepare_render_frame() run the animation with delta time 
4. pass data to renderer to update 
