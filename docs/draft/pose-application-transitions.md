# Transitionable pose applications (Phase 2)

This document reserves the persistent API that follows the stateless Phase 1 methods. It is
design-only; `PoseApplicationInstance` and `bind` are not implemented yet.

A `PoseApplicationInstance` represents one pose blended into one target GLTF. Instances are
GLTF-specific, so the same pose asset can be bound independently to multiple avatars. Each
instance exposes its pose, target GLTF, current blend amount, and `set_blend_amount(amount)`, and
owns a directly nested `Transition {}` component.

Calling `set_blend_amount` transitions the instance's scalar blend amount from its current value
to the requested value. Every transition sample reapplies the pose to that instance's target at
the sampled weight. Pose blend amount becomes one more transitionable parameter alongside
transform, emissive, and the other supported properties. The transition belongs to the
application instance; it does not install one transition per bone.

The existing `pose.apply_blended(target, amount)` remains the stateless v1 path. The future
factory/binding shape is intentionally reserved as:

```javascript
let pose_instance = pose.bind(gltf) {
    Transition {
        duration_beats(0.5)
        ease_in_out_sine()
        replace_same_target()
    }
}

pose_instance.set_blend_amount(1.0)
```
