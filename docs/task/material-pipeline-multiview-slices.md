# Material pipeline consolidation and XR multiview slices

Date: 2026-10-27
Status: proposed implementation and validation plan; no renderer changes applied.
Source checkpoint: `fc8bffa6`.

The aim is to reduce repeated stereo preparation and command recording while
making new material definitions inherit renderer support through their base
contracts. Each slice below should remain reviewable and independently testable.
Pipeline consolidation is useful groundwork, but its performance benefit must be
measured separately from multiview and the already identified upload/cache fixes.

Related: [measured Sakura CPU/GPU findings](sakura-rooftop-cpu-profile.md),
[material contracts](material-instances-and-family-contracts.md), and
[current built-in authoring API](../draft/material-builtin-authoring-api.md).

## What exists today

`MaterialBase -> MaterialDefinition -> Material` already provides common
registration, identity, schema-backed values and capability resolution for
built-in and custom definitions. Built-ins still have renderer-specific packing,
program selection and many explicit pipeline fields. Common material authoring
does not mean common GPU pipeline construction is complete.

| Contract or feature | Current boundary |
| --- | --- |
| Ordinary surface | `MaterialBase()` / `MeshSurfaceV1`; custom opaque/alpha shading with an engine-owned vertex interface |
| Extracted emissive | `MaterialBase.extracted_emissive()`; opaque static foreground surfaces reuse fragment radiance for extraction |
| Planar reflective | `MaterialBase.planar_reflective()`; supplies the current viewer/eye reflection capture |
| Sharp/rough transmission | Built-in capability and render paths exist; a general custom transmissive base remains pending |
| Cached deformation | Existing built-in definitions select deformed programs; custom definitions currently use the fixed mesh instance contract |
| Stereo | `OpenXRSystem` loops over eyes, building and submitting each eye's command buffer separately |

Custom pipeline construction clones one shared template and substitutes the
fragment program, creating surface, transparent and optional extraction variants.
That makes shared infrastructure changes useful to future definitions. It does
not merge different shader programs into one GPU pipeline. Changing values on an
existing material instance should continue to reuse its program/pipeline.

The backend still has explicit Toon, Anime, unlit, grid, mirror, deformation,
outline, clipping and emission variants. Some share construction templates, but
selection includes handle switches and pipeline pointer comparisons. These are
concrete migration sites rather than evidence that every material needs a bespoke
multiview implementation.

Useful source entry points:

- [Material contracts and capabilities](../../src/engine/graphics/materials.rs).
- [Custom program/pipeline construction](../../src/engine/graphics/vulkano_custom_materials.rs).
- [Pipeline selection and draw recording](../../src/engine/graphics/vulkano_cbb.rs).
- [Renderer initialization and stereo recording](../../src/engine/graphics/vulkano_renderer.rs).
- [Custom shader interface validation](../../src/engine/graphics/custom_material.rs).
- [Device feature negotiation](../../src/engine/graphics/renderer_context.rs).
- [Borrowed XR image views and ownership](../../src/engine/graphics/xr_renderer.rs).

## What multiview changes

