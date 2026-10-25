# Anime shading: first live slider slice and parameter panel

Status: phase 1b implemented on 2026-09-11; live visual acceptance remains open.
User verified Anime rim lighting and two-step shading in winit and OpenXR on
2026-09-08. Detailed five-control interaction checks and GPU allocation/frame-cost
measurements remain open. Phases 2–3 are planned.

2026-10-24 follow-up: Anime direct lighting now uses a fixed 90-degree facing
gate (`N·L > 0`). Admitted lights feed their full distance/cone-attenuated
intensity into the shade/lit ramp without cosine weighting. The experimental
normal-blend input/slider has been removed. The
[room lighting experiment](youtuber-room-lighting-diagnostic.md) retains raw
distance and Lambert reference views, alongside the new Anime behavior. An
angular gate ramp remains a follow-up.

## Implemented slice and manual verification (2026-09-08)

User verification: Anime's rim lighting and two-step shading are visible through
both winit and OpenXR. This confirms the rendering path in both environments.
It does not yet record controller dragging, per-eye inspection, Reset/restore,
or resource measurements as completed acceptance checks.

Automated verification: 10 focused shading tests, 2 material-cache tests, the
Bisket example URI check, and 2 existing transform-info-panel tests pass. The
release binary builds. The comparison test checks the models' own declarations,
allowing additional Anime scopes such as the floor/backdrop wrapper.
The broad library run is not green: 44 reported failures also reproduced in an
unchanged source snapshot, and the full runs did not finish. The broader
`info_panel` filter also enters microphone/audio-device tests that stall in this
environment; it is not recorded as passing.

- `Shading.anime()` / `.toon()` share one `ShadingComponent`; the old
  `AnimeShading` constructor and Rust alias remain temporarily compatible.
- Getter/setter pairs for shade strength, shade threshold, lit threshold, rim
  strength, and rim power expose normalized Anime state to MMS. Each setter
  emits the existing registration intent; generated
  projections and render parameter records update without a model reload.
- [anime_shading_controls.mms](../../assets/components/ui/anime_shading_controls.mms)
  supplies five Sliders, effective-value readouts, coupled threshold
  synchronization, and Reset as content for the
  existing `info_panel`. Its callbacks capture queried live control references.
  Track/thumb hit targets use priority 120 above the panel shell's priority 100.
- `info_panel_body(options)` rebuilds the existing panel's padded body consistently
  on restore. The caller retains its source and original Reset values.
- Material descriptors use a 512-entry least-recently-used cache. Eviction drops
  cache ownership only; recorded Vulkan commands keep their resource references.
  The cache stress test exercises 10,000 distinct values and retained Arc ownership.

Run the desktop/winit comparison:

```sh
cargo run --release -- load examples/shading-models.mms
```

Drag the shade-strength slider in the panel above the right model from 0 to 1.
Its shaded regions should visibly change while the explicit Toon model on the
left stays unchanged. Confirm the readout, Reset to 0.50, minimize/restore, and
drag release when the controls are removed. Shade and rim colors remain at the
Bisket preset; they are phase 2 work.

Run the separate XR acceptance fixture:

```sh
cargo run --release -- load examples/shading-models-xr.mms
```

This scene enables XR, provides controller pointers, and places a model and
ordinary sphere under one Anime source on the right, with Toon controls on the
left. Check both eyes, head rotation/translation and view-dependent rim lighting,
then drag with a controller and verify both Anime consumers update together.
Verify Reset and restore in-headset too. Basic Anime rendering is user-verified
in OpenXR; the detailed interaction and per-eye checks above remain open.

Source audit: `submit_xr_eye_offscreen` passes each eye's view/projection to the
same `build_draw_batches_command_buffer` used by window rendering. That path
selects Anime's static/skinned pipelines and uses the same parameter UBO/cache.
The UBO test and XR fixture materialization test cover these inputs and authoring,
but do not substitute for submitting and inspecting frames in a headset.

