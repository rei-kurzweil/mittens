# InputVR

This document describes the public `InputVR` authored surface, backed internally by
`InputXRComponent`, which acts as the **headset pose driver** for a VR rig subtree.

The goal is to make headset motion available in the ECS the same way desktop camera motion is
available through:

```text
Input {
    Transform {
        Camera3D {}
    }
}
```

For VR, we want the analogous authoring shape:

```text
VR {}

InputVR {
    Transform {
        CameraXR {}
        GLTF { "vtuber-model.gltf" }
    }
}
```

Authored MMS should use `InputVR`, even though the internal implementation type is still
`InputXRComponent`.

## Problem

Right now, attaching content under an XR rig transform does **not** automatically make it follow
the headset pose in the same way attaching content under a desktop input rig follows desktop
movement.

That is because the current XR stack splits responsibility like this:

- `OpenXRComponent` enables the OpenXR runtime/session
- `CameraXRComponent` selects which XR rig transform is considered the active XR camera rig
- `OpenXRSystem::render_xr(...)` publishes per-eye camera matrices into `VisualWorld`

What is **missing** is a component that says:

- “take the headset pose each frame”
- “drive a normal ECS `TransformComponent` child with that pose”
- “let everything parented under that transform follow naturally via topology”

So today, `CameraXRComponent` has a bit of “magic” around active-rig selection, but it is not the
same thing as a general-purpose ECS pose driver for the headset.

## Current behavior

### `CameraXRComponent` today

`CameraXRComponent` should be thought of as:

- an XR camera/routing marker
- an active-rig selector for `CameraTarget::Xr`
- the rig basis that `OpenXRSystem` uses when composing headset/controller poses for rendering and
  controller-driven transforms

It should **not** be thought of as:

- “the headset transform itself”
- “a component that drives its children from the HMD pose”

This distinction matters because it explains why a subtree attached “to the XR rig” may still fail
to move the way an author expects.

### VRHand vs headset tracking

We already have a useful precedent for controllers:

```text
VRHand {
    Transform {
        ... children to be driven by controller pose ...
    }
}
```

`VRHandComponent` works because `OpenXRSystem` explicitly finds a **direct
`TransformComponent` child** and writes the resolved controller pose into it.

There is no equivalent headset-driven component yet.

## Proposal

Introduce:

- public authored surface: `InputVR`
- internal engine component: `InputXRComponent`

Semantics:

- `InputVR` registers with `OpenXRSystem`
- each tick/frame, `OpenXRSystem` resolves the current headset/root XR rig pose
- it finds a direct `TransformComponent` child of `InputVR`
- it writes the headset/root pose into that transform
- everything under that transform inherits the motion through normal transform propagation

Authoring pattern:

```text
VR {}

InputVR {
    Transform {
        CameraXR {}
        AvatarRoot {
            GLTF { "vtuber-model.gltf" }
        }
    }
}
```

Or more compactly:

```text
InputVR {
    Transform {
        CameraXR {}
        GLTF { "vtuber-model.gltf" }
    }
}
```

This makes the headset-driven XR rig explicit and inspectable in ECS.

## Why `InputVR` instead of overloading `CameraXR`

`CameraXRComponent` already has a clear job:

- identify/select the active XR render rig

If we also make it implicitly drive child transforms from headset tracking, then `CameraXR` becomes
two things at once:

- render-target / active-rig selector
- headset input/pose driver

That coupling makes it harder to reason about topology and harder to reuse the same rig transform
for other patterns.

`InputXRComponent` keeps the responsibilities parallel with desktop input:

- `InputComponent` = source/driver of movement intent for desktop
- `InputVR` / `InputXRComponent` = source/driver of headset pose for VR
- `Camera3DComponent` / `CameraXRComponent` = camera/render selection components that live under
  the driven transform

## Proposed semantics in detail

### Minimal shape

`InputVR` should follow the same pattern as `VRHand`:

- marker/config component
- owns no transform directly
- drives a direct `TransformComponent` child, if present

Expected shape:

