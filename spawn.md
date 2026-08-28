# spawning


1. reegister assets
2. register entities using the assets (if applicable)
3. add entities to scene
4. scene_builder.create() -> scene_manager.add_scene()

- creates actual scene from the builder, and generates an ID
- add scene on the dep graph by creating the scene node with its children and assets

5. add instances to the scene
- add the arch data to the runtime spawn queue

6. set load level of the scene

- if the load level is being raised
    - for all assets that are LOWER than the requested level,
        - add asset to asset requests
        - increment a counter for this scene with the number of pending assets

- TODO: LOWER

7. world.update(), for each asset request, load_queue.add_load_job()

8. poll_jobs()
- for each active job
    - if pending, skip
    - set min load level
    - add to transitions and/ or pending GPU

9. for each transition reported by the laod queue,
- decrement the pending counter for each scene that is awaiting the asset
- once all pending assets are complete, add scene to ready queue


10. for those assets that are in the pending_gpu queue in the load queue
 push asset did load delta

11. emit bytecode for an asset upload

  - add asset
  - pnu/pnujw upload
  - emit (returns render update delta:: AssetGPULoaded)

12. post-frame update
    - mark asset as gpu loaded in the asset manager
    - for each scene that holds this asset, decrement the pending counter/ mark ready if necessary


13. NEXT FRAME UPDATE ->  process_scene_events adds to spawn queue

14. world.spawn(instances in the spawn queue) -> emit entity spawn delta
