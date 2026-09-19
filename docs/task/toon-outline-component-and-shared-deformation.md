# ToonOutline component and shared render instance

Date: 2026-09-18

Status: proposed

## Goal

Add a `ToonOutline` graphics modifier that can style one renderable or all renderables below a
wrapper. An outlined renderable produces two draws from one persistent visual instance:

1. an outline draw, recorded as an ordinary `RenderOp::DrawBatch` in a dedicated outline phase;
   and
2. the ordinary foreground draw, recorded unchanged in its existing opaque, cutout, emissive,
   transmission, transparent, or overlay phase.

Both draws reference the same `VisualInstance`, uploaded GPU mesh, transform, and deformation-cache
range. An outline must never create a second mesh upload, skinning allocation, morph input set, or
deformation compute job.

`GLTFSystem` must project a `ToonOutline` authored on or around a `GLTFComponent` onto each imported
primitive, following the existing `ShadingComponent` projection model.

## Current architecture findings

The requested design fits the current renderer without duplicate visual registration.

- `RenderableSystem::flush_pending` resolves one CPU mesh, calls
  `RenderAssets::gpu_mesh_handle`, and registers one `VisualWorld` instance. `RenderAssets` caches
  the resulting `MeshHandle` by `CpuMeshHandle`. The outline operation can use that instance's
  resolved handle and therefore the same vertex and index buffers without another upload.
- `VisualWorld` assumes one visual handle per ECS renderable:
  `component_to_handle: HashMap<ComponentId, InstanceHandle>`, and `RenderableComponent` stores one
  `handle`. Transform, texture, skin, morph, and removal paths all rely on that assumption. The
  outline design can preserve it.
- The deformation cache is currently owned per `VisualInstance`. `sync_deformation_ranges`
  allocates a range for every instance with bones, and `record_dirty_deformations` emits one compute
  job per dirty instance. Reusing the foreground instance means the existing ownership model
  already produces exactly one range and one job.
- The compute output already contains exactly what an outline vertex stage needs: a deformed
  position and packed deformed normal. The regular cached-skinned vertex shader reads it through
  `i_deformed_base + gl_VertexIndex`.
- Draw lists are built in `VisualWorld::prepare_draw_cache`, then recorded in a fixed order in
  `VulkanoRenderer`. A single instance can already be referenced by multiple render operations:
  stencil clip sources participate in `EnterClip`, an ordinary `DrawBatch`, and `ExitClip`. An
  outline is the same kind of renderer-level reuse, expressed as a `DrawBatch` in the outline
  stream plus a `DrawBatch` in the instance's ordinary foreground stream.
- `GLTFSystem` already resolves `ShadingComponent` at the GLTF scope, creates a non-serialized
  projected copy below every generated primitive renderable, and retains a source-component link
  so live source changes can fan out. Imported node transforms are attached to the nearest
  transform ancestor, not below the `GLTFComponent`, so ordinary ancestor lookup from an imported
  renderable cannot replace this projection step.

## Authoring contract

`ToonOutlineComponent` is an effect/modifier, not a replacement for the foreground material. Its
initial data is:

```rust
pub struct ToonOutlineComponent {
    pub color: [f32; 4],
    /// Expansion in world-space engine units for the first slice.
    pub width: f32,
    source_component: Option<ComponentId>,
}
```

Suggested defaults are opaque near-black (`[0.02, 0.02, 0.03, 1.0]`) and `width = 0.01`. Width must
be finite and non-negative. Zero width removes the renderable from the outline stream without
removing its visual instance. Alpha is retained in the authoring type for a future blended-outline
path; the first slice requires an opaque outline.

Suggested MMS:

```mms
// Immediate child: styles one renderable.
R.cube() {
    ToonOutline
}

// Wrapper: styles descendant renderables.
ToonOutline.width(0.015).color([0.04, 0.02, 0.06, 1.0]) {
    T { R.cube() }
    T { R.sphere() }
}

// GLTF-scoped: projected onto every generated primitive.
GLTF.new("assets/models/avatar.glb") {
    ToonOutline.width(0.008)
}
```

Resolution should use nearest-scope wins:

- an immediate `ToonOutline` child of a renderable is local and wins;
- otherwise walk ancestors and accept either an ancestor `ToonOutline` wrapper or an immediate
  `ToonOutline` child of an ancestor container;
- two equally local authored outlines are an error, not child-order precedence;
- a projected GLTF copy is local runtime data and points back to its authored source;
- generated projections receive `Serialize.off()` and never appear in saved MMS.

This matches the useful parts of `ShadingComponent` behavior while making ambiguity explicit.

## Rendering technique

Use the inverted-hull technique for the first implementation:

