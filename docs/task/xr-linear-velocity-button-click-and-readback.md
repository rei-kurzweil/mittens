# Task: XR velocity buttons, editor selection, and readback

Status: implemented in code on 2026-09-24; headless click/readback and topology checks pass. Headset confirmation of ray-clicking, gizmo exclusion, grabbing, and panel restore remains pending before calling the interaction smoke test complete. Do this before gravity/contact work.

## Outcome

In `examples/mittens-corp-linear-velocity.mms`, an XR pointer click on either bespoke button must change Bisket's outer-root velocity exactly once, without selecting button internals in the editor. Update an X/Y/Z readout once per accepted click, not every frame. Keep the Settings editor panel, grabbable info panel, and normal XR interactions.

## Selection and click routing

First check the actual XR hit target and `Click` route through the generated button background, text glyphs, panel layout, editor selection, and gesture systems. The existing headless callback test directly dispatches `Click` to the button root; it does **not** prove that a headset ray reaches that root. `assets/components/button.mms` adds `Raycastable.enabled()` to its root, and layout grafts a raycastable onto its generated `__bg`; the label's generated renderables may still win the raycast or editor selection.

Try a `Selectable.off()` wrapper around the bespoke panel subtree, as in `examples/world-panel.mms`, so its glyphs and button geometry do not become editor gizmo targets. Confirm this affects editor selection only: the two buttons must remain ray-clickable and the panel's `Grabbable` anchor must still work. If the selectable marker alone does not route hits to the button root, fix the hit/action-target routing at the appropriate reusable UI boundary, not with per-glyph handlers or by disabling the XR pointer. Preserve minimize/restore and its rebuilt button handlers. Do not change global editor-panel defaults or the other Mittens Corp examples for this task.

## Parent-local velocity invariant

The intended pose-driver topology is `Velocity { T { ... } }`, matching `InputXR { T { ... } }`: `Velocity` drives its child transform. The nearest transform ancestor of `Velocity` supplies the parent-local frame for that child. A distinct nested velocity layer therefore needs a distinct transform between the two `Velocity` components, for example `Velocity { T { Velocity { T { ... } } } }`. Two drivers aimed at the same child transform/channel must not silently double-integrate it. Define clear behavior for a missing or ambiguous driven child transform and for a root `Velocity` with no transform ancestor.

The intended state is a linear velocity vector expressed in that **ancestor transform's local axes**, in metres per second. This is not necessarily the driven child's own rotated axes: its translation channel is parent-relative. `translate(delta)` uses that ancestor's orientation by default, or an explicit `rotation_basis` such as the XR eye; convert the chosen command direction into parent-local velocity before accumulation. `translate_world(delta)` accepts a world-axis command but also converts it into the same parent-local stored state. Neither method changes the storage space.

The scene and system now use that topology and store `VelocityComponent.linear_local_mps`; the previous `T { Velocity {} }`/world-space implementation has been replaced. If the ancestor rotates after a click, existing parent-local velocity turns with it. Integration compensates for parent scale when writing child translation, so scale does not silently change physical speed; singular parent bases are rejected. XR eye orientation is sampled at click time, converted to parent-local state, and does not continuously steer it. Gravity and other future world-directed drivers likewise need an explicit world-to-parent-local conversion at their input boundary.

The [physics driver terminology](../spec/physics/driver-terminology.md#transform-inheritance-versus-velocity) distinguishes transform inheritance, the driven target, the command direction basis, and separate velocity layers. Do not treat inherited HMD motion as an additional velocity driver.

## MMS velocity readback

Add a live, zero-argument `Velocity.linear()` accessor returning a three-number MMS array `[x, y, z]` of the **stored parent-local velocity** in metres per second. It is a snapshot of actual stored state, not an inferred direction relative to `rotation_basis`, and it must not mutate or advance integration. An eventual world-space accessor should be named explicitly (for example, `linear_world()`), not change the meaning of `linear()`. Validate the receiver and make the return type usable by MMS `print` and array indexing. Document that reading before any command yields `[0, 0, 0]`; each call reads the current component, not an authored builder copy.

The current `Velocity.translate(delta)` and `translate_world(delta)` host methods emit a `VelocityTranslate` intent and return `null`. The system applies that intent later. Therefore this is **not** a valid same-callback readback:

```mms
vel.translate([0.0, 0.0, -0.25])
print(vel.linear()) // currently reads the pre-click value
```

The live mutation executor now applies `VelocityTranslate` and emits one `DataEvent` named `VelocityChanged` on successful application, scoped to the `Velocity` component. The example calls `linear()` from that notification and updates only the readout text. No notification is emitted per fixed integration step; rejected commands (for example, XR pose not ready or non-finite result) produce a diagnostic and no false success event.

## Checks

- Headless: `Velocity.linear()` returns a numeric vec3 snapshot; one valid command changes it after intent processing, and immediate same-callback reads are not misleadingly treated as post-apply values.
- Headless: each successful command has one after-apply readout/notification with the stored parent-local value; rejected commands have none. Repeated forward/back clicks accumulate/cancel as before.
- Space contract: test a parent rotation after a click, a rotated/scaled parent, and the inner XR heading reference. A rotated parent turns existing velocity; scale does not silently change its physical speed.
- Topology: `Velocity { T { ... } }` drives that child; two layers separated by a transform compose; nested velocity components without an intervening transform do not acquire independent targets; missing/ambiguous target handling is explicit.
- Interaction: exercise a ray hit on the actual button label/background hierarchy, not only a synthetic `Click` sent directly to the button root. Assert no editor `SelectionChanged`/gizmo attachment for glyphs, while the intended `Click` handler runs once.
- Headset smoke test: forward and back clicks display one new velocity each, including after panel minimize/restore; no per-frame readout updates; grabbing the panel, Settings, HMD, hands, gamepad locomotion, and mirror still work.

The frequent `[velocity_system] dropped ...` messages observed during a slow XR run are a separate fixed-step/performance diagnostic. They are now accumulated and reported in batches to avoid per-frame terminal overhead; this does not address their underlying cause or imply that hiding editor panels solved it.
