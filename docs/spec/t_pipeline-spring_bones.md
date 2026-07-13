# Secondary motion and transform springs

Avatar hair, tails, ears, and cloth use the chain-level `SecondaryMotionSystem`, modeled on VRM 1.0 SpringBone. It binds explicit ordered `SpringJoint` paths within the nearest ancestor `GLTF`, integrates tail positions with fixed-length Verlet steps, and writes the final joint rotations after AvatarControl and IK. A virtual endpoint lets the last imported bone rotate without changing glTF topology.

Authored metadata has this shape:

```mms
GLTF.new("assets/avatar.glb") {
    SecondaryMotion {
        SpringBone.new("hair").virtual_end_length_ratio(1.0) {
            SpringJoint.new("Armature[0]/Hair1[0]").stiffness(1.0).drag_force(0.4)
            SpringJoint.new("Armature[0]/Hair2[0]")
        }
    }
}
```

Paths contain escaped node-name segments and same-name sibling ordinals. They are resolved only through that GLTF instance's path map. Runtime IDs and imported nodes are not serialized. Generated `<asset>.glb.mms` sidecars export a generic `secondary_motion()` factory and refuse to replace files without the generated marker.

The initial implementation intentionally excludes VRM colliders, angular limits, stretch, grabbing, and parameter curves. Secondary motion and FABRIK/IK remain separate systems; see [dynamic-chain-unification.md](../task/wip/dynamic-chain-unification.md) for possible shared infrastructure.

A single-transform quaternion spring may eventually be useful as a general transform-pipeline signal operator. It is not the avatar-chain implementation: independent quaternion filters do not preserve bone lengths or reproduce VRM tail dynamics.
