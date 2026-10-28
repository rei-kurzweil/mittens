# Lumae

Lumae's runtime assets live together here. Other characters retain their current layout until
migrated and checked separately.

```text
lumae/
├── lumae.glb
├── colliders.mms
├── secondary_motion.mms
├── poses/
│   └── 000-relaxed.pose.mms
└── behavior/
    └── pacing_and_waiting_around.mms
```

From an MMS scene in `examples/`:

```mms
import { lumae_colliders } from "../assets/components/characters/lumae/colliders.mms"
import { lumae_secondary_motion } from "../assets/components/characters/lumae/secondary_motion.mms"
import { pose as relaxed_pose } from "../assets/components/characters/lumae/poses/000-relaxed.pose.mms"
import { pacing_and_waiting_around } from "../assets/components/characters/lumae/behavior/pacing_and_waiting_around.mms"

let avatar_root = T {
    GLTF("assets/components/characters/lumae/lumae.glb") {
        relaxed_pose()
        lumae_colliders()
        lumae_secondary_motion()
    }
}
let pacing = pacing_and_waiting_around(avatar_root, false)
pacing
// pacing.start()
```

The GLB embeds its buffers/textures. Collider and secondary-motion exported names are unchanged.
The relaxed pose changes only arms, preserving Lumae's leg lengths. The pacing controller starts
paused; it produces root motion and does not itself add NPC walking IK. See
[controller and walking eligibility](../../../../docs/task/lumae-pacing-and-npc-walking.md).

The pose-capture editor still saves generated libraries to the shared
`assets/components/poses/<asset_name>/` convention. Routing newly captured Lumae libraries here is
a separate tooling follow-up; the existing hand-authored relaxed preset is now in this folder.
Profiling scripts and regression tests stay in their normal tooling/source directories and use
the model's new URI.
