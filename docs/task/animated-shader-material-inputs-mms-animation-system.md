# Investigate animated shader-material inputs in MMS and the animation system

Status: proposed / investigation

2026-10-10 scope decision: first support **f32 inputs only**, proved by
[Material time-animation Phase 1](material-f32-time-animation-first-slice.md).
Named defaults and explicit retained setters driven by FrameTick are sufficient.
Fuller MMS type integration, vectors/colors/matrices/resources, automatic time
bindings, stored callback inputs, and Animation/Keyframe integration follow
after the working scalar upload/animation path. The questions and ocean example
below are the broader follow-up investigation, not Phase 1 requirements.

2026-10-09: included in [custom materials for 0.10.0](epic/custom-materials.md).
Use `Material` as the authored owner of typed retained inputs. Literal values,
explicit setters/animation, renderer globals, and any later provider/closure
binding need distinct update contracts; render callbacks never execute MMS.
The `ShaderMaterial` example below remains exploratory historical syntax.

Parent architecture: [Materials v2: shader programs, vertex-family resolution, and typed parameters](epic/materials-v2.md)

## Motivation

Scenes such as `examples/planar-auto-transparency-optimization.mms` should eventually be able to
animate material inputs—for example time, wave amplitude, wavelength, color, and light response on
an ocean shader—without rebuilding meshes or adding scene-specific Rust systems.

Before adding the ocean shader, determine how custom shader/material parameters should participate
in the Mittens engine's component, signal, animation, batching, and GPU-upload models.

## Questions to answer

- What is the supported ownership model for shader programs and material instances?
- Initial authored input type is f32. Which richer types should follow, and how
  should they reuse MMS type identities/checking and the material schema?
- Does each renderable own parameter overrides, or can many instances share one material state?
- How does MMS declare a shader material, its parameter schema, defaults, and shader asset paths?
- How does MMS obtain a handle that can be targeted by setters and `Animation`/`Keyframe` blocks?
- Should continuously advancing time be a renderer-provided global input, an animation channel, or
  an explicitly authored parameter?
- Which updates require descriptor changes, uniform-buffer writes, instance-buffer writes, pipeline
  changes, or draw-batch splits?
- How are invalid shader paths, compilation failures, schema mismatches, and unsupported parameter
  types reported to MMS authors?
- How should hot reload preserve or migrate existing animated parameter values?

## Candidate authoring shape to evaluate

The investigation should test an API approximately like this without treating the names as fixed:

```mms
let ocean_material = ShaderMaterial.from_files(
    "assets/shaders/ocean.vert",
    "assets/shaders/ocean.frag",
) {
    float("time", 0.0)
    float("wave_amplitude", 0.18)
    color("deep_color", [0.12, 0.02, 0.36, 0.72])
}

R.plane() {
    ocean_material
}

Animation.looping().length(8.0) {
    Keyframe.at(0.0) { ocean_material.set_float("time", 0.0) }
    Keyframe.at(1.0) { ocean_material.set_float("time", 8.0) }
}
```

Also evaluate a renderer-provided time input so authors do not need a long looping keyframe merely
to advance shader time.

## Investigation work

- [ ] Map the current `MaterialHandle`, material UBO, descriptor-set, and draw-batching paths.
- [ ] Map how existing animatable component properties become intents and update runtime state.
- [ ] Decide whether shader parameters are components, material-instance records, animation targets,
      or a combination of those concepts.
- [ ] Specify identity and lifetime for shared versus per-renderable material instances.
- [ ] Specify MMS constructors, typed parameter declarations, setters, and serialization.
- [ ] Specify animation interpolation rules for every supported parameter type.
- [ ] Specify dirty flags and the narrowest GPU update required for a parameter change.
- [ ] Measure the batching and memory consequences of shared and per-instance animated parameters.
- [ ] Prototype the smallest useful path with one scalar animated by the existing animation system.
- [ ] Validate point-light inputs and transparency continue to work with a custom ocean fragment
      shader.

## Performance questions

Record costs separately for:

- a renderer-global time value updated once per frame;
- one shared material parameter updated once per frame;
- many independently animated material instances;
- parameter changes that preserve batching;
- parameter changes that split batches or rebuild descriptor sets.

Avoid an API that silently turns one shared material and one draw batch into hundreds of material
instances or descriptor updates.

## Deliverable

Produce a short design recommendation covering:

- ECS and renderer ownership;
- the MMS surface;
- integration with `Animation` and `Keyframe`;
- GPU update and batching behavior;
- serialization and hot reload;
- an incremental implementation plan and benchmark scene.

## Non-goals

- Implementing the ocean shader as part of this investigation task.
- Designing a general visual node editor.
- Supporting arbitrary untyped byte buffers from MMS in the first version.
