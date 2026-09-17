# `Shader` component for MMS — draft

Status: draft for the first custom material slice. This records the intended authoring contract; it is **not implemented** today.

Related work: [Unified `Shading` and cascade](../task/shading-model-components-and-cascade.md), [Materials v2](../task/epic/materials-v2.md), and [animated shader inputs](../task/animated-shader-material-inputs-mms-animation-system.md).

## Purpose

`Shading` selects and configures the engine's built-in shading models. `Shader` is the custom-program counterpart: it gives one authored shader material a stable identity, a fragment-stage program, and named, typed material values that MMS can change while the scene runs.

The first motivating use case is a reusable burning material. The same source can shade the joint and every laser-hit object; changing its `burn_amount` changes all renderables that inherit that source.

```mms
let burning_thing = Shader.fragment("../assets/shaders/burning.frag")
    .param("burn_amount", "f32", 0.0)
{
    R.cylinder(16, 4) {}
    GLTF.new("../assets/models/joint.glb")
}

on_global("FrameTick", fn(event) {
    burning_thing.params.burn_amount += event.dt_sec * 0.05
})
```

The exact retained-property spelling is open. It may instead be an explicit setter such as `burning_thing.set_param("burn_amount", value)`. The required semantics are: named typed fields are validated, retain their material-instance identity, and a live write dirties GPU material data without recompiling the shader or rebuilding geometry.

## What exists now

There is a useful but narrow precedent:

- MMS `Shading.anime()` / `Shading {}` and `Shading.toon()` create one `ShadingComponent`; `AnimeShading` is a compatibility name.
- Anime accepts fixed builder fields. Retained Anime sources currently expose live getter/setter pairs for five scalar controls; source-linked GLTF projections update from that same source.
- The renderer packs those fixed Anime values into its fixed `MaterialUBO` and includes their bits in its bounded material descriptor-set cache key.

This is **not** generic shader-material support. `ShadingComponent` only knows Anime and Toon; shader modules are compiled from built-in source at Rust build time; `MaterialHandle` selects a finite set of renderer pipelines; and every scene mesh material currently uses the fixed set-1 layout:

```text
set 1, binding 0: MaterialUBO
set 1, binding 1: base-color sampled image + sampler
```

So the source-ownership and live-update patterns are available, but program registration/loading, shader validation, arbitrary schemas, material-instance storage, and dynamic pipeline/batch keys still need to be built.

## V1 authoring contract

### Fragment only, automatic vertex family

V1 supports a custom **fragment** stage. `Shader.fragment(path)` participates in the same precedence and ownership cascade as `Shading`:

1. An immediate custom `Shader` child of a renderable wins.
2. Otherwise the nearest enclosing `Shader` scope supplies the source.
3. An immediate built-in `Shading` or custom `Shader` declaration replaces the inherited declaration; competing declarations in one scope are an error.
4. GLTF-generated primitives retain the authored source identity, so later updates reach them too.

The renderer, not the author, chooses the compatible static or cached-deformed/skinned vertex program. The chosen vertex program must provide the versioned mesh-surface varying interface required by the fragment program.

`Shader.vertex(path).fragment(path)` is a later expert feature. It must validate vertex attributes, skinning/deformation support, varyings, descriptor layouts, and render state before a pipeline is created. It is deliberately not part of V1.

### Parameters are material values, not vertex attributes

For V1, `.param(name, type, default)` declares a named field in the custom shader material's typed parameter schema. It is limited to descriptor-backed material data: initially a single engine-owned material uniform block at `set = 1, binding = 0`.

```mms
Shader.fragment("../assets/shaders/burning.frag")
    .param("burn_amount", "f32", 0.0)
    .param("edge_color", "vec4", [1.0, 0.2, 0.0, 1.0])
```

Initially supported types should be `f32`, `vec2`, `vec3`, `vec4`, and `color` once their MMS value mappings and std140 packing are specified. A parameter declaration is matched against the shader's reflected/registered material-block field name, type, offset, and layout. It cannot invent a GPU input that the shader did not declare.

“Descriptor-backed” means the engine owns the UBO resource and its descriptor binding. Ordinary scalar writes update that resource's contents; they do **not** normally change descriptor layout or pipeline identity. A parameter that later becomes a texture/sampler can cause a descriptor write, but textures are a follow-up type with explicit asset and lifetime rules.

V1 explicitly excludes:

- author-defined per-vertex or per-instance vertex attributes;
- raw descriptor set/binding numbers, raw UBO byte offsets, storage buffers, arrays, structs, and push constants in MMS;
- specialization constants and parameters that change a pipeline;
- arbitrary render state selection, compute shaders, and explicit vertex programs.

Per-instance-rate vertex attributes need their own syntax, mesh/instance buffer layout, batching, and validation model. They must not be smuggled into `.param`.

### Sharing and mutation

The block in the example declares one shared material source: its children and projected GLTF primitives read the same `burn_amount`. A live update therefore updates that one material instance and every current consumer. To burn a laser hit independently, author or instantiate a distinct `Shader` source; a later copy/instance API can make that explicit.

`FrameTick` is valid for a deliberately animated shared value. Renderer-owned time should also be exposed as a read-only global shader input in a future common interface, so effects that merely need elapsed time do not require MMS to issue one setter per frame.

## Shader interface and runtime contract

The first custom fragment program must use the engine's versioned `MeshSurfaceV1` interface. It declares the expected vertex outputs, renderer-owned global bindings, fixed material block location, and allowed render phase/state. At registration the engine validates:

- fragment inputs against the selected static/deformed vertex implementation;
- descriptor sets, bindings, block layouts, and parameter schema;
- supported render phase/state and target format requirements; and
- that every MMS parameter matches a material-block field.

V1 needs a program loader/registry. The author-facing path may be GLSL source, but the runtime must compile or load SPIR-V, retain diagnostics, and cache the result. On compile or interface failure, leave the inherited/built-in material active and report the shader path plus the actionable validation error.

The renderer must cache pipelines by program identity, resolved vertex family, render phase/state, output format, MSAA, and clipping variant. Draw batching must include the resolved program and material-instance identities. Material value changes must not recompile a program or recreate a pipeline.

## Implementation slices

1. Define `ShaderComponent`, source ownership/cascade behaviour, a `MeshSurfaceV1` interface contract, and a validated scalar/vector material schema. Keep the existing `MaterialHandle` path as a compatibility bridge.
2. Add a fragment-program registry/loader and pipeline cache keyed by registered program plus automatically selected static/deformed vertex family.
3. Add a renderer-owned material-instance UBO allocation and dirty-upload path. It must be safe for frames in flight and not allocate a persistent UBO and descriptor set for every value encountered while a slider or `FrameTick` runs.
4. Expose MMS `.param(...)` defaults and retained live read/write. Test a scalar `burn_amount` across a primitive and GLTF projection, including repeated per-frame updates and two independently burning sources.
5. Only then evaluate texture/sampler fields, per-renderable overrides, animation integration, hot reload, and explicit vertex programs.

## V1 acceptance criteria

- One custom fragment material correctly shades both ordinary and GLTF-generated mesh primitives, including cached-deformed variants where supported.
- A declared `f32` field is schema-validated and updates from MMS without mesh rebuild, shader compilation, or pipeline recreation.
- One source update reaches all of its consumers; two source instances remain isolated.
- Per-frame updates have bounded descriptor/UBO allocation and preserve frames-in-flight safety.
- Invalid paths, compilation errors, mismatched varyings, and mismatched parameter/block layouts produce useful diagnostics and retain safe fallback rendering.
- MMS cannot author instance-rate vertex attributes through `Shader.param`.
