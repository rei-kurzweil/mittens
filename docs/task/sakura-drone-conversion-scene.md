# Sakura drone conversion: rooftop ceramic city

Date: 2026-10-26
Status: scene authored; headset acceptance pending.

Run `cargo run --release -- load examples/sakura-drone-conversion.mms`.
The obvious switches at the top select desktop preview, XR, or both. Default is
XR only to avoid rendering a desktop view. Desktop preview is a free camera;
XR uses Rei(mu)'s tracked rig, hand lasers/pointers, arm/crouch IK, mouth amplitude,
eye closure/idle gaze, secondary motion, collisions and CapsuleReady-gated gravity.
HandTracking.new().source("openxr") on the avatar opts its mapped finger bones
into OpenXR hand tracking.
XR B jumps, allowing access over low parapets. Spawn is on the 25.6 m roof.

Six eight-storey towers and three lower annexes use the same ceramic shader,
packed texture and defaults as the youtuber pool. pool_tile_material() now holds
those shared defaults; indoor_pool uses it too. All buildings have real floor
slabs, open window bays and balconies without glass. The spawn roof has a real
stairwell opening and stairs to its top floor. Floors are otherwise empty; there
is no complete stair network or furnished building interior yet.

Three instances of assets/models/sakura-machine-pistol.glb sit on a separate
table to the left of the spawn, at `[-20.3, 25.6, 0.2]`. The target workbench
retains its cans, lamp and parts tray. The original GLB is 6.593 m long along Z;
pistol_scale=0.085 makes it about 56 cm. Each pistol lies on its side above the
mat and preserves its textures and [Grabbable.to_hand(true)](grabbable-to-hand.md)
grip frame. All three independently select Semi (one shot), Burst (three), or
continuous Auto using three smaller, vertically stacked pink/black receiver
options. See [shared laser and firing modes](reusable-laser-and-pistol-burst.md).
Two display_car prefab instances sit on the road below. The conversion interaction
itself is still pending.

There are no Lumae NPCs by default (`initial_lumae_count = 0`). A ceramic pedestal
at `[-17, 25.6, 1.2]` has an Add Lumae button and count. Each click attaches another
avatar with the relaxed A-pose, Lumae collider profile and all 19 hair chains.
Nine safe roof anchors cycle along the sides and rear of the roof, with fresh
pseudo-random jitter of up to 20 cm on each axis. These keep arrivals clear of
the firing lane, stair opening, weapon table and planter. Additional clicks keep
adding avatars; revisited anchors can eventually overlap. The startup count is
also used by the CPU probe for repeatable three-avatar comparisons.

Six shootable metal cans are now on the roof: three on the workbench and three
on a small stand beside it. Bodies are lime green and lids white, sharing the
pool handrails' chrome matcap shader/texture through `matcap_metal()`. Explicit
mesh bounds make all body/lid surfaces BVH raycast targets at startup. Hits
trigger a short wobble/color response, an independent mini explosion, a temporary
floating "1" and a running total on the scene panel. There is no text attached
to the cans; each hit number rises/shrinks and removes itself after 0.6 seconds.
Target hit routing is ordinary MMS; destruction
and physics knockback remain deferred.

The pistol's flash stays attached to its muzzle while each beam copies a world
pose and finishes independently. The scene routes trigger input through
`use_start({ user })` and release/drop through `use_end()`. Its `on_hit` callback
receives projectile, tool and user identity and calls the hit surface's optional
`on_hit` function field. Each can publishes its shared handler on the root, body
and lids, with a `hit_count()` getter on the root. There is no six-can dispatch loop.

The ground is a muted pinkish grey-brown. The player building has separate
`sakura_rooftop_building_floor_0` through `_floor_7` sections and a separate
`sakura_rooftop_building_roof`. Each section has a local transform, white ceramic
and trim CombineMesh outputs, and its own collision boxes. Columns are split at
storey boundaries; the stairs belong to floor 7, while the roof slab/opening and
parapets belong to the roof section. This bounds future CSG edits/rebuilds to the
affected floor and tint group. A regression edits one floor's source geometry
and verifies that every other output retains its render handle.

Only the player building passes `per_floor = true` to `pool_building`; the eight
neighbors retain two CombineMesh outputs each, regardless of detail level. There
are 18 player-building outputs and 16 neighbor outputs. Tile textures and
filtering are attached to the combined outputs. The
engine retains those bindings when a combined mesh is published again and reuses
identical immutable combined geometry across regenerations. Building CombineMesh
roots now use keep_transforms and Raycastable.enabled: individual source boxes
remain registered in the BVH as spatial targets while only the combined meshes
draw. Rays hit individual slabs, walls, columns and rails; windows and stairwell
openings do not become a solid building-sized bounding box. Simplified neighbors
retain the intentional solid lower block and detailed top floors.

