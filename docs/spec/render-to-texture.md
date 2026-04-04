# Render to texture

This document describes the **implemented** runtime-texture bridge used by render-graph-published scene textures.

It covers what is currently true in `src/`.

For future `RenderImage*` abstraction ideas, see [docs/draft/render-image.md](docs/draft/render-image.md).

For the broader `TextureComponent` behavior across URI-backed, GLTF-imported, handle-backed, and
render-image-backed usage, see [docs/spec/texture.md](docs/spec/texture.md).

## 1. Scope

This spec covers Layer A only:

- exposing renderer-owned pass outputs as sampleable scene textures
- wiring those outputs into ordinary textured geometry
- keeping texture identity stable while image contents update over time

This spec does **not** cover:

- explicit capture cameras
- mirrors
- portals
- monitors / CCTV
- a first-class `RenderImageHandle` API

## 2. Current implementation summary

The current runtime path is:

1. MMS authors a `TextureComponent` with `render_image: Option<String>`
2. `TextureSystem` resolves that selector string to a stable `TextureHandle`
3. `VisualWorld` stores that stable handle in a `runtime_texture_handles` map keyed by selector string
4. `RenderGraph` configures which pass output should publish into which selector key
5. the renderer copies the pass output into a sampled image for that handle
6. the handle stays stable while the underlying image contents update across frames

This means the scene-facing bridge is currently:

- `Texture.render_image("render_graph.emissive_pass.output")`
- `Texture.render_image("render_graph.bloom.blur")`

not a dedicated `RenderImageHandle` API.

## 3. Scene-facing wiring

### `RenderGraph`

The scene-facing root is `RenderGraph`, not `PostProcessing`.

Current example shape:

```text
let emissive_debug_texture = Texture.render_image("render_graph.emissive_pass.output")
let bloom_debug_texture = Texture.render_image("render_graph.bloom.blur")

RenderGraph {
    EmissivePass {
        emissive_debug_texture
    }

    Bloom {
        intensity(1.25)
        radius_ndc(0.075)
        emissive_scale(1.35)
        half_res(true)
        bloom_debug_texture
    }
}
```

### Published selector keys

Implemented keys today:

- `render_graph.emissive_pass.output`
- `render_graph.bloom.blur`

These are ordinary authored selector strings today; the runtime no longer has a separate
renderer-side debug-overlay path for showing them.

## 4. ECS/runtime wiring

### `TextureComponent`

`TextureComponent` supports:

- URI-backed textures
- handle-backed textures
- runtime selector-backed textures via `render_image: Option<String>`

Relevant implementation:

- [src/engine/ecs/component/texture.rs](src/engine/ecs/component/texture.rs)

### `TextureSystem`

When a texture has `render_image = Some(key)` and no GPU texture yet:

- it looks up `VisualWorld::runtime_texture_handle(key)`
- if absent, it allocates a 1x1 placeholder sampled texture
- it stores that stable `TextureHandle` back into `VisualWorld`

Relevant implementation:

- [src/engine/ecs/system/texture_system.rs](src/engine/ecs/system/texture_system.rs#L175-L198)
- [src/engine/graphics/visual_world.rs](src/engine/graphics/visual_world.rs#L1432-L1445)

This is the current stable-handle bridge.

## 5. Render-graph publication

`SystemWorld::register_render_graph` translates the authored `RenderGraph` subtree into `PostProcessingConfig`.

Current pass-to-selector mapping:

- `EmissivePass { Texture {} }` defaults to `render_graph.emissive_pass.output`
- `Bloom { Texture.render_image("render_graph.bloom.blur") }` publishes the blurred bloom image

Relevant implementation:

- [src/engine/ecs/system/system_world.rs](src/engine/ecs/system/system_world.rs#L717-L804)

## 6. Renderer publication model

The renderer does **not** currently expose a first-class `RenderImageRegistry`.

Instead, it:

1. collects runtime texture publications from the active post-process config
2. copies the selected pass outputs into sampled GPU images
3. swaps those images onto the stable `TextureHandle` at the next frame boundary

Important behavior:

- the logical texture handle is stable
- the sampled image behind that handle updates over time
- updates are effectively **one frame delayed**
- swaps happen at frame boundaries to avoid Vulkan resource-tracking hazards

Relevant implementation:

- [src/engine/graphics/vulkano_renderer.rs](src/engine/graphics/vulkano_renderer.rs#L1489-L1538)
- [src/engine/graphics/vulkano_renderer.rs](src/engine/graphics/vulkano_renderer.rs#L1550-L1582)
- [src/engine/graphics/vulkano_renderer.rs](src/engine/graphics/vulkano_renderer.rs#L1306-L1318)
- [src/engine/graphics/vulkano_renderer.rs](src/engine/graphics/vulkano_renderer.rs#L2606-L2622)

## 7. What is not implemented yet

The following remain draft-level concepts, not current engine APIs:

- `RenderImageHandle`
- `RenderImageRegistry`
- a dedicated `RenderToTextureSystem`
- contextual reference-sharing where one authored `Texture {}` expression has live identity across multiple authored use sites
- explicit capture cameras / mirrors / portals / monitors

## 8. Current design summary

So the practical answer for the current codebase is:

- we **did not** end up using a first-class `RenderImage` type
- we **did** ship the Layer A functionality using selector strings and stable `TextureHandle`s
- this document is the source of truth for the implementation that exists now