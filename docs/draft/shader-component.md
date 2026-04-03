# Shader component draft

This note explores a future URI-backed shader component model for cat-engine, where a renderable
can point at shader source files similarly to how `TextureComponent` points at texture URIs.

The motivating authoring goal is something like:

```text
R {
    CUBE
    FragmentShader {
        "assets/shaders/custom-toon-mesh.frag"
    }
}
```

## Short version

- A shader-by-URI system is plausible and useful.
- It is **not** a small extension of the current renderer.
- The cleanest incremental path is:
  1. add a **`FragmentShaderComponent`** for renderables
  2. make it override only the fragment shader while inheriting the renderable's existing vertex
     shader / pipeline family from its current material
  3. later generalize into **`GraphicsShaderComponent`** for explicit vertex+fragment pairing
  4. keep **`ComputeShaderComponent`** separate rather than folding compute into the same authored
     node

So the recommendation is:

- **v1 authored component:** `FragmentShader`
- **future generalized graphics component:** `GraphicsShader`
- **future compute component:** `ComputeShader`

## Why this is different from textures

`TextureComponent` is relatively simple:

- URI → decode image data → upload texture → bind existing sampled image into existing pipeline

Shader URIs are more complicated because a shader does not stand alone. A graphics pipeline also
depends on:

- vertex input layout
- descriptor-set layouts
- push-constant layout
- render target formats
- depth/blend state
- cutout / transparent / opaque variant
- skinned vs unskinned vertex shader family
- full-screen vs mesh-drawing topology

So a URI-backed shader component cannot just mean “load file and run it”. It must participate in a
pipeline-family selection and cache key.

## Current renderer constraint

Today, `Renderable.material: MaterialHandle` is the shader/pipeline selector.

That current shape is documented in [docs/spec/mesh.md](docs/spec/mesh.md) and is reflected in the
renderer architecture:

- `Renderable.material` selects a built-in material/pipeline family
- shader modules are compiled by `vulkano_shaders::shader!` at build time
- `VisualWorld` batches by material handle
- pipeline dispatch is currently centered on those built-in material handles

So a shader component is not just “another descendant like `Texture`”; it pushes on the core
renderable/material model.

## Naming options

### `FragmentShaderComponent`

Pros:

- maps directly to the immediate use case
- avoids pretending we can safely infer arbitrary full graphics pipelines
- allows sensible defaults by inheriting the vertex shader and pipeline family from the renderable's
  current material

Cons:

- only solves fragment customization, not vertex customization

### `ShaderComponent`

Pros:

- short and pleasant

Cons:

- too ambiguous once compute enters the picture
- unclear whether it means fragment-only, graphics pipeline, compute pipeline, or generic shader
  asset reference

### `GraphicsShaderComponent`

Pros:

- clearly scoped to graphics rather than compute
- future-proofs the model for explicit vertex+fragment authoring

Cons:

- heavier than we probably want for the first implementation
- implies a larger compatibility/configuration surface on day one

## Recommendation

Use a staged naming model:

- **v1:** `FragmentShaderComponent`
- **future:** `GraphicsShaderComponent`
- **separate future:** `ComputeShaderComponent`

That gives us a clear incremental path without overcommitting the first implementation.

## Proposed authored syntax

### v1 fragment override

```text
R {
    CUBE
    FragmentShader {
        "assets/shaders/custom-toon-mesh.frag"
    }
}
```

Positional string means the fragment shader URI, just like the user suggested.

Equivalent explicit form could be:

```text
FragmentShader.uri("assets/shaders/custom-toon-mesh.frag")
```

### future explicit graphics shader

```text
R {
    CUBE
    GraphicsShader {
        vertex("assets/shaders/custom-toon-mesh.vert")
        fragment("assets/shaders/custom-toon-mesh.frag")
    }
}
```

### future compute shader

```text
ComputeShader {
    "assets/shaders/custom-effect.comp"
}
```

That should stay separate from renderable-attached graphics shader overrides.

## Defaulting behavior for `FragmentShader`

This is the key design choice.

When a `FragmentShaderComponent` is attached under a renderable, the engine should:

1. inspect the renderable's current material/pipeline family
2. keep the existing vertex shader choice from that family
3. keep the existing vertex layout and descriptor interfaces expected by that family
4. replace only the fragment shader stage with the URI-backed shader module

