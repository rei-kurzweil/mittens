# ToonOutline component and shared deformation

Date: 2026-09-18

Status: proposed

## Goal

Add a `ToonOutline` graphics modifier that can style one renderable or all renderables below a
wrapper. An outlined renderable produces two visual instances:

1. an outline instance, drawn in a dedicated outline phase; and
2. the ordinary foreground instance, drawn unchanged in its existing opaque, cutout, emissive,
   transmission, transparent, or overlay phase.

The two instances must reference the same uploaded GPU mesh. When the source is morphed or
skinned, they must also consume the same post-morph, post-skinning position and normal range rather
than allocate and compute a second deformation result.

`GLTFSystem` must project a `ToonOutline` authored on or around a `GLTFComponent` onto each imported
primitive, following the existing `ShadingComponent` projection model.

## Current architecture findings

The requested design fits the current renderer, but simple duplicate registration is not enough.

- `RenderableSystem::flush_pending` resolves one CPU mesh, calls
  `RenderAssets::gpu_mesh_handle`, and registers one `VisualWorld` instance. `RenderAssets` caches
  the resulting `MeshHandle` by `CpuMeshHandle`. Registering an outline from the same resolved CPU
  mesh can therefore share vertex and index buffers without another upload.
- `VisualWorld` currently assumes one visual handle per ECS renderable:
  `component_to_handle: HashMap<ComponentId, InstanceHandle>`, and `RenderableComponent` stores one
  `handle`. Transform, texture, skin, morph, and removal paths all rely on that assumption.
- The deformation cache is currently owned per `VisualInstance`. `sync_deformation_ranges`
  allocates a range for every instance with bones, and `record_dirty_deformations` emits one compute
  job per dirty instance. Two otherwise identical skinned instances would therefore duplicate the
  SSBO output and skinning work unless aliasing is made explicit.
- The compute output already contains exactly what an outline vertex stage needs: a deformed
  position and packed deformed normal. The regular cached-skinned vertex shader reads it through
  `i_deformed_base + gl_VertexIndex`.
- Draw lists are built in `VisualWorld::prepare_draw_cache`, then recorded in a fixed order in
  `VulkanoRenderer`. There is no outline phase today.
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
be finite and non-negative. Zero width disables creation of the derived instance. Alpha is retained
in the authoring type for a future blended-outline path; the first slice requires an opaque outline.

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
World-space expansion is enough to prove instance pairing, phase routing, mesh reuse, and shared
deformation.

The outline pipelines need:

- a static outline vertex shader reading the uploaded mesh position/normal;
- a cached-deformed outline vertex shader reading the same deformation SSBO as
  `cached-skinned-toon-mesh.vert`;
- a flat-color fragment shader;
- front-face culling, opaque color writes, depth test, and depth write; and
- the same winding/front-face convention as the existing mesh pipelines.

Do not generate an expanded CPU mesh, reverse indices, or upload an outline-specific vertex buffer.

## Visual identity and lifetime

Keep the ordinary foreground handle as the canonical handle stored on `RenderableComponent`.
Introduce a role-aware visual group for auxiliary instances rather than changing
`component_to_handle` to silently overwrite one of two handles. One possible shape is:

```rust
pub enum VisualInstanceRole {
    Foreground,
    ToonOutline,
}

pub struct RenderableVisualHandles {
    pub foreground: InstanceHandle,
    pub toon_outline: Option<InstanceHandle>,
}
```

The exact storage can live in `VisualWorld` or `RenderableSystem`, but these invariants are
required:

- stable `ComponentId` lookup returns the foreground handle by default;
- role lookup can find the outline handle;
- removal of an ECS renderable removes both handles;
- model-matrix updates reach both handles;
- foreground-only changes such as texture, shading material, transmission parameters, opacity,
  and emissive routing do not overwrite outline state;
- outline color/width changes update only the outline instance;
- adding, removing, or setting width to zero creates/removes only the derived instance without
  re-uploading the mesh or replacing the foreground handle; and
