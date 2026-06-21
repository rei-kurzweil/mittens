# Mirror component

This document explores how a planar mirror could fit into cat-engine **without changing `src/` yet**.

The sampling side of this feature is no longer hypothetical: the current engine already has a
runtime-texture bridge via `TextureComponent.render_image` / `Texture.render_image("...")`.
So the unresolved work for mirrors is primarily **capture/publication**, not “how can a scene
surface sample a runtime-produced image at all?”.

The goal is to describe:
- an authoring-facing `MirrorComponent`
- the runtime concept of a derived `MirrorCamera`
- the render-graph / render-loop changes needed to render mirrors correctly
- the main constraints, tradeoffs, and a reasonable first implementation

## Proposed authoring shape

```rust
pub struct MirrorComponent {
    pub quality: i32, // resolution of one axis of the mirror camera
}
```

Interpretation:
- `MirrorComponent` is attached to the entity that visually represents the mirror surface.
- `quality` controls the offscreen render target resolution for that mirror.
- The mirror itself does **not** become the active window camera.
- Instead, it causes a **runtime-only derived camera** to exist for rendering the mirror texture.

For a first pass, `quality` should be clamped to a sane range, e.g. `64..=2048`.

---

## High-level idea

A mirror is conceptually:
1. a visible surface in the main scene,
2. plus one or more offscreen cameras whose poses are derived from the active viewer family by reflection across the mirror plane,
3. plus one or more render targets whose color images are sampled by the mirror surface material.

So the runtime model is closer to:

```rust
struct MirrorRuntime {
    mirror_component: ComponentId,
    reflected_camera: MirrorCamera,
    color_image: OffscreenImage,
    depth_image: OffscreenDepth,
}
```

Where `MirrorCamera` is **not** authored directly and is **not** a normal scene camera component.

---

## Why this should not just be another normal camera target

Today, camera state is organized around a small set of global outputs:
- `CameraTarget::Window`
- `CameraTarget::Xr`

That works because those are effectively singleton outputs for a frame.

A mirror is different:
- there may be **multiple mirrors** in one frame,
- each mirror needs its **own view/proj**,
- each mirror renders into its **own offscreen image**,
- mirror cameras should **not compete** with the active window camera selection,
- mirrors are not user-authored cameras in the same sense as `Camera3DComponent`.

So the clean mental model is:
- keep authored cameras as-is,
- add a runtime-only class of **derived render views** for mirrors.

That can still be *handled by the camera system*, but it probably should not be represented as another `CameraTarget` enum value like `Mirror`, because one enum value cannot distinguish N mirror instances or N viewer families.

---

## Proposed runtime model

## 1. Authored component

```rust
MirrorComponent {
    quality: int
}
```

Responsibilities:
- declares that this entity behaves as a planar mirror,
- provides quality / resolution preference,
- does **not** directly store view/proj matrices.

## 2. Runtime-only `MirrorCamera`

Conceptually:

```rust
struct MirrorCamera {
    source_camera: CameraHandle,
    mirror_component: ComponentId,
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    extent: [u32; 2],
}
```

Responsibilities:
- derived each frame from the active monoscopic camera or active stereoscopic camera,
- reflects the viewer across the mirror plane,
- carries the render extent implied by `quality`,
- exists only for rendering, not as an ECS-authored camera component.

## 3. Runtime-only `MirrorViewRequest`

A renderer-facing shape may be even better than exposing mirror cameras directly:

```rust
struct MirrorViewRequest {
    mirror_component: ComponentId,
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    extent: [u32; 2],
    source_camera_transform: Transform,
}
```

This avoids overloading the existing camera registry and lets the renderer consume “extra views to render this frame” directly.

---

## Mirror plane definition

A planar mirror needs a plane in world space.

The mirror system needs to answer:
- where is the plane origin?
- what is the plane normal?
- what is the mirror surface aspect ratio?

Likely first-pass rule:
- the mirror plane is derived from the mirror entity’s `TransformComponent`,
- the mirror surface lies in local XY,
- the local +Z axis is the mirror normal,
- the world transform gives the plane origin + normal.

That is simple and explicit, but it implies the mirror mesh/material should follow that convention.

Open question:
- if the visual mesh is arbitrary, do we still define the mirror plane from transform only, or from mesh bounds?

For v1, **transform-defined plane** is the safest assumption.

---

## How the reflected camera is derived

Assume:
- there may be an active monoscopic camera,
- there may be an active stereoscopic camera,
- the mirror plane is defined in world space,
- the mirror wants a reflected version of each active viewer family.