The example-local info panel has a 0–8 slider for fully generated neighboring
buildings. Other neighbors have a solid lower block and two detailed top floors.
The player's tower is outside the regenerated subtree and stays unchanged. The
same panel has a scene bloom toggle; the general graphics panel does not yet
expose bloom. Slider regeneration preserves ceramic textures and retires old
visual outputs; revisiting warmed detail configurations reuses mesh assets.

The invisible teleport_pit has visual="none". Its existing -10 m internal sensor
offset is accounted for: the trigger spans y=0..4 m across the block, returning
falling XR movers to the rooftop before street contact and resetting velocity.
The destination is a capsule centre at roof+1.2 m; the prefab applies the mover
offset to position the locomotion root above the slab.
It intentionally also catches descent through the lowest floor. No spikes are
spawned. Landing on an intermediate balcony/floor does not trigger a reset until
the mover reaches the street-level trigger.

## Reusable sky

`assets/components/backgrounds/gradient_skybox.mms` exports:

- gradient_skybox(low_color, high_color, low_point, high_point): plain Background,
  independent of scene lighting, with sky-gradient.frag and typed vec3/f32 inputs.
  Stop positions are the normalized viewing direction's Y: -1 below, 0 horizon,
  +1 overhead. Between stops the blend has smooth endpoints; outside it clamps.
- background_sun(direction, angular_radius, color, intensity): a separate emissive
  sphere in Background.occlusion_and_lighting, which enables the existing bloom
  extraction path. Emission bypasses surface lighting. Foreground depth masks the
  source in extraction. Radius is in radians; direction is normalized by the prefab.

Both stay in the background as the camera translates. No renderer changes were
needed for this sky/sun setup. [Broader bloom pyramids](opt-in-bloom-pyramid.md)
are recorded as a later opt-in feature, with unchanged defaults.

## Sakura trees

The [transparent Noto sprite](../../assets/textures/cherry-blossoms/sakura-sprite.md)
is generated by the retained Rust example generate-sakura-sprite. It is one
256×256 RGBA glyph, not the existing opaque blossom-pattern background. Three
crossed cutout planes replace every terminal leaf sphere in the shared tree
prefab: 27 endpoints / 81 planes per tree, without per-frame billboarding. The
Sakura scene has one rooftop planter and two street planters; examples/rei(mu).mms
also uses the upgraded tree in its existing garden placement.

## Validation

- Release build, Rust generator and formatting checks pass.
- Both desktop and XR authoring paths bootstrap; six towers, two car imports,
  three pistol imports and no default Lumae, sky types/background modes and all leaf sprites are checked.
- Physical window and stairwell openings are checked against authored collision
  boxes, along with rooftop solidity and the invisible reset trigger placement.
- The reset callback requests the correct locomotion-root position with the
  capsule offset and clears fall velocity.
- The Rei(mu) example bootstraps with the shared tree; the sprite is exactly
  256×256 with opaque, transparent and antialiased pixels.
- The shared pool-material extraction preserves the existing youtuber scene's
  three view modes and material registration/update tests.
- Vulkan renders both eyes for rooftop, city, bench, targets, tree and sky views; verifies
  ceramic textures, the sky's typed shader, blossom cutout routing and sun bloom.
  The Vulkan fixture also clicks the pedestal to load three NPCs and renders the
  weapon table, pedestal, graphics panel and three receiver options. Review
  images are written to /tmp/sakura-*.png.

VR headset acceptance remains manual. Broad bloom pyramids remain deferred.

2026-10-26 VR tuning: the shared Rei(mu)/Bisket spring profile now uses a
0.103125 m head sphere (75% of its prior 0.1375 m radius) and 0.09 m hand spheres
(twice the prior 0.045 m radii). These are SpringCollider volumes in
assets/components/colliders/bisket.mms. The pistol has a 5 cm rearward held-model
offset, with compensation in its bench pose. Subsequent VR tuning put the held
grip offset at `[0, 0, -0.105]` m; the separate authored model offset remains.

The [2026-10-26 CPU profile](sakura-rooftop-cpu-profile.md) compares the full city,
three running/paused Lumae NPCs, disabled contacts and active hit/miss rays.
Its reusable headless probe is `examples/profile-sakura-cpu.rs`. Measured contact
and BVH traversal are small; repeated whole-scene transform dependency scans and
MMS hit delivery are the first CPU follow-up targets. GPU/live-XR timing remains
separate from these measurements.