- picking, bounds, raycasting, mirrors' source-instance exclusion, and semantic selection continue
  to treat the foreground renderable as the ECS object. The outline must not become a second
  selectable object.

Avoid putting two independent general-purpose handles into every existing update path. Instead,
separate shared instance state (mesh identity, model, deformation source) from role-specific state
(material, color, width, phase), and provide explicit grouped operations for shared updates.

## Sharing the GPU mesh

Both registrations pass the same resolved `MeshHandle`:

```text
CpuMeshHandle
  -> RenderAssets::gpu_mesh_handle (cached upload)
  -> one MeshHandle
       -> foreground VisualInstance
       -> outline VisualInstance
```

This is already supported by the renderer's mesh map and draw batches. A test uploader should
assert that creating the pair performs one upload and that both instances contain the same
`MeshHandle`.

UV-baked variants are also safe: share the final resolved variant handle used by the foreground,
not `Renderable::base_mesh`.

## Sharing post-morph/post-skinning deformation

Model deformation as owned output plus aliases. The foreground instance owns the deformation
inputs and range; its outline instance aliases that owner:

```rust
pub enum DeformationBinding {
    None,
    Owned,
    Alias(InstanceHandle),
}
```

Equivalent internal representations are acceptable, but behavior must be:

- only `Owned` instances allocate/free a `DeformationRange`;
- only owners hold/update bones and active morph inputs;
- only dirty owners produce `GpuDeformationJob`s;
- instance-buffer construction resolves an alias to the owner's `deformed_base` and
  `deformed_count`;
- the outline cached-deformed vertex shader reads that resolved base;
- removing an alias never frees the owner's range;
- removing an owner first removes or detaches all aliases, so no dangling handle can reach a
  recycled range; and
- changing the owner's mesh reconciles the owner range and keeps the alias paired with it.

The alias is valid because the outline is derived from the same renderable and therefore has the
same mesh, morph weights, skin matrices, and vertex indexing. Assert these conditions in debug
builds rather than permitting arbitrary public aliasing.

Do not copy `bones_base` onto the outline and let it allocate its own output. That shares the bone
palette but still doubles the expensive vertex result. Also do not have `SkinnedMeshSystem` update
both handles: it should continue to update only the canonical foreground/owner.

The present allocator is per instance and its accounting assumes unique ranges. Alias-aware
allocation is therefore required before enabling outlines for skinned GLTF primitives.

## Draw-phase integration

Add an explicit `outline_order`, `outline_batches`, and outline instance buffer to `VisualWorld` and
the renderer. Outline instances must be excluded from all existing phase classifiers even if their
fields happen to resemble an opaque object.

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
automatically. Mirror source exclusion must exclude both roles belonging to the source renderable.

## GLTF projection

Extend the existing GLTF-scoped modifier flow rather than relying on the generated hierarchy:

1. Before spawning, call `RenderableSystem::resolve_toon_outline(world, gltf_component)` alongside
   `resolve_anime_shading`.
2. Pass the resolved `(source_component, value)` through `spawn_node_recursive`.
3. Under every generated primitive `RenderableComponent`, add
   `value.projected_from(source_component)` plus `Serialize.off()`.
4. Let normal component initialization emit `RegisterToonOutline`; do not call renderer APIs
   directly from the importer.
5. When the authored source changes, update every projection that references it and reconcile the
   corresponding visual pair.

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
- `src/engine/ecs/system/renderable_system.rs`: resolve the effective outline, create/reconcile the
  foreground/outline pair after the final mesh resolves, and remove the pair.
- `src/engine/ecs/system/transform_system.rs`: use a grouped model update so both roles move
  together.
- `src/engine/graphics/visual_world.rs`: visual roles, group lookup, outline stream, and owned/alias
  deformation lifetime.
- `src/engine/graphics/vulkano_renderer.rs`: outline pipelines, buffers, and recording before
  foreground.
- `assets/shaders/`: static and cached-deformed outline vertex stages plus flat fragment stage.

## First implementation slice