Per frame:
1. Get the active source camera transform for each active viewer family.
2. Reflect the camera position across the mirror plane.
3. Reflect the camera basis vectors across the mirror plane.
4. Build a reflected world transform.
5. Invert it to produce the mirror view matrix.
6. Reuse the source camera projection, possibly adjusted for mirror texture aspect ratio.

This gives the expected “move left, reflection moves right” behavior.

### Projection choice

The simplest projection rule is:
- use the same FOV / near / far as the source camera,
- recompute aspect ratio from the mirror render target extent.

That is enough for a first implementation.

### Clip plane

A correct planar mirror usually also applies an **oblique clip plane** so the reflected camera only renders what is “in front of” the mirror plane from reflection space.

Without this, common artifacts include:
- geometry behind the mirror plane appearing in reflection,
- self-intersection / halo artifacts near the mirror surface,
- “seeing through the back side” of the reflection.

So the likely progression is:
- **v1**: reflected camera, no clip plane, plus mirror-surface exclusion and bias
- **v2**: add oblique near-plane clipping against the mirror plane

---

## Where this fits in the current renderer

Today, the renderer already knows how to render a scene for a given:
- `camera_target`
- concrete view index
- color attachment
- depth attachment
- extent

Relevant current shape:
- `VisualWorld` stores a small set of cameras (`Window`, `Xr`)
- `prepare_transparent_multi_draw_cache_for_eye(target, eye)` sorts transparency for one concrete stereoscopic view
- `build_draw_batches_command_buffer(...)` records all phases for one view into one target

This is encouraging because mirror rendering is also “render the same scene from another view into another target”.

The main mismatch is that current view selection is **global-target-based**, while mirrors need **many ad hoc views per frame**.

---

## Recommended renderer abstraction change

Instead of teaching `VisualWorld` about a new persistent `Mirror` camera target, introduce a renderer-local concept like:

```rust
struct RenderView {
    view: [[f32; 4]; 4],
    proj: [[f32; 4]; 4],
    viewport: [u32; 2],
    kind: RenderViewKind,
}

enum RenderViewKind {
    Window,
    XrEye { eye: usize },
    Mirror { mirror_component: ComponentId },
}
```

Then adapt the render path so phase recording works from a `RenderView`, not strictly from `CameraTarget + view-index`.

That would let the renderer:
- render the main window view,
- render XR eyes,
- render N mirror views,
- all using the same phase-recording helpers.

### Why this is better than adding `CameraTarget::Mirror`

Because `CameraTarget::Mirror` only tells you *what kind* of view it is, not *which mirror instance* it belongs to.

Mirrors need per-instance:
- matrices,
- render targets,
- extent,
- cached image resources,
- potentially visibility data.

So a per-frame `RenderView` object is the better fit.

---

## Render graph changes needed

## Current graph (simplified)

Today the window path is roughly:
1. Acquire swapchain image
2. Build one command buffer for the active window camera
3. Run phases in one dynamic-rendering scope:
   - background
   - background occluded+lit
   - clear depth
   - opaque
   - cutout
   - transparent single
   - transparent multi
   - overlay
4. Present

## Mirror-capable graph (conceptual)

With mirrors, the frame becomes:

1. Update authored cameras as usual.
2. Discover active/visible mirrors.
3. For each mirror:
   - derive a `MirrorCamera` / `RenderView`
   - allocate or reuse offscreen color/depth images
   - render the scene into the mirror color image
4. Render the main window camera.
5. In the main window pass, mirror surfaces sample their already-rendered mirror image.
6. Present.

That means the render graph becomes:

```text
for each visible mirror:
    mirror scene render -> mirror color image

main scene render -> swapchain / MSAA target
    mirror material samples mirror color image
```

This is the key render-graph change: **extra offscreen scene renders must happen before the main scene pass that samples them**.

---

## Mirror pass details

Each mirror pass is basically a normal scene render with some special rules.

### Attachments

For each mirror:
- color image: sampled later by the mirror surface
- depth image: local to the mirror pass

Suggested initial formats:
- color: same general format family as normal scene color, preferably sampleable
- depth: regular depth attachment format

The exact format choice depends on whether the main renderer path is later generalized for post-processing / HDR.

### Phase reuse

Mirror rendering should reuse the same scene phases as the window render:
- background
- background occluded+lit
- opaque
- cutout
- transparent single
- transparent multi

Likely **not** for v1:
- overlay (gizmos / editor overlays probably should not appear in mirrors)

That means the existing phase helpers are mostly reusable.

