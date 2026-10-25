# Youtuber room: first custom-f32 material checkpoint

Date: 2026-10-24
2026-10-25 API follow-up: the room now imports f32 and uses symmetric
`.inputs(...)`: colon-style type fields on MaterialDefinition and equals-style
value expressions on Material. Both multiline forms omit commas. Type schemas
are distinct immutable runtime values; GPU storage/reflection remains the same
bounded f32 UBO path. The earlier builder spellings remain supported for
compatibility. The 2026-10-25 common-flow slice subsequently moved built-in and
custom registration/resolution into MaterialSystem and replaced Anime-typed
registry storage with generic schema/default/value records. GPU layouts remain
validated separately. Broader renderer capabilities still have follow-up work.

Current scene: gray architecture, one monitor point light and white rim lighting
(strength 0.18, power 4). The temporary Distance/Diffuse/Anime menu and its debug
shader input/API have been removed. Facing-gated Anime shading still applies to
both room geometry and the seated Rei(mu) NPC at the middle of the left wall.
LED emission and Bloom remain enabled through led_effects_enabled. The resized
straw model is unscaled; its old 28 m recenter offset is corrected to 0.429 m.

Status: initial room tried in VR; combined cube LEDs, extracted emission and scene polish tested with headless Vulkan, awaiting another headset test.
Parent: [custom materials](epic/custom-materials.md).
Foundation: [accepted built-in instance checkpoint](material-instances-anime-vr-checkpoint.md).

## Run

```sh
./target/release/mittens-engine load examples/youtuber-room.mms
```

View flags at the top of the example select the same scene's outputs:

```mms
let desktop_view_enabled = false
let xr_view_enabled = true
```

These defaults select XR only. Use true/false for desktop only, or true/true
for both. The desktop flag gates the Camera3D and desktop input subtree;
without that camera the engine skips desktop rendering/presentation. The XR
flag gates its camera, controllers, locomotion and XR.on()/off() declaration.
This controls views; the normal launcher can still create its window shell.

The new [room](../../examples/youtuber-room.mms) replaces the previously proposed
four-strips-around-a-plane fixture as the current custom-material example.
Desktop movement uses WASD/mouse; the scene also includes XR locomotion and
avatar-free controller lasers using the already accepted controller topology.
No avatar, model download or texture assets are required for the furniture.

The room contains a box-built desk, monitor with a geometric editing interface,
keyboard/mouse, drawer pedestal, fan-front PC tower, acoustic panels, rug, a
fictional 100k creator award and a racing chair. The chair's contoured shell and
inset use `R.polygon`; the seat, arms and wheeled base use boxes.

Scene polish places the point light at the monitor and adds a weaker, soft
120-degree spotlight facing into the room (intensities 1.5 and 0.65). The award
text uses centered flex rows under a LayoutRoot. The floor consists of 2 cm
planks with 12 mm gaps exposing recessed brown wood and the lit plank edges.
The chair's vertical support post is centered on its wheel-spoke hub at local Z=0.

## Visible behavior

308 small LED cubes run around the top of the walls. `CombineMesh` bakes them
into four meshes and discards the source cube hierarchies. One retained material
is attached to all four outputs. Each cube's vertices carry its perimeter
position in UV X, so the whole chip changes brightness together. The backing is
separate white geometry with a shared, non-emissive Anime shading declaration.

[The custom fragment](../../assets/shaders/youtuber-led-wave.frag) displays one
uniform hue across all strips. Brightness waves travel along UV X while the
shared hue slowly rotates through the rainbow: each crest laps the perimeter
in 12 seconds, and one full hue cycle takes four laps (48 seconds). One FrameTick
setter updates `time_seconds` per frame. The complete scene phase repeats every
48 seconds and explicitly wraps at that period to bound f32 time precision.
This simplifies the initial two-hue palette at the user's request.

The LEDs now use `MaterialBase.extracted_emissive()`. The same shader output and
retained parameter revision feed visible color and emissive extraction. Bloom
is enabled in the room's RenderGraph; the white backing is excluded from
extraction. Bloom adds a screen-space halo; ordinary lights still light the room.
Bloom intensity is 1.65 and radius is 0.036 NDC (both three times their initial
settings). The LED units protrude only 8 mm from the backing while keeping their
35 mm square faces.

## Implemented authoring and backend path

```mms
import { f32 } from "types"

let base = MaterialBase.extracted_emissive()
let definition = MaterialDefinition(base)
    .fragment("assets/shaders/youtuber-led-wave.frag")
    .inputs({
        time_seconds: f32
        hue_speed: f32
        wave_speed: f32
        wave_count: f32
        intensity: f32
    })

let material = Material(definition).inputs({
    time_seconds = 0.0
    hue_speed = 0.020833333333
    wave_speed = 0.5
    wave_count = 6.0
    intensity = 1.0
})
```

The MMS host constructs these components directly. Completing a definition or
material expression queues registration with `MaterialSystem`, independently
of tree attachment or `Component::init`. An instance can prepare its referenced
definition even if registration events arrive in a different order. The system
uses the existing registry in VisualWorld, not a second material registry.