For example:

- `TOON_MESH` + `FragmentShader("custom-toon-mesh.frag")`
  - still uses the standard unskinned toon vertex shader
  - still expects the normal toon descriptor layout
  - still draws through a mesh graphics pipeline, not a fullscreen pipeline

- `SKINNED_TOON_MESH` + `FragmentShader("custom-toon-mesh.frag")`
  - still uses the skinned vertex shader family
  - still expects the skinned data / bone interface to remain compatible

This is why `FragmentShaderComponent` is a good v1: it inherits enough context from the current
material to make authoring feasible.

## Required compatibility rule

For v1, the URI-backed fragment shader is **contractually required** to match the descriptor and
varying interface of the inherited material family.

That means, for a toon-material fragment override, the shader must be compatible with the current:

- vertex outputs / fragment inputs
- descriptor sets / bindings
- push constants

If it is not compatible, pipeline compilation fails and the renderer should report a clear error.

This is acceptable for an advanced escape hatch.

## URI model

Shader components should mirror texture-style URI semantics as much as possible.

Conceptually:

```rust
pub enum ShaderSource {
    Uri(String),
}
```

Possible accepted source forms:

- relative asset path like `assets/shaders/custom-toon-mesh.frag`
- `file://...` URI if we decide to allow it consistently with texture-style path handling
- later: virtual asset key / package resource key

For v1, simplest is:

- treat the string exactly like current texture-path style: project-local file path / URI-ish string

## Runtime compilation model

Unlike the current `shader!` macro pipeline, this feature requires runtime shader compilation or
runtime SPIR-V loading.

There are two main approaches:

### Option A — GLSL source URI, compile at runtime

Pros:

- matches the authoring goal directly
- easiest mental model for users

Cons:

- requires shipping/using runtime GLSL → SPIR-V compilation
- compile errors happen at runtime rather than at `cargo build`

### Option B — SPIR-V URI, maybe with GLSL tooling outside runtime

Pros:

- simpler runtime if pipeline only loads `.spv`
- more deterministic deployment

Cons:

- worse authoring ergonomics
- less in line with the current asset style in the repo

## Recommendation on compilation input

For the authored component, still accept shader **source URIs**.

The runtime can internally decide whether to:

- compile GLSL on demand, or
- use a build/preprocess step later

The authored API should not force users to think in SPIR-V file management unless we discover that
runtime compilation is untenable.

## Pipeline caching implications

Once shaders are URI-driven, pipeline caching must no longer be keyed only by `MaterialHandle`.

For fragment-shader overrides, the cache key likely needs fields like:

- base material family
- fragment shader URI (or compiled module hash)
- output color format
- MSAA sample count
- transparency/cutout/opaque variant
- skinned vs unskinned mode
- vertex layout family

This means the renderer will need a new cache layer for “material family + shader override” rather
than only a fixed built-in pipeline bundle.

## Batching implications

Today `VisualWorld` batches heavily by built-in material handle.

With URI-backed fragment shaders, batching must treat shader override identity as part of the batch
key. Two cubes with different fragment URIs cannot share the same draw batch/pipeline bind state.

So the system likely needs a concept roughly like:

```rust
enum MaterialProgramKey {
    BuiltIn(MaterialHandle),
    FragmentOverride {
        base_material: MaterialHandle,
        fragment_uri: String,
    },
}
```

This is a non-trivial renderer/data-model refactor, but it is still narrower than a full arbitrary
graphics-shader system.

## Proposed ECS component shapes

### v1 `FragmentShaderComponent`

```rust
pub struct FragmentShaderComponent {
    pub uri: String,
}
```

Possible helpers:

```rust
impl FragmentShaderComponent {
    pub fn uri(uri: impl Into<String>) -> Self { ... }
}
```

MMS forms:

```text
FragmentShader { "assets/shaders/custom-toon-mesh.frag" }
FragmentShader.uri("assets/shaders/custom-toon-mesh.frag")
```

## Attachment-time runtime flow

The cleanest mental model is to follow the same broad shape as `TextureSystem`:

1. component is attached in ECS
2. component `init()` emits a registration intent
3. a dedicated runtime system records the component and finds the ancestor renderable
4. the system keeps the attachment **pending** until the renderable has a live visual/runtime
   handle