### Transparent sorting

Transparent multi-layer order depends on camera view.

So mirror rendering needs its own per-mirror transparent sort, exactly like XR already does per eye.

This is one of the strongest arguments for a generic `RenderView` concept.

---

## Mirror surface sampling path

Rendering the mirror view is only half of the feature. The mirror surface in the main pass must also sample the mirror texture.

The current render-to-texture bridge already gives us the basic sampling mechanism:

- runtime-owned images can be published behind stable `TextureHandle`s
- scene content can sample them through `TextureComponent.render_image`
- authored MMS can already write `Texture.render_image("render_graph....")`

So a mirror does **not** need a brand-new texture consumption path. It needs a way to publish its
offscreen color image into the same runtime-texture bridge used by render-graph outputs.

Conceptually, that means a mirror implementation could publish a selector key like:

```text
capture.mirror.<mirror_id>.color
```

and the visible mirror surface would sample that runtime image through an ordinary texture binding.

Whether the user explicitly authors `Texture.render_image("capture.mirror....")` or whether
`MirrorComponent` wires the associated surface texture automatically is a separate authoring choice.
But the renderer-side texture publication path already exists.

The key requirement is:
- during the **main scene pass**, the mirror mesh must bind the mirror’s offscreen color image as a sampled texture.

This is different from ordinary asset-backed textured meshes because the texture is generated at
runtime per frame, not loaded from disk — but it should still flow through the same stable-handle
runtime-texture bridge.

So there are really **two renderer changes**:
1. render an extra scene view into an offscreen image,
2. publish that image through the existing runtime-texture bridge so the mirror surface draw in the
    main pass can sample it.

---

## What the camera system would need to do

If we want mirrors to be “handled by camera system”, the camera system should probably own only the **view derivation**, not the render target management.

A good split would be:

### Camera system responsibilities
- find authored mirrors,
- derive reflected view/proj from the active source camera,
- expose per-frame mirror view requests,
- keep mirror views separate from `active_window_camera` and `active_xr_camera`.

### Renderer responsibilities
- allocate/reuse per-mirror offscreen images,
- execute mirror render passes,
- make mirror images available to mirror-surface materials,
- decide pass ordering and culling behavior.

That keeps the camera system focused on “what are the views?” and the renderer focused on “how do we render them?”.

---

## What `VisualWorld` would need

Today `VisualWorld` stores a small amount of camera state globally.

For mirrors, the cleanest direction is probably **not** to permanently store all mirror cameras inside `VisualWorld`, because they are:
- transient per-frame derived views,
- potentially many,
- renderer-driven.

Instead, `VisualWorld` can remain the scene source of truth, while mirror-specific views are passed into rendering as ephemeral view data.

What does need to become view-parameterized:
- transparent multi-layer sorting
- camera UBO creation
- possibly culling, if culling becomes view-dependent later

In practice, the renderer already does most of this per view.

---

## Visibility / culling policy

Rendering every mirror every frame could be expensive.

Possible policies:

### v1: render every enabled mirror
Pros:
- simple
- deterministic

Cons:
- expensive if many mirrors exist

### v1.5: render only mirrors visible in the main camera
Pros:
- avoids wasted offscreen renders

Cons:
- needs mirror visibility detection / bounding checks

### v2: cap mirror count
Example:
- render only the nearest 1–2 visible mirrors
- remaining mirrors show stale texture or fallback

For a first implementation, “visible mirrors only” is a good target if easy; otherwise “all mirrors” is acceptable for an experimental feature.

---

## Self-render and recursion rules

A mirror should generally **not** render itself recursively in v1.

Without guardrails, a mirror pass can contain:
- the mirror surface itself,
- another mirror that sees back into the first,
- infinite recursion / exploding cost.

Recommended first-pass rules:
- exclude the current mirror surface from its own mirror pass,
- disable mirror-in-mirror recursion entirely,
- render other mirrors as non-reflective fallback surfaces or skip them,
- recursion depth = 0.

This keeps the feature tractable.

---

## Material / scene authoring implications

For a mirror to work, there is an implicit pairing between:
- the `MirrorComponent`, and
- a renderable surface that uses the mirror texture.

Likely authoring assumption:
- the entity that owns `MirrorComponent` also owns the mesh/material used as the visible mirror plane.

Open questions:
- does the mirror texture apply to all renderables under that transform?
- is there exactly one mirror surface mesh per `MirrorComponent`?
- should mirror material be explicit, or should `MirrorComponent` imply it?

