# Sakura desktop first-person look-down lacks expected head/neck response

Date: 2026-10-28

Status: open reported regression; root cause unconfirmed.

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

Desktop baseline commit `db0ce631` predates the engine refactor. The subsequent
change keeps the desktop head attached to the imported neck, uses a virtual
look target, and solves head-local orientation rather than inheriting it from a
spliced head mount. XR still uses its displaced-head topology.

The desktop integration check proves camera movement/rotation, retained head
attachment, walking and fallback stability. It does not assert that look-down
produces the intended neck flexion and camera/body geometry in the actual MMS
scene. Passing isolated head/neck tests therefore does not close this report.

## Investigation tracker

- [ ] Compare the committed desktop baseline with the current working tree
  using the same scene, input angles and camera placement.
- [ ] Observe the real AVC's head/neck constraint status, bind diagnostics,
  enable flags and selected desktop/fallback path during downward look.
- [ ] Inspect the driven pose, virtual target, neck/head local/world transforms
  and C3D attachment/basis before and after the solve.
- [ ] Check pose-write/propagation order for later overwrites and distinguish
  a missing solve from incorrect camera-relative presentation of its result.
- [ ] Correct the shared setup/desktop embodiment boundary while preserving
  NPC skeleton placement and XR tracked-head/view authority.
- [ ] Add a regression that exercises the actual desktop scene and asserts neck
  flexion/connected geometry across downward look, not just camera rotation.

## Acceptance

- Downward desktop look produces the intended bounded neck/head behavior and
  connected camera/body geometry; repeated input does not accumulate drift.
- Looking back up restores the neutral response coherently.
- First-person and mirror observations agree with the solved skeleton.
- XR tracking and head/eye alignment, NPC initialization and desktop movement
  remain correct. Record visual desktop confirmation before closing the issue.