The smallest useful slice should still be vertical and should include cached-deformed GLTF. A
static-only slice would validate inverted-hull rendering but leave the central ownership question
unanswered and encourage a per-instance deformation design that later has to be replaced.

Implement one end-to-end slice with these boundaries:

- `ToonOutline` supports validated opaque color and world-unit width, construction, serialization,
  and live source updates.
- It resolves as an immediate renderable child or ancestor wrapper.
- `GLTFSystem` projects it onto every imported primitive using source-linked non-serialized copies.
- Ordinary opaque static renderables and opaque static/skinned GLTF primitives create a foreground
  and outline visual pair.
- Both instances use exactly one uploaded `MeshHandle`.
- Skinned/morphed pairs use one owned deformation range and one compute job; the outline aliases
  the cached position/normal result.
- One scene outline phase is recorded after the foreground depth clear and before opaque/cutout
  foreground.
- Width is world-space, the shader uses inverted hulls, and the pipeline culls front faces.
- Transform updates and removal affect the pair atomically.
- The outline instance is excluded from bounds, picking, raycasting, emissive extraction, and all
  ordinary draw lists.
- The slice explicitly rejects or skips outlines on overlay/background, blended transparency,
  transmission, stencil-clipped content, and alpha-shaped cutouts. These need phase- and
  alpha-specific decisions rather than accidental opaque behavior.

Although this crosses several files, splitting before the shared-deformation path lands gives no
representative result for the motivating humanoid case. The slice can still be kept narrow by not
adding pixel-width outlines, multiple outline layers, per-material glTF extensions, alpha-aware
outline fragments, or overlay routing.

## Test plan

### Component and resolution tests

- Defaults, finite/range validation, builders, serialization, and round-trip MMS.
- Immediate-child and ancestor-wrapper resolution, nearest override, and equal-scope duplicate
  error.
- Setting width to zero removes the derived visual while preserving the foreground handle.

### GLTF tests

- A direct `ToonOutline` child of a GLTF produces one source-linked, `Serialize.off()` projection
  under every imported primitive.
- A wrapper above a GLTF resolves identically.
- Updating the source changes every projected outline without re-importing meshes.
- A model with both static and skinned primitives chooses the matching outline vertex path.

### VisualWorld and renderer-structure tests

- Registration creates two handles with distinct roles and the same `MeshHandle`.
- A counting uploader observes one upload.
- The outline role appears only in `outline_order`; the foreground remains in its prior phase.
- Draw ordering records outlines before opaque foreground.
- Model updates reach both roles, while texture/emissive/material updates remain foreground-only.
- Removing the renderable removes both handles and all role/group lookup entries.
- Mirror exclusion removes both roles from the mirror view.

### Deformation tests

- A skinned pair allocates one live vertex range, and both emitted instance records reference the
  same `deformed_base`/count.
- One dirty source produces one deformation job, not two.
- Morph changes dirty the owner once and are visible to both vertex paths.
- Alias removal does not free the range; owner removal frees it once and cannot leave a dangling
  alias.
- Allocator reuse after removal cannot make a surviving outline read another renderable's range.

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
- Every outlined ECS renderable has exactly one foreground and at most one outline visual role.
- The pair has the same GPU `MeshHandle`; no outline geometry upload occurs.
- A skinned/morphed pair has one deformation allocation and one deformation compute job per dirty
  update, and both draws consume the same cached result.
- Outlines for multiple primitives/characters batch in a dedicated phase before normal foreground
  phases.
- GLTF projection and source updates work without serializing generated copies or re-importing the
  asset.
- Transform, removal, and mirror exclusion cannot leave one half of a visual pair behind.
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

Stop the first slice when the static and animated/skinned example both render through the dedicated
outline phase, structural tests prove one mesh upload and one deformation result per pair, GLTF
projection/live updates work, and unsupported phase combinations are explicitly rejected or
skipped. Do not broaden the slice to solve alpha silhouettes, pixel-constant width, overlay
clipping, or general custom-material authoring.