For live resource measurements, prefix either command with
`CAT_DEBUG_MATERIAL_CACHE=1`. Every 256 material cache misses the renderer reports
retained entries, allocations, hits, and evictions. Record these alongside frame
cost and primitive count during sustained edits; this manual measurement remains
open. Headless runtime coverage verifies pre-import edits, GLTF projections,
render parameter records, isolation, normalization, Reset, and restored controls.

The wider default migration, remaining three `Shading` constructors, legacy
component removal, comprehensive conflict diagnostics, and tree-change
invalidation stay in the unified cascade task. Unconfigured mesh defaults have
not changed in this first controls slice.

Authoring direction updated 2026-09-08: [Unified Shading and cascade](shading-model-components-and-cascade.md)
is authoritative for Anime as the default, `Shading.anime()` and the other
built-in constructors, descendant inheritance, and immediate-child overrides.
The panel's target API is now an authored `Shading.anime()` source; references
below to `AnimeShading` describe the existing implementation to migrate.
Keep the left comparison model explicitly `Shading.toon()`. Custom shading uses
`Shader`; defining the built-in `Shading` API is no longer deferred to phase 3.

The native Slider is implemented; use [slider.mms](../../examples/slider.mms)
as the working reference for `SliderChanged`, readouts, and silent `sync_value()`.
Use that control to configure the right-hand Bisket in
[shading-models.mms](../../examples/shading-models.mms) through the reusable
[info_panel](../../assets/components/ui/info_panel.mms). Keep the left-hand
model and matching lights as the comparison baseline.

This task includes the engine/MMS update path needed to change anime shader
inputs while the scene is running, as well as the authored panel. Constructor
configuration alone is insufficient: edits must reach the rendered material.

## First verifiable slice: one live shade-strength slider

Status: implemented, awaiting the manual acceptance checks above.
This is phase 1a; the five-scalar panel below is phase 1b. Do not require
colors, custom `Shader` schemas, or every control before proving this path.

Reuse the existing panel prefab at `assets/components/ui/info_panel.mms`.
Add a reusable content prefab at `assets/components/ui/anime_shading_controls.mms`,
exporting `anime_shading_controls(target, reset_values)`. It returns only the
controls subtree: one row with a label, native `Slider`, and numeric
effective-value readout, plus Reset. The caller passes a retained reference to
the authored `Shading.anime()` component, not a generated primitive projection,
and the Reset values captured when the editor is first mounted.

`examples/shading-models.mms` composes this content using
`info_panel({ ..., content = anime_shading_controls(target, reset_values) })`
and targets the right-hand Bisket's authored source. The existing `info_panel`
owns title chrome and accordion behavior. The example owns panel placement,
retained target/Reset state, and the restore handler; the content prefab owns
its controls and their bindings. The left-hand model is explicitly
`Shading.toon()`.

### Scope and API contract

- Expose `shade_strength` first: range 0–1, step 0.01, initially 0.50 for the
  Bisket preset. Other Anime parameters keep that preset's values.
- Add a typed live setter and getter to the unified component's MMS dispatch.
  Proposed names are `set_shade_strength(value)` and `get_shade_strength()`;
  validate final spelling against existing live method conventions. Reject a
  target whose selected model is not Anime with a useful runtime error.
- Use the existing Anime builder's normalization in the setter. Read back the
  effective value from the component after mutation; do not maintain a second
  validation implementation in MMS. The setter must make normalized CPU state
  available to subsequent readback even if renderer updates are deferred.
- Initialize the control from the getter. The caller captures the initial
  effective value in `reset_values` outside the removable panel body and passes
  it to each content instance. Thus the reusable content also works with
  customized sources without hardcoding Bisket settings.
- On each `SliderChanged`, set the source value, read it back, update the numeric
  label, and silently `sync_value()` the control. Apply changes during dragging.
  Reset uses the same mutation/readback path.