1. read the ordinary or cached-deformed vertex position and normal;
2. transform both to world space;
3. expand the world position along the normalized world normal by the authored width;
4. render a flat outline color with front faces culled;
5. depth-test and depth-write the expanded back faces; and
6. render all ordinary foreground phases afterward.

Drawing every scene outline together before foreground geometry gives the intended pipeline
coherence: static outlines batch together, cached-deformed outlines batch together, and all regular
objects retain their current material batches. It also preserves normal inter-object depth
occlusion: a closer foreground surface can overwrite a farther outline.

The initial width is in world units. Constant-pixel width is a later vertex-stage option because it
needs projection/viewport-aware clip-space expansion and a decision about stereo consistency.
World-space expansion is enough to prove multi-phase instance reuse, phase routing, mesh reuse, and
shared deformation.

The outline pipelines need:

- a static outline vertex shader reading the uploaded mesh position/normal;
- a cached-deformed outline vertex shader reading the same deformation SSBO as
  `cached-skinned-toon-mesh.vert`;
- a flat-color fragment shader;
- front-face culling, opaque color writes, depth test, and depth write; and
- the same winding/front-face convention as the existing mesh pipelines.

Do not generate an expanded CPU mesh, reverse indices, or upload an outline-specific vertex buffer.

## Visual identity and lifetime

Keep exactly one `VisualInstance` and one `InstanceHandle` for each ECS renderable. Add optional
outline parameters to that instance:

```rust
pub struct ToonOutlineParams {
    pub color: [f32; 4],
    pub width: f32,
}

pub struct VisualInstance {
    // existing fields ...
    pub toon_outline: Option<ToonOutlineParams>,
}
```

The component-to-handle map and `RenderableComponent::handle` remain unchanged. Adding or removing
an outline only changes `toon_outline` and dirties the outline draw cache/instance data; it does not
register or remove a visual instance.

Required invariants:

- one ECS renderable maps to one `InstanceHandle` and one `VisualInstance`;
- model, mesh, texture, material, bones, morphs, bounds, and lifetime remain ordinary instance
  state;
- outline color and width are additional per-instance draw parameters, not a second material
  identity on the instance;
- removal of the ECS renderable removes the one instance and therefore both of its phase
  appearances;
- setting width to zero or removing the component removes the instance index from the outline
  stream while leaving its foreground phase unchanged;
- picking, bounds, raycasting, mirror source exclusion, and semantic selection require no duplicate
  suppression because the outline introduces no second selectable instance; and
- foreground-only updates cannot accidentally overwrite outline parameters, while an outline
  update cannot change the foreground material.

## Sharing the GPU mesh

Both draw operations address the mesh on the same instance:

```text
CpuMeshHandle
  -> RenderAssets::gpu_mesh_handle (cached upload)
  -> one MeshHandle
       -> one VisualInstance
            -> outline-stream DrawBatch
            -> foreground-stream DrawBatch
```

This is already supported by the renderer's mesh map. A test uploader should assert that enabling
an outline performs no additional upload and that both render operations resolve the same
instance index and `MeshHandle`.

UV-baked variants are also safe: share the final resolved variant handle used by the foreground,
not `Renderable::base_mesh`.

## Sharing post-morph/post-skinning deformation

No deformation alias type is needed. The one `VisualInstance` continues to own its existing
`bones_base`, active morph inputs, `deformed_base`, and `deformed_count`. The compute scheduler sees
one dirty instance and emits one `GpuDeformationJob` exactly as it does today.

When the outline phase builds instance data, it copies the same `deformed_base` and
`deformed_count` already used by the foreground phase. The cached-deformed outline vertex shader
then reads the same post-morph/post-skinning position and normal:

```text
one dirty VisualInstance
  -> one DeformationRange
  -> one GpuDeformationJob
  -> one cached position/normal range
       -> outline-stream DrawBatch cached-deformed vertex shader
       -> foreground-stream DrawBatch cached-deformed vertex shader
```

`SkinnedMeshSystem`, morph updates, allocator ownership, and removal do not need outline-specific
branches. This is an important reason to model the outline as a second render operation rather
than a second visual instance.

## Draw-phase integration

Add an explicit `outline_order`, outline render stream, and outline instance buffer to `VisualWorld`
and the renderer. `outline_order` contains the same instance indices used by existing foreground
streams; it is not another instance collection. Existing phase classification remains unchanged,
and outline eligibility independently adds an instance index to the outline stream.

Do not extend `RenderOp`. The outline stream contains ordinary `RenderOp::DrawBatch` operations.
The phase already tells the renderer that these batches are outlines; a distinct operation would
duplicate that information. `EnterClip` and `ExitClip` are special operations because they mutate
stencil state inside a stream, while an outline is an ordinary graphics batch recorded in a
different phase.

