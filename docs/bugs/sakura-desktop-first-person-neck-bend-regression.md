# Sakura desktop first-person look-down lacks expected head/neck response

Date: 2026-10-28

Status: body-surface clipping reproduced; camera mount and near-plane corrections implemented.
CPU surface checks and headless Vulkan captures pass. User desktop retest reports improvement
(2026-10-28); broader pose/visibility tuning remains open.

Related: [shared AVC setup](../task/shared-avc-setup-and-untracked-walking.md),
[native head/neck/body constraint](../task/xr-head-neck-bend-and-body-offset.md),
[desktop/XR camera attachment contract](../task/avc-desktop-xr-camera-attachment-contract.md).

## User observation

After the shared AVC/untracked walking change, looking down in first person in
`examples/sakura-drone-conversion-desktop.mms` does not show the expected
head/neck solver behavior. Other tested behavior generally seems to work.

The user suspects the change introduced an overly simple desktop/XR behavior
split. Treat that as an investigation hypothesis: visual feedback does not yet
establish whether the solver is skipped, falls back, receives the wrong basis,
or has its output hidden/overwritten by another transform owner.

## Reproduction and expected behavior

```sh
cargo run --release -- load examples/sakura-drone-conversion-desktop.mms
```

While standing still, look down in first person and inspect the visible body and
mirror. Expected: the intended bounded neck/head response and connected camera
arc remain active, with no body/input drift. Desktop look supplies an intentional
orientation, whereas XR supplies a measured pose; that input distinction must
not silently remove the coordinated head/neck behavior from desktop.

## Relevant recent change and test gap

Desktop baseline commit `db0ce631` predates the engine refactor. The `1c3ef331`
change kept the desktop head attached to the imported neck, used a virtual
look target, and solved head-local orientation rather than inheriting it from a
spliced head mount. XR still uses its displaced-head topology.

At the time of the report, the desktop integration check proved camera rotation,
walking and fallback stability, but did not assert neck flexion or connected
geometry in the actual scene. The strengthened check now asserts those properties
as well as return to neutral and absence of planar body-anchor drift.

## Investigation tracker

- [ ] Compare the committed desktop baseline with the current working tree
  using the same scene, input angles and camera placement.
- [ ] Observe the real AVC's head/neck constraint status, bind diagnostics,
  enable flags and selected desktop/fallback path during downward look.
- [ ] Inspect the driven pose, virtual target, neck/head local/world transforms
  and C3D attachment/basis before and after the solve.
- [ ] Check pose-write/propagation order for later overwrites and distinguish
  a missing solve from incorrect camera-relative presentation of its result.
- [x] Correct the shared setup/desktop embodiment boundary while preserving
  NPC skeleton placement and XR tracked-head/view authority.
- [x] Add a regression that exercises the actual desktop scene and asserts neck
  flexion/connected geometry across downward look, not just camera rotation.

## Implemented correction

Input and InputXR both initialize the authoritative spliced head. The added
attached-head orientation path and its special legacy fallback have been removed.
Desktop still derives its head target from look and locomotion, while XR retains
its measured pose; both calculate the body from the desired head position and
calibrated neck displacement. Untracked NPC setup and walking remain separate.

Automated checks confirm real-scene downward look bends the neck within limits,
keeps the neck/head endpoints connected, preserves the planar body anchor and
returns to neutral on upward look. Existing XR alignment and NPC walking checks
also pass. These geometry checks alone did not establish the later-reported visible clipping failure;
the surface reproduction below supplies that missing evidence.

## Second smoke report: shirt/chest clipped away

After restoring the shared splice, the user still saw torso/leg interiors during
first-person downward look. Joint closure and neck bending did not catch that
visible failure. The next regression probes the actual CPU-skinned body mesh
from the scene's authored C3D, including front/back face orientation and clip depth.

The original scene fails that check near vertical downward look: the first outer
shirt/chest surface is at camera-space depth `0.08076686 m`, while the C3D near
plane is `0.1 m`. That front surface is discarded, exposing geometry farther
inside/below the avatar. This establishes a clipping failure even though the
head/neck solver is active; it does not mean every reported view issue has the
same cause.

The scene also attached its camera beneath the head in a GLTFInitialized
callback, outside AVC's camera-discovery/calibration contract. It used two nested
transform wrappers, whereas AVC supports one direct wrapper around C3D/CXR.

Corrections:

- Declare one camera transform wrapper directly under AVC so its eye offset is
  discovered during initialization and it mounts at the mapped camera anchor.
- Set `near(0.01)` on this first-person C3D, matching the existing XR near distance.
  The general C3D default is unchanged.
- Assert head translation relative to the body, actual outer-body front faces
  and near-plane survival during downward pitch and combined +/-35-degree roll.
  Reuse the existing XR CPU-skinned surface probe for the real desktop scene.

The ignored Vulkan check `sakura_desktop_first_person_body_surfaces_render` uses
the authored C3D pose/projection and writes `/tmp/sakura-desktop-down.png`,
`/tmp/sakura-desktop-down-roll-left.png` and
`/tmp/sakura-desktop-down-roll-right.png`. The captures were inspected: the
downward view shows the outer shirt/chest. Hair occupies the foreground in one
rolled view; head/hair visibility policy remains separate from body clipping.

## Acceptance

- Downward desktop look produces the intended bounded neck/head behavior and
  connected camera/body geometry; repeated input does not accumulate drift.
- Looking back up restores the neutral response coherently.
- First-person and mirror observations agree with the solved skeleton.
- XR tracking and head/eye alignment, NPC initialization and desktop movement
  remain correct. Record visual desktop confirmation before closing the issue.

## User retest

On 2026-10-28, after the single-wrapper camera calibration and 1 cm near-plane correction, the
user reported that desktop first-person body visibility seems better. Record this as improvement
confirmation, not complete acceptance of every pitch/roll, mirror or hair-visibility case. Desktop
gun holding remains tracked separately and was not changed by this correction.