- On restore, the example calls the content prefab again and mounts the fresh
  subtree using `info_panel_body(options)`. Initialize from the
  current source value, keep the original Reset values, and avoid duplicate
  subscriptions. A source
  getter is required here, but a general external-change subscription API is
  outside this slice.

Prerequisite from [the unified cascade task](shading-model-components-and-cascade.md):
`Shading.anime()` and `.toon()` construction, typed Anime state, retained MMS
references, and source resolution/projection propagation must work. Reuse the
existing Anime propagation machinery behind that API. Do not build a second
long-lived live API on the retiring `AnimeShading` component. Completing custom
shader loading is not a prerequisite.

### End-to-end verification gate

1. Run `cargo run --release -- load examples/shading-models.mms`. Drag shade
   strength between 0 and 1 under the matching lights. The shaded regions on
   the right-hand model visibly change while the left-hand model stays unchanged;
   the readout follows the effective value throughout the drag. Retain a short
   capture or before/after screenshots and record the reproduction steps.
2. Add a focused MMS/runtime regression that invokes the setter through normal
   live dispatch and processes the usual update intents. Verify the source,
   its generated GLTF projections, and corresponding VisualWorld GPU parameter
   records agree. Include multiple consumers and an independent Anime source
   or immediate-child override to prove isolation, not just a Toon baseline.
3. Cover an edit before generated primitives exist, then create them and verify
   they receive the latest value. Cover out-of-range normalization and readback
   without bypassing the live API.
4. Verify Reset, minimize/restore, and removal during a drag. Restored controls
   show the current source value and a single change produces one logical edit.
   Parameter edits do not reload the GLTF, rebuild rows/geometry, or recompile
   shaders.
5. Bound material descriptor/UBO cache retention as part of this slice. Exercise
   sustained changing values, including a runtime-driven sequence beyond the
   slider's 101 quantized values, and record cache/allocation counts, primitive
   count, and frame cost. Retention must be bounded by an explicit policy rather
   than all historical values, while preserving in-flight GPU resource lifetimes.

This slice is complete only when both runtime regression coverage and the live
rendered example pass. A moving slider, a changed CPU field, or a static authored
builder example alone does not satisfy the gate. If visual execution is blocked,
record the blocker and leave that acceptance item open.

After this gate, phase 1b adds the remaining four scalar rows and coupled
threshold synchronization using the same verified source-to-GPU path.

## Phases

### Default material migration (2026-09-08 source audit)

Anime can become the default for ordinary lit meshes, but replacing every
`TOON_MESH` occurrence would also change material dispatch and explicit pipeline
tests. Change default selection separately from retaining the existing handles.
Primitive factories in `src/engine/ecs/component/renderable.rs` and GLTF primitive
creation in `src/engine/ecs/system/gltf_system.rs` currently select Toon; audit
direct Rust scene constructors as well. Both static and skinned defaults need
to select their corresponding Anime variants.

The generic `AnimeShadingComponent::new()` defaults are:

| Parameter | Generic default | Bisket preset |
| --- | --- | --- |
| Shade color | [0.72, 0.50, 0.54] | [0.40, 0.40, 0.65] |
| Shade strength | 0.30 | 0.50 |
| Shade threshold | 0.35 | 0.40 |
| Lit threshold | 0.55 | 0.55 |
| Rim color | [1.0, 0.85, 0.92] | [1.0, 1.0, 1.0] |
| Rim strength | 0.18 | 0.38 |
| Rim power | 4.0 | 4.0 |

`AnimeShadingParams::default()` duplicates the generic values; keep these in
sync, preferably by deriving GPU defaults from the component's canonical values.

Before changing the default, resolve these behavioral differences:

- `anime-mesh.frag` uses light intensity, direction, attenuation, and spot cones,
  but ignores ambient light and light RGB. It shades between tinted albedo and
  original albedo, with a rim capped at original albedo.
- The shader does not consume emissive intensity or light quantization.
  `material_with_emissive` currently switches only Toon variants into emissive
  variants. Preserve working Emissive/Unlit behavior and bloom classification
  when ordinary meshes start with Anime handles.
