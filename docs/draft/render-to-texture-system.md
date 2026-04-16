# Render-to-Texture System (draft)

## Goal

Introduce a `RenderToTextureSystem` that owns the ECS/runtime side of render-to-texture requests while the renderer owns the GPU execution side.

Initial scope is intentionally narrow:

- support internal renderer image publication only
- keep `Texture.render_image("...")` as the authoring/API entry point
- do **not** add scene captures, mirrors, portals, or cube cameras yet

This is the first step toward a broader render-to-texture architecture that will later support realtime reflections, mirror surfaces, portals, and cube-camera captures.

## Current behavior

Today, `Texture.render_image("selector")` spans three layers:

1. `TextureComponent::render_image(...)` stores the selector string on the texture component.
2. `TextureSystem`/runtime state ensures there is a `TextureHandle` associated with that selector.
3. `VulkanoRenderer` decides which internal/intermediate images to publish and copies those images into the selector-backed runtime texture.

That works, but the ownership is blurred:

- selector discovery lives near ECS texture registration
- selector → handle mapping lives in `VisualWorld`
- publication policy and GPU copy logic live in `VulkanoRenderer`

## What `Texture.render_image(...)` does today

When a texture component is registered with `render_image` set, the side effects are currently:

1. a sampled runtime texture handle is created if the selector does not already have one
2. the consuming renderable binds that handle through the normal `TextureSystem` attachment path

The important clarification is that this does **not** create new render pipelines per selector.

What may be allocated:

- a placeholder/uploaded texture handle for the selector
- a renderer-owned sampled destination image matching the source extent/format
- temporary offscreen targets for debug-only paths (for example, stencil visualization)

What is **not** created per selector:

- new graphics pipelines
- a new shader permutation

Publication itself is still renderer-driven: if the renderer knows how to produce the named image, it copies that image into the selector-backed runtime texture.

## Desired ownership split

### `RenderToTextureSystem`

Owns the runtime/ECS side:

- discover `Texture.render_image(...)` usage from components
- maintain a **consumer registry** describing who samples a runtime-produced texture
- maintain a **producer registry** describing what is expected to populate a selector
- ensure selectors have stable runtime `TextureHandle`s
- provide the renderer with the set of requested runtime publications

#### Consumer registry

The consumer registry answers:

- which component wants a render-to-texture result?
- which selector does it sample?
- what kind of consumer is it?

For the initial implementation, the only consumer kind is:

- `Texture.render_image("selector")`

#### Producer registry

The producer registry answers:

- what is supposed to populate a selector?
- is the selector backed by an internal renderer image, a scene capture, a cube capture, a mirror, or a portal?

This keeps the shape correct for future authored capture systems even though the current implementation only activates one producer kind.

Planned producer kinds:

- `InternalRendererImage`
- `SceneCapture`
- `CubeCapture`
- `Mirror`
- `Portal`

### `VisualWorld`

Continues to hold the selector → `TextureHandle` mapping used by texture attachment and rendering.

Later it may also hold renderer-friendly render-to-texture request snapshots, but that is not required for the initial skeleton.

### Renderer (`VulkanoRenderer`)

Owns GPU execution only:

- produce internal images (post-process, debug views, future captures)
- allocate GPU images needed to publish them
- copy or render into the destination texture associated with a selector

## Initial skeleton scope

The initial implementation should support only internal renderer image publication.

That means:

- the consumer registry is real and active
- the producer registry is real but currently only populated with `InternalRendererImage`
- future camera/mirror/portal producers are represented in shape only, not behavior

Examples:

- `render_graph.emissive_pass.output`
- `render_graph.bloom.blur`
- `render_graph.stencil_clip.debug`

Non-goals for the initial skeleton:

- camera capture components
- cube capture components
- per-surface mirrors
- portals
- author-defined render passes

## Proposed evolution path

### Phase 1: internal publication skeleton

- add `RenderToTextureSystem`
- split it into consumer and producer registries
- move selector-handle bootstrap out of `TextureSystem`
- keep renderer-side publication logic as-is
- populate only `InternalRendererImage` producer requests

### Phase 2: renderer publication helper

- move internal publication bookkeeping out of `VulkanoRenderer` into a focused helper/module
- unify post-process and debug publication paths under one renderer-facing interface

### Phase 3: authored scene captures

- add components such as `SceneCaptureComponent`, `CubeCaptureComponent`, `MirrorSurfaceComponent`, `PortalSurfaceComponent`
- system populates non-internal producer requests in the producer registry
- renderer executes those requests

## Immediate checks after skeleton

After the skeleton lands, verify:

- `Texture.render_image(...)` still binds a texture to consuming renderables
- bloom/emissive internal publications still appear
- stencil debug publication still has a stable selector-backed handle path
