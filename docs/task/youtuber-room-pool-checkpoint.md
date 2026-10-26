# Youtuber room: connected pool and first optical materials

Date: 2026-10-25
Status: scene/material slice implemented; interactive headset review in progress.

The same youtuber-room.mms now opens through its existing front doorway into a
2 m hallway and a parameterized pool hall. The resized hall is 8.2 x 14 x 8.91 m
(width/length/height); the water/basin opening is 3.5 x 9 m. The four size variables
are at the top of the scene. The latest 1 m width increase is entirely on +X: the room
interior spans X=-3.6..4.6 and the basin X=-1.25..2.25. Both centres shift
+0.5 m, while the entrance and pool access ladder stay fixed. The complete slide
assembly is currently at X=-0.70 m; its landing, ladder, supports and collision
zones move together. The basin is 1.4 m deep, with deck strips around an actual opening;
there is no solid floor covering the water. Toggle pool_water_enabled to inspect
the empty basin. Desktop/XR flags and the original room are preserved.

XR-only mode (desktop_view_enabled=false, xr_view_enabled=true) replaces the
seated wall NPC with the tracked Rei(mu) player rig. Desktop-only and both-view
modes retain the NPC; both-view XR retains its avatar-free controllers. The
tracked rig uses the same shared room shading plus the canonical humanoid map,
AVC arm IK/hand smoothing, crouch sole offsets, head-mounted CXR, tracked hand
lasers and bone-relative pointers. Microphone RMS (80 ms, 120 Hz high-pass) drives
the mapped mouth via AVC with the main Rei(mu) example's calibration; XR/OSC eye
closure drives blink while the idle animation owns gaze direction. Hair, tail,
shirt and bow secondary motion and collider presets are retained. XR-only
locomotion now has Velocity and gravity, gated on AVC's CapsuleReady event (with
a late-subscriber readiness check). Crouch IK is explicitly enabled with the
Rei(mu) sole offsets. Static collision zones cover room/furniture boxes, hallway,
tile shell, basin, landing and metal supports. The slide has 64 x 12 short
oriented wall-panel zones forming a hollow proxy, with low friction; it is not
blocked by a solid central tube collider. Water remains non-solid and there is
no swimming/buoyancy model. The 12 existing crouch checks, including actual
Rei(mu)/Bisket bind frames and full avatar-stage knee solving, pass.
The view-switch check covers avatar/NPC ownership, microphone setup, mouth/blink
mapping and camera counts for all three modes. Headset acceptance is pending.

The reusable asset is assets/components/rooms/indoor-pool.mms. White tiled walls
have three real high rectangular openings on each side, recessed emissive white
panels, and six inward/downward white spotlights. The tiled basin/deck, white
ceiling and wall jambs remain separate geometry. The custom ceramic definition
uses facing-gated distance/cone intensity for its Anime-style shade ramp, a small
rim, and a mild light-dependent specular highlight. This is a custom definition
using the common material path, rather than another renderer-specific shading
branch. It reads the engine camera and light buffers through the fixed interface.
Window spotlights now aim across the room at the opposite wall, one quarter
of the room width below their source height at the same Z (2.05 m / about
14 degrees downward at the current width), instead of steeply down toward the basin. Cone width/intensity are kept.
The tile instance exposes shade_threshold/lit_threshold and now uses 0.05..2.0
instead of the previous hardcoded 0.35..0.55 for a much broader diffuse gradient.
Specular settings and spotlight cone softness remain unchanged.
Tile specular now uses normal/view-dependent Blinn-Phong with N dot L weighting,
strength 0.65 and shininess 64. Its range fade is linear rather than the squared
fade feeding the Anime diffuse ramp, so highlights retain influence at greater
distances. Spotlight cone masking applies to both. Specular adds after the
albedo-capped base/rim, allowing reflected light to exceed albedo. The Anime
diffuse ramp remains distance-driven with a facing gate.

Doorway join correction: hallway walls/ceiling now span Z=2.41..4.41, between
the bedroom's exterior wall face and the pool entrance's exterior face. The pool
origin is Z=4.57, allowing for its 16 cm entrance wall; its previous Z=4.35 origin
overlapped the hallway. Only the hallway floor extends through both thresholds
(Z=2.35..4.57). This removes coplanar jamb/header overlap at both joins while
retaining a clear 2 m corridor. Position variables are beside the size controls.

## Procedural assets

All images are generated from retained Rust source, with no AI images:

```sh
cargo run --release --example generate-pool-tiles
cargo run --release --example generate-metal-matcap
```

Files in assets/textures/pool:

| Image | Data |
| --- | --- |
| white-tiles-albedo.png | Seamless white ceramic with subtle grey grout |
| white-tiles-normal.png | Tangent-space normals for a 1 mm bevel on a 25 cm tile |
| white-tiles-roughness.png | Smooth faces and rougher grout |
| white-tiles-surface.png | Active one-texture material image: RGB normal, A grayscale albedo |
| chrome-matcap.png | Synthetic chrome environment with broad ceiling/ground tones and bright reflection bands |

The tile shader ignores the packed image's alpha for opacity and uses it as
albedo; ceramic gloss is derived from its face/grout mask. Separate source maps
remain available for inspection and a later multi-resource input interface.
Existing RGBA8 texture upload is UNORM, appropriate for the packed normal data.

The chrome definition samples its matcap using the view-space surface normal.
Ordinary Texture.with_uri supplies each geometry's image. Binding 0 of material
set 1 is the numeric UBO; binding 1 is the image/sampler. Custom descriptor
snapshots now include texture identity and filtering, so sharing one material
across differently textured consumers cannot accidentally reuse the wrong image.
The same bounded, frame-safe snapshot pool is retained.

## Geometry API

```mms
R.cylinder(radius, height, sides)
R.torus(radius, tube_radius, segments, sides)
R.helix(inner_radius, outer_radius, height, start_angle, sweep_angle, segments, wall_height)
R.helix_tube(radius, tube_radius, height, start_angle, sweep_angle, segments, sides)
```

All are about local +Y; angles are radians and height is total rise. Helix sweeps
may exceed 2*pi in either direction. R.helix makes an open ribbon with optional
vertical trough walls. R.helix_tube sweeps a round cross-section, also supporting
zero-height arcs. Cylinders have end caps; helical tubes have open ends.
Dimensions/segment limits are checked and authored parameters serialize.
The chrome pool ladder uses straight cylinders for rails/rungs and semicircular
tube handles. The slide is a two-turn cyan tube with a dedicated plastic
definition (assets/shaders/plastic.frag). Its square landing and reinforcement
collars share the exterior plastic instance. The landing has a chrome ladder,
supports and side guardrails. Both platform support posts stay clear of the
spiral/collars: one is in the central opening, extending
from the basin floor, and the other is outside the coil on the deck. The landing
stays 2.2 m square. The latest helix radius is 1.62 m (+50%), with constructed height 4.5696 m
(-15% from 5.376 m) and lower centre Y=0.62. The assembly is now
rotated clockwise 90 degrees from the side-facing layout: the lower mouth
points along -Z toward the pool access ladder. The assembly is at X=-0.70 m after moving 2 m left from the previous trial,
with the complete landing, ladder, supports and collision shell moving together.
The room and basin remain fixed for this placement trial. The unchanged
landing and slide ladder now extend toward the rear deck. The hall gains 2 m
only at the back (rear deck 1.5 -> 3.5 m); the front deck, basin and water stay
fixed. The slide centre remains 1.25 m back from the far basin edge. The hall height is now 8.91 m (+10% from 8.1 m); window openings
follow the ceiling while retaining their previous height and head clearance.
The tube/liner radii remain 0.60/0.56 m, unchanged in this latest resize. The
height is passed to the helix generator, not applied as a Y scale, preserving
round tube cross-sections. The landing height, ladder rungs/rails, support lengths,
collar spacing/orientation and hollow collision panels follow the new centreline.
Reinforcement collars follow the local helix tangent every 1.5 m of arc length.
They use R.torus(0.68, 0.11, 64, 4), compressed along the torus axis for a thin
washer. Four-sided torus profiles have flat top/bottom/inner/outer faces and split
corner normals; higher side counts use smooth round-profile normals. The torus
axis is +Y, radius is the centreline radius and tube_radius is the profile's
circumradius. Torus dimensions, segment limits, winding and serialization are
checked. The slide has an approximate hollow contact shell; ladder climbing
and swimming are not implemented.
The earlier wide Anime ramp experiment (0.05..4.0) is superseded by smooth
normal-based plastic diffuse with a broad saturating response, a restrained rim,
and normalized Blinn-Phong dielectric highlights with Fresnel. Direct specular
adds above albedo. Diffuse and specular scales are independent named f32 inputs.
Other inputs are ambient_strength, shininess, rim_strength and normal_sign.

Two renderables follow the identical centreline: a 60 cm-radius exterior and a
56 cm-radius liner (both doubled). Their material instances share one definition; exterior
diffuse_scale/specular_scale are 1, interior scales are both 1/3. normal_sign=-1
turns the liner's lighting normals inward. The shader discards reverse-facing
fragments so the two-sided mesh pipeline does not let the exterior's backside
hide the liner. Thin washer lips bridge the shell thickness at both open mouths.
Interior darkening is an authored occlusion approximation, not shadow tracing;
ambient/rim are included in the diffuse scale, so they cannot bypass it.

