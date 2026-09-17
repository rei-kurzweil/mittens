# MMS custom fragment shader: first testable slice

Status: planned.

Parent design: [`Shader` component for MMS](../draft/shader-component.md).

## Goal

Prove the smallest complete path from an MMS-authored custom fragment shader to
a live GPU parameter update:

```mms
let burning = Shader.fragment("../assets/shaders/mms-burn-test.frag")
    .param("burn_amount", "f32", 0.0)
{
    R.cube() {}
}

on_global("FrameTick", fn(event) {
    let amount = burning.get_param("burn_amount")
    let next = amount + event.dt_sec * 0.25
    burning.set_param("burn_amount", if next > 1.0 { 0.0 } else { next })
})
```

The cube must keep rendering while the shader is pending, switch to the custom
fragment program when it is ready, and animate `burn_amount` without compiling
another shader, creating another pipeline, rebuilding the mesh, or allocating a
new persistent descriptor set for each value.

This slice establishes the internal seams needed by later custom materials. It
does not attempt the complete API from the parent draft.

## Deliberate scope

Support exactly:

- `Shader.fragment(path)` in MMS;
- one declared parameter type: `f32`;
- `.param(name, "f32", default)` at construction time;
- retained `get_param(name)` and `set_param(name, value)` methods;
- one `Shader` source wrapping one or more ordinary static mesh primitives;
- the ordinary opaque mesh render phase;
- the engine's existing static mesh vertex stage and a fixed, versioned
  `MeshSurfaceV1` varying/descriptor contract;
- asynchronous source compilation, followed by renderer-owned pipeline creation;
- built-in Toon rendering as the pending/error fallback; and
- one reusable custom pipeline per compatible program key, regardless of how
  many wrapped primitives or material instances use it.

Explicitly exclude:

- GLTF projection, cached-deformed/skinned meshes, and implicit surfaces;
- custom vertex stages;
- transparent, cutout, emissive, clipped, transmissive, mirror, grid, overlay,
  XR-specific, and render-to-texture variants beyond whatever falls out from
  the already shared ordinary render target;
- textures or samplers declared through `.param`;
- vectors, colors, matrices, arrays, structs, storage buffers, push constants,
  and specialization constants;
- per-instance or per-vertex authored attributes;
- shader hot reload, serialization, editor controls, `Animation`/`Keyframe`
  integration, and a general material-copy API; and
- arbitrary shader-reflected descriptor layouts or render state.

If an excluded target inherits this `Shader`, report that the first slice does
not support it and leave its existing built-in material active.

## Fixed shader contract

The test fragment shader is not an unrestricted Vulkan program. It must conform
to `MeshSurfaceV1`:

- consume the standard static mesh vertex outputs at their documented
  locations;
- use the existing renderer-owned global set 0;
- use an engine-owned material uniform buffer at set 1, binding 0;
- preserve set 1, binding 1 as the existing base-color texture/sampler, even if
  the test shader only samples the default white texture;
- leave the existing renderer-owned set 2 layout compatible; and
- write one color output for the ordinary opaque pass.

For this slice, reflection must find one material-block `float` field matching
the MMS parameter name `burn_amount`. Reject a missing field, duplicate MMS
declaration, mismatched type, unexpected descriptor binding, or incompatible
fragment input before pipeline creation.

The test asset should make the update unmistakable without introducing another
feature. For example, `mms-burn-test.frag` can interpolate the ordinary base
color toward orange and dark ash using `burn_amount`; it does not need noise,
dissolve textures, or transparent edge effects yet.

## Runtime lifecycle

### Discovery and compilation

Materializing `Shader.fragment(path)` registers a request with a shader-program
service. The request key contains the canonical source identity and content
revision, not the component ID. Two components requesting identical source must
share one compilation result.

The component moves through observable internal states:

```text
Unrequested -> Compiling -> ModuleReady -> PipelineReady
                       \-> Failed
```

File reading, GLSL-to-SPIR-V compilation, and reflection run off the frame
thread. Completion is returned through a queue polled at a normal engine
synchronization point. Compilation failure records an actionable diagnostic
containing the source path and compiler/reflection error.

### Pipeline creation and reuse

Vulkan shader-module and graphics-pipeline ownership remains in the renderer.
After a compiled module is available, the renderer validates it and resolves a
pipeline cache key containing at least:

```text
fragment program revision
+ standard static MeshSurfaceV1 vertex program
+ fixed pipeline layout
+ opaque render state
+ color/depth target formats
+ sample count
```

The `ShaderComponent` ID and `burn_amount` value are not pipeline-key fields.
All compatible users of `mms-burn-test.frag` share the same pipeline. Until the
pipeline becomes ready, or after a failure, each renderable continues using its
Toon fallback.

Pipeline creation may initially occur on the renderer thread after background
compilation. Record its duration and creation count. Moving Vulkan pipeline
creation onto a dedicated cache/worker mechanism is follow-up work if the first
measurement shows a frame-time problem.

