# Ship interior demo

All four cafeteria vendors are restored with their accordion controls, including
the hot-dog stand, Lumae, grill, drink fountain and mini-fridge pickup.
Performance isolation remains active for bridge equipment, orchard trees and barracks stock. Four simple desk
blocks fill the bridge consoles' former positions beside the existing chairs.
The barracks instantiates only a plain table and its three weapons. Commented
code and full stock prefabs remain available for restoration. Two orchard pickups remain. Mirror controls,
bloom and graphics settings are unchanged. The layout descriptions below also
document the content that is currently commented out.

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
| `view_far_distance` | Shared desktop/XR far plane in metres; defaults to 1,000 |
| `bloom_enabled` | Initial bloom graph; the cafeteria panel can change graphics settings live |

Desktop only uses true/false for the first two flags, XR only uses false/true,
and both uses true/true. Both false runs simulation without graphics. With the
desktop flag off, the host creates no desktop window, Vulkan surface, or desktop
swapchain. XR-only startup requires an available OpenXR runtime.

With `xr_avatar_enabled`, Mitten uses microphone amplitude for mouth opening
(80 ms window, 120 Hz high-pass), HTC tracking for independent eyelid closure,
and an idle saccade loop for pupil direction. HTC gaze direction is disabled so
it does not compete with that loop. Hair, shirt hem, tail and bow use the shared
secondary-motion presets and avatar colliders. Microphone capture starts only
when the embodied XR avatar is enabled.

Desktop controls are WASD and mouse. In the grocery/security store off the bridge hall, grab a machine pistol and
use the XR trigger or desktop Space to fire. XR height controls are on the bridge
wall. Each machine pistol starts with 75 rounds and stops firing at zero;
releasing, dropping or changing fire mode does not refill it. A floating emissive
number above the scope shows remaining ammunition, refreshed at most once per
second, including a final refresh after trigger release. Reloading is not yet
implemented. Graphics controls are on the cafeteria wall. The scene has no editor wrapper
or editor UI. The washroom has a wide mirror,
three stalls opposite two sinks, hand dryers, towels, and a recessed waste bin.

The cafeteria is 22 m wide with an 8.5 m ceiling. Go up its stairs to the
5.2 m landing: the washrooms are through the west passage, opposite the east
exit to Cherry Grove. Follow the blue polygon pictogram signs at the stair foot,
landing, junction and door. The women's corridor turns around the corner into
the room; the men's branch currently ends at a labelled closed cap. The tiled
women's washroom retains its 5 m ceiling, six clerestory windows and clear space
behind the mirror on the west side. The bridge hall serves the side grocery/security shop
and cafeteria only.

Cherry Grove is a 64 x 48 m rectangular clear enclosure with twenty cherry trees.
Two accordion controls beside its west entrance independently unload/rebuild the
north and south orchard halves (ten trees each), including trunk collision.
The hills, paths, benches and enclosure remain loaded.
Two drivable pickups now sit off the main paths in the orchard. The former
trailer park, its second enclosure, RVs, motorcycles and two shops are no longer
instantiated by this example. The orchard's east enclosure wall is closed, and
the old connector and trailer streaming zones have been removed from the scene.
Reusable vehicle, shop and trailer-park component files remain available.

Both orchard pickups now share the same transform-fork suspension prefab and
its dashboard ride-height controls. The alternate sibling layout and outdoor
A/B controls are removed. See the [pickup model schema](pickup-model-schema.md)
for names, pivots and the remaining physics limitations.

The cafeteria also has an open hot-dog stand along its west wall, beside the
three restaurant bays. Lumae holds a tray behind the grill; a four-flavor drink
fountain sits on the counter, and a mini-fridge rides in the pickup bed. These
are visual prototypes, without purchasing or dispensing interactions yet.

The four cafeteria vendors (coffee, noodles, grill and hot dogs) each retain an
independent accordion. The grocery/security inventory and both orchard tree
halves also have controls. The bridge's `BRIDGE / COMPUTERS & CABINETS` control
beside its aft doorway unloads both rows of computer, tape and monitor cabinets;
desks, chairs, room lighting and XR settings remain available for comparison.
Collapsing a control removes its content; restoring
constructs fresh content, while paths and room shells remain loaded. Traffic
lights indicate state: red is collapsed, yellow is loading, green is expanded.
The persistent `ship_input_controller` forwards input to live mounted pickups
and held weapons. No streamed asset registers its own global input callback.

