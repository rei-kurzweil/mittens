# Task: separate material lighting, visible emission, and extraction

Date: 2026-10-13
Status: proposed behavior and investigation; documentation only.
Parent: [Materials v3 / custom materials](epic/custom-materials.md).
Related implementation: [emission capabilities and LED proof](material-emission-capabilities-and-led-strip.md).
Runtime settings: [graphics settings and mirror controls](runtime-mirror-resolution-info-panel.md).

## Outcome

Surfaces can opt out of lighting and remain visible without feeding emissive
post-processing. Selected emission sources still contribute to extraction and
Bloom. Disabling extraction globally preserves the authored surface appearance
and lighting choice while skipping extraction and its dependent work.

2026-10-13 proposal: an emissive/unlit surface at intensity **0** keeps its
base color, receives no lighting, and is omitted from extraction. Increasing
intensity makes it an eligible source when its material supports extraction
and the renderer enables that route. Zero intensity must not implicitly select
a lit surface model. This is proposed behavior, not the current Toon behavior.

Investigate whether that intensity-based choice is sufficient for the first
built-in slice. A separate extraction opt-out is needed only if authors also
need positive visible emission without contributing to post-processing. Keep
capability, input consumption, and source selection distinct in the internal
contract even if the first public control uses intensity alone.

Mirror resolution and secondary-motion settings are the next runtime settings
slices. Review this material policy before implementing an extraction toggle;
do not make an emissive redesign a prerequisite for those two settings.

## Materials v3 alignment

The current v3 integration plan is [custom materials](epic/custom-materials.md),
with [stable instances and family contracts](material-instances-and-family-contracts.md)
and the [resolved material contract](material-component-and-resolved-contract.md).
The [Shader draft](../draft/shader-component.md) records earlier authoring syntax,
superseded by the Material anchor; [Materials v2](epic/materials-v2.md) supplies
earlier definition/instance and vertex-family architecture.

V3 already distinguishes standard emission input consumption, a validated
emission output/pass capability, and renderer activation. This task specifies
lighting and source-selection semantics within that contract. It does not add
a competing material registry or require emission before the
[opaque f32 time-animation proof](material-f32-time-animation-first-slice.md).

| Decision | Owner | Required independence |
| --- | --- | --- |
| Lit versus unlit surface behavior | Material definition and resolved authored surface selection | Extraction state and zero intensity cannot silently change lighting. |
| Visible emission/base-color behavior and emission intensity | Material shader contract and supported instance inputs | Define how the main color uses intensity; consuming an input alone grants no extra pass. |
| Supported emission output/variant | Validated family/definition capability | An arbitrary custom fragment cannot use the fixed Toon extraction shader. |
| Effective extraction source selection | Resolved material/draw contract | Initially consider supported emission with positive intensity; retain explicit provenance for defaults and any later opt-out. |
| Authored effects, parameters, and outputs | Scene RenderGraph/PostProcessing configuration | Keep authored configuration intact while runtime settings suppress work. |
| Global extraction enabled state | GraphicsSettingsSystem | MMS host calls update/read this owner; the panel displays its applied state. |
| Targets, pipelines, submission, and texture publication | Renderer | Derive required work/resources from supported sources, consumers, and effective settings. |

Freeze public constructor/setter spelling in the material-contract work.
`emissive = true` in the epic is a proposed capability shorthand, not an
implemented method or a global Bloom switch. Define whether an emissive preset
also chooses unlit shading; do not assume every lit-plus-emission material must
stop receiving lighting.

## Checked current behavior

- [RenderableSystem](../../src/engine/ecs/system/renderable_system.rs)
  switches Toon/skinned Toon to emissive handles when intensity is positive,
  and back to ordinary Toon when it becomes zero.
- [Toon main shading](../../assets/shaders/toon-mesh.frag) bypasses lighting
  for its emissive material flag or positive instance intensity. Its emissive
  branch displays base color; the
  [extraction shader](../../assets/shaders/emissive-toon-mesh.frag) multiplies
  textured/vertex color by intensity. Main-color brightness and extracted
  radiance already have different behavior that needs an explicit contract.
- `Unlit` already routes a static mesh to `UNLIT_MESH`, whose
  [fragment shader](../../assets/shaders/unlit-mesh.frag) displays textured
  base color without lighting or extraction. The routing test asserts an
  empty emissive draw list. This is a foundation to retain; it does not prove
  all GLTF/deformed, inherited, cutout, or background combinations work.
- [VisualWorld](../../src/engine/graphics/visual_world.rs) and draw recording
  classify/select extraction using fixed emissive material identities.
  Positive input alone cannot express the general v3 contract.
- [PostProcessingConfig](../../src/engine/graphics/post_processing.rs)
  activates on enabled Bloom or Bokeh configuration. An EmissivePass alone
  does not currently activate standalone extraction.