Outline batch construction cannot blindly copy the instance's foreground material. Build the
outline stream with an outline-specific batch builder that retains the instance's mesh and chooses
the static or cached-deformed outline material/pipeline. Dedicated internal material handles such
as `TOON_OUTLINE` and `SKINNED_TOON_OUTLINE` fit the existing `DrawBatch.material` dispatch model.
Color and width should remain per-instance data so differently styled outlines can still batch
when the outline material and mesh match.

For the normal scene domain, record:

```text
background phases
clear foreground depth
scene outline phase
opaque foreground
cutout foreground
scene-color capture / transmission
transparent foreground
overlay domain
```

This ordering draws outlines for many characters with at most the static/cached-deformed outline
pipeline split, then returns to the existing foreground pipeline families. Within the outline
phase, sort and batch by outline pipeline variant, mesh, outline parameters that are not per
instance, and stencil state if clipping is supported.

Overlay content needs a separate outline subphase after the overlay depth clear and before overlay
foreground. Background, stencil-clipped, transparent, cutout-textured, and transmissive edge cases
should not be allowed to fall accidentally into the scene outline phase; each gets either an
explicit supported policy or an explicit first-slice exclusion.

Mirrors and XR use the same `VisualWorld` render streams, so they should receive the outline phase
automatically. Mirror source exclusion already identifies the one instance; outline-stream
filtering must apply that same exclusion before recording its `DrawBatch` operations.

## GLTF projection

Extend the existing GLTF-scoped modifier flow rather than relying on the generated hierarchy:

1. Before spawning, call `RenderableSystem::resolve_toon_outline(world, gltf_component)` alongside
   `resolve_anime_shading`.
2. Pass the resolved `(source_component, value)` through `spawn_node_recursive`.
3. Under every generated primitive `RenderableComponent`, add
   `value.projected_from(source_component)` plus `Serialize.off()`.
4. Let normal component initialization emit `RegisterToonOutline`; do not call renderer APIs
   directly from the importer.
5. When the authored source changes, update every projection that references it and update the
   corresponding instance's outline parameters and stream membership.

A primitive-local authored outline, if generated or attached later, overrides the projection using
the same nearest-scope rule. Re-registering a GLTF-scoped source must update existing imported
primitives; it must not require re-importing the GLTF.

This first implementation need not interpret a custom glTF material extension. “GLTF aware” means
the engine's component modifier is projected onto spawned primitives.

## Proposed implementation areas

- `src/engine/ecs/component/toon_outline.rs`: component, validation, source link, lifecycle intent,
  and MMS serialization.
- `src/engine/ecs/component/mod.rs`: export.
- `src/scripting/component_registry.rs`, runtime config, and method registry: construction and live
  `width`/`color` methods.
- ECS signals and mutation execution: register/update/reconcile intent.
- `src/engine/ecs/system/gltf_system.rs`: scoped resolution and primitive projections.
- `src/engine/ecs/system/renderable_system.rs`: resolve the effective outline and attach/update its
  parameters on the ordinary visual instance after the final mesh resolves.
- `src/engine/graphics/visual_world.rs`: optional per-instance outline parameters,
  outline-specific `DrawBatch` construction, and outline stream construction from ordinary instance
  indices without changing `RenderOp`.
- `src/engine/graphics/vulkano_renderer.rs`: outline pipelines, per-phase instance buffer, and
  outline-stream `DrawBatch` recording before foreground.
- `assets/shaders/`: static and cached-deformed outline vertex stages plus flat fragment stage.

## First implementation slice

The smallest useful slice should still be vertical and should include cached-deformed GLTF. A
static-only slice would validate inverted-hull rendering but would not prove that the same instance
and cached deformation can be consumed by both render operations.

Implement one end-to-end slice with these boundaries:

- `ToonOutline` supports validated opaque color and world-unit width, construction, serialization,
  and live source updates.
- It resolves as an immediate renderable child or ancestor wrapper.
- `GLTFSystem` projects it onto every imported primitive using source-linked non-serialized copies.
- Ordinary opaque static renderables and opaque static/skinned GLTF primitives retain one visual
  instance that appears in both the outline and foreground streams.
- Both render operations use the instance's one uploaded `MeshHandle`.
- Skinned/morphed content keeps one deformation range and one compute job; both operations read the
  instance's cached position/normal result.
- One scene outline phase is recorded after the foreground depth clear and before opaque/cutout
  foreground using ordinary `RenderOp::DrawBatch` operations.
- Width is world-space, the shader uses inverted hulls, and the pipeline culls front faces.
- Transform updates and removal need no duplication because both draws reference the same
  instance.
