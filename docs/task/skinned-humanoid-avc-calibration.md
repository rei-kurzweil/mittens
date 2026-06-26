# Task: Skinned humanoid AVC calibration and comparison demo

Date: 2026-06-26

Status: active investigation and instrumentation task.

This task exists because `pc-rei.hoodie.glb` and `bisket.11.0.glb` are both
VRoid-style humanoid avatars, but they do not behave the same under the same
`AvatarControlComponent` VR configuration.

Current observed issue:

- `examples/vtuber-mirror-example.mms` with `bisket` looks broadly usable
- `examples/vr-input.rs` / updated `pc-rei` setup does not
- head position and arm IK for `pc-rei` still look wrong even after aligning
  the authored AVC setup more closely with the bisket example
- the arms often look close to inverted, and the result feels like the arm IK
  basis may not match the model's actual shoulder / torso / head layout

This is both:

- a bug note, because the current behavior is wrong for `pc-rei`
- and an implementation task, because we need a dedicated comparison demo and
  instrumentation path to diagnose it cleanly

Related context:

- [docs/task/openxr-parity-gate-before-openvr.md](./openxr-parity-gate-before-openvr.md)
- [docs/task/avatar-control-head-driven-redesign.md](./avatar-control-head-driven-redesign.md)
- [docs/task/avatar-control-desktop-vs-vr-divergence.md](./avatar-control-desktop-vs-vr-divergence.md)
- [examples/vr-input.rs](../../examples/vr-input.rs)
- [examples/vtuber-mirror-example.mms](../../examples/vtuber-mirror-example.mms)
- [examples/bisket-vr-demo.mms](../../examples/bisket-vr-demo.mms)

---

## 1. Problem statement

The current problem is not just "different avatars need different tuning."

The stronger concern is:

- two similar humanoid rigs are being driven by the same AVC/XR topology
- one behaves plausibly
- the other behaves badly enough that head/arm placement looks structurally wrong

That suggests one or more of these may be true:

- the two armatures differ in important local-bone orientations or offsets
- the shoulder / upper-arm / hand rest transforms differ more than expected
- the head bone pivot and camera wrapper offset interact differently across rigs
- the arm IK pole / grip basis is being interpreted in a way that only happens
  to look correct on `bisket`
- some part of AVC is implicitly assuming a rig convention that is not actually
  shared by both models

We need a dedicated way to compare both models under the same runtime and the
same authored XR topology, rather than inferring from separate examples.

---

## 2. Goal

Build a dedicated calibration and comparison workflow for skinned humanoid AVC
behavior.

The workflow should answer:

1. what is structurally different between the `bisket` and `pc-rei` rigs?
2. which measured differences line up with the bad head/arm behavior?
3. is the problem primarily:
   - camera/head calibration
   - hand grip basis
   - pole-vector basis
   - shoulder/arm rest geometry
   - or a deeper AVC topology assumption?

The result should be one focused example and one clear instrumentation path,
not more ad hoc debugging inside unrelated demos.

---

## 3. Proposed demo

Create a new example:

- [examples/skinned-humanoid-avc-calibration.mms](../../examples/skinned-humanoid-avc-calibration.mms)

Even if the top-level scene file is `.mms`, the task explicitly allows most of
the logic to live in Rust if that is simpler.

The intended split is:

- `.mms` for lighting, simple environment, desktop camera placement, and UI shell
- Rust for instrumentation, model loading helpers, topology comparison, runtime
  swapping, and any debug output / overlays

---

## 4. Phase plan

### Phase 1: Rust-side rig comparison and measurement

Before building the full swap demo, add a focused Rust-side comparison path that
loads both models and reports the differences most likely to affect AVC.

Primary purpose:

- compare the rigs, not just the rendered appearance

Phase 1 should inspect and print or otherwise expose:

- head bone local transform
- neck bone local transform
- upper chest / chest / spine / hips local transforms where available
- left/right upper-arm local transforms
- left/right lower-arm local transforms
- left/right hand local transforms
- distances between:
  - head ↔ neck
  - neck ↔ upper chest
  - shoulder ↔ upper arm
  - upper arm ↔ lower arm
  - lower arm ↔ hand
- left/right shoulder width and hand rest offset relative to torso/head
- bone forward/up axes or enough transform data to derive likely basis mismatches

The emphasis should be on the torso/head/shoulder/arm chain, because that is
where the visible bug presents.

Phase 1 does not need full interactive avatar swapping yet.

It can be:

- a Rust example-side report in terminal output
- optional simple debug markers
- optional overlay labels

But it should give us comparable data for both models in one run.

### Phase 2: Interactive calibration demo

After the structural comparison exists, build the actual interactive demo scene.

Requirements:

- load both models in the same example
- allow switching between `bisket` and `pc-rei`
- keep one shared `InputVR {}`-driven setup
- detach and reattach the relevant subtree under the active `InputVR {}` path
  so the active avatar can be swapped without rebuilding the entire app
- expose clickable desktop-window controls to choose the active avatar

The scene therefore needs:

- a desktop `C3D`
- `Input {}` so the desktop camera can move
- a `Pointer`
- clickable UI options visible from the desktop camera

The point of Phase 2 is:

- switch between the two rigs under equivalent conditions
- make the mismatch obvious
- give us one stable place to iterate on AVC calibration

---

## 5. Intended scene behavior

The calibration example should make it easy to answer:

- does the bad `pc-rei` result persist when both rigs share the same authored
  topology in the same scene?
- does the issue track the model, or the scene setup?
- which measurements correspond to the visible arm inversion / head offset?

Useful runtime features for the example:

- a clear active-model indicator
- one-click swap between `bisket` and `pc-rei`
- optional side-by-side numeric readout or terminal dump of key measurements
- optional bone markers on head, neck, shoulders, elbows, and hands
- optional toggles for:
  - grip vs aim hand pose display
  - pole-vector visualization
  - camera wrapper visualization
  - rest-pose markers vs runtime-driven markers

Not all of those toggles are required in the first pass, but the example should
be designed to make them easy to add.

---

## 6. Implementation notes

### A. Keep the comparison conditions tight

Do not compare:

- one model in one example
- and the other model in a completely different example

That makes it too easy to confuse model differences with scene/setup
differences.

The new example should put both through the same AVC path as directly as
possible.

### B. Prefer one shared XR authored path

The demo should use one shared `VR` / `InputVR` topology and swap the avatar
subtree under it, rather than duplicating large amounts of scene logic.

### C. Favor instrumentation over guesswork

We should not keep tweaking:

- camera wrapper offsets
- hand grip rotations
- pole vectors

blindly.

The first milestone is to measure and compare the rigs well enough that tuning
changes have an explanation.

---

## 7. Deliverables

### Minimum deliverable

- this task note
- a Phase 1 comparison example or helper path in Rust
- ability to inspect or print the key torso/head/arm measurements for both rigs

### Next deliverable

- `examples/skinned-humanoid-avc-calibration.mms`
- desktop camera + pointer + clickable model switch controls
- runtime swap between `bisket` and `pc-rei` under one shared `InputVR` setup

### Stretch deliverable

- in-scene debug overlays/markers for relevant bones and measurement lines
- quick toggles for pole/grip/camera-wrapper visualization

---

## 8. Acceptance criteria

This task is complete when:

- there is one dedicated calibration example for comparing the two humanoid rigs
- we can switch between `bisket` and `pc-rei` without changing examples
- we have objective measurements for the torso/head/arm chain on both models
- the measurement output is good enough to support the next AVC fix
- we can state more precisely why `pc-rei` head/arm behavior is wrong under the
  same current AVC configuration

This task does not require fully fixing the `pc-rei` rig behavior yet.

Its purpose is to make the fix tractable.
