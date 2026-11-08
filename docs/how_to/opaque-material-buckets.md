# Opaque material ordering buckets

Surface material definitions can choose `near`, `middle`, or `far`. The default
is `middle`. The base defines the renderer contract; each definition chooses its
ordering hint independently.

```mms
let vehicle_definition = MaterialDefinition.anime().opaque_bucket("near")
let vehicle_material = Material(vehicle_definition)
let wall_definition = MaterialDefinition.anime().opaque_bucket("far")
let wall_material = Material(wall_definition)

R.cube() { vehicle_material }
R.cube() { wall_material }
```

A custom definition uses the same builder before its fragment/schema:

```mms
import { f32 } from "types"
let definition = MaterialDefinition(MaterialBase())
    .opaque_bucket("near")
    .fragment("assets/shaders/youtuber-led-wave.frag")
    .inputs({ time_seconds: f32, hue_speed: f32, wave_speed: f32, wave_count: f32, intensity: f32 })
```

Match the actual shader's input schema when adapting this pattern. Bucket names
are metadata, not fragment-uniform fields. `Material(definition)` instances share
that definition's bucket. Independent built-in definitions reuse the existing
GPU programs; bucket authoring does not change their shader or lighting settings.
Plain `MaterialDefinition.anime()` calls retain their shared default identity.

Only `MaterialBase()` (Surface) supports explicit buckets, including the implicit
Surface base of built-in Anime/Toon definitions. Planar-reflective and
extracted-emissive bases reject this builder. There is no bucket on MaterialBase
itself and no per-instance override in this slice.

## Choosing a bucket

- Near: geometry likely to cover much of the view, such as an occupied vehicle's
  cabin/body or a foreground character.
- Middle: ordinary props and unassigned Surface definitions.
- Far: broad room/environment surfaces expected behind the other objects.

These are authored hints, not automatic distance tests. A truck outside the
camera can be behind a wall; assigning it Near is not universally optimal. Keep
separate definitions for interior/exterior finishes when the model supports them.
There is no automatic mount-state or distance-based bucket reassignment.

Within each opaque stencil scope the renderer orders by bucket, then retains
material/texture/mesh grouping. Every bucket shares ordinary depth testing and
writes. A closer Far object still occludes a more distant Near object. Changing
opaque order can change which coplanar surface wins, so inspect decals and labels.

Far opaque is not `Background`. Background has its own phase and foreground depth
is cleared afterwards; its optional occluded/lit mode can use depth internally.
Background, transparent, cutout and overlay ordering ignore these opaque hints.

## Native updates and command reuse

Rust authoring uses `OpaqueBucket::{Near, Middle, Far}` and
`MaterialDefinitionComponent::with_opaque_bucket(bucket)`. For a registered
Surface definition use `VisualWorld::set_surface_opaque_bucket(id, bucket)`;
it validates scope and invalidates retained ordering only when the value changes.
It does not change uniform values, compile a shader or create a new pipeline.
Do not mutate only the low-level registry and expect visual-cache invalidation.

MMS exposes a creation-time definition builder, not a live bucket setter.
Ordering is rebuilt after a relevant change and reused between frames; camera
and joint updates do not require sorting every frame. More buckets can split
batches, so compare actual views and frame timings before broad adoption.

See the [implementation and controlled benchmarks](../task/opaque-material-ordering-bands.md).
