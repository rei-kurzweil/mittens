# Curved smoke tubes and spin blur

Follow-ups to the straight rocket exhaust capsule
([STA rocket launcher](sta-rocket-launcher.md)). The two effects are
tubes that bend and lag (cigarette smoke, drifting trails), and a
speed-dependent swap from a spinning mesh to a blurred disc (propellers,
tires).

Both stay within the current scope rules in `AGENTS.md`: generic custom
material definitions only, no legacy pipelines, no new intermediate targets.
Anything needing a custom vertex stage or a new renderer feature is an
explicit scope change and must be approved before implementation.

## 1. Curved smoke tube (skinned)

The leg rockets use one connected, skinned tube each. The previous
soft-edged sphere emitter gave each particle its own silhouette and opacity
falloff. The desktop transparent pipeline depth-tests against the scene with
depth writes disabled.

### Geometry and rig

- A cylinder with axial edge loops supplies one connected surface. The
  existing `R.helix_tube(0, radius, height, 0, 0, segments, sides)` gives an
  open straight tube with any required number of rings. The ordinary
  `R.cylinder` puts its side vertices at the two ends.
- Use the gohei's authored `Joint` chain and
  `Armature.joints(root).give_automatic_weights()` for a straight-tube
  proof. Its cold-path binder weights the geometry once. The live tube binds
  each ring to its matching joint. The faces between rings interpolate the
  shape, keeping the rings coherent as the tube grows wider.
- Shape the taper and rounded end with the joint palette. Use UVs with one coordinate
  along the full trail and one around the ring. The existing straight-tube
  generator uses U along the length. The launched capsule's shader uses V
  along the length, so match the coordinates when adapting the shader.
- Use a custom surface material with one transparent surface. Preserve the
  two-tone side view and blend to a soft radial profile when seen end-on.
  Apply one longitudinal fade to both views so the wider tail goes
  translucent. For a tube that doubles back, test the overlap from both
  eyes before changing depth policy.

### Motion

The gohei's `SecondaryMotion` with `SpringBone` makes a chain lag behind a
moving mount and settle. The live trail instead samples a bounded world-space
nozzle history after IK. Each ring corresponds to a different sample age, so
the older end stays behind when the nozzle turns. The engine updates the
existing skin palette without per-frame MMS callbacks or mesh uploads.

### Validation

- A headless rigging fixture combines the subdivided tube, the gohei's
  automatic binding path, and the translucent smoke material. It checks
  the selected deformed program and CPU deformation of the tail and seam.
  Headset validation covers the rendered silhouette, end-on softness, UV
  seam, tight turns, and transparency where the tube folds across itself.
- Test hover-to-thrust length changes, teleport/reset, and fade on stop.
  Compare GPU timing against the sphere emitter in the same scene and pose.
- If the rigging test file grows, move procedural surface and custom
  material deformation cases into a fixture separate from paper craft and
  attachment lifecycle tests. Share the headless uploader and compare
  material routing, joint palettes, and transient mesh release before and
  after a split.

## 2. Spin blur (propellers, tires)

Below a speed threshold, spin the real mesh. Above it, cross-fade to a disc
whose shader draws the blur. Tire texture is pending (user is authoring one).

### Proposed shape

- Component/prefab takes the spinning node, a disc radius, blade count and the
  angular speed source. Rotation itself stays native (angular velocity on
  `Velocity`, or an existing spin driver) — no MMS per-frame rotation.
- Threshold around 60 rpm (tunable), with a short hysteresis band or a
  cross-fade range so it does not flicker at the boundary.
- Blur disc fragment: polar coordinates from UV; blade streaks as soft angular
  bands whose width grows and contrast drops with rpm; radial falloff;
  translucent gaps. Optional slow pattern rotation (aliasing "wagon wheel"
  stand-in) driven by an input.
- Tires: the disc is the side wall; tread is a cylinder whose UV scroll/blur
  is driven by the same speed. Needs the tire texture.

### Open questions

- [ ] What supplies rpm: authored animation, `Velocity` angular velocity, or a
      game value (engine throttle)? The swap needs to read it without an MMS
      FrameTick per object if possible.
- [ ] Cross-fade vs. hard swap: does the renderer handle the mesh fading in the
      alpha-blended stream cleanly (instance alpha < 0.999 routing)?
- [ ] Does the blur disc need per-frame `set_input` for rpm, or can an
      animated material input / clock time drive it?

### Checklist

- [ ] Propeller prefab: mesh + blur disc, threshold swap, rpm input.
- [ ] Blur disc material + fragment (`assets/shaders/`), blade count input.
- [ ] Example scene with a throttle to sweep rpm across the threshold.
- [ ] Tire variant once the tire texture exists.
- [ ] Validate on headset: no flicker at the threshold, disc readable in stereo.

## Android notes (from the 2026-11-20 renderer audit)

- Android culls back faces on non-background surfaces. A crossed-plane
  prototype would need to account for the two renderers' cull policies.
- The rocket explosion (`MaterialDefinition.toon()` + `Emissive`) and the GLB
  body's imported materials are rejected by the Android renderer, which fails
  the whole frame. The rocket is desktop-only until those have custom materials.
- New shaders/textures must be added to `android/src/demo_assets.rs`.
- Skinning/deformation is rejected on Android, so the skinned smoke tube
  needs the Android renderer to support it first (scope change).

## Work log

- The launched rocket uses a straight capsule with a view-dependent smoke
  shader. The leg rockets use the same wave structure on a native skinned
  tube. Both use generic surface materials.
