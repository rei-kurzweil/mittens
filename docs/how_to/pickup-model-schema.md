# Pickup model and suspension schema

The procedural pickup now has two suspension variants with the same dimensions,
materials, combined body, destroyability, cargo mounting and driving controls:

- `pickup_truck(color, cargo)` uses a scale-dropping transform fork per wheel.
- `pickup_truck_sibling_suspension(color, cargo)` uses sibling spring and wheel
  branches, with no suspension transform forks.

Both variants now use `assets/models/suspension-spring.glb` for all four spring
visuals, with the station's Anime material. The coil is 0.50 m long, extending
down local -Y from its upper-mount origin; no centering offset is applied.
The rest of the truck remains procedural placeholder geometry. Regenerate the
spring with `assets/models/generate-suspension-spring.py` using headless Blender.

Both exports live in `assets/components/vehicles/pickup_truck.mms`. The orchard
parks A (fork) north of the entrance path and B (siblings) south of it. A small
panel near the entrance controls both: Rest, Compress, and Left Bump. These are
static comparison poses, not simulated suspension, terrain following or rollover.

## Coordinates and names

The `pickup_truck` root is the chassis transform. It is the immediate driven
child of `pickup_velocity`; mounting and damage still address this root.
Coordinates are metres, +Y up, +X right and -Z forward. The root origin is at
ground level beneath the chassis. Wheel radius is 0.40 m and suspension rest
length is 0.50 m. The fixed mounts are at Y=0.90 m:

| Corner suffix | Chassis-local X | Chassis-local Z |
|---|---:|---:|
| `fl` | -0.96 | -1.65 |
| `fr` | 0.96 | -1.65 |
| `rl` | -0.96 | 1.55 |
| `rr` | 0.96 | 1.55 |

Each truck has all four suffixes. Names repeat between trucks; resolve them
within the owning truck, rather than using an unscoped world query.

## A: inherited travel through a transform fork

```text
pickup_truck                         chassis / driven root
  suspension_mount_fl               fixed mount and future probe reference
    spring_fl                       scale Y for compression
      [spring geometry]             spans local Y=0 to Y=-0.50
      suspension_endpoint_fl        local Y=-0.50
        TOP.fork_trs().drop("scale")
          steering_fl               local Y steering pivot
            wheel_fl                local X spin pivot, origin at hub
              [wheel geometry]
```

The endpoint receives the spring's scale before the fork strips scale. With
spring scale Y=0.50, the hub rises from chassis-local Y=0.40 to Y=0.65; the tyre
keeps its original dimensions. The fork drops all incoming scale, so author
the truck at real dimensions instead of relying on root scaling to resize it.

## B: explicit travel with sibling branches

```text
pickup_truck
  suspension_mount_fl
    spring_fl                       scale affects spring geometry only
      [spring geometry]
    suspension_endpoint_fl          explicitly positioned at -0.50 * ratio
      steering_fl
        wheel_fl
          [wheel geometry]
```

The compression function updates spring scale and endpoint translation together.
The wheel needs no scale filter because it is outside the scaled spring subtree.
Both variants expose `truck.set_suspension_compression(corner, ratio)`: use a
corner suffix or `null` for all four, with ratios clamped to 0.35–1.35. Ratio 1
is the authored rest pose. This API is local MMS prefab behavior.

## Preparing the Blender replacement

Use named objects/empties matching the chosen hierarchy, with wheel origins at
the hubs and steering/spin axes as above. Keep the body separate from moving
wheel and spring geometry. A Blender object hierarchy does not need an armature
for these rigid parts. The MMS transform fork is an engine-side operator, so
leave space for it between the exported endpoint and steering branch in A;
it is not a Blender object type or a glTF physics feature.

The current body/interior remains the retained `pickup_painted_body` combined
mesh. A future GLB can replace the procedural geometry, but arbitrary GLBs are
not automatically wired into these prefabs yet. Once the model is available,
bind its named transforms after `GLTFInitialized`, retaining the existing
vehicle controller and its seat/cargo anchors around that imported hierarchy.