Pickups now use `assets/models/mitten_pickup.glb`, including its body, two seats,
steering wheel, four wheels and four springs. The prefab normalizes its front
direction and ground height, inserts the suspension pivots/scale forks, and
combines stationary body/interior parts while keeping springs and wheels movable.
The former procedural truck, pedals, dashboard fixtures and gauges remain in
`pickup_truck_prototype.mms`. Cube-based damage still works on that prototype;
mesh-cut destruction does not yet support the imported GLB geometry.

To drive, stand beside the left door and point at the truck. Use XR grip or a
desktop left click to enter; use the left grip or another left click to exit.
W/S drive forward/reverse, A/D steer, and Space brakes. XR uses the left stick.
Each vehicle factory returns an existing `Velocity.rotation_basis(body)` component
with one immediate child vehicle transform. Native Velocity integrates translation
and turning; `reset()` stops it. Input publishes changed velocity commands, and
only a mounted truck that is driving and steering needs its forward velocity
refreshed by the controller as its heading changes. No MMS script writes the
vehicle pose every frame. There are no new Velocity modes or MMS event APIs.
The Rider system attaches the player to the driver's eye anchor and suspends
pedestrian movement until exit. This remains a kinematic driving prototype;
it does not yet solve vehicle-to-vehicle crash physics. RVs are near-white with
single grey translucent panes over actual wall openings.

Imported pickups use a lit paint/plastic finish with scene-light specular
highlights, an explicit rim and a small matcap contribution. The reflection-only
pool matcap finish is no longer used. Gravity is enabled by default; parked
trucks fall and settle against their sliding collision zones. Drive commands
preserve vertical velocity when braking, turning or changing riders.

Dashboard RIDE buttons, to the right of the steering wheel, select all corners,
front/rear axles, left/right sides or individual corners. UP/DOWN adjusts ride
height and REST resets all four. Their combined extensions raise, lower, pitch
and roll the visible body and its driver anchor. This is an event-driven
hydraulic preview, without spring stiffness, damping or weight-induced spring sag.
Body pose and spring extension share a 0.6-beat native transition, with sine
easing in/out. Further presses retarget from the current pose.

At Lumae's vendor head-height position (approximately [-7.3, 1.6, 16.8]), three
point lights pass the distance checks: the stand light at [-7.3, 2.7, 16.4]
and two coffee-shop lights at [-5, 2.4, 17.3] and [-5, 2.7, 19.2]. All three
are above her. The cafeteria window spotlights miss that position by cone or
range. Actual contribution varies with surface position and normal; the scene
also has low ambient fill and no dedicated face-height fill light for the vendor.
Her imported `Face` renderable receives `ship_face_material()` from the existing
`GLTFInitialized` callback. This face-only anime preset reduces shadow strength
to 0.28 with a lighter tint; her body retains the regular avatar material.

The regular park's implicit hills rise about 1.2 m above the deck, with bases
about 8 m across. They are clipped into the upper deck slab,
textured in the X/Z plane, and use their generated triangles for collision.
The enlarged ground is split into four implicit fields so each grid remains
within the engine's axis limit. Unchanged fields retain their visual and collision
meshes. Paths remain level.

For other terrain components, `ImplicitSurface.clip_floor(y)` intersects the
field with the region above a local Y plane. A direct `Zone` child receives the
generated contact mesh; add `Collidable.static()` beneath that zone to walk on
it. Texture coordinates project local X/Z at one repeat per metre, and texture
bindings follow regenerated surface outputs.

Pickups now have four rotation-safe spherical sliding contact zones, correcting
the chassis root while ignoring its own static panels. They can slide against
walls and terrain but do not implement terrain-following suspension or rollover.
Larger hills do not yet make them flip. The required native dynamic chassis,
wheel contact and rollover work is described in
[vehicle terrain contact](../draft/vehicle-terrain-contact.md).

