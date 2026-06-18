# RenderView / Mirror Inventory

## Scope

This note inventories the current state of:

- `MirrorComponent`
- `RenderView` as the renderer-facing concept of "one scene draw from one view"
- how many scene draws we do today for:
  - desktop / main `Camera3D`
  - XR / `CameraXR`
  - offscreen cameras / render targets, especially mirrors

It also records what is already documented elsewhere, what is now stale, and what is still missing.

## Existing docs worth keeping

- [docs/spec/mirror-component.md](/home/rei/_/cat-engine/docs/spec/mirror-component.md)
  - Good conceptual design doc.
  - Partially stale: it says mirrors are still hypothetical in `src/`, but `MirrorComponent`, `MirrorSystem`, `VisualMirror`, and `RenderView` now exist.
- [docs/draft/mirror-implementation-plan.md](/home/rei/_/cat-engine/docs/draft/mirror-implementation-plan.md)
  - Useful short implementation sketch.
  - Also partially stale for the same reason.
- [docs/spec/render-to-texture.md](/home/rei/_/cat-engine/docs/spec/render-to-texture.md)
  - Correct source for the implemented runtime-texture bridge.
  - Important because mirrors can reuse this publication/sampling path.
- [docs/analysis/renderer-cpu-time-complexity.md](/home/rei/_/cat-engine/docs/analysis/renderer-cpu-time-complexity.md)
  - Correct source for current XR cost: we render each eye offscreen and block on completion.

## Current code inventory

### Mirror authoring/runtime pieces

- `MirrorComponent` exists in [src/engine/ecs/component/mirror.rs](/home/rei/_/cat-engine/src/engine/ecs/component/mirror.rs).
- `MirrorSystem` exists in [src/engine/ecs/system/mirror_system.rs](/home/rei/_/cat-engine/src/engine/ecs/system/mirror_system.rs).
- `VisualWorld` stores `Vec<VisualMirror>` in [src/engine/graphics/visual_world.rs](/home/rei/_/cat-engine/src/engine/graphics/visual_world.rs).
- `RenderView` / `RenderViewKind` exist in [src/engine/graphics/vulkano_renderer.rs](/home/rei/_/cat-engine/src/engine/graphics/vulkano_renderer.rs).

### Camera state pieces

- `CameraSystem` still owns authored active camera selection:
  - window target via `Camera3D` / `Camera2D`
  - XR target via `CameraXR`
- `VisualWorld` still stores persistent target-scoped cameras as:
  - `CameraTarget::Window`
  - `CameraTarget::Xr`

So today there are two separate concepts:

- persistent authored camera state in `VisualWorld`
- ephemeral renderer-facing `RenderView`

That split is reasonable. It is already how XR is rendered. Mirrors should fit the same pattern.

## Scene-draw inventory today

### 1. Desktop / window rendering

Current path:

- `Universe` calls `render_visual_world(...)`
- renderer builds one `RenderViewKind::Window`
- renderer records one full scene pass for the active window camera

So in normal desktop mode, scene draw count is:

- `1` scene draw for the main window camera

Post-processing may add more full-screen passes, but that is not another scene draw.

### 2. XR rendering

Current path:

- `OpenXRSystem` computes per-eye camera data and writes it into `VisualWorld`
- for each eye, it calls `render_xr_eye_offscreen(...)`
- renderer builds one `RenderViewKind::XrEye { eye }` per eye
- each eye renders the full scene into an offscreen target
- then `xr_renderer::copy_offscreen_to_xr_layers(...)` copies those images into the XR swapchain

So in XR mode, scene draw count is:

- `2` scene draws for the XR eyes

Desktop mirror / companion window behavior is separate:

- if the normal window renderer is still running, that is another `1` scene draw
- total combined desktop+XR frame cost is therefore effectively `3` scene draws today:
  - `1` window
  - `2` XR eyes

This should be verified against intended runtime behavior, but it matches the current render entry points.

### 3. Mirror / offscreen rendering

Current state is split:

- `MirrorSystem` does derive reflected views every frame
- `MirrorSystem` does register `VisualMirror`
- `MirrorSystem` does assign a runtime texture key like `capture.mirror.<guid>.color`
- `MirrorSystem` does force the source renderable to use `MaterialHandle::MIRROR`
- `MirrorSystem` does attach or update a `TextureComponent.render_image(...)`

But I do not see any code that actually:

- iterates `visual_world.mirrors()`
- allocates per-mirror color/depth targets
- constructs `RenderViewKind::Mirror { ... }`
- records mirror scene passes before the main window pass
- publishes rendered mirror images into the runtime texture handle for the mirror key

So current mirror scene draw count is:

- `0` actual mirror scene draws

In other words:

- mirror discovery exists
- mirror sampling hookup exists
- mirror pass execution appears missing

## Mirror unit vs mirror render views

We should distinguish between:

- one mirror as an authored/runtime unit
- the set of render views required to service that mirror for the currently active viewers

The mirror itself is singular:

- one `MirrorComponent`
- one discovered `VisualMirror`-like logical mirror record
- one mirror surface in the world

But the render work for that mirror is not necessarily singular.

The correct mental model is:

- one mirror can require multiple related `RenderView`s
- those views are grouped under the same mirror, but are still separate scene draws

Examples:

- window-only frame:
  - `1` mirror
  - `1` mirror render view
- XR-only stereo frame:
  - `1` mirror
  - typically `2` mirror render views, one per eye