5. once attachable, the system resolves / compiles / caches the shader program override
6. the system updates the renderable's runtime program key
7. the renderer naturally re-batches / rebinds based on the new key on the next frame

Conceptually:

```text
FragmentShader attached
  -> RegisterFragmentShader intent
  -> FragmentShaderSystem stores ShaderRecord
  -> finds ancestor RenderableComponent
  -> pending_attach[renderable] = shader_component
  -> when renderable instance/program slot exists:
     resolve URI
     compile shader or load compiled module
     create/find ProgramOverrideHandle
     apply override to renderable runtime state
```

## Suggested runtime pieces

### 1. `FragmentShaderSystem`

Parallel to `TextureSystem`, a dedicated system owns registration and attachment bookkeeping.

Conceptual responsibilities:

- store `ComponentId -> FragmentShaderRecord`
- remember `RenderableComponent -> FragmentShaderComponent` pending attachments
- retry pending attachments until the renderable exists in runtime form
- request shader compilation / module loading
- apply the resulting program override handle back onto the renderable/runtime instance

Conceptual record shape:

```rust
struct FragmentShaderRecord {
  uri: String,
  status: ShaderLoadStatus,
  compiled: Option<FragmentShaderHandle>,
  last_error: Option<String>,
}

enum ShaderLoadStatus {
  Unresolved,
  Loading,
  Ready,
  Failed,
}
```

### 2. a shader compiler / loader service

This should be a lower-level service used by the system, not logic embedded directly in ECS code.

Conceptual interface:

```rust
trait ShaderProgramLoader {
  fn request_fragment_shader(
    &mut self,
    base_material: MaterialHandle,
    fragment_uri: &str,
  ) -> ShaderRequestId;

  fn poll_fragment_shader(
    &mut self,
    request: ShaderRequestId,
  ) -> Option<Result<FragmentShaderHandle, ShaderLoadError>>;
}
```

That separates:

- ECS attachment logic
- file IO / compilation
- renderer pipeline-module caching

### 3. a renderer-owned program override cache

The compiled shader module alone is not enough. The renderer ultimately needs a cached program key
or pipeline-family override key.

So after compilation, the runtime should create or look up something like:

```rust
enum ProgramOverrideKey {
  FragmentOverride {
    base_material: MaterialHandle,
    fragment_uri: String,
  },
}
```

And then resolve that to a compact runtime handle:

```rust
struct ProgramOverrideHandle(u32);
```

The renderable/runtime instance would then carry either:

- built-in material only, or
- built-in material + optional program override handle

## Synchronous vs asynchronous loading

There are two viable strategies.

### Option A — synchronous compile on attach

Flow:

- component attaches
- system immediately reads file and compiles shader
- if successful, override becomes live that frame or next frame

Pros:

- simplest implementation
- easiest to reason about initially

Cons:

- can hitch the main thread badly
- shader compile errors occur inline during gameplay/editor interaction

### Option B — asynchronous/background compile

Flow:

- component attaches
- system queues a compile request
- renderable keeps using base material while compilation runs
- once ready, override swaps in atomically on a later frame

Pros:

- much better editor/runtime responsiveness
- scalable once hot reload and more shader variants exist

Cons:

- requires request/poll state management
- slightly more moving parts

## Recommendation on runtime loading strategy

Prefer **asynchronous/background compile** for the real design, even if a first experiment uses a
synchronous path.

Best user experience is:

- attach component
- object keeps rendering with base material
- shader compiles in background
- if compile succeeds, object flips to override
- if compile fails, object stays on base material and error is surfaced

That is much more robust than stalling the frame or making the object disappear immediately.

## Initial fallback behavior

When a fragment shader component is attached, the runtime should not immediately mutate the base
renderable into an invalid/incomplete state.

Instead:

- **base material remains authoritative** until override compilation succeeds
- successful compile installs the override
- failed compile records an error and leaves base material active

That gives an iterative editing loop with graceful failure.

## URI resolution and invalidation

At minimum, attachment-time runtime loading needs these cache rules:

- same `(base_material, fragment_uri)` should reuse the same compiled module / override handle
- changing the URI should invalidate the old override binding for that component
- detaching the component should restore pure base-material rendering
- if hot reload exists later, file content changes should invalidate the matching cache entry

