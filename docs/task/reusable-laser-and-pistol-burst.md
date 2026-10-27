# Tools, muzzle flashes and laser projectiles

Date: 2026-10-26
Status: MMS refactor implemented; VR acceptance pending.

The user maps input to tool use. The tool owns firing policy, sound, muzzle and
flash. A launched laser owns its copied world pose, raycast, short visual lifetime
and hit report. Impact effects and target responses are callbacks.

## Weapon-mounted flash

`assets/components/muzzle-flash.mms` exports `muzzle_flash(options)`, returning
`{ root, animation, play, stop }`. Attach `root` beneath the weapon muzzle and
call `play()` for each shot. `stop()` pauses the clip and hides both sprites.
The flash stays attached as the weapon moves.

Options: `name`, `size` (default 0.35 m), `textures` (two red flash PNGs by default),
`intensity` (8), `frame_seconds` (1/75), and `beats_per_second` (2, matching
Clock.bpm(120)). This asset contains the reusable two-frame animation and has
no input handling, sound, raycast or projectile.

## Independent laser

`assets/components/laser.mms` exports `laser(options)`, returning
`{ root, animation, raycaster, launch }`. Construction initializes a world-root
projectile slot. Do not attach it to the weapon. Local -Z is forward.
Options: `name`, `beam_name`, `length` (40 m), `beam_radius` (0.008 m),
`ray_origin_offset` (0), and `beats_per_second` (2).

```mms
let projectile = laser({ name = "example_laser" length = 40.0 })
projectile.launch({
    pose = muzzle.world_trs()
    source_tool = tool_root
    source_user = avatar
    on_hit = handle_hit
})
```

Launch copies world TRS into the slot, requests a cast and plays its 40 ms beam.
Moving or releasing the weapon afterward does not move the beam or ray. These
are hitscan shots with brief stationary visuals, not travelling projectiles.
Reuse a slot after its animation finishes; concurrent shots need separate slots.
The pistol reserves three, with one stable event handler per slot, rather than
allocating global listeners on every shot.

Each slot has its own event-driven Raycast with `parent_forward(true)` and
`nearest_hit_only(true)`. Its handler accepts only that caster's pending hit;
hand-pointer events cannot produce weapon impacts. `ray_origin_offset` moves
only the cast origin forward along local -Z. Sakura uses 6 cm to clear the barrel.
The actual Raycast handle remains separate from its offset Transform.

`on_hit(hit)` receives `projectile`, `raycaster`, `renderable`, `hit_point`,
`direction`, `origin`, `distance`, `source_tool` and `source_user`. Attribution is
captured at launch so later grabs/releases cannot retag a shot. Misses do not
call it. The laser asset has no sound, flash, damage application or impact effect.

`transform.world_trs()` returns a copied table with `translation`,
`rotation_quat_xyzw` and `scale` arrays; `world_trs(pose)` sets it. `trs()` and
`trs(pose)` use local coordinates. These are direct RuntimeSpec host methods;
the older `.world.trs()` syntax is unavailable through the current RuntimeSpec.

## Pistol tool and input adapter

`assets/components/laser-machine-pistol.mms` exports
`laser_machine_pistol(options)`, returning
`{ root, animation, semi_animation, auto_animation, device_state, use_start, use_end, flash, mode_selector, set_mode }`. `animation` is the finite Burst clip.

```mms
let pistol = laser_machine_pistol({
    name = "my_pistol" length = 40.0 flash_size = 0.28
    beam_radius = 0.025 ray_origin_offset = 0.06
    on_hit = handle_laser_hit
})
// Attach pistol.root beneath the weapon muzzle.
// The scene input adapter supplies the current user:
pistol.use_start({ user = avatar })
// On release/drop:
pistol.use_end()
```

`use_start` accepts a fresh trigger press when idle. Semi (the default) fires
one shot; Burst fires three at 0, 33.3 and 66.7 ms, with increasing deterministic
spread. Semi plays `laser_machine_pistol_A.0.wav`; Burst plays
`laser_machine_pistol_C.0.wav`. Both retain the 450 ms firing lock.
Holding the trigger does not repeat either mode.
Each shot samples its aim frame's current world pose and restarts the flash.
`use_end()` cancels pending shots and hides the flash; launched projectiles finish
independently. Release/regrab cannot bypass a finite clip's lock.

Auto runs at 10 shots/s, spaced 100 ms apart, instead of Burst's unchanged
33.3 ms spacing. Its native three-shot loop lasts 300 ms,
with no pause between cycles. Trigger release/drop pauses it immediately.
Every visual `AutoCycle` event plays `laser_machine_pistol_B.0.wav`; keeping this
outside audio lookahead prevents scheduling an extra recording after release.
Mode changes away from a running Auto loop stop it and require a new press.

