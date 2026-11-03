# Ship interior demo

Run one scene file for every view preset:

```sh
cargo run --release -- load examples/ship_interior.mms
```

The flags at the top of that MMS file control startup:

| Flag | Effect |
| --- | --- |
| `desktop_view_enabled` | Desktop window, renderer output and fly-camera/input rig |
| `xr_view_enabled` | OpenXR startup and tracked camera/input rig |
| `xr_avatar_enabled` | Mitten embodiment when XR is enabled; false uses a free XR rig |
| `bloom_enabled` | Initial bloom graph; the cafeteria panel can change graphics settings live |

Desktop only uses true/false for the first two flags, XR only uses false/true,
and both uses true/true. Both false runs simulation without graphics. With the
desktop flag off, the host creates no desktop window, Vulkan surface, or desktop
swapchain. XR-only startup requires an available OpenXR runtime.

Desktop controls are WASD and mouse. In the barracks, grab a machine pistol and
use the XR trigger or desktop Space to fire. XR height controls are on the bridge
wall; graphics controls are on the cafeteria wall. The scene has no editor wrapper
or editor UI. The washroom has a wide mirror,
three stalls opposite two sinks, hand dryers, towels, and a recessed waste bin.

The cafeteria is 22 m wide with an 8.5 m ceiling. Its east ground-floor door
leads through a tiled passage around the corner into the washroom. The bridge
hall connects only to the cafeteria and side barracks. Pool ceramic tiles cover
the washroom shell, stalls, vanity, and its approach passage. The washroom has a
5 m ceiling and six small clerestory windows; ceiling spotlights light the barracks.

The cafeteria stairs reach the 5.2 m upper landing above the washroom. Continue
into Cherry Grove, then through its east exit and short connector to the larger
vehicle biodome. The vehicle lot is no longer opposite the park across the
cafeteria. Both domes retain open frames without alpha pane layers.

One motorcycle in the trailer park carries Lumae. Its passenger is attached to
the seat with an authored pose, and its small state object alternates waiting,
driving along the central lane, and a smooth half-turn. The demo is kinematic;
it is not a player riding controller. The second motorcycle remains parked.

The cafeteria also has an open hot-dog stand along its west wall, beside the
three restaurant bays. Lumae holds a tray behind the grill; a four-flavor drink
fountain sits on the counter, and a mini-fridge rides in the pickup bed. These
are visual prototypes, without purchasing or dispensing interactions yet.

All pickups default to beige-brown paint with a custom metallic material:
scene-light specular highlights, a chrome matcap, and a strong rim. Lighting can
exceed the authored paint albedo. `pickup_truck(color, cargo)` accepts an optional
paint override and a cargo component mounted in the bed; the body still uses
retained destroyable mesh panels. Finish parameters live in
`assets/components/materials/pickup_painted_metal.mms`.

The regular park's low implicit hills are clipped into the upper deck slab,
textured in the X/Z plane, and use their generated triangles for collision.
Unchanged fields retain their visual and collision meshes. Paths remain level.

For other terrain components, `ImplicitSurface.clip_floor(y)` intersects the
field with the region above a local Y plane. A direct `Zone` child receives the
generated contact mesh; add `Collidable.static()` beneath that zone to walk on
it. Texture coordinates project local X/Z at one repeat per metre, and texture
bindings follow regenerated surface outputs.

Default ship surfaces inherit `MaterialDefinition.anime()` from the scene root;
custom metal, sky, emission, and unlit materials keep their authored appearance.
The window panes and dome panels currently render as clear openings with their
frames retained, avoiding overlapping alpha layers in front of the space sky.
Bridge and cafeteria window collision barriers remain in place.

The washroom mirror uses a single plane and an open border. The east wall behind
the vanity is visually open, with an invisible collision boundary; keep that
space empty so the reflected camera can see the player through the wall opening.

Outside the bridge windscreen, three cafeteria clerestory windows, a dining-level side window and the
park dome is a shared black/purple nebula sky, a crescent with a purple halo,
and golden kawaii stars at 60% of their usual size. Nebula clouds are baked into
one panorama; the sky shader samples it without evaluating noise at runtime.

Regenerate procedural textures:

```sh
cargo run --release --example generate-ship-grass-texture
cargo run --release --example generate-peanut-butter-texture
cargo run --release --example generate-ship-nebula-texture
```

The generators accept an optional output directory after `--`. Palette and seed
constants live in the Rust source so the artwork can be refined and reproduced.