```text
InputVR
  Transform
    CameraXR
    ... anything that should follow the headset rig ...
```

### Pose source

The pose source should be the XR headset/root rig transform, not per-eye transforms.

That means:

- `CameraXR` still renders stereo left/right views
- `InputVR` drives the shared parent rig transform that both eyes conceptually move with

This avoids exposing “two cameras” as two independent ECS transforms for authoring. The authoring
model should remain one headset rig transform.

### Space

`InputVR` should drive its child transform in the same reference space used for XR camera rig
composition today.

That keeps headset motion and controller motion in the same rig space and avoids a mismatch where:

- controllers are composed relative to the active XR rig
- but the headset-authored subtree lives somewhere else

### Lifecycle

On init:

- `InputVR` should register with the XR system

On cleanup:

- it should unregister

At runtime:

- if XR is unavailable or session is not running, the driven transform simply keeps its last value
  or remains unchanged, depending on engine policy

## Why this helps the VTuber case

For the VTuber setup, we want a single subtree that follows the headset rig, while also allowing
controller-driven wrists/hands to be inserted into the armature.

That looks like:

```text
VR {}

InputVR {
    Transform {
        CameraXR {}
        AvatarRoot {
            GLTF { "vtuber-model.gltf" }
            ... later splice VRHand-driven wrist branches into the spawned armature ...
        }
    }
}
```

Benefits:

- avatar root follows HMD movement naturally
- camera and avatar share one explicit rig transform
- controller-driven hand/wrist branches can still be inserted deeper in the armature
- transform filters/pipelines remain normal ECS topology rather than XR-specific special cases

## Non-goals

This proposal does **not** mean:

- `CameraXR` should disappear
- per-eye views should become ECS child cameras with separate transform drivers
- `InputVR` should replace `VRHand`
- headset motion filtering must be built into `InputVR`

Instead:

- `InputVR` is for headset/root pose driving
- `VRHand` is for controller/hand-root pose driving
- transform pipelines stay a separate concern that can be inserted below either one

## Interaction with transform pipelines

`InputVR` should be pipeline-friendly in the same way `VRHand` is intended to be.

That means the useful authored topology is:

```text
InputVR {
    Transform {
        TransformForkTRS {
            ... optional smoothing / remapping / offsets ...
            CameraXR {}
            GLTF { "vtuber-model.gltf" }
        }
    }
}
```

or a simpler direct form with no pipeline.

The key point is that `InputVR` should drive a real transform node, not bypass the ECS transform
graph with camera-only special handling.

## Suggested API/component sketch

Minimal starting point:

```text
InputXRComponent {
    enabled: bool,
}
```

Possible future fields:

- tracking origin override / reference-space preference
- pose source mode if we later distinguish stage/head/local-floor semantics
- optional recenter behavior flags

But the first version should stay minimal.

## Expected engine changes later

When we implement this, likely changes would include:

1. Keep `InputXRComponent` as the engine-facing implementation type
2. Add register/remove intent types similar to `VRHand`
3. Track registered `InputVR` IDs in `OpenXRSystem`
4. Resolve headset/root pose each frame/tick
5. Drive the direct `TransformComponent` child
6. Keep `CameraXRComponent` focused on active XR rig/camera selection

## Open question

One subtle design question is whether the active XR rig should be:

- the `CameraXRComponent` node itself, or
- the driven `TransformComponent` child under `InputVR`

Recommendation:

- keep `CameraXRComponent` parented under the driven transform
- treat the driven transform as the actual rig space the rest of the world should attach to
- let `CameraXRComponent` continue to be the active camera marker within that rig subtree

That matches the desktop pattern more closely and keeps authored topology intuitive.

## Summary

`CameraXRComponent` is currently doing camera-selection work, not general headset transform driving.

We should keep `InputXRComponent` internally so authors can write:

```text
InputVR {
    Transform {
        CameraXR {}
        GLTF { "vtuber-model.gltf" }
    }
}
```

and have that subtree follow the headset/root XR pose through a normal ECS transform node.
