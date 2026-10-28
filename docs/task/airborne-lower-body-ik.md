# Airborne lower-body IK

Date: 2026-10-28

Status: planned. This tracker records the behavior and design work; the airborne
solver is not implemented.

Related: [procedural walking IK](procedural-lower-body-walking-ik.md) and
[untracked AVC/NPC walking eligibility](lumae-pacing-and-npc-walking.md).

## Intended behavior

When an avatar moves without ground support, its legs hang with a modest
trailing bend. As movement slows or stops in midair, the legs smoothly settle
toward a relaxed pose: slightly bent knees or the authored rest pose, to be
chosen during implementation design. Landing smoothly transfers lower-body
ownership back to supported standing, crouching or walking.

Use actual avatar root movement, including Velocity, scripted transforms and
inherited ancestor translation. Do not require a separate flight command or
infer motion solely from a Velocity component's requested value.

## Architecture and constraints

- Add an airborne policy module alongside walking and crouching under
  `src/engine/ecs/system/ik/lower_body/`.
- Keep `LowerBodyIkSystem` as the single scheduler and leg-pose writer. The
  airborne module produces goals; it must not become a competing bone writer.
- Distinguish validated loss of ground support from missing calibration,
  invalid tracking or incomplete rig/capsule bindings. Those failures must not
  activate an airborne pose merely because walking is unavailable.
- Filter measured root motion and stabilize support transitions so tracker
  noise and brief contact changes do not flicker between policies.
- Reset retained motion history after teleports and rig changes rather than
  interpreting discontinuities as extreme flight speed.
- Preserve segment lengths and bounded joint motion. The pose must not drive
  locomotion, alter gravity or grounded state, or reposition the tracked camera.
- Preserve mounted-state exclusions and allow future tracked-foot ownership
  to take precedence over procedural goals.

Current walking releases its contacts after loss of support or excessive body
rise. The new policy should handle eligible unsupported avatars after that
release, rather than reusing planted walking contacts in midair. The shared
coordinator must also handle the transition back on landing.

## Decisions before implementation

- [ ] Choose whether legs trail opposite travel direction or primarily behind
  the avatar's facing, including backward and sideways travel.
- [ ] Choose the stationary airborne default: slightly bent knees or authored
  rest, and whether authors need a choice in the first slice.
- [ ] Define how vertical ascent/descent affects posture, including ordinary
  jumps and falls. Lack of support alone does not identify intentional flight.
- [ ] Define trailing limits, motion deadbands, smoothing and support-transition
  timing, including the treatment of a single supported foot.
- [ ] Define opt-out controls and how they compose with existing walking and
  crouch settings. No new public API is prescribed by this tracker.

## Work tracker

- [ ] Resolve the posture and eligibility decisions above.
- [ ] Implement retained airborne policy state and bounded leg goals.
- [ ] Integrate policy selection and smooth takeoff/landing transitions through
  the shared lower-body owner.
- [ ] Add automated coverage for the acceptance scenarios below.
- [ ] Provide a headset smoke test and record appearance/comfort tuning results.

This is a separate follow-up to enabling movement-driven walking for untracked
AVCs. It should reuse that shared rig/support setup rather than introduce a
second NPC-specific solver.

## Acceptance scenarios

- Forward, backward and sideways unsupported travel produce the selected
  trailing posture without stretching bones or producing nonfinite poses.
- Decelerating to a hover smoothly reaches the chosen relaxed posture; small
  motion noise does not repeatedly restart the moving pose.
- Ascent, descent, ordinary jumps and falls follow the chosen vertical-motion
  policy without requiring an explicit flight signal.
- Takeoff releases planted contacts; landing transfers to standing, crouch or
  walking without stale world-space foot targets or competing leg writers.
- Brief support loss, support regain and a single supported foot follow the
  agreed transition policy without visible state flicker.
- Teleports, invalid frame times, rig replacement and scale changes do not
  retain stale motion or produce exaggerated trailing poses.
- Mounted avatars, opted-out avatars and invalid/unready bindings retain their
  existing ownership/release behavior. Avatar instances keep independent state.
- Automated checks cover representative frame rates and both scripted and
  Velocity-driven root motion. Headset checks cover visible posture, transition
  smoothness and unchanged tracked head/camera placement.