- window + XR frame:
  - `1` mirror
  - separate window-derived and XR-derived mirror render views

So we should not think of "one mirror = one render target" as a guaranteed rule.
The more accurate rule is:

- one mirror owns a family of related capture views
- the renderer decides how many concrete `RenderView`s are needed for that mirror in the current frame

This also means mirror runtime texture publication may need to be keyed by both:

- mirror identity
- viewer family or eye

instead of only by mirror identity.

## What `RenderView` means in current code

`RenderView` already exists as the right renderer-local abstraction:

- `Window`
- `XrEye { eye }`
- `Mirror { mirror_component }`

But only these are currently exercised by frame scheduling:

- `Window`
- `XrEye { eye }`

`RenderViewKind::Mirror` exists structurally but appears unused.

That means the conceptual migration from "camera target + eye" to "draw this scene for this render view" is partly done, not fully done.

## What is missing

### Missing implementation

1. Mirror pass scheduling
- Before the main window pass, renderer needs to walk `visual_world.mirrors()` and draw them.

2. Per-mirror render targets
- Need allocation/reuse of color + depth images for each mirror.
- Need a stable policy for extent derived from mirror quality and aspect.

3. Runtime texture publication for mirrors
- Mirror color output needs to be copied or swapped into the stable runtime texture handle already associated with `capture.mirror.<guid>.color`.

4. Actual use of `RenderViewKind::Mirror`
- The renderer should build a real `RenderView` for each mirror and route it through the same draw path as window/XR.

5. Explicit render ordering
- Mirror passes must happen before the main pass that samples them.

6. Exclusion / recursion policy
- Current code marks the source instance for the mirror surface, but I do not see renderer-side exclusion logic yet.
- Without that, self-reflection and mirror-in-mirror behavior are unresolved.

### Missing validation / observability

1. No stats for render-view count per frame
- We should be able to answer "how many scene draws did this frame do?" directly from renderer stats.

2. No tests around mirror execution
- There should be at least one test or debug path that proves mirror textures are being rendered and published, not only bound.

3. No single source-of-truth doc
- The mirror docs currently mix:
  - conceptual future design
  - now-implemented runtime-texture bridge
  - partially implemented mirror plumbing

## Open questions

1. Window + XR policy
- When XR is active, do we intentionally keep rendering the desktop window scene every frame?
- Intended policy:
  - if no `Camera3D` or `Camera2D` is active, and only `CameraXR` is active, only XR should render
  - in that case, total scene-view count should be `xr_eye_count + mirror_render_view_count`
  - there is no separate window scene render in that mode
- If both window and XR cameras are active, we should still document whether both families render every frame.

2. Mirror source camera policy
- `MirrorSystem` currently prefers XR if available, otherwise window.
- The intended model should be:
  - mirrors are singular logical units
  - mirror captures are derived per active viewer family
  - when both window and XR are active, a single shared mirror capture is generally not correct if viewer poses differ
- This implies mirror capture should usually be split into multiple related render views under one mirror, rather than one globally shared mirror image.

3. Mirror extent policy
- `quality` currently becomes a `resolution_scale` relative to `1024.0`, while aspect comes from bounds.
- Is the renderer supposed to interpret this as:
  - one axis equals `quality`
  - square target
  - target scaled from window size
  - target matched to mirror surface aspect

4. Visibility policy
- Render all mirrors every frame, only visible mirrors, or a capped set?

5. Recursion policy
- Skip current mirror only?
- Skip all mirrors inside mirror passes?
- Allow limited recursion later?

6. Clip plane policy
- Is v1 acceptable without oblique clip-plane support?

7. Material contract
- Is `MirrorComponent` supposed to force `MaterialHandle::MIRROR`, or should mirror sampling remain authored and explicit?

8. Ownership boundary
- Should `MirrorSystem` continue mutating renderable material/texture state directly, or should it only publish mirror view requests and let renderer/material systems handle the rest?

## Recommended task split

### Task 1: make draw-counts explicit

Add renderer stats for:

- window scene views rendered this frame
- XR scene views rendered this frame
- mirror scene views rendered this frame
- mirror logical units discovered this frame
- total scene views rendered this frame

This gives us a concrete answer to the RenderView inventory question in runtime, not just in code inspection.

### Task 2: finish mirror render execution

Implement the missing renderer path:

- iterate `visual_world.mirrors()`
- create/reuse offscreen targets
- create `RenderViewKind::Mirror`
- render mirror views before the main pass
- publish the result into the mirror runtime texture handle

### Task 3: settle policy decisions

Document and decide:

- XR + desktop concurrent rendering policy
- mirror source-camera selection policy
- mirror target extent policy
- mirror recursion / exclusion policy

### Task 4: clean up docs

After implementation decisions:

- update [docs/spec/mirror-component.md](/home/rei/_/cat-engine/docs/spec/mirror-component.md) so it clearly separates:
  - implemented
  - partially implemented
  - future work
- either delete or fold [docs/draft/mirror-implementation-plan.md](/home/rei/_/cat-engine/docs/draft/mirror-implementation-plan.md) into the spec/task docs
- add a short renderer doc specifically for `RenderView`

## Proposed immediate next step

The next concrete engineering step should be:

1. add render-view counters/stats
2. wire real mirror pass execution through `RenderViewKind::Mirror`
3. then update the mirror spec to match the code

Until that happens, the main gap is not authoring or texture sampling. The main gap is that mirrors are described and partially wired, but not actually rendered as offscreen scene views.