- Keep explicit specialized materials such as grid, mirror, and transmission.
- The example's left model currently relies on the GLTF default. A migration
  requires an explicit Toon override exposed to MMS to preserve its pipeline
  comparison; no `ToonShading` component is currently registered.

The live-control audit below remains current: native Slider and source projection
propagation exist, while Anime live methods and bounded material descriptor cache
retention remain implementation work. No general custom-shader API is needed to
wire this panel.

### Panel phases

1. First prove the single-slider phase 1a above, then extend to five scalar rows
   in `info_panel` for phase 1b.
2. Live shade/rim color setters and color controls.
3. Design custom `Shader` definitions and typed input schemas under Materials v2,
   using the unified shading cascade. This phase is a design session, not a
   prerequisite for phases 1–2 or the built-in `Shading` migration.

Phase numbering here is local to this task, independent of the
[Materials v2 epic](epic/materials-v2.md).

## Phase 1: scalar panel composition and settings

Pass a block container as `info_panel({ ..., content = rows })`. Each row
uses `display("flex")`, `flex_direction("row")`, and centered cross-axis
alignment: fixed-width label, growing slider cell, fixed-width value readout.
The enclosing `display("block")` container stacks rows. Explicit row heights
avoid depending on block auto-height behavior for the first example.

`info_panel` already has a flex-column content wrapper; place the block rows
container inside it. Reuse the slider task's authored input wrapper and native
track/thumb mounts for presentation. No custom MMS drag implementation is needed.

Start with these controls. Defaults come from
[bisket_anime_shading.mms](../../assets/components/materials/bisket_anime_shading.mms),
not the engine's generic AnimeShading defaults:

| Control | Initial value | Proposed UI range | Step |
| --- | --- | --- | --- |
| Shade strength | 0.50 | 0–1 | 0.01 |
| Shade threshold | 0.40 | 0–2 | 0.01 |
| Lit threshold | 0.55 | 0–2 | 0.01 |
| Rim strength | 0.38 | 0–1 | 0.01 |
| Rim power | 4.0 | 0.01–16 | 0.01 |

Keep shade and rim colors at their Bisket preset values in phase 1. Color
controls and live color setters belong to phase 2.

These are useful example ranges, not new engine limits: thresholds currently
accept nonnegative values and rim power is clamped to 0.01–128. Keep
`shade_threshold <= lit_threshold`. Match existing builder normalization:
raising shade above lit also raises lit; lowering lit below shade also lowers
shade. Synchronize both affected controls and readouts silently.

Keep settings outside the panel body. Initialize the material and controls
from the same settings, and provide Reset to restore the five scalar defaults.

### Live shader-input update path

Resolve and retain the authored AnimeShading component beneath the right-hand
GLTF. Slider value changes should update its typed material parameters through
the normal live component mutation API. Inspect existing builder/live method
dispatch first, then implement any missing methods or update notifications.
Keep the API consistent with other component updates; final method names are
an implementation decision, not an assumed existing interface.

The required path is:

1. A slider emits its normalized numeric value; the panel changes the associated
   scalar in its retained settings.
2. The live AnimeShading component validates the update using the same rules as
   initial construction, including threshold ordering and non-finite handling.
3. The material update reaches every generated renderable projection associated
   with that authored component, including projections created after the edit.
4. The renderer refreshes the affected shader parameter data so the next
   rendered frame using that update reflects the new values.
5. Controls and readouts synchronize with the effective sanitized values,
   including any coupled threshold change, without emitting recursive edits.

Audit registration, projection ownership, material caches, and GPU parameter
upload/invalidation so an update cannot stop at a CPU-side component field.
Reuse existing update machinery where possible and fill its gaps. This task
targets typed AnimeShading inputs; a general arbitrary shader/uniform API is
outside phases 1–2.

