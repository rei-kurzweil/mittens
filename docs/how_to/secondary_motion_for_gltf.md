# Secondary motion for glTF

Place `SecondaryMotion` beneath the authored `GLTF`. Each `SpringBone` contains an ordered root-to-tip list of imported transforms:

```mms
GLTF.new("assets/avatar.glb") {
    SecondaryMotion {
        SpringBone.new("hair").virtual_end_length_ratio(1.0) {
            SpringJoint.new("[name='Hair1']").stiffness(1).drag_force(0.35).gravity(3, [0,-1,0])
            SpringJoint.new("[name='Hair2']").stiffness(1).drag_force(0.35).gravity(3, [0,-1,0])
            SpringJoint.new("[name='Hair3']").stiffness(1).drag_force(0.35).gravity(3, [0,-1,0])
        }
    }
}
```

Selectors must uniquely match nodes spawned by that GLTF instance. Chains must be ancestral and must not overlap. A virtual endpoint lets the final imported joint rotate.

For an unbranched skin-joint subtree, `from_root` discovers the chain automatically. Collider and helper children outside the GLTF skin are ignored. Chain-level tuning is copied to every discovered joint:

```mms
SpringBone.from_root("[name='tail']")
    .virtual_end_length_ratio(1.0)
    .stiffness(1.0)
    .drag_force(0.25)
    .gravity(0.8, [0, -1, 0])
```

An automatic leaf root requires `virtual_end_length_ratio`; a branch is rejected with a binding diagnostic.

## Presets

Functions exported by MMS modules can return a complete `SecondaryMotion` subtree:

```mms
import { bisket_secondary_motion } from "../assets/components/secondary_motion/bisket.mms"

GLTF.new("assets/models/bisket.glb") {
    bisket_secondary_motion(false)
}
```

Preset config tables override selectors while retaining the library's heuristic physics settings. For example, `soft_hair_chain({ root = "..." middle = "..." tip = "..." })`. When providing a config, supply every selector field documented by that preset; MMS does not currently support partial table-field defaults.

Tune `stiffness` for return-to-rest, `drag_force` for damping, and `gravity` for world-space sag. Test with:

```bash
CAT_DEBUG_SECONDARY_MOTION=1 cargo run --release --example vtuber-secondary-motion
```

Expect `bound_chains` to equal the authored chain count and `failed_chains=0`. Colliders, limits, and center-relative inertia are not yet implemented.

## Return a chain to rest when still

Place `ReturnToRestWhenStill` inside the `SpringBone` it controls:

```mms
SpringBone.from_root("[name='head_bow.001']")
    .virtual_end_length_ratio(1.0) {
        ReturnToRestWhenStill {
            motion_threshold(0.028)
            still_for(0.4)
        }
    }
```

`motion_threshold` measures the spring anchor's motion relative to the avatar transform above its GLTF, in metres per second equivalent. Rotation contributes as motion at a 10 cm radius. `still_for` is seconds below that threshold. The chain blends toward imported rest rotations and parks while settled, then wakes when motion rises. Rei(mu)'s two ribbon chains use this in [`rei-mu-bow.mms`](../../assets/components/secondary_motion/rei-mu-bow.mms). Test the XR scene with `cargo run --release -- load 'examples/rei(mu).mms'`. Set `CAT_DEBUG_SECONDARY_MOTION=1` to print each chain's measured speed, still time, and rest blend weight while tuning in VR.
