# Task: Runtime mirror resolution and info panel slider

Date: 2026-10-12
Status: investigation and implementation planned; documentation only.

## Outcome

Investigate the smallest supported way to change mirror capture resolution at
runtime. Add a reusable `info_panel` containing a slider and a live label
showing the current resolution. Use it in mittens-corp and
mittens-corp-desktop to adjust their mirrors without restarting the scene.

## Existing foundation

- `src/engine/ecs/component/mirror.rs`: `Mirror.quality(N)` initializes the
  resolution preference, clamped to 64..=2048, default 512.
- `src/engine/ecs/system/mirror_system.rs` reads live component quality and
  publishes `resolution_scale = quality / 1024` to VisualWorld.
- `src/engine/graphics/vulkano_renderer.rs` derives the actual capture width
  and height from that scale and mirror aspect ratio.
  `ensure_mirror_offscreen_targets` already compares extent, format, and view
  count before recreating targets. Verify the complete update/publication path
  before assuming a live component edit is sufficient.
- `assets/components/ui/info_panel.mms` provides the existing panel factory
  and accordion restore behavior. `mouth_response_panel.mms` demonstrates
  sliders, `SliderChanged` callbacks, and readouts in this panel system.

## Investigation and decisions

1. Trace quality from a runtime edit through MirrorSystem, VisualWorld, target
   allocation, and texture/material publication. Inspect the current MMS
   runtime method surface; add a narrow getter/setter if needed, using the
   existing `quality` terminology and clamp. Do not require scene rebuilding.
2. Define what the displayed resolution means. Quality is a base resolution
   preference, not necessarily a square extent. The label should distinguish
   the selected value from actual capture dimensions, for example
   `Mirror resolution: 1024 (capture 1536 × 1024)` for a 1.5-aspect mirror.
   If actual dimensions are not currently observable, add the smallest
   readout needed or label the selected value explicitly as the preference;
   do not display it as an exact square capture size.
3. Prefer one scene-wide slider that applies the selected preference to the
   scene's mirrors. Define handling for mirrors created later and for multiple
   aspect ratios; show actual dimensions per mirror when they differ.
4. Choose discrete resolution steps within the existing range, initially
   considering 64, 128, 256, 512, 1024, 1440, and 2048 so the current corp
   setting remains representable. Apply only changed values; coalesce rapid
   slider motion if necessary to avoid allocating a new target for every
   pointer event.
5. Verify safe replacement of GPU targets, descriptors, and runtime textures
   while frames are in flight. Old resources must remain alive as required,
   then be released; returning to a previous setting must not accumulate
   orphaned captures. Preserve mono and stereo viewer-family behavior.

## Panel implementation

- Build a small reusable panel factory around `info_panel`, with a resolution
  slider and live resolution readout. Initialize from current mirror state.
- Include it in both corp examples where it can be reached with XR pointers
  and desktop input without blocking the mirror or walking route.
- Keep the label synchronized with the clamped/applied value, including
  changes through another supported runtime path. State how differing initial
  mirror preferences are shown before the slider applies one common value.
- Restore the body with its current value after accordion minimize/restore;
  avoid duplicate listeners or retained deleted slider components.
- Runtime resolution control does not require implementing adaptive mirror
  LOD. Keep that separate plan's authored-ceiling semantics in mind.

## Acceptance

- Changing the slider updates mirror sharpness and the label without a scene
  reload, on desktop and in XR. Both XR eyes remain correctly associated with
  their capture views.
- Exercise the minimum, maximum, current 1440 preference, rapid dragging,
  repeated changes, and panel minimize/restore. Invalid values are handled
  consistently by the runtime setter.
- Non-square mirrors preserve aspect ratio and show truthful dimensions or
  clearly named preferences. Mirror geometry and reflection orientation stay
  stable during target changes.
- Automated checks cover the runtime value path and unchanged-target reuse;
  a renderer smoke test verifies target/texture replacement and resource
  lifetime during repeated live changes.

## Related work

- [Mirror component](../spec/mirror-component.md)
- [Mirror system](../spec/mirror_system.md)
- [Adaptive mirror detail](../desktop/adaptive-mirrors.md)
- [Mirror viewer-family captures](mirror-viewer-family-captures.md)
