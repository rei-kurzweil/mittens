# Avatar third-person camera for posture diagnostics

AvatarControl can publish a third-person view for its attached Camera3D and
CameraXR. The view starts at an offset from the movement root and follows that
root in world space. CameraXR applies live headset motion and stereo eye
separation relative to the headset pose when third person starts. The tracking
frame, collision capsule, hand rays in XR, and network pose samples keep their
existing inputs.

## Sakura Twilight

Open Humanoid from the Panels dock and use Third person camera. V on desktop
and Y on the left XR controller switch the same setting. Set
`start_in_third_person` at the top of the scene to start there. The setting
follows the local player through avatar changes.

Launch with `CAT_DEBUG_BODY_SOLVER=1` and compare the local avatar alone with the same
avatar observed from a second peer. Try standing, kneeling, crawling, and
standing again on the roof and at street level. Check the solver's mode, head
height, floor height, and lower-body status when the knees bend backward or a
transition stalls. Start with the same avatar at its default height, then
repeat with a desired-height override on the tracked player.

Sakura transmits root, head, and hand poses. The avatar channel sends the model
choice. Desired height is read only when loading the local avatar, so a peer
loads that choice at its default height. Each peer evaluates the automatic
posture thresholds against its own retained floor and scaled standing height.
Entry is below 42% of standing head height, or below 55% if crouch is limited.
Exit is above 60%. The minimum dwell is 0.4 seconds. Without a retained floor,
the solver holds its current mode. If a fresh support sample is unavailable,
it can still use a previously acquired floor. The grass tiles are visuals
over a separate static ground collider.

The offline Lumae rooftop fixture enters horizontal at authored head-target
heights of 0.6 and 0.7 m above the roof. At 0.8 and 0.9 m it stays vertical,
reports `NoSupport` for the lower body, and puts the knees below the roof
despite retaining the roof height. This reproduces a stalled transition
without network traffic. A backward knee bend during horizontal crawling
still needs a separate reproduction.

## Source boundaries

- Keep movement-root framing with AvatarControl. If its component and MMS
  registries need further growth, split authored options and their bindings by
  component family, preserving the current direct dispatch.
- Keep XR eye projection in the projection module. A future split of
  `openxr_system.rs` should keep frame acquisition and submission together
  and move tracking and render camera publication behind their existing
  interfaces. Keep stage and world pose conversions explicit.
- A future split of `camera_system.rs` should separate window view publication
  from camera selection and lifetime. Retain transform-driven publication and
  reuse the cached projection when only the avatar view moves.
- Keep the rooftop player prefab responsible for local view state across avatar
  replacements. As the prefab and scene grow, extract their multiplayer and
  avatar-loading responsibilities into cohesive modules. Keep panel controls
  synchronized on change and rebuild them from the current state when opened.
- If the Sakura scene tests grow further, group dock and settings interaction
  checks separately from vehicle and city checks. Retain coverage for both XR
  and desktop and keep each scene loaded once per fixture.

Keep per-frame work limited to the active view. Avoid scanning the component
tree, adding collision queries, changing render passes, or introducing new
per-eye allocations. Validate desktop toggling while stationary, world-space
follow, XR stereo and headset motion, and existing tracking and lower-body tests.