Apply updates during dragging, not only on commit. Do not reload the GLTF,
recompile shaders, recreate geometry, or rebuild panel rows for parameter-only
changes. Preserve the latest value if updates are coalesced within a frame.
Reset must use the same live update path and restore the phase's exposed
parameters coherently.

### Pre-implementation baseline and remaining expansion work (2026-09-07 audit)

The implementation summary above supersedes this pre-implementation baseline:
all five scalar getter/setter pairs, panel rows, coupled threshold readback, and
bounded descriptor retention are implemented.

- `src/scripting/runtime_config.rs` registers AnimeShading constructors and
  builders, but no live methods. Add five typed scalar setters and dispatch in
  `src/scripting/component_method_registry.rs`. Proposed names are
  `set_shade_strength`, `set_shade_threshold`, `set_lit_threshold`,
  `set_rim_strength`, and `set_rim_power`.
- Reuse `AnimeShadingComponent` builder normalization. Specify effective-value
  readback or setter results so the panel can synchronize coupled thresholds
  without duplicating engine validation in MMS.
- `Emissive.set_intensity()` is the closest existing live-method precedent:
  validate the target and route an update through the normal intent machinery.
  Anime already has `RegisterAnimeShading`; decide whether setters can mutate
  the source and emit that existing intent, or need a dedicated update intent.
  Keyframe/Transition support is not required for this panel.
- `RenderableSystem::register_anime_shading` already propagates authored source
  values to GLTF primitive projections, including pending renderables.
  `VisualWorld::update_anime_shading` already invalidates draw caching. The GLTF
  system has a regression test for manual source mutation and re-registration;
  extend coverage through MMS rather than rebuilding this propagation path.
- `VulkanoRenderer::get_or_create_material_set` keys cached UBO/descriptor sets
  by all anime parameter values. Previously unseen combinations allocate new
  resources. The observed eviction handles texture replacement, not historical
  slider values. Phase 1 must bound retention during sustained dragging, using
  reclamation or reusable storage that respects in-flight GPU work. Do not
  require the general phase-3 material architecture to solve this.
- The stale “native Slider is not implemented” comment in
  `examples/shading-models.mms` was corrected during the 2026-09-08 audit.

### Panel lifecycle

Minimizing currently removes the accordion body. On
`AccordionRestoreRequested`, build fresh rows from retained settings, wrap
them with `info_panel_body(options)`, and attach them to the event's
body mount. Reinstall handlers only for the new controls; removed rows must
not retain active subscriptions. Material settings survive minimize/restore.

### Phase 1 implementation and acceptance

1. Verify and complete live AnimeShading mutation, normalization, projection
   propagation, and renderer parameter refresh before wiring the full panel.
2. Build the reusable panel content and instantiate it in `shading-models.mms`.
3. Verify all five scalars visibly update the anime instance while
   the default instance remains unchanged, including under active animation.
4. Verify threshold coupling, effective-value readouts, silent synchronization,
   and Reset against the shared Bisket preset.
5. Verify minimize/restore preserves settings without duplicate handlers, and
   removing controls during a drag releases interaction safely.
6. Add focused regression coverage for a live authored material update reaching
   its generated projections and renderer parameters. Include distinct material
   instances to catch updates leaking to unrelated renderables.
7. Exercise the example to confirm dragging produces live visual feedback
   without model reloads or panel rebuilds.
8. Cover an edit before GLTF projection creation, and verify the latest source
   values reach newly created projections.
9. Exercise sustained changing values and verify GPU material cache retention
   is bounded. Record allocation/cache counts and frame cost, including the
   number of primitive projections, rather than assuming a slider is cheap.

## Phase 2: color controls

Add typed live setters for shade and rim RGB colors through the same source
update path. Reuse component color validation. Choose RGB sliders or a reusable
color control at implementation time; a new color-picker widget is not a
phase-1 dependency.