`device_state` holds `firing`, `cancelled`, `trigger_held`, `mode`, `burst_mode`,
`bursts_fired`, `shots_fired`, `ammunition`, `user`, `requested_user` and `tool`.
Ammunition starts null; capacity, consumption and reloading are not implemented.
Without `on_hit`, the tool uses a mini explosion as its default impact policy.
Sakura supplies its own callback for target routing.

The Sakura scene gates use on GrabStart/GrabEnd and the holding hand's trigger;
desktop holding uses Space. These scene adapters translate input into tool
requests. `AudioClip.play()` uses direct RuntimeSpec AudioSchedulePlay at the
recording's original rate; no MusicNote is used. Audio lookahead cannot duplicate
visual shots or advance counters/the lock. Shot accounting and completion use
DataEvent during visual evaluation.

The controlled display car composes the same separate flash, sound and laser
assets. It supplies its mounted rider as `source_user` and retains single-shot
input behavior.

[HapticActuator.xr() and recoil routing](haptic-actuator-xr-and-tool-recoil.md)
are proposed separately; there is no controller motor output in this slice.

## Provisional targets and impact policy

`assets/components/targets/metal-can.mms` exports `metal_can(options)`, returning
its root component with `on_hit(hit)` and `hit_count()` function fields. The same
`on_hit` function is assigned to the body and both lid renderables. It returns
true only for this can's surfaces, increments private `state.hits`, stores
`state.last_hit`, plays
a short wobble/color animation, spawns a floating hit number and optionally calls
the factory option `on_hit(hit, state)`. Cans have no permanent text labels. `hit-number.mms` creates
an independent world-root "1" at the impact point facing the launch origin. A
Transition lifts/shrinks it over 0.6 seconds, then its Animation removes the
entire subtree. Each hit creates a separate effect. "1" currently means one hit;
damage amounts and health are not defined yet.
This is an MMS experiment, not a new Damageable engine component.

In `examples/sakura-drone-conversion.mms`, `handle_laser_hit` spawns an independent
mini explosion at the world hit point and calls the hit renderable's optional
`on_hit` function field. There is no target-list search or implicit inheritance.
The matching can's callback updates the panel's total hit count.
The report preserves shooter/tool attribution for future damage policy. No holes,
fragments, health system, physics knockback or destruction are added. See the
[deferred damage/CSG proposal](damageable-surfaces-csg-and-fragments.md).

`mini-explosion.mms` remains a separate factory returning `root` and `animation`.
Its emissive sphere expands white/yellow, dims and shrinks, then removes its
subtree at 300 ms, using the existing Toon+Emissive extraction path.

## Validation

Focused tests cover held/wrong-hand gating, one sound per accepted burst, three
casts/clips and their spacing, firing lock, tool/user attribution, target-specific
reactions, transient number motion/removal and the panel counter, fixed world projectile poses, and release
stopping the flash without stopping launched beams. The existing Mittens Corp
car fixture exercises the new composition too.

Component-reference equality now compares opaque component handles, independent
of dispatch type labels. An authored handle, query result and event reference
to the same live component therefore compare equal. A live-world callback test
covers routing a queried can surface into its target handler.

The Vulkan scene test verifies all 18 can surfaces have the shared custom metal
material and matcap texture, casts through the real BVH to an individual can,
and renders the bench and target stand with the existing stereo views. VR grip
alignment, flash motion, beam readability and target feedback remain manual.

## Receiver mode selector and automatic firing

The returned `mode_selector` is a Selection with Semi, Burst and Auto stacked
vertically. Mount it independently of the muzzle effects. Sakura places it on
the right receiver face, with click-only raycasts and Grabbable.off so the other
hand can change mode while the holding hand keeps its grip. A `#` marks the active
mode. All options use pastel pink backgrounds and black text. Their root scale
is now 0.0018 (another 40% reduction from 0.003); each option is 2.52 cm wide.
The mount remains at held-local `[0.028, 0.085, 0.0402]` m, including the earlier
rearward shift. `set_mode("semi" | "burst" | "auto")` exposes the same policy.
Releasing the other hand's pointer trigger leaves firing alone.

`RayIntersected` retains surface/ancestor and global delivery, and also reaches
a scoped listener on the casting component itself, once. Projectile slots use
this route, so idle pistol slots do not enter MMS for unrelated pointer hits.
The three Sakura pistols each keep independent modes, animations and input state.

Deferred audio work: add reverb and compression audio units before shaping the
last automatic shot's ring-out. The desired effect is that the final shot's
reverb/tail becomes audible as the firing stops driving the compressor. Keep
this separate from the current B/C recording selection; no effects are added yet.