For v1, the simplest rule is:
- `MirrorComponent` implies that the associated renderable surface samples the mirror’s published
  runtime texture.

With the current engine shape, that likely means one of two authoring/runtime models:

1. `MirrorComponent` runtime automatically binds the associated surface texture to the mirror’s
    published selector/image
2. the authored surface uses `Texture.render_image("capture.mirror....")`, while `MirrorComponent`
    is responsible only for producing that capture

Option 1 is nicer authoring. Option 2 is closer to the already-shipped `render_image` bridge.
Either way, the mirror should reuse the existing runtime-texture publication/sampling path.

---

## Resolution strategy

The prompt defines:

```rust
quality: int // resolution of one axis of mirror camera
```

This leaves one choice open: how to derive the other axis.

Reasonable options:

### Option A: square mirror target
- extent = `[quality, quality]`
- simplest
- wastes pixels for non-square mirrors

### Option B: preserve source viewport aspect
- extent derived from active window camera aspect
- easy, but not ideal for tall/wide mirror surfaces

### Option C: preserve mirror surface aspect
- derive aspect ratio from mirror geometry / authored size
- compute secondary axis from `quality`
- best visual fit

Recommended direction:
- **v1**: square target or source-viewport aspect
- **later**: derive aspect from mirror surface bounds

If the mirror mesh convention is well-defined, preserving mirror-surface aspect is the best long-term answer.

---

## Likely minimum implementation plan later

When implementation starts, a practical sequence would be:

1. Add `MirrorComponent` as authored data only.
2. Add mirror discovery + reflected-view derivation.
3. Introduce a renderer-local `RenderView` abstraction.
4. Render one offscreen mirror pass before the main window pass.
5. Add a mirror-surface material that samples the rendered image.
6. Exclude the current mirror surface from its own reflection.
7. Disable recursion.
8. Add quality clamping + cached image reuse.

Because the stable runtime-texture bridge already exists, step 5 should preferably be implemented
as “bind/publish a mirror runtime texture through the same `render_image` path” rather than as an
entirely separate one-off sampled-image mechanism.

This keeps the risk concentrated in renderer plumbing rather than broad ECS churn.

---

## Main code areas that would eventually change

No changes are proposed in this document, but the likely implementation touch points are:
- `src/engine/ecs/component/` for `MirrorComponent`
- `src/engine/ecs/system/camera_system.rs` for reflected mirror view derivation
- `src/engine/graphics/visual_world.rs` only if additional view-parameterization helpers are needed
- `src/engine/graphics/vulkano_renderer.rs` for offscreen mirror passes and render ordering
- `src/engine/graphics/vulkano_cbb.rs` if phase helpers need to become more view-generic
- material / shader setup for the mirror surface sampling path

---

## Important design conclusion

The biggest structural insight is:

> A mirror is less like “another active camera target” and more like “an extra derived render view with a sampled output image”.

And that sampled output image should plug into the same runtime-texture bridge already used by
`Texture.render_image(...)` and render-graph-published pass outputs.

So the likely architecture is:
- author a `MirrorComponent`,
- derive a runtime `MirrorCamera` from the active viewer camera,
- render that view offscreen,
- sample the result in the main pass,
- keep mirror views out of the normal active-camera selection path.

That gives a design that fits the current engine shape without forcing mirrors to pretend they are ordinary singleton cameras.

---

## Open questions

1. **Plane convention**: should mirror plane come purely from transform, or from mesh bounds/orientation?
2. **Aspect ratio**: should `quality` imply square, viewport-matched, or mirror-surface-matched render targets?
3. **Visibility policy**: render all mirrors, only visible mirrors, or a capped set?
4. **Clip plane**: is a no-clip v1 acceptable, or do we need oblique clipping immediately?
5. **Mirror material binding**: should `MirrorComponent` imply a dedicated shader/material, or should that be authored separately?
6. **Other mirrors in reflection**: should they be skipped entirely, rendered opaque/fallback, or supported later with limited recursion?
7. **Editor overlays**: should overlays/gizmos ever appear in mirror passes, or always be excluded?

---

## Recommended v1 stance

If the goal is “get a convincing planar mirror working with minimal architectural damage”, the best v1 is:
- planar mirrors only,
- one runtime-derived `MirrorCamera` per active mirror,
- offscreen render before main window render,
- mirror surface samples that texture in the main pass,
- no recursion,
- no overlay in reflections,
- probably no clip plane initially,
- renderer-local `RenderView` abstraction rather than expanding `CameraTarget`.

That matches the current renderer well and leaves room to refine correctness later.