The station uses two local presets in `materials/ship_anime_materials.mms`.
Walls, architecture and ordinary props inherit `ship_environment_material()`:
a dark blue-purple shadow, broad light/shade ramp and strong, broad cool rim.
Mitten and both Lumae characters use `ship_avatar_material()`, with gentler
shadows and a narrower ramp to preserve face and clothing detail. Both use the
existing Anime material definition. Shared Anime defaults and presets retain
their original appearance; custom metal, sky, emission and unlit materials
keep their authored appearance.
The building window panes and enclosure panels currently render as clear openings with their
frames retained, avoiding overlapping alpha layers in front of the space sky.
Bridge and cafeteria window collision barriers remain in place.

The washroom mirror uses a single plane and an open border. The wall behind
the vanity is visually open, with an invisible collision boundary; keep that
space empty so the reflected camera can see the player through the wall opening.

Outside the bridge windscreen, three cafeteria clerestory windows, a dining-level side window and the
park enclosure is a shared Sakura-style peach-to-blue gradient sky with
independently tinted cloud detail and a glowing crescent. Pale cream-white kawaii stars
have half their previous radius (30% of the shared default), with the same
positions and count. The two-stop gradient follows `sky-gradient.frag`, as used
by `sakura-drone-conversion.mms`. Palette, stop positions, cloud tint and strength
are editable at the top of `ship_interior.mms`.

The cloud mask is a monochrome spherical panorama generated offline. Its equal
RGB channels contain no baked palette; the shader samples one channel and blends
toward `sky_cloud_color`. Longitude edges match and pole rows are uniform. Noise
is never evaluated in the fragment shader. The old purple nebula generator and
texture remain available but are no longer used by this example.

Regenerate procedural textures:

```sh
cargo run --release --example generate-ship-grass-texture
cargo run --release --example generate-peanut-butter-texture
cargo run --release --example generate-sky-cloud-texture
```

The sky keeps its original 120 m radius. Its former clipping was caused by the
XR camera's old 100 m far plane; both ship cameras now use 1,000 m, adjustable
with `view_far_distance` at the top of the example.

The generators accept an optional output directory after `--`. Palette and seed
constants live in the Rust source so the artwork can be refined and reproduced.

The former barracks is now a grocery/security shop. It reuses the retail shelves
for banana and orange produce bins, peanut butter, toilet paper and three machine
pistols on a dedicated shelf. Pegboard stock and a small checkout replace the
bunks. Its existing inventory accordion still unloads all stock and weapons;
the room shell and overhead spotlights stay lit. Historical `ship_barracks*`
asset names remain for compatibility with the existing scene/controller setup.

Each grill sausage and the vendor tray's sausage and bun are separately
Grabbable. Each part owns its own combined mesh and uses `to_hand(true)` with a
3.5 cm palm offset, the same solved-hand attachment path used by the gun. The
mustard follows the sausage, while the vendor keeps holding the tray. Ordinary
camera-only desktop pointers retain the existing gun-style desktop clearance;
XR/avatar pointers with a solved hand attachment seat the food at that hand.

The bridge, cafeteria and former barracks/grocery shop use warm cream/beige
walls and ceilings, with soft peach in the cafeteria. Cyan and lilac are limited
to object accents such as writing surfaces, gallery guards, table pedestals and
the grocery checkout. Three invisible, softly tinted rose/coral/peach spotlights
sit inside the cafeteria's large window apertures. They shine across the room
with a shallow downward angle, without adding visible light strips or fixtures.

The women's washroom mirror starts paused. In the approach corridor, the inner
zone (Z=12.8, before the door) enables its captures; the outer zone closer to
the cafeteria exit (Z=14.1) freezes them again on the way out. Disabled mirrors
keep displaying their last captured image(s). The global graphics mirror switch
still overrides capture updates. Other mirrors stay enabled by default.

MMS authors can use `Mirror.quality(512).enabled(false) { name="my_mirror" }`
and `query("#my_mirror").set_enabled(true)` / `.set_enabled(false)` at runtime.
This toggles updates, not the mirror surface or its retained textures.
