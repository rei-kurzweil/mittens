# Task: prepare LED strip asset and custom-materials example

Date: 2026-10-10
Status: focused preparation plan; documentation only, implementation pending.
Parent: [custom materials epic](epic/custom-materials.md).
First material milestone: [f32 time-animation Phase 1](material-f32-time-animation-first-slice.md).
Emission milestone: [capability and LED proof](material-emission-capabilities-and-led-strip.md).
Release: [0.10.0](epic/0.10.0/README.md).

## Goal

Prepare one small, attractive scene at **`examples/custom-materials.mms`**:
four LED strips, one on each edge of a plane; an FPS-controlled desktop camera;
a small amount of ambient light; and the reusable implicit-surface clouds in
the background. The LED rectangles use a custom **`animated_led_strip`**
material. The plastic/backing of each strip uses regular Toon shading.

Keep this scene focused on preparing the asset and proving custom animated
materials. Wall/baseboard installation is a later reuse; the first scene is
just the four-edge plane, not a room or rhythm-game venue. The asset remains
reusable, but this is its only live example scene for now.

## Checked starting point and current usage

- [light_strip.mms](../../assets/components/rhythm_game/light_strip.mms)
  already separates `light_strip_base` (`R.plane`) from `light_strip_lights`
  (`CombineMesh` containing shallow cubes). The default LEDs are square
  rectangles with a little depth, facing local +Z along a local-X strip.
- Its length is derived from LED count, width, clear gap, and end padding.
  Color and emissive intensity currently come from identical immediate
  children of each LED source cube. It has no custom material input yet.
- A repository usage search found no live scene import in `examples/` or
  `assets/world/`. The factory is exercised by
  [the asset smoke test](../../src/scripting/tests.rs), and described by
  [the CombineMesh appearance review](../review/rhythm-game-assets-and-combine-mesh-appearance.md).
  Keep these verification/history references; there is no existing live scene
  instance to remove.
- The [rhythm-game slices](rhythm-game-prototype-slices.md) proposed future
  runway strips. Remove that venue requirement from the current plan so the
  first LED visual/material work belongs to this example. Recheck references
  when implementing; do not remove unrelated pose-marker assets or tests.

## Asset boundary: LED material vs backing

Preserve this structure, with names/configuration spelling finalized in the
implementation slice:

```text
strip transform
  backing renderable                 -> regular Toon
  animated_led_strip Material wrapper -> shared live material instance
    CombineMesh
      individual LED rectangles       -> inherit that instance
    generated combined primitive      -> retains that instance after baking
```

Combine only the LED meshes, never the backing and LEDs together. One combined
output has one appearance/material binding; this split permits two independent
materials without adding multi-material CombineMesh. The LEDs need no separate
draw per rectangle if the custom shader can animate their combined surface.

The current [CombineMeshSystem](../../src/engine/ecs/system/combine_mesh_system.rs)
copies the first source material/color/emission into its output and normally
collapses source geometry after baking. Wrap the complete LED CombineMesh subtree
with the custom Material, leaving the backing outside that wrapper. All LED
rectangles inherit the same material; the first source's **resolved** material
therefore supplies the combined primitive's material, even for a strip with
hundreds of LEDs. Transfer the live definition/instance identity to the
**generated combined output** rather than copying only its default source handle.
Do not animate a material on a source cube that disappears after baking or
let the first-source Toon fallback permanently overwrite the custom binding.
Define/test generated-output attachment and late pipeline-ready replacement
as a narrow dependency of this scene, not a general multi-material bake rewrite.

Allow the factory to configure the LED Material wrapper explicitly. Material
also supports direct-child placement on a renderable, like Shading, but wrapper
inheritance is the preferred form for this asset. Keep the wrapper itself alive
outside the geometry that CombineMesh collapses. Keep backing selection outside
that scope. Maintain stable unique strip roots so four instances are independently
addressable; repeated internal names must be queried relative to each root.
Preserve the existing defaults/test path until a replacement is deliberately
migrated. Do not require an asset-path move out of `rhythm_game/` for this proof.

## Scene composition and appearance

