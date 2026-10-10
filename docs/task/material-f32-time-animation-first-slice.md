# Phase 1: custom material with f32 time-animation inputs

Date: 2026-10-10
Status: agreed first testable slice; documentation only, implementation pending.
Parent: [custom materials epic](epic/custom-materials.md).
Release: [0.10.0](epic/0.10.0/README.md).

## Goal

Prove a custom opaque fragment material on ordinary static meshes with named
**f32-only inputs**. Animate one `time_seconds` field through normal MMS updates
and show that the shader changes continuously without mesh rebuilds, shader
recompilation, pipeline creation, or growing persistent descriptor allocations.

This is the first material implementation target. Build only the Material
component/definition/instance and resolver infrastructure needed for this proof;
the full cascade migration, extra geometry families, emission/Bloom, and richer
types are subsequent slices. Preserve existing built-in rendering while adding
the new path. Reuse the loading/validation/storage tests from the earlier
[custom-fragment task](mms-custom-fragment-shader-first-slice.md).

## Input and authoring contract

Every authored shader input is a named finite `f32` with a default. Support
more than one such field if the schema/storage path naturally permits it, but
the first example needs just `time_seconds`. An explicit f32 builder avoids a
general type registry or string-selected list of types as a prerequisite.

Illustrative syntax to freeze during implementation, not available MMS API:

```mms
let animated_material = Material.custom("assets/shaders/mms-time-test.frag")
    .input_f32("time_seconds", 0.0)
{
    R.cube() {}
    R.sphere() {}
}

let animation = { elapsed_seconds = 0.0 }
on_global("FrameTick", fn(event) {
    animation.elapsed_seconds = animation.elapsed_seconds + event.dt_sec
    animated_material.set_input("time_seconds", animation.elapsed_seconds)
})
```

The method spelling is exploratory; the required contract is named f32
declarations plus retained getter/setter operations. Validate numeric-to-f32
conversion, finiteness after conversion, names/defaults, stale handles, and
shader field compatibility. Reject unsupported authored types explicitly.
Time values have no implicit `[0, 1]` clamp; units are declared by field name/
documentation. A separate `phase` or `glow_amount` can be a finite scalar too.

The MMS handler computes and writes a value. The material input is not a
stored `fn()` evaluated during rendering. Only the script/main-thread update
path invokes that handler; every consumer/view reads the same retained material
revision. A shared material source shares its value; separately constructed
instances can animate independently while sharing program/pipeline resources.

`time_seconds` in this example means elapsed FrameTick time since scene setup.
It does not promise wall-clock precision, renderer-global time, or audio
transport phase. Start here; an explicit `transport_beat` f32 can follow once
the transport read/pause/seek contract is established. Bound GPU precision for
long-running animations using an explicitly chosen elapsed/phase strategy;
do not silently wrap time and break arbitrary shaders. A global time binding
or Animation/Keyframe channel is a later input-source feature, not required to
upload an f32 from an existing callback.

## Renderer and validation scope

- Ordinary opaque static meshes, the engine vertex stage, and the validated
  versioned `MeshSurfaceV1` interface only. No authored vertex programs.
- Async custom fragment loading/compilation/reflection, useful diagnostics,
  built-in fallback while pending/failed, and cached compatible pipelines.
- Match each input name to a shader material-block float. Validate layout,
  varyings, descriptors, and target state; MMS never packs GPU bytes itself.
- Stable material instance identity and dirty-tracked, bounded frames-in-flight
  UBO/descriptor slots. Values do not enter program/pipeline cache identity.
- Custom vectors/colors, matrices, integers/booleans, arrays/structs,
  textures/samplers, per-instance attributes, and raw buffers are excluded.
  Existing engine-owned descriptors and built-in inputs can still be used
  according to the fixed interface.
- Emission/extraction, transparency/cutout, custom clipping variants, GLTF/
  cached-deformed support, hot reload, and complete material serialization/
  migration remain later acceptance slices.

## Proof and exit gate

Use the planned `examples/custom-materials.mms` from the
[LED-strip preparation ticket](custom-materials-led-strip-example-preparation.md).
Four strips surround a plane. Their combined LED rectangles use the custom
`animated_led_strip` material with a clearly time-varying opaque pattern/color,
without needing emission or textures for this first proof. Three strips share
one instance; the fourth uses an independently timed instance of the same
shader. Separate Toon backings and the plane provide the built-in comparison.
The simple primitive snippet above remains an API illustration, not a separate
required example scene. Verify CombineMesh's generated-output binding explicitly.
The LED Material wraps each LED-only CombineMesh subtree; every source rectangle
inherits it. The first resolved source supplies the combined primitive's live
material instance. Preserve this wrapper through baking and test direct-child
Material attachment on a renderable as the other supported authoring form.

- [ ] Construction and retained f32 get/set work in top-level and runtime MMS.
- [ ] Invalid/nonfinite/out-of-range conversion, unknown fields, incompatible
  shader layouts, and failed compilation produce useful errors/fallback.
- [ ] Shared consumers stay synchronized; independent instance values stay
  independent. Compatible instances share one compilation/pipeline.
- [ ] At least 10,000 distinct finite updates cause uploads but no further
  shader compilation/pipeline creation and no persistent UBO/descriptor growth
  after documented warm-up. Record compile/pipeline/upload/allocation counters.
- [ ] Removing source/consumers safely retires resources while frames are in
  flight and leaves the remaining shared-program instances rendering.
- [ ] Record the runnable example, visible animation, measured update costs,
  and bounded storage before marking Phase 1 complete.

Fuller MMS types should later supply value type identities/checking rather than
introduce a competing material type system. Material-specific shader layout,
ownership, range, and update metadata still remains necessary. That integration
is not a dependency of this f32-only phase.
