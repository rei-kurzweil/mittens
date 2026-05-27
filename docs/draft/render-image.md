# RenderImage draft

This document captures the **future** `RenderImage*` abstraction that may eventually replace or formalize the current selector-string runtime-texture bridge.

For the implemented behavior in `src/`, see [docs/spec/render-to-texture.md](docs/spec/render-to-texture.md).

## 1. Purpose

The current engine already exposes some renderer-owned images to scene textures, but it does so via:

- `Texture.render_image("...")`
- stable `TextureHandle`s stored in `VisualWorld`
- renderer-side copy/publication by selector string

That works for Layer A, but it is not yet a fully explicit image model.

This draft describes a likely future direction where runtime-produced images become first-class values with dedicated handles, metadata, and publication/lookup systems.

## 2. Core runtime concepts

### `RenderImageHandle`

A stable renderer-managed handle for a sampleable image.

```rust
pub struct RenderImageHandle(u32);
```

This may be runtime-owned rather than authored directly.

It represents things like:

- imported texture assets
- render-graph intermediate attachments
- explicit capture outputs

The key idea is: **materials should not care where the image came from**.

### `RenderImageRegistry`

A renderer/runtime-side registry of live render images.

Conceptually:

```rust
pub struct RenderImageRegistry {
    named: HashMap<String, RenderImageHandle>,
    metadata: HashMap<RenderImageHandle, RenderImageInfo>,
}

pub struct RenderImageInfo {
    pub extent: [u32; 2],
    pub format: RenderImageFormat,
    pub sample_count: u32,
    pub usage: RenderImageUsage,
}
```

This is where the renderer / post-processing renderer could publish:

- `render_graph.main_color`
- `render_graph.emissive_pass.color`
- `render_graph.bloom.source`
- `render_graph.bloom.blur_a`
- `render_graph.bloom.blur_b`

and where future capture systems could publish:

- `capture.mirror.17`
- `capture.portal.42`

### `RenderToTextureSystem`

Layer A and future explicit capture both benefit from a dedicated seam between producers and consumers.

That seam could be called `RenderToTextureSystem`.

Its responsibilities would be:

- receive published images from `Renderer`, `PostProcessingRenderer`, and future graph passes
- assign or look up `RenderImageHandle`s
- expose them to texture binding / material setup
- resolve contextual selectors like "containing pass output"

## 3. Unified texture source

We likely want one abstraction for both asset textures and runtime textures:

```rust
pub enum TextureSource {
    Asset(TextureHandle),
    RenderImage(RenderImageHandle),
}
```

Then materials and renderables can sample either kind through the same path.

That avoids building a separate debug-only texturing path.

## 4. Authored texture binding

The authored `TextureComponent` may eventually resolve to either an asset URI or a runtime image source.

Conceptually:

```rust
pub enum TextureBinding {
    Uri(String),
    RenderImage(RenderImageSelector),
}

pub enum RenderImageSelector {
    Named(String),
    FromPassReference,
}
```

Rules:

- `Texture { uri("assets/textures/foo.dds") }` → asset texture
- `let tex = Texture {}` with no URI starts unresolved
- `tex` placed under `EmissivePass` / `BloomBlurA` / similar pass nodes → `FromPassReference`
- `Texture.render_image("render_graph.bloom.blur_a")` → named runtime image

The no-URI case is what enables the clean reference/reuse syntax.

The important behavior is that `RenderGraph` assigns the runtime image source to the referenced texture component, and later uses of the same texture handle see that bound image data.

More explicitly: the preferred implementation is to keep the referenced texture handle stable and refresh the image data produced by the pass each frame. That is a per-frame image update/copy, not a per-frame material or renderable rebind.

## 5. Why this is still draft-only

The current codebase ships Layer A without introducing a first-class `RenderImageHandle`.

Today the bridge is instead:

- `TextureComponent.render_image: Option<String>`
- selector strings such as `render_graph.emissive_pass.output`
- `VisualWorld::runtime_texture_handle(key)` storing a stable `TextureHandle`
- renderer-side publication that updates the image behind that stable handle across frames

So the `RenderImage*` types here remain a **future cleanup direction**, not the concrete API in `src/` today.