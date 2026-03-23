# Transform Pipeline — Spring Bones

Date: 2026-03-23

Spring bones and jiggle bones are both FK — no IK involvement. They live entirely in the
`TransformPipelineSystem` as a new quaternion operator: `QuatSpringComponent`.

---

## One operator, not two

"Spring" (pulls back to rest, oscillates) and "jiggle" (follows parent with inertia, hangs
under gravity) are the same spring-damper equation with different parameter ranges:

```
angular_acceleration = stiffness × (rest_rot − current_rot)
                     + gravity_torque(gravity, current_rot)
                     − damping × angular_velocity

angular_velocity += angular_acceleration × dt
current_rot       = quat_integrate(current_rot, angular_velocity, dt)
```

- `stiffness → 0`, moderate `damping` = jiggle / inertia follow (loose hair, tail)
- `stiffness` high, moderate `damping` = spring / oscillate (ear tips, antenna, stiff tail)

Authoring two named components would be sugar over the same code. One `QuatSpringComponent`
with named parameters covers both semantics cleanly.

---

## Component

```rust
pub struct QuatSpringComponent {
    /// Spring constant — how strongly the bone is pulled back toward its rest rotation.
    /// 0.0 = no pull (pure jiggle/inertia). Higher values = tighter spring.
    pub stiffness: f32,

    /// Damping — energy dissipation per second. Prevents infinite oscillation.
    /// Too low = bouncy forever. Too high = overdamped, no secondary motion.
    pub damping: f32,

    /// Constant world-space force direction and magnitude (e.g. [0, -1, 0] for gravity).
    /// Converted to a torque on the bone each tick. Zero = no gravity effect.
    pub gravity: [f32; 3],

    /// Inertia scale — how much the bone resists changes in angular velocity.
    /// 1.0 = standard. Higher = heavier / slower to start and stop.
    pub inertia: f32,
}
```

Typical presets:

| Use case | stiffness | damping | gravity | inertia |
|---|---|---|---|---|
| Loose hair / ponytail | 0.0 | 0.3 | [0, −0.8, 0] | 1.0 |
| Stiff tail / antenna | 4.0 | 0.5 | [0, −0.2, 0] | 0.8 |
| Ear tips | 6.0 | 0.6 | [0, −0.1, 0] | 0.5 |
| Chest / sleeve cloth | 0.5 | 0.4 | [0, −0.5, 0] | 1.2 |

---

## Placement in the pipeline

`QuatSpringComponent` is a child of `TransformMapRotationComponent`, same as
`QuatTemporalFilterComponent` and `QuatYawFollowComponent`:

```
TransformComponent          ← the spring bone's TC (local TRS lives here)
  TransformPipeline
    TransformForkTRS
      TransformMapRotation
        QuatSpring {         ← new op
            stiffness: 0.0
            damping: 0.3
            gravity: [0, -0.8, 0]
            inertia: 1.0
        }
      TransformMergeTRS {}
    TransformPipelineOutput
      TransformComponent    ← child bone, inherits spring output
        ...
```

The pipeline input is the parent's world transform (same as any other pipeline block).
The spring operator replaces the normal rotation passthrough with a physics-filtered
rotation. Translation and scale pass through unchanged unless also mapped.

---

## Spring chains (VRM secondary motion)

For VRM secondaries (hair, tail, skirt panels), the pattern is a **chain** of bones each
with their own spring state. Each bone's pipeline reads from its parent, applies the spring
filter, and its output becomes the next bone's parent world transform.

```
// VRM hair chain — each bone in the strand has its own QuatSpring
HairRoot (TC)
  TransformPipeline
    TransformForkTRS
      TransformMapRotation { QuatSpring { stiffness: 0, damping: 0.3, gravity: [0,-0.8,0] } }
      TransformMergeTRS {}
    TransformPipelineOutput
      HairMid (TC)
        TransformPipeline
          TransformForkTRS
            TransformMapRotation { QuatSpring { stiffness: 0, damping: 0.3, gravity: [0,-0.8,0] } }
            TransformMergeTRS {}
          TransformPipelineOutput
            HairTip (TC)
              TransformPipeline
                ...
```

Each bone accumulates its parent's spring-filtered pose and adds its own spring response on
top. The result is the characteristic whip / follow behaviour of VRM secondary motion — the
tip lags more than the root because each stage adds its own inertia.

---

## State in TransformPipelineSystem

Spring state (angular velocity, previous rotation) lives in `TransformPipelineSystem`
alongside the existing `QuatTemporalFilter` state, keyed by **stage path** (component ID
chain from pipeline root to this op). This means:

- State persists across ticks automatically.
- State is reset if the pipeline component is removed or the path changes.
- No state is stored in the component itself (matches existing temporal filter design).

---

## MMS authoring

```
TransformPipeline {
    TransformForkTRS {
        TransformMapRotation {
            QuatSpring.jiggle()          // preset: stiffness=0, damping=0.3, gravity down
            // or:
            QuatSpring.spring()          // preset: stiffness=5, damping=0.5, light gravity
            // or explicit:
            QuatSpring {
                with_stiffness(2.0)
                with_damping(0.45)
                with_gravity([0, -0.6, 0])
                with_inertia(1.0)
            }
        }
        TransformMergeTRS {}
    }
    TransformPipelineOutput { T {} }
}
```

---

## Open questions

1. **World-space vs local-space gravity**: gravity as a world-space vector is natural for
   hair hanging down. But if the avatar tilts (e.g. lying down), world-space gravity keeps
   pulling the same direction, which may or may not be desired.

2. **Collision**: VRM spring bones support simple sphere/capsule collision to prevent hair
   clipping through the head. This is out of scope for an initial implementation but the
   spring state (world bone positions each tick) is the natural place to run it.

3. **Max angle constraint**: VRM spring bones can clamp the bone's rotation to a cone around
   its rest pose. Useful for skirt panels that shouldn't fold backward. A `max_angle: f32`
   field would express this.

4. **Reset on teleport**: if the avatar teleports (large sudden position change), spring
   state should be reset to rest to avoid violent snapping. The pipeline system's state
   reset path (already used when component IDs change) would handle this if the pipeline
   is re-initialised on teleport.
