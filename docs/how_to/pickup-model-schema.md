# Pickup model and suspension schema

All pickups now use one prefab and one suspension implementation:
`pickup_truck(color, cargo, gravity_enabled)` in
`assets/components/vehicles/pickup_truck.mms`. Both orchard trucks and the
hot-dog stand pickup share it. The sibling-suspension factory, alternate code
paths, and outdoor A/B comparison panel have been removed.

## Model and coordinates

The prefab loads `assets/models/mitten_pickup.glb`, including its body, seats,
steering wheel, four wheels and four springs. It does not overlay procedural
truck geometry or load extra spring meshes. The authored +Z front is turned to
vehicle-local -Z and the model is raised 0.180146 m to place its lowest tyre at
the ground. Coordinates are metres, +X right, +Y up, -Z forward.

Static body, truck bed, seats, doors, headlights, hood, roof, wheel wells and
steering wheel are combined; suspension branches remain independently movable.
The `.10.blend` export supplies these named parts, including `car_seat`,
`car_seat.passenger_seat` and `truck_bed`; imported whitespace becomes underscores, so
Blender's `wheel wells` is queried as `wheel_wells` at runtime.
Imported mesh overrides are resolved before combining, so all 2,287 body
triangles are included rather than unresolved placeholder geometry.

| Corner | Authored spring node | Authored wheel node |
|---|---|---|
| `fl` | `suspension_spring` | `front-left-WHEEL` |
| `fr` | `suspension_spring.002` | `front-left-WHEEL.003` |
| `rl` | `suspension_spring.001` | `front-left-WHEEL.001` |
| `rr` | `suspension_spring.003` | `front-left-WHEEL.002` |

`pickup_model_suspension.mms` binds these names after `GLTFInitialized`. Exact
name selectors preserve Blender's dotted suffixes. Each authored wheel keeps
its own rotation and nonuniform scale; compression requested before loading
is retained until binding completes.

```text
truck
  suspension_mount_fl             fixed authored pose
    spring_fl                     Y scale controls extension
      [imported spring geometry]
      suspension_endpoint_fl      authored wheel offset
        TOP.fork_trs().drop("scale")
          steering_fl
            wheel_fl              hub-centred spin pivot
              [imported wheel geometry and intrinsic scale]
```

The endpoint receives the spring scale before the fork strips inherited scale
from the wheel branch. No separate wheel-position update is needed. Resolve
names within each truck because all instances share the same labels.

## Lighting, gravity and collision

The current finish uses `pickup_lit_finish` from `pickup_painted_metal.mms`:
scene-light diffuse/specular response, a strong explicit rim and a restrained
matcap contribution. The pool's reflection-only matcap shader is no longer used
on the trucks. Authored colours remain the default; an optional colour overrides
the static body batch.

Gravity is enabled by default through a direct child of native `Velocity`.
Changing throttle, steering, braking or rider state preserves vertical velocity.
Pass `false` as the third factory argument only for an isolated pose fixture
that deliberately has no ground. Parked trucks fall and settle, and fall again
if their support is removed.

Four spherical sliding zones approximate the lower chassis and upper cab;
static contact proxies leave cab sides open. Contacts ignore the truck's own
parts and accumulate corrections for zones sharing its movement root. These
are rounded upright collision approximations, not oriented moving boxes or
wheel-ground suspension forces. Every closed imported primitive has its own
Destroyable marker; only the open `truck` lower cabin is excluded. All three
ship pickups use the same prefab and the current `assets/models/mitten_pickup.glb`.
Three nearby unit-damage impacts reach the default local threshold. The first
successful hit on combined body geometry unpacks that truck's batches; intact
parts and the excluded cabin stay visible. Other instances keep their own damage
state and share only immutable source geometry and local query BVHs.

Imported cuts run on the bounded CSG worker. Visuals, precise ray geometry and
opted-in panel collision publish after successful upload; failures retain the
previous output. Source geometry is never replaced with a box for subtraction.
Doors, roof, bed and wheel wells opt into triangle contact; decorative tires and
coils remain ray-queryable/cuttable without continuously rebuilding contact BVHs.
The lower cabin keeps its structural contact proxies. New cut surfaces retain the
lit material, but general imported UV/material-island preservation is still pending.

## Dashboard and controls

Two warm-white headlight spotlights point along vehicle-local -Z with 25 m range.
RPM, speed, fuel and odometer instruments remain static placeholders. Their
geometry is combined; labels remain separate. The driver eye anchor is
[-0.33, 1.78, 0.30] and the cargo mount is [0, 0.64, 1.50] in the rest pose.

RIDE buttons to the right of the steering wheel select all corners, front/rear
axles, left/right sides or FL/FR/RL/RR. UP/DOWN changes requested spring lengths
in 5% steps, clamped to 65–135%; REST resets every corner. The readout shows the
selected group's average percentage.

Hydraulic travel retains a 0.45528 m reference; each model binding converts that
travel to its authored spring/hub offset, so Blender origin revisions do not
make uniform lift pull the wheels through the deck. The raw compression method
still directly scales the springs for model checks.

The temporary layout editor, selection tabs and save panel have been removed.
The authored gauge placement is embedded in its component; the hydraulic panel
is at chassis-local [0.04, 1.80, -0.70], 20 cm above its previous position.
Both widgets follow `pickup_chassis_frame`. The ship has no Editor/ED wrapper.

The four requested extensions fit an average body support plane: their mean
raises/lowers the body, front/rear differences pitch it, and left/right
differences roll it. Model, instruments, headlights, cargo and driver anchor
follow the body frame. Updates occur only on button events, with no per-truck
frame callback. The raw `set_suspension_compression` method remains for isolated
compression tests and resets body tilt; there is no alternate suspension layout.

Native `Transition` components smooth `update_transform` on both
`pickup_chassis_pose` (translation/rotation) and each `spring_<corner>` (scale).
They share `pickup_ride_transition()`: 0.6 beats, sine easing in/out and replacement
of the active target from its current pose. Interpolation runs in the engine,
without a per-frame MMS callback. At 120 BPM this takes 0.3 seconds. The dashboard
readout shows the requested height while the mechanism moves toward it.

Gravity now moves the truck as a whole, but there is still no sprung-mass,
stiffness, damping or independent wheel-contact solver. Four arbitrary corner
requests need not define a plane, so the ride-height preview averages them.
Collision volumes remain upright while the visible body tilts. This is not
rollover physics or automatic weight-induced spring compression.

The archived procedural truck remains in `pickup_truck_prototype.mms` for
cube-based damage fixtures. It also uses only the transform-fork layout, with
`suspension-spring.glb` generated by `generate-suspension-spring.py`.