- The outline draw does not create additional bounds, picking, raycasting, emissive extraction, or
  ordinary draw-list entries.
- The slice explicitly rejects or skips outlines on overlay/background, blended transparency,
  transmission, stencil-clipped content, and alpha-shaped cutouts. These need phase- and
  alpha-specific decisions rather than accidental opaque behavior.

Although this crosses several files, splitting before the shared-deformation path is exercised
gives no representative result for the motivating humanoid case. The slice can still be kept
narrow by not adding pixel-width outlines, multiple outline layers, per-material glTF extensions,
alpha-aware outline fragments, or overlay routing.

## Test plan

### Component and resolution tests

- Defaults, finite/range validation, builders, serialization, and round-trip MMS.
- Immediate-child and ancestor-wrapper resolution, nearest override, and equal-scope duplicate
  error.
- Setting width to zero removes outline-stream membership while preserving the instance and its
  foreground membership.

### GLTF tests

- A direct `ToonOutline` child of a GLTF produces one source-linked, `Serialize.off()` projection
  under every imported primitive.
- A wrapper above a GLTF resolves identically.
- Updating the source changes every projected outline without re-importing meshes.
- A model with both static and skinned primitives chooses the matching outline vertex path.

### VisualWorld and renderer-structure tests

- Registration creates one handle and one instance with optional outline parameters.
- A counting uploader observes one upload before and after outline enablement.
- The same instance index appears in `outline_order` and its ordinary foreground stream.
- The outline and foreground streams both contain ordinary `DrawBatch` operations, with their
  respective outline and foreground materials.
- Draw ordering records the outline stream before opaque foreground operations.
- Model updates need one write and are observed by both draws; texture/emissive/material updates
  affect only ordinary pipeline selection.
- Removing the renderable removes the one handle and both stream appearances.
- Mirror exclusion removes the one instance index from both outline and foreground streams.

### Deformation tests

- An outlined skinned instance allocates the same one live vertex range as an unoutlined instance.
- One dirty outlined instance produces one deformation job, not two.
- Outline and foreground phase instance records contain the same `deformed_base`/count.
- Morph changes dirty the instance once and are visible to both vertex paths.
- Enabling/disabling an outline does not allocate or free a deformation range; removing the
  renderable frees its range once.

### Visual validation

Add a small example with a rotating static mesh and one animated/skinned GLTF using contrasting
outline colors and widths. Capture at least one deterministic desktop frame if the repository's
render-test harness can do so; otherwise retain it as a manual Vulkan validation scene. Verify:

- no front-face fill leaks through the foreground;
- silhouettes track animation and morphs;
- no pose lag between foreground and outline;
- depth occlusion between two overlapping outlined objects;
- acceptable behavior under non-uniform scale; and
- Vulkan validation reports no range lifetime, descriptor, or pipeline-layout errors.

## Acceptance criteria

- An authored ordinary renderable or GLTF-scoped modifier visibly produces an inverted-hull
  outline and leaves the original foreground material unchanged.
- Every outlined ECS renderable has exactly one `VisualInstance` and one `InstanceHandle`.
- The outline-stream and foreground-stream `DrawBatch` operations reference that same instance and
  GPU `MeshHandle`; no outline geometry upload occurs.
- An outlined skinned/morphed instance has one deformation allocation and one deformation compute
  job per dirty update, and both draws consume the same cached result.
- Outlines for multiple primitives/characters batch in a dedicated phase before normal foreground
  phases.
- GLTF projection and source updates work without serializing generated copies or re-importing the
  asset.
- Transform, removal, and mirror exclusion apply consistently to both stream appearances of the
  instance.
- Unsupported phase/material combinations are deterministic and diagnosed, not silently routed as
  opaque outlines.

## Follow-ups

- Alpha-tested outline fragments for hair cards and cutout textures.
- Blended outlines and a defined interaction with single- versus multi-layer transparency.
- Overlay and stencil-clipped outline subphases.
- Background and transmissive-material policy.
- Constant-pixel width, including XR stereo and mirror behavior.
- Non-uniform-scale-correct normal transformation (inverse transpose) if the first visual tests
  expose unacceptable width distortion.
- Optional crease-aware or authored outline normals for hard seams and split vertices.
- Multiple outline layers or per-primitive opt-out.

## Stop condition

Stop the first slice when the static and animated/skinned example both render through ordinary
`DrawBatch` operations in the dedicated outline phase, structural tests prove one visual instance,
one mesh upload, and one deformation result per renderable, GLTF projection/live updates work, and
unsupported phase combinations are explicitly rejected or skipped. Do not broaden the slice to
solve alpha silhouettes, pixel-constant width, overlay clipping, or general custom-material
authoring.