`MaterialBase()` remains the default surface contract with no extraction.
`MaterialBase.extracted_emissive()` is an opaque static-mesh contract whose
fragment RGB output is emitted radiance in both passes. It selects extraction
through definition metadata, independently of legacy Toon handle identity.
Extraction uses the same mesh, shader, UVs and material descriptor snapshot,
with depth testing (`LessOrEqual`) and depth writes disabled. Pending/failed
custom shaders do not extract their fallback rendering. No per-pass authored
callback or extra scalar upload is needed. Bloom activation stays in RenderGraph.

CombineMesh retains the first source's shared custom-material declaration when
it collapses source geometry, and bakes authored vertex UV overrides. Later
scalar updates keep the same combined mesh and material identity. Removing a
material retires the combined output's binding and extraction eligibility.

Definitions declare 1..=256 unique custom-named scalar f32 fields. Material
instances hold independent values; omitted declared fields default to zero.
Initializers/setters reject unknown fields, nonnumeric values, nonfinite values,
out-of-range conversion and type tags supplied as numeric data. `set_input` and
`get_input` work before scene attachment. Ordinary capitalized `Type(args)` syntax
lowers to the registered `.new(args)` constructor; lowercase builder/function
calls retain their existing interpretation.

Local GLSL fragment files compile on a background thread using shaderc. SPIR-V
reflection validates the MMS field names/count/types against a uniform block at
set 1, binding 0 and supplies field offsets/block size. Other authored descriptors
and push constants are rejected in this slice. The existing mesh vertex stage,
renderer-owned descriptor layout and standard mesh streams are reused. Shader
or pipeline failures report diagnostics and retain built-in fallback rendering.

The renderer uses reflected, dynamically sized uniform buffers. Compatible
definitions can reuse the descriptor binding layout; each float does not need
its own descriptor. Each material has at most eight custom UBO/descriptor
snapshots. Revision reuse, exclusive descriptor ownership and Vulkano write
guards protect recorded/unsubmitted and in-flight commands. Exhaustion returns
explicit backpressure. Scalar edits do not alter draw identity or compile a
shader/pipeline. Definition/source removal retires bindings; command buffers
retain their old resources. Stale updates cannot resurrect a deleted definition.

## Validation and VR checklist

Focused CPU checks cover real room loading, unattached definition preparation,
four simultaneous material attachments, FrameTick updates with stable identity,
independent MMS values, invalid declarations/values, reflected field reordering,
different field counts and definition retirement. Existing renderable-system
and shared-material/controller-pointer compatibility checks pass.
CombineMesh checks additionally cover UV transfer, shared-material survival
after source deletion, updates without reuploading geometry, and retirement.

Two headless Vulkan checks cover:

- Rendering the actual room into both eye layers, checking four shared custom
  draws and their selected program, coherent eye images, and changed LED pixels
  after a time update. Extraction readback confirms matching stereo images,
  matching visible/extracted brightness, one uniform hue, bright/dim wave
  regions, and black extraction when intensity is zero. Only the four combined
  LED outputs enter extraction. The test saves previews to
  `/tmp/youtuber-room-0.png`, `/tmp/youtuber-room-1.png` and corresponding
  `/tmp/youtuber-room-emission-*.png` extraction images.
- Holding eight custom revisions unchanged, rejecting a ninth while all are
  retained, then performing 10,000 updates without further snapshot allocation.
  A second definition with a different name/count uses a separate 16-byte block,
  alongside the room shader's 32-byte block.

The MMS suite and focused material/controller checks pass. A broader `material`
test filter also selects unrelated legacy example tests: the shading-models
example fails on undefined `is_string`, and migrated keyframe examples fail on
unsupported legacy Animation evaluation. Both failures reproduce at the prior
commit (`1413ec9a`). The broad run also stalls in the microphone/eye-tracking
fixture after audio-device errors, so it is not recorded as a passing suite.

The room subtree now uses neutral Anime shading with a slight smooth transition
between 65% albedo in shade and full albedo in light. White rim light is enabled
at strength 0.16 for comparison; set `.rim_strength(0.0)` in the room declaration
to try it without rim light. Engine-wide defaults remain unchanged for this
visual trial. The monitor stem sits behind its casing, and the chair has red
upholstery on both front and rear with outward-facing polygon normals.

In VR, check that all strips share one slowly changing hue, brightness
waves cross wall corners continuously, backings stay unchanged, and controller
poses/lasers follow the hands. Walk around the desk/chair and inspect the keyboard
and plaque at normal room scale. The initial room was tried in VR; these follow-up
changes still need the next headset test.

## Remaining work

- Built-in/common source registration, resolution, retained updates and
  retirement now share MaterialSystem and the same schema-backed registry.
  Complete renderer feature/state capability migration and richer public
  built-in definition authoring remain follow-ups. Shading compatibility
  authoring and GLTF projections remain supported.
- The broader mixed/typed-value record API, `Material(base)`
  implicit-definition convenience, other base presets, vector/matrix/resource
  fields and general source serialization remain pending.
- Custom transparent/cutout/clipped/background variants, skinned/GLTF geometry,
  separate surface/emission programs and hot reload remain subsequent slices.
  Custom extraction currently admits only opaque, unclipped foreground meshes.
  An arbitrary authored `intensity` field can produce black radiance but is not
  interpreted by the renderer as permission to omit the entire draw/pass.

The accepted Anime tests still establish the compatibility baseline. This room
proves the first custom-f32 path; it does not complete the full renderer capability migration or Materials v3 epic.
