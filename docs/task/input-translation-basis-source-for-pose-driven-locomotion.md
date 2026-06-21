# Input Translation Basis Source for Pose-Driven Locomotion

## Summary

Add a generic way for `Input` / `InputTransformMode` to apply translation on its own driven
`Transform` while calculating movement directions from some other pose-driven transform.

This is needed for VR locomotion where:

- an outer `Input` should own locomotion translation
- an inner `InputXR` should continue to own HMD translation + rotation for the avatar
- `WASDRF` movement should be calculated relative to the HMD pose, not the outer locomotion
  transform's own rotation

The feature should be generic. It should not special-case `InputXR`; instead `InputTransformMode`
should accept a reference or selector identifying the transform whose rotation should be used as
the translation basis.

## Intended Runtime Behavior

Target authored topology:

```text
Input
  InputTransformMode
  Transform              <- locomotion transform written by InputSystem
    InputXR
      Transform          <- HMD-driven transform written by OpenXRSystem
        AVC
          ...
```

Behavior:

- `InputSystem` still writes translation only to the direct child `Transform` of `Input`
- `OpenXRSystem` still writes full pose to the direct child `Transform` of `InputXR`
- `InputSystem` may ignore its own controlled transform rotation and instead read rotation from a
  referenced pose-driven transform
- the referenced transform is used only as the movement-direction basis
- the referenced transform is never mutated by `InputSystem`

For the initial VR use case:

- outer `Input` rotation is disabled
- outer `Input` translation is enabled
- translation basis is the inner XR pose transform
- movement uses full HMD orientation, including pitch

## API / Data Model Changes

Extend `InputTransformModeComponent` with:

- `rotation_enabled: bool`
- `translation_basis_source: Option<ComponentRef>`

Semantics:

- `rotation_enabled = true` preserves current behavior
- `rotation_enabled = false` disables mouse-drag and Q/E rotation updates for the controlled
  transform while still allowing translation
- `translation_basis_source = None` preserves current behavior: translation uses the controlled
  transform's rotation
- `translation_basis_source = Some(...)` makes translation use the referenced transform's world
  rotation instead

Use the existing generic reference mechanism:

- `ComponentRef::Guid(uuid)`
- `ComponentRef::Query(selector)`

Prefer full `ComponentRef` rather than a raw selector-only string so this matches existing ECS
reference patterns such as `TransformParentComponent`.

## MMS Surface

Add builder/config support on `InputTransformMode` for:

- `rotation_disabled()`
- `translation_basis(ref_or_selector)`

Examples:

```mms
InputTransformMode.forward_z() {
    rotation_disabled()
    translation_basis("#xr_pose")
}
```

or, when available through object references:

```mms
InputTransformMode.forward_z() {
    rotation_disabled()
    translation_basis(@uuid:...)
}
```

`translation_basis(...)` should parse through the same `ComponentRef` path already used by other
 components that store references.

## InputSystem Changes

Update `InputSystem::process_input` so rotation ownership and translation basis are independent.

Required behavior:

- continue resolving the controlled transform as the direct child `Transform` of the `Input`
  component
- read `InputTransformModeComponent` if present
- skip rotation updates when `rotation_enabled == false`
- continue applying translation to the controlled transform
- choose the translation basis rotation as follows:
  - if `translation_basis_source` is unset, use the controlled transform rotation as today
  - if set, resolve the `ComponentRef`
  - from the resolved component, use that transform if it is a `TransformComponent`
  - otherwise walk to the nearest transform self-or-ancestor and use that
  - use the resolved transform's world rotation as the direction basis

Initial scope:

- use the referenced transform's full world quaternion
- do not add per-axis filtering or yaw-only projection in this task
- do not mutate or synchronize any state onto the referenced transform

Failure behavior:

- if the reference does not resolve, fall back to current self-rotation behavior
- avoid hard failure or panics
- optional one-time debug logging is acceptable

## `vtuber-mirror-example` Changes

Update [examples/vtuber-mirror-example.mms](/home/rei/_/cat-engine/examples/vtuber-mirror-example.mms)
to use nested locomotion + XR pose ownership:

- wrap the avatar XR subtree in an outer `I.speed(...)`
- configure outer `InputTransformMode.forward_z()` with:
  - `rotation_disabled()`
  - `translation_basis("#xr_pose")`
- keep the outer `Input` direct child `T` as the locomotion transform
- keep the inner `InputXR.on() { T { AVC { ... } } }` structure
- name the inner XR-driven transform `#xr_pose` so it can be referenced cleanly

This preserves the existing AVC/OpenXR ownership split:

- locomotion translation comes from outer `Input`
- HMD pose continues to come from inner `InputXR`
- AVC still sees the XR-driven transform as its direct parent pose driver

Also update the example's desktop camera:

- remove the current desktop `Input` free-fly camera wrapper
- replace it with a fixed authored `Transform` + `Camera3D`
- do not add `GrabHandle` in this task

## Constraints and Non-Goals

- do not special-case `InputXR` in the engine API
- do not redesign `InputComponent` ownership of its direct child transform
- do not change `OpenXRSystem`'s current direct-child transform ownership
- do not introduce a new transform-layer system just for this use case
- do not implement yaw-only, XZ-only, or translation filtering in this task
- do not implement VR grabbing of the desktop camera in this task

## Test Plan

### Unit / behavior coverage

- `InputTransformMode` round-trips `rotation_enabled` and `translation_basis_source`
- `translation_basis("#target")` resolves through `ComponentRef::Query`
- guid-backed refs resolve through `ComponentRef::Guid`
- when `rotation_disabled()` is set, mouse drag and Q/E do not rotate the controlled transform
- translation still works when rotation is disabled
- with no `translation_basis_source`, existing movement behavior is unchanged
- with a valid `translation_basis_source`, movement uses the referenced transform's world rotation
- if the ref resolves to a non-transform descendant, nearest transform self-or-ancestor resolution
  still finds the expected basis transform
- if the ref does not resolve, movement falls back to self-rotation behavior

### Example validation

- `vtuber-mirror-example` still loads and preserves XR avatar/controller behavior
- `WASDRF` moves the avatar rig through the world in VR
- movement follows full HMD orientation, including pitch
- desktop window uses a fixed spectator camera instead of the current free-fly camera

### Regression checks

- existing desktop `Input` examples keep current behavior by default
- `bisket-vr-demo` behavior is unchanged
- non-VR scenes that use `InputTransformMode.forward_z()` without new options are unchanged

## Assumptions

- the referenced pose source will usually be a named `Transform` under the same subtree, referenced
  by selector
- using full HMD orientation for movement is intentional for this task
- fallback-to-self behavior is preferable to failing closed when the translation basis cannot be
  resolved