So the runtime cache should distinguish:

- **component attachment identity**
- **program cache identity**

Those are not the same thing.

## Where compilation result should be applied

Do not patch pipelines directly from ECS systems.

Instead, the system should update renderer-facing runtime state, and the renderer should resolve the
correct pipeline lazily from cache during draw preparation.

That means the handoff should look more like:

- ECS/system produces a `ProgramOverrideHandle`
- `VisualWorld` / runtime renderable stores that handle
- renderer maps `(material family, override handle, pass variant, format, samples, ...)` to a
  pipeline object

This keeps pipeline lifetime and Vulkan object ownership inside renderer code.

## Practical staged version

A realistic first implementation path could be:

### Stage A — attach-time registration only

- add `FragmentShaderComponent`
- add registration intent and `FragmentShaderSystem`
- detect ancestor renderable and hold pending attachments

### Stage B — synchronous prototype

- compile fragment shader from URI when pending attachment flushes
- keep base material on failure
- store override key on the runtime renderable

### Stage C — renderer cache integration

- pipeline cache keyed by base material + fragment override
- batch key includes override identity

### Stage D — async compile / hot reload

- move compile to background worker
- swap in override once ready
- invalidate and rebuild on source changes

### future `GraphicsShaderComponent`

```rust
pub struct GraphicsShaderComponent {
    pub vertex_uri: Option<String>,
    pub fragment_uri: String,
}
```

Semantics:

- if `vertex_uri` is omitted, inherit the default vertex shader from the base material family
- if present, the user is now responsible for matching the correct vertex layout / interface

## Why not start with `GraphicsShaderComponent`

Because the moment user-authored vertex shaders enter the picture, the engine must answer all of
these questions immediately:

- which vertex layout is bound?
- how does skinning work?
- what varyings are required downstream?
- do we still support cutout/transparent variants automatically?
- how are overlay/unlit/toon families mapped?

That is a much larger v1 surface than fragment-only override.

## Compute shader note

`ComputeShaderComponent` should be separate.

Reason:

- compute is not attached to a mesh renderable in the same way
- it wants different scheduling, resource binding, and likely render-graph integration
- combining compute and graphics shader authoring into one `ShaderComponent` muddies the model

So if we later want compute, it should be a separate component family or render-graph node.

## Error handling expectations

When URI-backed shader compilation fails, the engine should:

- keep the scene running
- log a clear compile/link/interface error
- fall back to the base built-in material shader if possible
- surface the failing URI in diagnostics

That fallback behavior is important because shader authoring errors will be common during iteration.

## Suggested staged implementation plan

### Stage 1 — spec only

- define `FragmentShaderComponent`
- define URI semantics
- define inheritance-from-base-material rules

### Stage 2 — runtime fragment override for toon/unlit families

- support fragment-only overrides on existing mesh material families
- compile and cache fragment shader modules at runtime
- extend batch keys / pipeline cache keys to include fragment override identity

### Stage 3 — optional `GraphicsShaderComponent`

- allow explicit vertex URI override
- require explicit compatibility with base mesh layout

### Stage 4 — compute path

- separate `ComputeShaderComponent` or render-graph compute node

## Recommendation summary

- Start with **`FragmentShaderComponent`**.
- Let it take a **single positional URI string** in MMS.
- Make it **inherit the vertex shader and pipeline family** from the renderable's current material.
- Treat it as a **renderable-local advanced override**, not a replacement for `MaterialHandle` on
  day one.
- Add **`GraphicsShaderComponent`** later if/when explicit vertex override becomes worth the extra
  complexity.
- Keep **`ComputeShaderComponent`** separate.

## Open questions

- Should the fallback shader always be the base material family shader, or should failure make the
  renderable invisible?
- Should fragment shader overrides be allowed on all material families, or only a subset at first
  (`TOON_MESH`, `UNLIT_MESH`, skinned toon)?
- Should the engine accept only GLSL-like source paths initially, or also `.spv` URIs?
- Do we want hot-reload behavior for shader URI files, or only compile-on-first-use for v1?
- Does a future `GraphicsShaderComponent` remain a descendant of `RenderableComponent`, or should it
  become a more explicit material/program asset reference?