- [ ] Create the planned `examples/custom-materials.mms` with exactly four
  light-strip factory instances around one ordinary plane. Choose a front-facing
  vertical XY presentation plane initially so the camera sees local +Z LED faces.
  Top/bottom strips run along X; left/right rotate by a quarter-turn about Z.
  Match strip lengths to the plane and keep backing corners tidy, without
  overlapped LEDs, unexplained gaps, or coplanar z-fighting.
- [ ] Keep the plane and plastic backing explicitly Toon. Choose restrained
  neutral/dark colors and tune LED spacing, width, extrusion, end padding,
  and small forward offsets at normal viewing distance. The backing must remain
  visibly distinct from the LED surfaces throughout the animation.
- [ ] Use a movable `I` -> `T` -> `C3D` topology with
  `InputTransformMode.forward_z() { fps_rotation() roll_axis_y() }`, following
  the existing [desktop camera example](../../examples/implicit-surface-refraction-clouds.mms).
  Start far enough in front to see all four strips and allow close inspection.
  No avatar or hand-tracking setup is needed for this scene.
- [ ] Add a small nonzero ambient light, initially around `AL.rgb(0.03, 0.03, 0.04)`
  and tune visually. Avoid a bright environment that hides the strip contrast;
  do not add stronger light sources unless the Toon backing needs one.
- [ ] Import [clouds](../../assets/components/backgrounds/clouds.mms), which
  uses [implicit cloud surfaces](../../assets/components/backgrounds/cloud.mms).
  Put a modest cluster count in the background, following
  [e2.mms](../../examples/e2.mms)'s background composition. Preserve the clouds'
  own unlit/opacity contract; custom LED material scope must not reach them.

## Staged material integration

1. **Preparation:** get the four-edge composition and material attachment scopes
   correct using current built-in LED appearance. Keep the scene runnable while
   the custom program is pending/unavailable. Record a baseline view.
2. **f32-only animation proof:** bind the custom material named
   `animated_led_strip` only to combined LED surfaces. Drive `time_seconds`
   through explicit FrameTick setters using the Phase 1 contract. An opaque
   time-varying pattern/color is sufficient initially; no extraction/Bloom
   dependency is introduced just to prove an animated fragment.
3. **Emission proof:** once capabilities exist, use the same pattern and retained
   f32 revision in visible LED shading and emission extraction. Add the explicit
   emission/Bloom render graph, and validate lit/dark rectangles and backing
   exclusion against the emission ticket. Built-in fallback glow is not evidence
   that the custom emission path works.

One shared material can initially animate all four strips. For the Phase 1
isolation proof, use three strips sharing one instance and the fourth using a
separate instance of the same shader with a different time/phase value. That
still leaves exactly four strips, and demonstrates shared versus independent
updates without adding a second example scene. The backing is the built-in
comparison and must not pulse.

The combine bake preserves source UVs; cube UVs repeat per LED and do not
automatically provide a continuous strip coordinate. Begin with uniform pulse
or repeated per-LED animation. Before promising a chase travelling along the
strip, verify a suitable existing position/varying or explicitly prepare strip
UVs. Avoid making new per-instance attributes or richer-than-f32 input types
prerequisites for this phase.

## Acceptance and scope

- [ ] Only `custom-materials.mms` instantiates the strip as a live example;
  current usage is rechecked and unrelated scene/game work is preserved.
- [ ] Exactly four strips frame the plane, with four separate Toon backings and
  four LED-only combined outputs; appearance remains clear close up and at the
  initial camera distance. FPS controls and implicit clouds work independently.
- [ ] The combined outputs retain live custom material identity after baking and
  late shader readiness. Shared/independent f32 changes animate only LED surfaces;
  the plane, backings, and clouds retain their own materials.
- [ ] Wrapper inheritance resolves on the first LED before its material is
  transferred to the combined primitive. Verify all LEDs in the batch resolve
  the same instance, without creating a separate material instance per rectangle.
- [ ] Phase 1's compile/pipeline/upload/allocation and cleanup gates are recorded
  using this scene; material updates do not rebake the combined mesh.
- [ ] Emission/Bloom is tested only in its explicit later milestone. Record
  extraction-source and visible screenshots, settings, and measured costs.

This ticket prepares the focused example/asset integration. It does not add a
room, wall/baseboard system, rhythm-game integration, arbitrary input types,
multi-material mesh batching, or an alternative material renderer. Implement
the shared material machinery through the linked tickets.