For dynamic rendering, the pipeline's view mask must match the rendering
invocation's view mask. An XR stereo variant would normally use `0b11`, while
the existing ordinary single-view path uses zero. Graphics pipelines participating
in that stereo pass therefore need compatible objects; desktop and single-view
captures may still need their existing variants. Creation happens at initialization
or on demand, with reuse thereafter. Compute deformation is outside this graphics
view-mask change. See [VkPipelineRenderingCreateInfo](https://docs.vulkan.org/refpages/latest/refpages/source/VkPipelineRenderingCreateInfo.html).

The device must support and explicitly enable multiview, with limits sufficient
for two views and the scene's draw instance indices. Current device creation
enables dynamic rendering but does not enable multiview. Capability negotiation
must preserve a working single-view fallback on the runtime-selected GPU. See
[multiview features](https://docs.vulkan.org/refpages/latest/refpages/source/VkPhysicalDeviceMultiviewFeatures.html).

An engine camera interface must supply both eyes' matrices and select the eye in
vertex and any camera-dependent fragment calculations. Vulkan supplies
`ViewIndex` for this purpose. Shaders that need no eye-dependent fragment inputs
may keep their fragment logic. See [ViewIndex](https://docs.vulkan.org/refpages/latest/refpages/source/ViewIndex.html).

Current built-in and several authored custom fragments directly declare the
single-view `CameraUBO`. The custom compiler validates that physical layout.
Changing only the engine vertex shader would therefore leave camera-dependent
shading incorrect or incompatible. Specify a versioned interface and migration
path for those fragments, including reflection and transmission sampling. Reuse
one engine declaration/helper convention rather than duplicating eye selection
in each newly authored material.

XR already uses an array swapchain, but the renderer currently exposes individual
eye views. Multiview needs a view spanning both layers, matching layered depth/MSAA
attachments, resolves, and correct image transitions. Runtime-owned image lifetime
and completion-before-release requirements remain unchanged.

Expected savings are fewer repeated scene draw commands, bindings and associated
Vulkano resource tracking. Shared CPU preparation and fewer queue submissions
require deliberate scheduling changes. Pixels still render for both eyes.
Transparency ordering, reflection captures and postprocessing are separate work;
they do not become correct or cheaper merely by setting the view mask.

## Proposed internal organization

Keep the three public material concepts. Resolve base capabilities into a small
internal pass/state description, then select a cached pipeline variant using
program/interface identity, pass role, alpha/depth/stencil state, geometry input,
view mode, attachment formats and sample count where applicable. This is a
proposed internal design, not a new public pipeline-authoring API.

Emission support, transmission, reflection resources and deformation should keep
their distinct meanings. Do not grow a separate material-base enum for every
combination. A base defines allowed contracts; a definition supplies shading and
a material instance supplies values. Pass eligibility and active scene effects
still determine which supported variants are actually needed.

Construct only required supported combinations and retain them safely. Measure
creation latency, count and memory, including desktop/XR switching and shader
retirement. Pipeline consolidation alone does not eliminate necessary GPU variants.

## Slices and acceptance checks

Smoke checks establish rendering correctness and resource safety. Performance
checks then compare warmed workloads without Nsight or validation layers. Every
slice records its revision, settings and results, and retains the old stereo path
for comparison until the replacement passes its gates.

### 0. Establish comparable CPU scopes and instrumentation controls

Split preparation, material/resource synchronization, instance uploads, command
recording/build and queue submission. Record both per-eye and whole-stereo scopes
without double-counting nested timers. Separate GPU execution and completion wait.
Add opt-in controls for detailed timing, timestamps and any future labels.

Smoke: toggle detailed instrumentation and confirm identical images, draw/job
counts, submission behavior and frame ownership. Ordinary application builds
must omit test-only hooks. Record whether validation or an external profiler is on.

Performance: repeat the current Sakura matrix and a many-draw opaque scene.
Measure instrumentation enabled/disabled separately; establish normal run-to-run
variation before attributing a small gain. The existing 6.698 ms is whole-stereo
CPU preparation/submission, not isolated command recording.

### 1. Consolidate pipeline recipes while preserving single-view rendering

Replace scattered selection/construction with shared internal recipes, starting
with ordinary custom surfaces and Toon/Anime opaque/cutout variants. Move remaining
families in small follow-ups, preserving their current state and shaders.

Smoke: compare both eye images against the old path for opaque, alpha cutout,
outlines, clipping and independent animated material revisions. Exercise material
retirement and MSAA changes. No feature should silently select a default shader.

Performance: hold draw count and view mode constant; compare preparation/recording
time, pipeline creation/count, binding allocations and retained memory. This
slice should show parity; do not credit it with multiview savings.

### 2. Introduce a shared camera interface in single-view mode

Define engine camera access for both stages and a versioned shader/reflection
contract. Migrate built-ins and representative custom fragments, with explicit
handling for existing `MeshSurfaceV1` declarations. Keep legacy definitions on a
compatible path until migrated; reject incompatible layouts clearly.

Smoke: asymmetric cameras expose wrong-eye matrices; test camera-dependent
specular/rim lighting, backgrounds, custom pool materials, desktop and mirrors.
Check schema reflection, authored material values and descriptor compatibility.

Performance: measure uniform bytes, descriptors and CPU preparation before/after.
Single-view output and timing should remain comparable. An interface refactor
alone should not trigger repeated pipeline creation or new per-draw uploads.

### 3. Prove opaque static multiview in an isolated fixture

Negotiate the feature/limits, add layered output/depth attachments, and create
one stereo pipeline through the shared recipe. Draw a minimal opaque scene once
into both layers using deliberately different cameras. Disable unrelated effects
in this fixture; do not introduce a mixed rendering path into the full scene yet.

Smoke: read back both layers and compare each to the corresponding single-view
reference. Test occlusion, instancing, layer isolation and forced fallback. Run
validation. Verify actual recorded draw count drops to one per shared batch.

Performance: sweep draw count at fixed geometry/pixel load. Compare recording,
build and submit CPU time plus GPU time. Report submission count explicitly;
multiview does not itself guarantee fewer submissions in our implementation.

### 4. Cover ordinary scene geometry and cached deformation

Extend the recipe to custom ordinary fragments, Toon/Anime, cutout, outlines,
backgrounds and supported clipping. Use one preparation of shared scene data
where valid, with stereo visibility covering objects visible to either eye.
Keep deformation output independent per avatar and shared across the two views.

Smoke: use two copies of one avatar with deliberately different poses and morph
weights, including unchanged poses and a moving tracked player. Test eye-edge
visibility, alpha holes, outline width, clip boundaries, sun/sky and shared
materials with distinct revisions. Existing custom deformation restrictions
remain explicit; extending that authoring contract is separate work.

Performance: repeat 0/1/2 Lumae static/moving and the hidden-draw control. Dirty
jobs, palette traffic and morph weights must not double with stereo. Attribute
recording savings separately from deformation/upload changes.

### 5. Add extracted emission and preserve postprocessing

Generate stereo emission variants from the same contract/recipe. Preserve main
surface/extraction material revisions and coverage. Initially retain per-eye
postprocessing if that is the simplest correct integration; combining its passes
is a separate measured change.

Smoke: compare emitted patterns, occluded LEDs, zero emission, cutout boundaries,
4x MSAA resolves and bloom in both eyes. Check that one eye's output cannot leak
into the other and that ordinary rendering remains correct with bloom disabled.

Performance: measure main scene, extraction and postprocessing separately, with
and without bloom. Record the remaining per-eye commands and attachment memory;
do not extrapolate the opaque-only improvement to the entire frame.

### 6. Integrate alpha ordering and built-in transmission

Choose an explicit policy for draws whose correct order differs between eyes.
They can remain per-eye initially. If mixing multiview opaque rendering with
per-eye effects, implement depth/color load/store, resolves and barriers as part
of this slice. Decide scheduling before assuming one global stereo draw stream.

Smoke: overlapping transparent surfaces whose ordering changes between eyes,
opaque occlusion, sharp refraction, rough transmission pyramids and stencil clips.
Verify scene color/depth snapshots belong to the correct eye and phase.

Performance: compare opaque-heavy and transparency-heavy scenes. Quantify fallback
draws, extra passes/transitions/copies and GPU time. A general custom transmissive
base can build on the proven resource contract later; it need not block the
built-in compatibility path.

### 7. Integrate planar reflection and other capture consumers

Make reflection camera data and sampled captures explicitly eye-aware. Choose
between layered captures and existing separate per-eye captures based on measured
cost and contract compatibility. Existing custom fragments use a non-arrayed 2D
sampler; layered sampling needs an interface migration or an explicit fallback.
Reflection generation and drawing the reflective surface are distinct tasks.

Smoke: oblique mirror/water views with different left/right reflection content,
reflected avatar, handedness, behind-plane clipping, multiple mirrors, resizing
and capture retirement while older frames retain resources. Check desktop too.

Performance: measure capture production, reflective surface recording, sampled
resources and memory separately. Establish whether layering captures is useful;
do not duplicate all reflection rendering to make the main stereo path simpler.

### 8. Enable full XR scenes and decide defaults

Connect layered views to acquired runtime images, preserve completion before
release, and exercise session/swapchain recreation and supported MSAA settings.
Select fallback for unsupported contracts/devices. Make pipeline reuse explicit
across view modes and measure startup/switching separately from steady state.

Smoke: run Sakura, the custom-material room and pool/reflection examples through
both paths, then check the headset for stereo depth, UI/pointers, clipping,
tracking and effects. Repeated acquisition/release and resize must not leak or
reuse resources still referenced by in-flight work.

Performance: repeat controlled comparisons and headset captures. Enable multiview
by default only for supported paths that retain correctness and demonstrate a
repeatable benefit without unacceptable GPU/memory regressions. Record paths that
still render per eye and the reason, rather than implying full conversion.

## Keep measured upload/cache fixes independent

The repeated immutable morph upload and 16-entry per-material binding cache churn
are confirmed separate problems. Fix and measure each in its own change. They can
precede the slices above; rerun the baseline afterward and keep all comparison
conditions identical. Multiview should not receive credit for their gains.

For morph reuse, cover arena growth and in-flight lifetime, independent weights
and poses, and unchanged frames. For binding retention, cover more than 16 resource
variants, animated revisions and retained command buffers without unbounded growth
or unsafe overwrites. Use the existing upload/allocation/eviction counters.

## Reusable validation and performance protocol

Existing smoke-test anchors, to extend as each slice lands:

```sh
cargo test --release --lib headless_tests -- --ignored --test-threads=1
cargo test --release --lib borrowed_stereo_outputs_render_resolve_composite_and_reuse -- --ignored --test-threads=1
cargo test --release --lib youtuber_room_renders_custom_wave_in_stereo -- --ignored --test-threads=1
cargo test --release --lib youtuber_room_shared_anime_shading_and_seated_reimu -- --ignored --test-threads=1
cargo test --release --lib sakura_rooftop_scene_renders_typed_sky_and_background_sun -- --ignored --test-threads=1
```

Run GPU tests sequentially, with validation for correctness runs. These current
tests establish reference behavior; new multiview image/count assertions are
proposed, not already implemented. Check CPU-side material/interface tests too
when those contracts change.

Use the [existing fixture and runner](sakura-rooftop-cpu-profile.md#artifacts-and-reproduction)
with 240 warmup frames, 600 measured frames, three independent repetitions and
alternating case order. Add explicit old/new view-mode selection before using it
for multiview comparisons. Keep resolution, MSAA, effects, camera paths, poses,
instrumentation and frames-in-flight policy identical. Include many-draw opaque,
transparent, emissive and reflective workloads beyond the avatar matrix.

Save per-frame samples and report per-run median/p95, CPU stage times, GPU stage
times, recorded draw calls versus rendered view instances, submission count,
uploads, allocation/cache churn and retained capacity. Compare the spread across
runs; if a gain is within normal variation, extend the measurement rather than
claiming an improvement. Do not add overlapping CPU/GPU columns into a frame total.

Use warmed Nsight traces to explain results separately. Publish unprofiled timings
for performance claims. Headset confirmation should repeat both modes using the
same view/motion instructions and record compositor pacing; a short human run is
not a controlled FPS experiment. Leave transforms/IK, collision workers and
spring sleeping outside this renderer change's claimed benefit.

## Instrumentation audit: overhead when Nsight is not running

The checkpoint added no NVTX labels, Vulkan debug-label commands or Vulkan object
names. Nsight captures Vulkan API calls externally, plus CPU samples and OS runtime
events, using the runner's explicit `--capture` path. Named JSON fields and pass
names in the fixture are diagnostic report data, not driver label calls. Installing
Nsight does not make the application launch it on an ordinary run.

| Mechanism | Present in ordinary application builds? | Work it performs |
| --- | --- | --- |
| Fixture CPU timers, per-pass draw maps, dirty-component attribution and GPU timestamp queries | No; `cfg(test)` | Detailed measurement only in test builds; most renderer hooks also require the fixture's active profile state |
| Skin tick timer and binding-update counters added for the fixture | No; `cfg(test)` | Run in library test builds even without the fixture; a release test binary still has `cfg(test)` enabled |
| New morph-byte, deformation-allocation and buffer-growth counters; active-weight byte accounting | Yes | Ordinary integer updates at existing upload/allocation events; no added GPU queries, buffer allocation or queue submission |
| XR timing/report collection | Available, enabled by collector/profile settings | Headset timing and reports when explicitly configured; the example configures this automatically |
| Vulkan debug-utils validation messenger in the borrowed-output test | No; test-only | Receives validation messages; separate from Nsight and not introduced by this profiling checkpoint |
| External Nsight tracing/sampling | Only when attached/launched for capture | Measurable capture overhead; not part of ordinary unprofiled timings |

Thus the added production instrumentation is not literally free, but the remaining
new work is small integer accounting, not per-draw label formatting or timestamp
recording. Its exact cost has not been isolated. Existing other engine diagnostics
are not all covered by this statement; it describes this checkpoint's additions.
Custom SPIR-V debug names also support uniform-name reflection and are unrelated
to runtime Nsight markers.

The measured capture perturbation was approximately 4.6%, 17.6%, 5.9% and 2.5%
for 0-static, 2-static, 0-moving and 2-moving respectively. Those are capture-versus-
uncaptured fixture comparisons, not estimates of the tiny retained counters.
The report contains the methodology and limitations.

For future CPU labels/timers, use a dedicated opt-in feature or cached enable flag;
avoid constructing label strings or calling clocks before the enabled check.
GPU queries must also be opt-in, use bounded resources and avoid new blocking waits
in the normal render loop. Include an instrumented/uninstrumented comparison in
slice 0 if production timing scopes or labels are added.
