# Dead / unused code (rustc warnings)

This doc is a snapshot of what `cargo check --examples` currently reports as unused/dead code.
It’s meant to help decide what to delete vs. what to keep (but rename/underscore) to avoid warning spam.

## “Dead code” vs “unused warnings”

- **dead_code**: functions/fields/variants that are never referenced in the current build.
- **unused_variables**: locals/params that exist but aren’t used.
- **private_interfaces**: a public API exposes a type that isn’t public.

Not all of these should necessarily be deleted; some are placeholders for future features.

## Current warnings (as of 2026-03-02)

### examples/vtuber-joints-example.rs

These helpers were used by the old joint-animation path and are now unused:

- `quat_from_axis_angle`
- `quat_conjugate`
- `quat_normalize`
- `quat_rotate_vec3`
- `quat_mul`

Suggested cleanup:
- Delete them if you don’t plan to re-enable joint animation soon.
- Or gate them behind `#[cfg(test)]` / move into a shared math util module if they’ll be reused.

### src/engine/ecs/system/animation_system.rs

- Unused parameter: `queue: &mut CommandQueue` (warning suggests renaming to `_queue`).

Suggested cleanup:
- If the trait/signature requires it, rename to `_queue`.
- If it’s not required, remove the parameter from the function signature and update callers.

### src/engine/ecs/system/light_system.rs

- Locals assigned but never read:
  - `visited_nodes`
  - `updated_lights`

Suggested cleanup:
- If they’re intended debug counters, rename to `_visited_nodes` / `_updated_lights`.
- Or print/log them behind a debug flag.

### src/engine/ecs/command_queue.rs

- `Command::RemoveRenderable` contains `component_id` but the field is never read.
- Variants never constructed:
  - `Command::RemoveTransform`
  - `Command::RemoveCamera`

Suggested cleanup:
- If `RemoveRenderable` doesn’t need the payload, remove the `component_id` field.
- If `RemoveTransform` / `RemoveCamera` are planned but not implemented, consider:
  - deleting them for now, or
  - leaving them but adding a comment + `#[allow(dead_code)]` on those variants to document intent.

### src/engine/ecs/system/animation_system_evaluator.rs

- Associated items never used:
  - `AnimationEvaluator::new`
  - `AnimationEvaluator::with_lookahead_sec`

Suggested cleanup:
- Remove if this evaluator is obsolete.
- Or if it’s an alternate path, ensure the code path is actually referenced (or annotate with `#[allow(dead_code)]` + comment).

### src/engine/ecs/system/bvh_system.rs

- `RenderableAabb::new` is never used.

Suggested cleanup:
- Delete if construction always happens via other means.
- Or use it in the one obvious call site (if it was meant as the canonical constructor).

### src/engine/ecs/system/gizmo_system.rs

- `GizmoSystem::gizmos_for_hit_renderable(...)` is never used.

Suggested cleanup:
- Delete if the function is a leftover from an older approach.
- Or use it from the picking path if the intent is “find all gizmos associated with this hit”.

### src/engine/ecs/system/gltf_system.rs

- Field never read: `ImportedSkin::skeleton_root`.

Suggested cleanup:
- If unused, remove the field.
- If it will be used for retargeting / skeleton debugging, keep it but add a comment and `#[allow(dead_code)]` on the field.

### src/engine/ecs/system/openxr_system.rs

- Fields never read:
  - `OpenXRSessionState::vk_command_pool`
  - `ControllerInput::{aim_pose, grip_pose, left, right}`

Note: This is very likely “future plumbing” for XR interaction.

Suggested cleanup:
- If these are required for later features, add targeted `#[allow(dead_code)]` + intent comments.
- If they’re truly unused, remove them to reduce maintenance burden.

### src/engine/graphics/vulkano_swapchain.rs

- Field never read: `VulkanoSwapchainState::surface`.

Suggested cleanup:
- Remove the field if it isn’t used.
- Or if it’s useful for debugging/recreation, either use it or annotate `#[allow(dead_code)]`.

### src/engine/ecs/rx/signal.rs (private_interfaces)

- `SignalValue::OscillatorScheduleSetNote::pitch` is `pub` but its type `NotePitch` is `pub(crate)`.

Suggested cleanup:
- Either make `NotePitch` public (if it’s truly part of the public signal API), or
- make the signal field/type not publicly exposed (e.g. keep it `pub(crate)` or change the representation to a public type).

## A note on existing `#[allow(dead_code)]`

Some files already contain `#[allow(dead_code)]` blocks (e.g. parts of `openxr_system.rs` and `vulkano_renderer.rs`).
That’s fine when it’s intentional, but it’s best when:

- the allow is **as narrow as possible** (field/function-level, not whole module), and
- accompanied by a short comment explaining the intended future use.
