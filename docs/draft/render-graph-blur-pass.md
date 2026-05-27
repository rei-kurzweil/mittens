# RenderGraph blur pass draft

This note explores whether blur should exist as its own optional render-graph node instead of
being baked only into `Bloom` or hidden as an internal implementation detail.

Short version: **yes, a standalone blur node probably makes sense**, and for authored scene syntax
we will call it **`BlurPass`**.

## Motivation

Right now, blur exists in two different conceptual roles:

- as an **internal step of bloom**
- as a more general image-processing operation we may want to apply to other render-graph outputs

Examples of the second category:

- blur the emissive extraction directly for debugging or stylised glow
- blur a render-to-texture mirror or portal feed
- blur a UI/background layer
- blur some future mask/coverage/debug image before compositing

That suggests blur should be thinkable as a first-class graph node, not only as a hardcoded detail
inside bloom.

## Naming

### `RenderTargetBlur`

Pros:

- concrete and descriptive
- makes it obvious that the input/output is an image target

Cons:

- sounds very implementation-first
- ties the concept to “render target” storage rather than “effect node in a graph”
- awkward if the source later becomes a named graph image, not strictly a user-authored render
  target

### `BlurPass`

Pros:

- graph-oriented and renderer-oriented
- sounds cozy nested under `EmissivePass`
- matches the fact that this is recorded as one or more fullscreen passes
- fits current render-graph naming better than `RenderTargetBlur`

Cons:

- emphasizes implementation mechanics a bit more than a name like `BlurEffect`

## Recommendation

For authored scene syntax, prefer **`BlurPass`**.

For renderer/runtime terminology, it is also fine to describe the implementation as one or more
blur passes, so the naming stays aligned all the way down.

## Relationship to `EmissivePass`

The user intuition is good:

```text
RenderGraph {
    EmissivePass {
        BlurPass { ... }
    }
    Bloom { ... }
}
```

That is attractive because it reads as:

- produce the emissive image
- optionally post-process that emissive image
- then bloom can consume either the raw or blurred result

This nesting also communicates ownership naturally: the blur is “over the output of
`EmissivePass`”.

## Why generic blur is still better than a bloom-only child

Even if `BlurPass` is commonly nested under `EmissivePass`, it should not be emissive-specific.

Blur is fundamentally an **image operation**:

- one image source
- one blurred image output
- one blur algorithm/configuration

So the more general model is:

- `EmissivePass` produces an image
- `BlurPass` consumes an image and produces another image
- `Bloom` consumes one or more images and composites glow into the final scene

That dataflow composes much better than an emissive-special blur feature.

## Proposed authored semantics

### Option A — nested source by default

```text
RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.04)
            half_res(true)
        }
    }

    Bloom {
        source(blur_pass)
        intensity(0.95)
    }
}
```

Rule:

- when `BlurPass` is nested directly under another pass node, its default source is that parent
  node's primary output

Pros:

- very ergonomic
- visually expresses local graph structure

Cons:

- needs a clear rule for which parent nodes expose a “primary output”

### Option B — explicit source reference

```text
RenderGraph {
    EmissivePass {}

    BlurPass {
        source("render_graph.emissive_pass.output")
        radius_ndc(0.04)
        half_res(true)
    }

    Bloom {
        source("render_graph.blur_pass.output")
    }
}
```

Pros:

- more explicit
- easier to generalize to arbitrary graphs

Cons:

- more verbose
- less pleasant for common/simple cases

### Recommended shape

Use **both**:

- nesting gives a default source
- an explicit `source(...)` override is allowed when needed

That yields a pleasant v1 while preserving future flexibility.

## Proposed config surface

Conceptual config:

```rust
pub struct BlurPassConfig {
    pub enabled: bool,
    pub radius_ndc: f32,
    pub half_res: bool,
    pub algorithm: BlurAlgorithm,
    pub output_texture: Option<String>,
}

pub enum BlurAlgorithm {
    Gaussian,
    Kawase,
    Box,
}
```

Notes:

- `radius_ndc` matches the existing bloom authoring model and keeps the blur screen-relative
- `half_res` is a direct quality/performance knob
- `output_texture` matches the existing render-image publication direction in the render-to-texture
  docs
- `algorithm` can default to `Gaussian`; other variants can stay aspirational initially

## Relation to `Bloom`

There are two plausible relationships:

### Model 1 — blur is internal to bloom

`Bloom` continues to own all blur passes internally.

Pros:

- simplest authored API
- easiest to keep optimized internally

Cons:

- no reusable blur node
- hard to inspect or repurpose the blur result
- weak graph composability

### Model 2 — bloom can consume a blurred source

`Bloom` remains the high-level glow/composite effect, but it may optionally consume the output of a
preceding `BlurPass`.

Pros:

- graph becomes more explicit
- easier debugging and reuse
- lets users insert prefilters or custom blur quality tradeoffs

Cons:

- more authored complexity
- slightly harder to define default behavior cleanly

## Recommended bloom relationship

Keep `Bloom` as the default “one-node glow effect”, but allow an explicit blur node in front of it.

That means:

- `Bloom { ... }` alone remains valid and easy
- advanced users can author:
  - `EmissivePass`
  - optional `BlurPass`
  - `Bloom` consuming either raw emissive or blurred emissive

In other words:

- `Bloom` = artistic glow/composite feature
- `BlurPass` = generic image-processing primitive

## Runtime interpretation

This draft does **not** require immediate support for arbitrary DAG scheduling.

A practical staged implementation could be:

1. Support `BlurPass` only as a child of `EmissivePass`.
2. Interpret it as a well-defined extra stage between emissive extraction and bloom.
3. Optionally publish its output as a runtime texture.
4. Later generalize it into a true graph node with explicit source references.

That gives us a low-risk path from today's renderer to a more declarative render graph.

## Suggested authored examples

### Simple default blur under emissive pass

```text
RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.025)
            half_res(true)
        }
    }

    Bloom {
        intensity(0.9)
    }
}
```

Interpretation:

- render emissive source
- blur that emissive source once using the authored settings
- bloom composites from that blurred source

### Debuggable blur output

```text
RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.025)
            half_res(true)
            Texture.render_image("render_graph.emissive_blur.output")
        }
    }

    Bloom {
        intensity(0.9)
    }
}
```

That aligns naturally with the existing render-to-texture direction.

## Design preference summary

- Prefer **`BlurPass`** as the authored name.
- Treat it as a **generic image operation**, not an emissive-only special case.
- Allow it to be **nested under `EmissivePass`** for ergonomic default-source semantics.
- Keep `Bloom` as the simpler high-level glow feature, not merely “blur + add”.
- Implement nested-emissive blur first if we want a low-risk incremental rollout.

## Open questions

- Should `Bloom` implicitly consume a child `BlurPass` under `EmissivePass`, or should the source
  edge be explicit?
- Should `BlurPass` publish a default runtime image name like
  `render_graph.blur_pass.output` or derive one from its parent context?
- Do we want a single generic blur API now, even if runtime only supports Gaussian initially?
- Is `radius_ndc` the right authoring unit for generic blur, or should generic blur use pixels
  while bloom continues to use NDC-derived sizing?