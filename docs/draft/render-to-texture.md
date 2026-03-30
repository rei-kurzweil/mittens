# Render To Texture / View Capture Draft

This draft describes a general render-to-texture direction for cat-engine.

It starts from the immediate need to debug bloom and emissive extraction, but it is meant to grow into a broader system that can power:

- bloom / render-graph debug panels
- mirrors
- portals
- in-world monitors / CCTV
- minimaps and picture-in-picture views

The key design decision is that the scene-facing concept should be **`RenderGraph`**, not `PostProcessing`.

`PostProcessing` is too narrow once the graph can contain:

- optional passes like `Bloom`
- optional sub-passes like `EmissivePass`
- debug geometry that displays pass outputs
- future copy/downsample/extract stages

So this draft assumes the top-level graph node is called **`RenderGraph`**, and render-to-texture Layer A is primarily about exposing renderer-owned pass outputs to ordinary textured geometry.

---

## 1. Immediate problem

The current bloom implementation is hard to debug because intermediate images are invisible unless we:

- inspect them in RenderDoc
- temporarily rewrite shaders
- guess from the final composite

That makes basic questions unnecessarily hard to answer:

- Is the emissive extraction pass correct?
- Is the source full-res or half-res?
- Is the downsample working?
- Is the blur operating over the right area?
- Is the composite sampling the expected UV range?

What we want is to be able to show renderer-owned images directly in the scene, for example on quads:

- emissive pass color
- bloom source
- bloom blur A
- bloom blur B
- main color intermediate

That would have made the recent top-left-quarter bloom bug obvious immediately.

---

## 2. Goals

### Short-term

Add a way to:

- name renderer-produced images
- expose them as sampleable textures to normal renderables
- author debug displays directly inside `RenderGraph` / pass subtrees
- bind a renderer-owned image without inventing a fake asset URI

### Mid-term

Add a way to render a scene/view intentionally into a texture target.

That enables:

- mirrors
- portals
- monitors
- minimaps
- picture-in-picture cameras

### Long-term

Support render targets driven by:

- implicit cameras spawned by systems
- alternate subtrees or worlds
- throttled update policies
- recursion policies

---

## 3. Non-goals for v1

The first version does **not** need to solve:

- recursive portal rendering
- arbitrary graph scheduling for many dependent captures
- full multi-world scene isolation
- zero-latency feedback inside the same pass
- every future camera-authoring case
- temporal reprojection / history buffers

Those can come later.

---

## 4. Proposed layering

There are two related features here.

### Layer A — exposed internal attachments

Expose already-existing renderer images as sampleable runtime textures.

Examples:

- `render_graph.main_color`
- `render_graph.emissive_pass.color`
- `render_graph.bloom.source`
- `render_graph.bloom.blur_a`
- `render_graph.bloom.blur_b`

This is the smallest feature that solves bloom debugging.

### Layer B — explicit view capture

Render from a camera/view into a named texture target.

Examples:

- mirror surface texture
- portal destination view
- in-world monitor
- rear-view camera

This is the broader render-to-texture system.

### Why split them?

Because Layer A can ship much sooner.

The renderer already has the images — it just does not publish them in a reusable way. Exposing those attachments first gives immediate debugging value and forces us to define the texture plumbing that Layer B will also need.

---

## 5. Scene-facing model

### `RenderGraph` replaces `PostProcessing`

The graph root should be:

```text
RenderGraph {
    Bloom { ... }
    Bokeh { ... }
}
```

not:

```text
PostProcessing { ... }
```

because the thing we are authoring is no longer just a boolean "do post-processing" switch. It is a declarative render graph that can contain passes, sub-passes, and visualisation nodes.

### Pass-owned texture references

For Layer A, the better authoring model is not "debug geometry lives under the pass".

Instead, the pass should **own a texture reference**, and other geometry in the scene can reuse that same reference later.

That means the pattern is:

1. author a `Texture {}` component expression once
2. bind that texture reference under a render-graph pass
3. let `RenderGraph` / `RenderToTextureSystem` attach image data to that texture reference
4. reuse the same texture reference on ordinary renderables elsewhere in the scene

Example sketch:

```text
let emissive_texture_reference = Texture {}

RenderGraph {
    EmissivePass {
        emissive_texture_reference
    }
}

T {
    R {
        QUAD_2D
        emissive_texture_reference
    }
}
```

The important idea is that `EmissivePass` does not itself need to contain the display quad. It only needs to claim ownership of the texture reference and define where its image data comes from.

Later, when the same texture handle/reference is used elsewhere, it already has image data attached.

That should mean a **stable texture identity** at runtime: the authored `Texture {}` reference resolves to one logical texture handle, and the render graph updates the image contents behind that handle. Ordinary consumers of that texture should not need a per-frame rebind just because a new frame was rendered. Rebinding should only be needed if the runtime must actually replace the underlying GPU image object, such as on resize, format change, or other reallocation.

### Why `Texture {}` should work by reference

The user-facing texture syntax should stay close to ordinary texturing.

We should be able to author a texture component expression once:

```text
let emissive_texture_reference = Texture {}
```

and then use that same reference in two places:

```text
RenderGraph {
    EmissivePass {
        emissive_texture_reference
    }
}

T {
    R {
        QUAD_2D
        emissive_texture_reference
    }
}
```

This is much nicer than:

- requiring a fake URI scheme
- inventing a one-off debug-only component
- requiring the debug geometry to be nested inside the pass node

Ordinary textured geometry sampling a runtime-produced image is exactly what we will also want for mirrors and portals later.

---

## 6. Core runtime concepts

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

This is where the renderer / post-processing renderer can publish:

- `render_graph.main_color`
- `render_graph.emissive_pass.color`
- `render_graph.bloom.source`
- `render_graph.bloom.blur_a`
- `render_graph.bloom.blur_b`

and where future capture systems can publish:

- `capture.mirror.17`
- `capture.portal.42`

### `RenderToTextureSystem`

Layer A still benefits from a dedicated runtime seam between producers and consumers.

That seam can be called `RenderToTextureSystem` even before we support explicit cameras.

Its responsibilities would be:

- receive published images from `Renderer`, `PostProcessingRenderer`, and future graph passes
- assign or look up `RenderImageHandle`s
- expose them to texture binding / material setup
- resolve contextual selectors like "containing pass output"

So even for Layer A, `RenderToTextureSystem` is useful.

### Unified texture source

We likely want one abstraction for both asset textures and runtime textures:

```rust
pub enum TextureSource {
    Asset(TextureHandle),
    RenderImage(RenderImageHandle),
}
```

Then materials and renderables can sample either kind through the same path.

That avoids building a separate debug-only texturing path.

### Authored texture binding

The authored `TextureComponent` likely needs to resolve to either an asset URI or a runtime image source.

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

---

## 7. Layer A MVP for bloom debugging

The smallest useful MVP is:

1. rename the scene-facing concept to `RenderGraph`
2. publish renderer-owned pass outputs into `RenderToTextureSystem`
3. allow `Texture {}` component expressions to exist without a URI
4. when such a texture reference is attached under a render-graph pass, bind that pass output into it
5. reuse the same texture reference on ordinary quads in the `bloom` example

Example desired scene:

- main scene in front
- off to the side, three floating quads:
  - emissive pass output
  - bloom blur A
  - bloom blur B

Example sketch:

```text
let emissive_tex = Texture {}
let bloom_blur_a_tex = Texture {}
let bloom_blur_b_tex = Texture {}

RenderGraph {
    EmissivePass { emissive_tex }
    BloomBlurA   { bloom_blur_a_tex }
    BloomBlurB   { bloom_blur_b_tex }
}

T.position(-2.4, 2.0, -2.0) {
    T.scale(1.2, 1.2, 1.0) {
        R {
            QUAD_2D
            emissive_tex
        }
    }
}

T.position(0.0, 2.0, -2.0) {
    T.scale(1.2, 1.2, 1.0) {
        R {
            QUAD_2D
            bloom_blur_a_tex
        }
    }
}

T.position(2.4, 2.0, -2.0) {
    T.scale(1.2, 1.2, 1.0) {
        R {
            QUAD_2D
            bloom_blur_b_tex
        }
    }
}
```

