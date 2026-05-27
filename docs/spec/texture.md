# TextureComponent

This document describes the current implemented behavior of `TextureComponent`.

It covers the scene-facing authoring forms, how they map to runtime behavior, and how the
backend resolves each source type.

For render-graph-published runtime textures specifically, also see
[docs/spec/render-to-texture.md](docs/spec/render-to-texture.md).

## 1. What `TextureComponent` is

`TextureComponent` is the component that gives a renderable a texture image.

Current implementation:

- authored component type: `TextureComponent`
- intended placement: as a descendant of a `RenderableComponent`
- registration path: `TextureComponent::init()` emits `RegisterTexture`
- runtime owner: `TextureSystem`

The component stores three related pieces of state:

- `source: TextureSource`
- `format: CatEngineTextureFormat`
- `render_image: Option<String>`

The important distinction is:

- `source` describes an asset or runtime-provided GPU handle
- `render_image` describes a renderer-published runtime image selector

Today those are separate fields rather than one unified source enum.

Relevant implementation:

- [src/engine/ecs/component/texture.rs](src/engine/ecs/component/texture.rs)

## 2. Authoring modes

There are three practical ways textures show up in the current engine.

### A. URI-backed authored textures

This is the ordinary asset path mode.

Examples:

```text
Texture.with_uri("assets/textures/cat-face-neutral.dds")
Texture.from_dds("assets/textures/cat-face-neutral.dds")
Texture.from_png("assets/images/foo.png")
```

Equivalent engine-side constructors:

- `TextureComponent::new(uri)`
- `TextureComponent::with_uri(uri)`
- `TextureComponent::from_png(uri)`
- `TextureComponent::from_dds(uri)`

Implications:

- the texture is expected to come from disk
- `TextureSystem` is responsible for reading, decoding, uploading, and caching it
- if multiple components use the same URI, they share the uploaded `TextureHandle` through the
  URI cache

### B. Render-image-backed authored textures

This is the runtime image / render-graph publication path.

Examples:

```text
Texture.render_image("render_graph.emissive_pass.output")
Texture.render_image("render_graph.bloom.blur")
```

Implications:

- this is **not** a disk-backed image load
- `TextureSystem` allocates or reuses a stable runtime `TextureHandle`
- the renderer updates the GPU image behind that handle as frames are rendered
- this is the current way MMS and scene content sample post-process outputs

### C. Handle-backed runtime textures

This is mostly an internal/runtime path:

- `TextureComponent::from_handle(handle)`
- `TextureSource::Handle(TextureHandle)`

Implications:

- the image is already uploaded
- `TextureSystem` does not need to decode or load it from disk
- this is runtime-only state and is not serialized the same way URI-backed textures are

This is the lowest-level mode and is typically used by engine subsystems rather than authored
scene files.

## 3. URI-backed textures: PNG / DDS / generic URI

`TextureComponent` currently distinguishes formats with `CatEngineTextureFormat`:

- `Rgba8`
- `DdsBc7`

Format inference is simple:

- `.dds` → `DdsBc7`
- everything else → `Rgba8`

That means:

- `Texture.with_uri("foo.dds")` and `Texture.from_dds("foo.dds")` both resolve to BC7 mode
- `Texture.with_uri("foo.png")` and `Texture.from_png("foo.png")` both resolve to RGBA8 mode
- `from_png` / `from_dds` are mostly explicit authoring helpers, not a separate backend system

### PNG / generic image path

For `Rgba8` textures, `TextureSystem`:

1. resolves the filesystem path
2. reads the bytes from disk
3. decodes them with the `image` crate
4. converts to RGBA8
5. uploads the pixels through `TextureUploader::upload_texture_rgba8(...)`

This is the path for PNG and other image formats supported by `image`.

### DDS / BC7 path

For `DdsBc7` textures, `TextureSystem`:

1. resolves the filesystem path
2. reads the bytes from disk
3. parses the DDS container with `ddsfile`
4. verifies the top level is BC7
5. uploads the BC7 block data through `TextureUploader::upload_texture_bc7(...)`

Important current limitation:

- only BC7 DDS is supported here
- the top mip is used

Relevant implementation:

- [src/engine/ecs/system/texture_system.rs](src/engine/ecs/system/texture_system.rs)

## 4. Render-image mode

`render_image` is the bridge between renderer-owned images and scene-facing textured content.

Example:

```text
let bloom_tex = Texture.render_image("render_graph.bloom.blur")
```

Backend flow:

1. `TextureComponent` stores `render_image = Some(selector)`
2. `TextureSystem` asks `VisualWorld` for a stable runtime `TextureHandle` for that selector
3. if none exists yet, it allocates a small placeholder texture and stores that handle
4. the render graph publishes a pass output to the same selector
5. `VulkanoRenderer` copies the pass output into the GPU image behind that stable handle

Implications:

- the `TextureHandle` identity is stable
- the image contents update over time
- consumers sample it like an ordinary texture once attached to a renderable
- updates are effectively frame-boundary / one-frame-late style runtime updates, not immediate

This is the current implemented path used for bloom/emissive preview surfaces and other
render-graph-published images.

Relevant implementation:

- [docs/spec/render-to-texture.md](docs/spec/render-to-texture.md)
- [src/engine/ecs/system/texture_system.rs](src/engine/ecs/system/texture_system.rs)
- [src/engine/graphics/visual_world.rs](src/engine/graphics/visual_world.rs)
- [src/engine/graphics/vulkano_renderer.rs](src/engine/graphics/vulkano_renderer.rs)

## 5. GLTF-imported textures

GLTF textures use a slightly different path from explicit PNG/DDS authoring.

When a GLTF primitive has a base-color texture:

1. `GLTFSystem` imports and decodes the image during GLTF loading
2. it assigns a virtual texture key, typically shaped like `gltf_name:image_name_or_index`
3. later, `GLTFSystem::flush_imports(...)` uploads that texture and registers the uploaded handle
   in `TextureSystem` via `register_cached_texture(...)`
4. spawned renderables get a `TextureComponent::new(image_name_or_index)` child using that virtual key
5. `TextureSystem` resolves the virtual key from its cache instead of reading a file from disk

Implications:

- GLTF textures are still represented through `TextureComponent`, but they are **not** loaded from
  disk by `TextureSystem`
- the URI-like string is a virtual cache key, not necessarily a real filesystem path
- this is why `TextureSource::Uri` covers both real file paths and virtual keys

The current v1 heuristic for virtual GLTF keys is basically “contains `:` and is not `file://...`”.

Relevant implementation:

- [src/engine/ecs/system/gltf_system.rs](src/engine/ecs/system/gltf_system.rs)
- [src/engine/ecs/system/texture_system.rs](src/engine/ecs/system/texture_system.rs#L341-L347)

## 6. Caching and attachment behavior

`TextureSystem` is attachment-driven.

When a `TextureComponent` is registered:

- it records the texture source info
- it walks upward to find an ancestor `RenderableComponent`
- it defers actual attachment until that renderable has a `VisualWorld` instance handle

At flush time:

- if the texture is already a GPU handle, it attaches immediately
- if it is a render-image selector, it resolves/allocates the stable runtime handle
- if it is a cached URI key (including GLTF virtual keys), it reuses the cached handle
- if it is a real URI, it loads and uploads it, then caches the result by URI

This means authored texture components can exist before the renderable is fully registered,
and texture uploads happen only when the renderable is actually attachable.

## 7. Serialization behavior

Current encode/decode behavior is intentionally simple:

- URI-backed textures serialize as `uri`
- render-image-backed textures serialize as `render_image`
- handle-backed textures are runtime-only and are not serialized as stable authored data

This is one reason the current API feels split: `render_image` is persisted alongside `uri`,
but `TextureSource::Handle` is not.

## 8. Current limitations and future direction

Current limitations:

- `TextureComponent` splits ordinary sources and render-image selectors awkwardly
- `render_image` is selector-string based rather than a first-class render-image handle/reference
- authored reuse of one `Texture` binding in MMS is still a reused component expression, not a
  single live shared texture object
- the final authored form where plain `Texture {}` can be contextually written/read by different
  systems is still future work

Future design work is tracked in:

- [docs/draft/render-image.md](docs/draft/render-image.md)
- [docs/refactor/render-image-contextual-binding.md](docs/refactor/render-image-contextual-binding.md)

## 9. Practical guidance

Use:

- `Texture.from_dds(...)` for authored BC7 DDS assets
- `Texture.from_png(...)` or `Texture.with_uri(...)` for ordinary file-backed images
- `Texture.render_image("...")` when you want to sample a renderer-published runtime image

Think of the current modes like this:

- **URI mode** = load this image asset
- **GLTF virtual URI mode** = reuse an imported texture already uploaded by GLTF
- **render_image mode** = sample a runtime image published by the renderer
- **handle mode** = internal/runtime pre-uploaded texture binding