- [VulkanoRenderer](../../src/engine/graphics/vulkano_renderer.rs)
  records extraction and blur together in `record_bloom_passes`. With eligible
  content, Bloom intensity 0 still records that work before suppressing the
  returned composite image. Extraction instance buffers are also built
  without first checking whether an extraction pass will consume them.
- Window/XR post-process target reuse checks extent, format, MSAA, and
  frame/view count, but not Bloom presence or effective blur extent. Audit
  allocation keys before adding independently changing effect requirements.

This is a source/design audit, not a measured performance result.

## Focused implementation slices

1. [ ] Specify and prove the built-in zero-intensity behavior: authored
   emissive/unlit shading stays unlit and shows base color at 0; extraction
   excludes that source. Separate surface selection from the current
   positive-intensity Toon handle conversion. Define `Emissive.off()` versus
   intensity 0, removal/reset, and transitions explicitly: `off()` currently
   constructs intensity 0, so it cannot silently retain contradictory meaning.
2. [ ] Publish the compatibility/authoring table in the resolved material
   contract. Preserve existing positive-intensity scenes and existing Unlit
   appearance. Inventory scenes relying on zero/off to restore lighting;
   provide an explicit lighting choice and document any migration. Resolve
   wrapper/local precedence, GLTF source projections, CombineMesh output,
   and mutation/removal invalidation using v3's existing resolver ownership.
3. [ ] Carry separate lighting, emission-consumption, supported-pass, and
   source-selection metadata through the material bridge and draw cache.
   If positive-emission/no-extraction is required, add a supported authored
   opt-out here rather than changing intensity's shader meaning. Keep
   unsupported custom programs/geometry combinations actionable.
4. [ ] In the existing LED emission task, prove shared animation revision,
   pattern, alpha coverage, and deformation in main color and extraction.
   Whole-draw omission at zero is valid only when the program contract
   guarantees zero output at that intensity; arbitrary custom input use is
   not evidence of zero radiance. Dark pixels within a varying pattern remain
   a shader responsibility.
5. [ ] Add global extraction enable/readback through GraphicsSettingsSystem
   and the existing MMS host boundary. Derive effective pass requirements
   over authored scene configuration. Re-registering a RenderGraph, adding
   surfaces, or restoring a panel must not overwrite the applied setting.
   Define scene reset/defaults and changed-event behavior consistently with
   mirror settings. Choose the MMS namespace and panel control after review.

## Runtime extraction and dependent work

When global extraction is off, omit extraction-only instance packing/uploads,
draws, resolves/copies, dependent blur, Bloom compositing, and extraction/Bloom
output publication. This suppression must not alter main-color emission,
unlit shading, authored intensities, or unrelated effects.

Follow actual dependencies rather than bypassing the entire post-process path.
Bloom currently has only an emissive source, so its extraction-dependent work
is suppressed. Refraction scene-color/depth snapshots and rough-transmission
filtering have their own consumers and must keep working without Bloom.
Account for the current pre-refraction Bloom composite and final-composite
ordering. The known [refraction/Bloom visibility issue](../bugs/refraction-behind-emissive-bloom-depth-ordering.md)
remains a separate policy task.

Specify how named extraction/Bloom outputs behave while suppressed: clear,
invalidate, or retain a documented last image. Do not accidentally publish
stale storage as fresh output. Decide whether named outputs can independently
demand extraction without Bloom; the current renderer does not implement that
general resource graph. An empty eligible source set must also produce
defined output behavior when sources become zero or are removed.

On re-enable, derive compatible targets and descriptor bindings from the
current effective requirements and refresh outputs before consumers use them.
Retain or replace resources safely across frames in flight; test effect
presence and blur-resolution changes as well as ordinary off/on reuse. Track
the dependency/resource inventory in the
[material renderer-resource graph](material-renderer-resource-graph.md).

## Acceptance

- An emissive/unlit surface remains visible and unaffected by lights at
  intensity 0, contributes no extraction draw, and resumes extraction when
  positive without implicitly switching its lighting model.
- Ordinary lit surfaces still receive lighting. Existing Unlit surfaces
  remain unlit without extraction. A supported lit-plus-emission material
  keeps its documented lighting behavior regardless of extraction settings.
- Zero/positive intensity changes, authored opt-out if introduced, source
  removal, shared/local overrides, and panel-independent host calls produce
  correct source lists and authoritative global state.
- Global off skips extraction and its dependent work; on restores fresh
  images without scene reload. Surface appearance and unrelated refraction
  behavior remain correct. Verify named outputs and no-source transitions.
- The selected LED pattern and coverage agree across supported passes, on
  desktop and both XR eyes. Explicitly record support for static/deformed,
  cutout, background, and capture views rather than assuming parity.
- Pass/upload/allocation counters and CPU/GPU measurements distinguish
  omitted work from an unchanged headset refresh rate. Do not promise a
  frame-rate gain without identifying the limiting work.