### Material parameter storage

Each authored `Shader` source owns one material instance and one current
`burn_amount`. All primitives beneath that source share the value.

Allocate bounded, frame-safe material storage: at most one writable uniform
slot and compatible descriptor binding per configured frame-in-flight slot per
material instance. Reuse those slots only after their frame has retired.
Updating `burn_amount` marks the material instance dirty and copies its current
value into the next safe slot. It must not key an immutable cache by the float's
bits as the current Anime path does.

The exact first implementation may use a small per-material ring of UBOs and
descriptor sets. Its important contract is bounded allocation proportional to
`material instances * frames in flight`, not to the number of parameter values
ever written. A later shared dynamic-uniform arena may replace this without
changing MMS.

## ECS and MMS ownership

Add a `ShaderComponent` distinct from the built-in `ShadingComponent`.
Construction stores the source path, validated schema/default, load state, and
stable material-instance identity. Initialization emits a registration intent;
a focused shader-material system resolves the nearest supported renderable
descendants and retains pending attachments until both their visual instances
and custom pipeline are ready.

For this slice, use explicit retained methods:

```mms
burning.get_param("burn_amount")
burning.set_param("burn_amount", 0.65)
```

This avoids making dynamic `burning.params.burn_amount` member assignment a
dependency. The future property facade must route through the same validated
setter and dirty tracking.

Reject unknown names, non-numeric values, non-finite values, and calls against
a stale/non-live component reference. Clamp `burn_amount` to `[0, 1]` only if
that range is explicitly part of the test parameter's schema; otherwise accept
all finite `f32` values and let the test script perform its own animation range.

## Test and instrumentation scene

Add `examples/custom-fragment-shader.mms` and
`assets/shaders/mms-burn-test.frag`. The example should show:

- two cubes beneath one `Shader` source, proving shared material state;
- one ordinary Toon cube beside them as a stable comparison;
- a `FrameTick` update that repeatedly changes `burn_amount`; and
- visible fallback while compilation is pending, with an eventual atomic switch
  to the custom program.

Add debug/test counters for:

- shader compile requests and completed compilations;
- reflected-interface validation failures;
- custom pipeline cache hits, misses, and creations;
- material UBO-slot and descriptor-set allocations; and
- material parameter uploads.

Counters may be test-only or behind a focused debug flag, but the integration
test must be able to assert them rather than relying only on visual inspection.

## Acceptance tests

1. **MMS construction:** the example materializes one `ShaderComponent` with
   source path, one `f32` schema entry, and the declared default.
2. **Asynchronous fallback:** before completion, both wrapped cubes remain
   visible using Toon. A successful completion switches them to the custom
   program without respawning the components or meshes.
3. **Program deduplication:** two independently authored `Shader` sources using
   the same file produce one compilation and one compatible pipeline, while
   retaining independent `burn_amount` values.
4. **Live update:** at least 10,000 distinct finite values produce parameter
   uploads but no additional shader compilations or pipeline creations.
5. **Bounded descriptors:** after warm-up, the same update loop does not grow
   UBO-slot or descriptor-set allocation beyond the documented
   frames-in-flight bound.
6. **Batch correctness:** primitives sharing one material source can batch when
   their other draw state matches. Independent material instances render their
   own values even if that requires separate material binds/draw batches.
7. **Validation:** missing files, invalid GLSL, a missing/mistyped
   `burn_amount`, incompatible varyings, and unexpected bindings all yield a
   useful diagnostic and preserve Toon fallback rendering.
8. **Cleanup:** removing the `ShaderComponent` or its subtree retires material
   resources safely and does not invalidate in-flight command buffers or the
   shared pipeline used by another source.

## Implementation checklist

- [ ] Define and document `MeshSurfaceV1` from the current static mesh vertex
      outputs and descriptor-set layouts.
- [ ] Add `ShaderComponent`, registration/removal intents, MMS constructor and
      `.param`, plus retained scalar getter/setter methods.
- [ ] Add asynchronous source compilation/reflection with canonical request
      deduplication and diagnostics.
- [ ] Add renderer-owned custom fragment module and pipeline caches, with Toon
      fallback while pending or failed.
- [ ] Extend renderer-facing instance/batch identity to select a custom program
      and material instance without replacing the built-in compatibility path.
- [ ] Add bounded frames-in-flight material UBO/descriptor storage and dirty
      scalar uploads.
- [ ] Add the test shader, runnable MMS example, counters, unit tests, and
      renderer integration tests described above.
- [ ] Record measured compilation time, pipeline creation time, warm update
      allocation counts, and draw/batch changes in this task before marking the
      slice implemented.

## Exit condition

This task is complete only when the runnable example visibly animates one
custom scalar and the automated counters prove that changing the scalar does
not compile shaders, create pipelines, or grow persistent descriptor/UBO
allocation after warm-up. GLTF and skinned support begin as a separate follow-up
using this tested program/material infrastructure.
