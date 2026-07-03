# Render-image contextual binding refactor

Date: 2026-04-03

This note captures the next-step refactor direction for render-graph-produced textures.

The goal is to move away from authored selector-string plumbing like:

```text
Texture.render_image("render_graph.emissive_pass.output")
```

and toward pass-owned texture references that can be reused elsewhere in the scene by ordinary MMS
reference semantics.

Desired authored shape:

```text
let emissive_tex = Texture {}
let bloom_tex = Texture {}

RenderGraph {
    EmissivePass {
        emissive_tex
    }

    Bloom {
        bloom_tex
    }
}

T.position(-2.0, 1.5, -2.0) {
    R {
        QUAD_2D
        emissive_tex
    }
}

T.position(2.0, 1.5, -2.0) {
    R {
        QUAD_2D
        bloom_tex
    }
}
```

The important property is: the debug renderables do **not** use a special debug-only rendering
path. They just reuse the same authored texture references that the render graph populated earlier.

## Current situation

Today, the runtime bridge is implemented via selector strings and stable `TextureHandle`s:

- `TextureComponent.render_image: Option<String>`
- keys such as `render_graph.emissive_pass.output`
- `TextureSystem` / `VisualWorld` stable runtime texture handles
- renderer-side publication/copy into sampled images

That works, but it has two awkward consequences:

1. authored scenes must know string keys like `render_graph.bloom.blur`
2. there is a separate ad hoc debug-overlay path for on-screen inspection panels, instead of just
   reusing normal textured geometry

So the current implementation is useful, but it is not the end-state authoring model we want.

## Desired model

The preferred user-facing model is:

- a `Texture {}` component expression has stable identity
- a render-graph pass can **claim** that texture as its published output
- later uses of the same texture reference sample the live image data produced by that pass

This means the authored texture reference is the thing that is shared, not a string key.

### What we want to avoid

We do **not** want ordinary scene authoring to require this style forever:

```text
let emissive_debug = Texture.render_image("render_graph.emissive_pass.output")
```

That should remain available as an explicit/advanced escape hatch if needed, but not be the nicest
or primary authoring path.

## Principle: pass-owned texture reference, not pass-owned display geometry

The pass should own the **texture reference**, not the debug display quad.

That is:

- `EmissivePass` publishes into `emissive_tex`
- `Bloom` publishes into `bloom_tex`
- any later `Renderable` can sample those textures

This is better than nesting debug geometry under the pass because:

- the pass stays about image production, not scene layout
- the scene decides where and how to display the result
- the same texture can be reused multiple times in different places

## Proposed end-state authoring rules

### 1. `Texture {}` can be unresolved by default

An authored `Texture {}` with no URI is a valid reference object.

It starts as “unbound”, and later some system can attach live image data to it.

### 2. When placed under a render-graph pass, that texture becomes the pass output binding

Examples:

```text
RenderGraph {
    EmissivePass {
        emissive_tex
    }

    Bloom {
        bloom_tex
    }
}
```

Interpretation:

- `emissive_tex` is the output texture reference for `EmissivePass`
- `bloom_tex` is the published blurred/composited bloom texture reference for `Bloom`

### 3. Reusing that same texture reference elsewhere samples the same runtime-updated image

The same authored texture component can later appear under ordinary renderables:

```text
R {
    QUAD_2D
    emissive_tex
}
```

No special debug component is required.

### 4. Named selector binding can remain as an explicit fallback

We should likely keep an explicit named form for tools / advanced references / backwards-compatible
migration paths:

```text
Texture.render_image("render_graph.bloom.blur")
```

But it should be secondary, not the nicest authored path.

## Recommended runtime direction

This should be treated as a staged refactor, not a rewrite.

### Stage 1 — contextual binding on top of the existing selector bridge

Fastest practical route:

- keep the current stable `TextureHandle` publication model
- keep renderer-side copy/publication logic
- add a higher-level authored binding model where `RenderGraph` maps a referenced `TextureComponent`
  to the appropriate selector internally

In other words:

- **user-facing API changes first**
- **internal selector-string bridge stays temporarily**

This gives us the nicer authored syntax without requiring immediate introduction of a first-class
`RenderImageHandle` runtime layer.

Concretely, `SystemWorld::register_render_graph` would stop looking only for
`texture.render_image = Some(...)` and instead support:

- “this `TextureComponent` child means pass-owned output reference”
- internally assign the selector string for now

That keeps the runtime implementation stable while improving authored ergonomics.

### Stage 2 — remove dependence on debug overlay panels in examples/demos

Once pass-owned texture references exist:

- bloom/emissive demo surfaces should be ordinary renderables with reused texture refs
- examples should continue showing pass outputs through ordinary textured scene content, not through
    a dedicated renderer-side debug-overlay path

This is important because it proves the regular scene path is good enough.

### Stage 3 — unify texture source model

Current `TextureComponent` splits runtime images from ordinary texture sources awkwardly:

- `source: TextureSource` (`Uri` or `Handle`)
- `render_image: Option<String>`

The cleaner end-state is something like:

```rust
pub enum TextureSource {
    Uri(String),
    Handle(TextureHandle),
    RenderImage(RenderImageHandle),
}
```

or an equivalent `TextureBinding` layer.

That is where the `RenderImage` draft comes back into play.

### Stage 4 — introduce a true `RenderImageHandle` / registry if still worth it

Only after Stage 1 proves the authored semantics do we need to decide whether a dedicated runtime
`RenderImageHandle` layer is worth the additional machinery.

This is the right order because the user-facing ergonomic win does **not** depend on solving the
entire runtime abstraction first.

## Minimal concrete next steps

The next implementation-focused docs/code planning should target these specific changes:

1. `RenderGraph` registration learns that a nested `Texture {}` under `EmissivePass` means
   “publish pass output into this texture reference”.
2. Same for `Bloom`.
3. MMS/examples are updated to bind those same texture refs into ordinary quads elsewhere.
4. The string-selector authored form becomes optional rather than required.
5. The old debug HUD overlay path is demoted to tooling-only status.

## Why this ordering is important

If we start by introducing a full `RenderImageRegistry` first, we risk spending effort on runtime
plumbing before verifying the authored model we actually want.

The higher-value ordering is:

1. make authoring nice
2. prove shared texture references work ergonomically
3. then decide whether the current internal bridge is good enough or whether a first-class
   `RenderImageHandle` layer is justified

## Relation to existing docs

- Current implemented bridge: [docs/spec/render-to-texture.md](../../spec/render-to-texture.md)
- Future runtime abstraction draft: [docs/draft/render-image.md](../../draft/render-image.md)
- Older pass-owned texture reference direction: [docs/draft/render-to-texture.md](../../draft/render-to-texture.md)

This refactor note is specifically about the **migration path** from the implemented selector-string
bridge toward contextual/pass-owned texture binding.

## Recommendation summary

- Keep the current runtime publication mechanism temporarily.
- Change the authored model first.
- Make nested `Texture {}` under `EmissivePass` / `Bloom` the preferred binding form.
- Reuse those same texture references later in ordinary renderables.
- Treat `Texture.render_image("...")` as an explicit fallback/tooling API, not the primary scene
  authoring pattern.