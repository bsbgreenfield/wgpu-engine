1. world.despawn()
2. instance_manager.despawn()
- gpu bind registry unregister. find the GPUInstanceHandle corresponding to this instance handle and remove it from the registry
- clear any active animations
- remove archetype data, and the matching record index from the archetype table

3. push a WorldUpdateDelta::InstanceDespawn(GPUInstanceHandle)
- produces Operations::DespawnInstance, constant = gpu instance handle

4. renderer.despawn(gpu_instance_handle)
5. bind_groups.despawn(handle)
 - get the instance prototype, decrement ref count

6. despawn on all bind groups

- for shared data
- remove gpu handle from alloc table
- decrement ref count