This directly solves the current debugging need.

---

## 8. Why this is enough for now

Layer A avoids the hardest future problems:

- no extra cameras yet
- no alternate worlds yet
- no recursion yet
- no portal visibility rules yet

But it still creates the reusable seam we will need later:

- renderer-owned images become sampleable texture sources
- ordinary renderables can consume them
- the registry / handle system is already in place for explicit captures

---

## 9. Full render-to-texture direction

Once the sampling side exists, we can add active capture.

### Explicit capture targets

A likely future shape is:

```text
RenderTarget("mirror.left") {
    resolution(1024, 1024)
    format(rgba16f)
    update_mode(every_frame)

    CaptureView {
        camera_source(ImplicitMirrorCamera)
        scene_source(CurrentWorld)
    }
}
```

A material elsewhere can then sample that target via the same texture-source system.

### Implicit cameras

Mirrors and portals usually want cameras that are not authored directly as ordinary scene cameras.

Examples:

- mirror camera reflected across a plane
- portal exit camera transformed by an entrance→exit mapping
- monitor camera attached to an entity

So a future `RenderToTextureSystem` will probably need to cooperate with a system that synthesizes temporary views each frame.

### Update modes

Not every capture should update every frame.

Useful policies:

- every frame
- every N frames
- on demand
- manual trigger

That matters for mirrors, monitors, editor previews, and any latency-tolerant debug usage.

---

## 10. Scheduling implications

Once runtime-produced images feed materials, scheduling matters:

- capture before sampling
- avoid sampling an image while still writing it
- decide whether same-frame or previous-frame sampling is allowed

For the Layer A bloom-debug case, we can keep it simple initially:

- publish pass outputs after those passes complete
- allow debug quads to sample them later in the frame if ordering permits, or from the previous completed frame if that is simpler

For mirrors and portals, dependency handling becomes much more important.

---

## 11. Open questions

### Should `Texture {}` with no URI always mean runtime texture?

Probably it should begin unresolved. It only becomes a runtime texture once a render-graph pass binds image data into that same texture reference. A standalone no-URI texture with no render-graph binding should probably be an error.

Once resolved, the referenced texture should remain logically stable. The renderer should update the produced image content behind that reference across frames, rather than requiring downstream quads/materials to swap to a different texture handle every frame.

### Should pass-local debug geometry live *inside* pass nodes?

Not necessarily. The newer reference-based pattern is better: the pass owns the texture source, but the geometry that displays it can live anywhere.

### Do we need a separate debug-display component?

Maybe later for special visualisation modes like depth linearization or luminance-only views, but ordinary `Texture {}` binding is probably enough for v1.

### Should pass outputs be globally named as well as contextually addressable?

Yes. We want both:

- contextual/default-by-reference binding (`FromPassReference`)
- explicit named binding (`render_graph.bloom.blur_a`)

The first is ergonomic. The second is useful for tools and cross-pass references.

---

## 12. Recommended next step

The next implementation step should be Layer A, not full mirrors/portals.

Specifically:

1. rename the scene-facing concept to `RenderGraph`
2. add a renderer-owned `RenderImageRegistry` / `RenderToTextureSystem`
3. publish attachments from `Renderer` and `PostProcessingRenderer`
4. allow MMS `Texture {}` component expressions to exist as shareable references
5. when those references are attached under `RenderGraph` passes, bind the pass output image into them
6. reuse those same texture references on quads in the `bloom` example

That gives immediate leverage on the current bloom debugging issue and creates the runtime image seam needed for mirrors and portals later.
