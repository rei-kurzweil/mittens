# Vtuber Mirror Example Checklist

Follow-on task for [render-view-mirror-inventory.md](/home/rei/_/cat-engine/docs/task/render-view-mirror-inventory.md).

This doc focuses on the first concrete implementation steps and the example/demo surface we should use while finishing mirror rendering.

## Goal

Build a `vtuber-mirror-example` that is useful for both:

- validating mirror render-view scheduling
- validating mirror material / runtime-texture publication on a recognizable scene

The intended scene is:

- VR-first
- based on the `bisket-vr-demo` topology
- without the bone-debug scaffolding in the Rust harness
- staged inside a Roman temple environment with white marble columns
- with a cloud background spawned from the existing Rust helper into a `Background.with_occlusion_and_lighting()` layer

## Related source files

- Inventory / architecture: [render-view-mirror-inventory.md](/home/rei/_/cat-engine/docs/task/render-view-mirror-inventory.md)
- Base MMS scene: [examples/bisket-vr-demo.mms](/home/rei/_/cat-engine/examples/bisket-vr-demo.mms)
- Base Rust harness: [examples/bisket-vr-demo.rs](/home/rei/_/cat-engine/examples/bisket-vr-demo.rs)
- Cloud helper: [examples/example_util/mod.rs](/home/rei/_/cat-engine/examples/example_util/mod.rs)
- Existing cloud usage examples:
  - [examples/vtuber-example.rs](/home/rei/_/cat-engine/examples/vtuber-example.rs)
  - [examples/vtuber-joints-example.rs](/home/rei/_/cat-engine/examples/vtuber-joints-example.rs)

## First-step checklist

### 1. Create the example pair

- [ ] Add `examples/vtuber-mirror-example.mms`
- [ ] Add `examples/vtuber-mirror-example.rs`
- [ ] Base both files on `bisket-vr-demo.{mms,rs}`
- [ ] Remove the bone-dump / marker-debug logic from the Rust file
- [ ] Keep the OpenXR bring-up and MMS loading flow as close to `bisket-vr-demo.rs` as possible

The Rust file should be a clean runtime harness, not a debugging harness.

### 2. Preserve the useful VR topology

- [ ] Keep the `InputXR` + `AVC` + `CameraXR` topology from `bisket-vr-demo.mms`
- [ ] Keep the avatar asset and hand/controller routing initially unchanged
- [ ] Keep any minimum floor / collision setup needed for VR movement and visual reference
- [ ] Remove unrelated repro/debug scene dressing from the current demo once the new temple scene replaces it

The example should stay close enough to `bisket-vr-demo` that mirror regressions are easier to compare.

### 3. Add a mirror-focused scene

- [ ] Author at least one obvious planar mirror surface in MMS
- [ ] Place the mirror where both head motion and controller motion make the reflection easy to judge
- [ ] Make the mirror large enough that per-eye issues will be visible in XR
- [ ] Keep the surrounding geometry simple enough that incorrect recursion / self-inclusion is easy to spot

For the first pass, prefer one hero mirror over several mirrors.

### 4. Build the Roman temple environment

- [ ] Replace the current simple wall/cube dressing with a Roman temple composition
- [ ] Use white / near-white marble columns
- [ ] Use rectangular columns if we do not already have a practical cylinder path in example authoring
- [ ] Add `circle_2d` rings or trim pieces near the top and bottom of columns where useful
- [ ] Add a floor, raised platform, lintel/roof mass, and enough symmetry to make mirror errors visually obvious

Notes:

- `circle_2d` support already exists in the engine mesh set, so decorative ring caps are viable.
- We do not need perfect classical architecture. We do need high-contrast, repeated forms that make reflection mistakes obvious.

### 5. Add the cloud background from Rust

- [ ] In the Rust harness, create a `BackgroundComponent::new().with_occlusion_and_lighting()`
- [ ] Add that background root to the universe
- [ ] Import and use `example_util::CloudRingParams`
- [ ] Call `example_util::spawn_cloud_ring(&mut universe, bg_root, cloud_params)`
- [ ] Tune cloud count / jitter / height for a broad background layer behind the temple

This should stay on the Rust side for now rather than trying to proceduralize the clouds in MMS.

### 6. Decide the initial render-view expectations for this demo

- [ ] Document the intended behavior for the example when only XR is active:
  - no desktop scene view if no `Camera3D` or `Camera2D` is active
  - XR eye views plus the mirror render views required for XR
- [ ] Treat one mirror as one logical unit that may require multiple related render views
- [ ] Use this example as the first regression surface for verifying mirror render-view count

### 7. Add lightweight observability

- [ ] Print or expose enough renderer stats to confirm:
  - mirror logical unit count
  - mirror render-view count
  - XR scene-view count
  - total scene-view count
- [ ] Make it easy to tell whether mirrors are actually rendered, not just bound to placeholder textures

This can start as temporary debug logging if the renderer stats component is not ready yet.

## Recommended implementation order

1. Scaffold `vtuber-mirror-example.{rs,mms}` from `bisket-vr-demo`
2. Remove Rust-side bone-debug code
3. Add the temple scene and one hero mirror in MMS
4. Add Rust-side cloud background using `example_util::spawn_cloud_ring`
5. Use the example to finish mirror render-pass scheduling
6. Add render-view counting/observability while validating the example

## Open scene-design constraints

- If cylindrical columns are awkward in current example authoring, use stacked rectangular shafts and decorative `circle_2d` trim instead of blocking on new geometry.
- Keep the temple materials bright enough that mirror and stereo errors are obvious.
- Keep the background clouds behind the temple mass so they help depth/readability without competing with the mirror.

## Definition of done for the example

- `vtuber-mirror-example` runs as a normal example
- scene is visibly a temple-like mirror demo, not just a copied debug room
- clouds are spawned through the shared Rust helper into a background layer
- mirror behavior is visible and testable in XR
- the example is suitable as the primary repro surface for ongoing mirror/render-view work