## Water and renderer boundary

The first custom water definition uses a flat cyan alpha-blended surface with
light-dependent specular highlights and subtle analytic normal ripples. One
shared time input updates per frame, wrapping at 20*pi seconds (the common wave
period). There is no fluid simulation or geometry displacement in this slice.
The water now uses MaterialBase.planar_reflective(): a MeshSurfaceV1 preset
that requests the existing planar-mirror captures without replacing its custom
fragment program. A shared base capability marks consumers, each flat surface
gets its own capture identity, and the renderer chooses the proper desktop or
XR-eye image at set 1 binding 1. No authored Mirror/Texture component is needed.
The default capture quality is 256 (subject to the existing mirror quality
setting); captures use the surface aspect ratio. This occupies the current single
image slot rather than introducing an arbitrary resource schema.
The capture camera is each viewer eye reflected across the water plane. Its near
plane clips geometry behind/below the surface, so the virtual underwater camera
cannot be blocked by the basin floor. The water instance itself is excluded from
its own capture, including the transparent stream; there is no self-reflection
feedback. The new base keeps legacy Mirror authoring's clipping policy unchanged.

The water sampler uses (1-U, V): the capture's horizontal basis is reversed,
but its vertical sampling already matches the quad's top-to-bottom UV convention.
The earlier extra V flip swapped the pool ends and showed landmarks behind the
viewer. The GPU regression places red/blue landmarks at opposite ends and checks
both viewing directions, independently of animated distortion and Fresnel.

Fragment ripples distort the planar sample, with reflection_strength=0.85 and
reflection_distortion=0.012. Fresnel increases reflection at grazing angles, and
straight-alpha compositing preserves reflected radiance while leaving the
unreflected part translucent. There is no refracted scene lookup, environment
cubemap, absorption or caustics yet. The metal matcap remains an authored
approximation, independent of the water's live planar capture.

MirrorSystem still refreshes requests in the normal simulation tick, after XR
cameras publish. A named refresh_captures reconciliation after renderable flush
handles newly published surfaces in their first frame; it does not run another
simulation tick or submit duplicate reflection renders. Material-base consumers
retain their custom program and do not get legacy Mirror texture children.
Detaching that material stops its requests. Runtime publications are applied
before draw recording, and custom descriptor snapshots include image-view
identity as well as texture handle/filtering/revision, so a newly published
capture cannot accidentally sample an old image view. Free snapshot slots
rebuild their descriptor when the view changes while retained ones remain safe.

Custom static materials now have opaque and alpha-blended pipeline variants;
the transparent variant tests depth and does not write it. Renderable alpha
selects the existing transparent phase. Shader-only opacity does not select a
phase. Clipping, custom cutout/background variants, custom skinned geometry,
arbitrary resource declarations and authored vertex shaders remain separate work.

Custom reflection permits only the validated engine CameraUBO, readonly 64-light
SSBO, named f32 material block and optional float sampler2D base texture. It checks
reserved layouts/types and rejects other descriptor locations and push constants.
These resources use existing pipeline layouts; they are not user-declared f32s.

## Validation

CPU checks cover multi-turn geometry, finite unit normals, winding, bounds,
invalid geometry and serialization; shader reflection validates the pool and plastic
programs and rejects incompatible camera/light/image declarations. The actual
room still loads under desktop-only, XR-only and both-view configurations.

The headless Vulkan room check renders the pool and ladder in stereo, verifies
actual texture bindings and a non-depth-writing custom water pipeline. A controlled
receiver verifies that a correctly aimed spotlight feeds the same distance ramp
as a point light, loses influence with distance, rejects back-facing receivers
and rejects receivers outside its cone. Grazing but facing receivers keep the
same distance-fed ramp intensity. Existing custom snapshot/10,000-update and
LED-extraction checks remain in use; the latter isolates the original LED fixture
from the new pool and monitor emissions.
The controlled light cases exercise both built-in Anime and the actual custom
tile program, with tile normal/specular/rim detail disabled for the measurement.

Plastic GPU checks measure diffuse-only, specular-only and combined pixel output
at scales 1 and 1/3, independently of the different shell normal directions.
Previews: /tmp/youtuber-pool-hall.png, /tmp/youtuber-pool-ladder.png,
/tmp/youtuber-plastic-slide.png and /tmp/youtuber-plastic-slide-inside.png.

Next optical slices: scene refraction, then a supported vertex/deformation
contract for shared analytic waves. Planar reflection is already connected. General
physics-based water simulation is not a prerequisite for those visual studies.
