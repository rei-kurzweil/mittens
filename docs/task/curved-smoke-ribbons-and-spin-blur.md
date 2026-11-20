# Curved smoke ribbons and spin blur

Follow-ups to the straight rocket exhaust ribbon
([STA rocket launcher](sta-rocket-launcher.md)). Two independent effects:
ribbons that bend and lag (cigarette smoke, drifting trails), and a
speed-dependent swap from a spinning mesh to a blurred disc (propellers,
tires).

Both stay within the current scope rules in `AGENTS.md`: generic custom
material definitions only, no legacy pipelines, no new intermediate targets.
Anything needing a custom vertex stage or a new renderer feature is an
explicit scope change and must be approved before implementation.

## 1. Curved smoke ribbon (skinned)

A ribbon mesh along a bone chain. Secondary motion moves the bones; the
existing toon-exhaust style fragment draws the smoke along UV y (0 at the
source, 1 at the tail). The rocket ribbon needs none of this because it is
straight.

### Proposed shape

- Strip mesh: N segments (8–16) along one axis, two or three crossed strips
  (or a camera-facing strip if a vertex stage becomes available), UV y running
  root → tail, each ring weighted to its nearest one or two bones.
- Bone chain driven as a lagged follow chain: root pinned to the source tip
  (cigarette ember, exhaust nozzle); each following bone eases toward its
  predecessor's previous position plus upward drift (buoyancy) and noise. This
  is a spring/verlet chain, not physics collision.
- Width/opacity growth toward the tail in the shader, as in the rocket ribbon.
- One shared material definition; per-instance inputs only where needed.

### Open questions

- [ ] Can a `MaterialDefinition(MaterialBase())` custom fragment be used on a
      skinned mesh today? Check `cached-skinned-toon-mesh.vert`, the
      deformation cache (`src/engine/graphics/deformation.rs`) and how custom
      programs pick their vertex stage. If not, decide whether skinned custom
      materials are an approved scope change.
- [ ] Where does the follow chain run? Native component (preferred: no MMS
      FrameTick per ribbon, matches the rocket flight design) vs. an MMS
      prototype calling bone pose updates each frame.
- [ ] Does an existing secondary-motion / spring-bone system exist that can
      drive arbitrary bone chains? Reuse it rather than adding another.
- [ ] Alternative if skinning is blocked: CPU-rebuilt strip mesh each frame.
      Rejected by default for Android/XR2 (per-frame upload); record measurements
      if reconsidered.
- [ ] Transparency: crossed strips self-overlap and double opacity at the
      intersection. Acceptable for toon smoke? Compare against one strip.

### Checklist

- [ ] Answer skinned-custom-material question; record the decision here.
- [ ] Generate the strip mesh + armature (script under `assets/models/`, like
      `generate-smoke-rounded-box.py`).
- [ ] Follow-chain driver with source pin, drift, noise, damping.
- [ ] Smoke ribbon fragment (reuse/generalize `toon-rocket-exhaust.frag`;
      world-phase waves, tail fade). Consider sampling
      `assets/textures/noise/perlin-*.dds` for breakup.
- [ ] Cigarette integration: pin to the burning tip, coexist with or replace the
      rounded-cube `smoke.mms` pool.
- [ ] Validate: teleport/reset, fast hand motion, cleanup, stereo look on
      headset, GPU timing.

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

- Android culls back faces on non-background surfaces; desktop culls none. The
  rocket's crossed single-sided planes each vanish from one side on Android.
  Doubling the planes back-to-back would double opacity on desktop. This needs
  a material-level cull choice or one consistent policy across renderers.
- The rocket explosion (`MaterialDefinition.toon()` + `Emissive`) and the GLB
  body's imported materials are rejected by the Android renderer, which fails
  the whole frame. The rocket is desktop-only until those have custom materials.
- New shaders/textures must be added to `android/src/demo_assets.rs`.
- Skinning/deformation is rejected on Android, so the skinned smoke ribbon
  needs the Android renderer to support it first (scope change).

## Work log

- Created tracker. Rocket straight ribbon landed first (see the rocket task's
  work log); nothing here is implemented yet.