| Color | Preset RGB | Proposed channel range / step |
| --- | --- | --- |
| Shade | 0.40, 0.40, 0.65 | 0–1 / 0.01 |
| Rim | 1.0, 1.0, 1.0 | 0–1 / 0.01 |

Extend retained settings, effective-value synchronization, Reset, restore,
instance-isolation tests, and cache checks to these fields.

## Phase 3: design custom Shader / ShaderComponent

The built-in API is `Shading` in MMS and `ShadingComponent` in Rust, as specified
in the unified cascade task. Custom models use `Shader` / `ShaderComponent` and
share its source ownership and precedence rules. Reconcile custom registration
and input syntax with the illustrative `Material` API in
[Materials v2](epic/materials-v2.md); do not introduce a competing public
`Material` authoring component.

### Input terminology

Descriptors and push constants are likely the two Vulkan mechanisms intended
by “Descriptors and positional arguments.” Push constants are named fields
packed at validated byte offsets, not positional function arguments.
Descriptors bind resources such as uniform/storage buffers, images, and
samplers; multiple scalar fields normally live together inside one bound
buffer. Vertex inputs and specialization constants are additional mechanisms
with different purposes, not ordinary editable material values.

Keep the MMS parameter vocabulary separate from GPU binding layout. Authors
should update named typed fields; the validated shader schema determines their
storage and packing. A schema authored in MMS must match the shader interface:
declaring a field cannot add a corresponding input to an already compiled shader.

### Boring starting design to discuss

- An immutable, registered shading definition contains program identity,
  compatible vertex interface, render state, and a typed parameter schema.
  Validate and resolve names, bindings, offsets, alignment, and defaults once
  when registering/loading it.
- A custom ShaderComponent references that definition and holds mutable parameter
  values with stable instance identity and dirty tracking. Decide explicitly
  how multiple renderables share that instance and how independent copies are
  requested; preserve authored-source ownership across GLTF projections.
- Start with a packed material uniform block for floats/vectors/colors.
  Descriptors bind that block; sliders update its contents. Add image/sampler
  inputs and push-constant fields only when concrete use cases justify them.
- Prefer automatic static/deformed vertex selection for a compatible fragment
  program, following Materials v2. Define what “custom” initially permits:
  registered programs or user-supplied shader files. The latter additionally
  needs compilation/loading, interface validation, and useful errors.
- Allow MMS schema construction at definition time if needed, but resolve it
  to engine metadata once. Avoid interpreting schemas or rebuilding layouts
  for every setter or frame. Consider shader reflection as validation rather
  than making arbitrary reflection output the public MMS API.

### Lean update requirements

- Ordinary value edits must not change pipeline identity, rebuild geometry,
  or compile shaders. Switching shading models can require pipeline selection
  or creation and must have a separate cost/lifecycle contract.
- Resolve field names to stable slots; coalesce dirty writes per material per
  frame. Upload only changed data through renderer-owned storage safe for
  frames in flight. Do not allocate persistent descriptor/UBO entries for every
  historical value combination.
- Share immutable schemas/programs and deliberately shared material state.
  Define batch keys using stable identities and relevant render state; measure
  the batching consequences of independent material instances. Per-renderable
  values may need instance/storage data if separate UBOs would split batches.
- Benchmark one shared material across many primitives and many independently
  changing materials. Record CPU update/upload time, GPU time, allocations,
  descriptor writes, cache size, and draw count. Set numerical budgets after
  collecting a baseline; “lean” needs measured acceptance criteria.

### Design-session deliverable

Agree on definition versus instance ownership; MMS schema and setter syntax;
initial field types and storage; shader/interface validation; model switching;
serialization/reload behavior; and a small measured prototype. Start with one
custom compatible shading program and one live scalar. Plan animation support
with [animated shader inputs](animated-shader-material-inputs-mms-animation-system.md)
as a separate integration step. Do not expand phase 1 into this architecture.

Phase 1a is implemented as recorded above; later phases remain planned.
Runnable examples should not reference additional shader-update APIs until
those implementations land.
