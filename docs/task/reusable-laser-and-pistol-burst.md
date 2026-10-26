# Shared laser and held-pistol burst

Date: 2026-10-26

`assets/components/laser.mms` exports `laser(options)` and `laser_burst(options)`.
Both return a table with `root`, `animation`, `stop`, and either `fire_laser` or
`fire_burst`. Attach `root` at the muzzle: local -Z is forward, distances are metres.
Options include `name`, `length`, `flash_size`, `beam_radius` and
`beats_per_second` (default 2, matching Clock.bpm(120)).

Each shot plays an Animation containing two transparent PNG muzzle-flash frames,
an emissive cylindrical beam. Standalone lasers play
`assets/audio/tool/laser_machine_pistol_C.0.wav` at its original playback rate;
`sound=false` suppresses this sound. The asset directory is singular `tool`, not
`tools`. Audio has a separate zero-time keyframe for lookahead scheduling; visual
effects disappear after 40 ms. `stop()` pauses the clip and hides its effects.

`AudioClip.play()` is declared in RuntimeSpec and its bound host operation emits
AudioSchedulePlay directly, with no note, pitch, rate or duration override. This
method bypasses the legacy component-method registry and evaluator. It is gated
to the audio phase during deferred animation evaluation so the sound is not
duplicated by visual evaluation. No MusicNote is constructed or invoked.

The burst plays three independent shot clips at 0, 33.3 and 66.7 ms: about 2.5
frames between shots at 75 fps. Each flash frame lasts one frame at 75 fps;
the beam persists for three frames (40 ms). The first shot is
accurate; later roots have small, increasing deterministic angular offsets.
Independent clips preserve each beam's direction. The complete recording plays
once per accepted burst; its three child lasers are silent. A burst lasts 450 ms,
including the recording's 421 ms tail. Repeated triggers are ignored until its
animation completes. Completion emits a DataEvent only during visual evaluation;
audio lookahead cannot unlock the device early.

`assets/components/laser-machine-pistol.mms` owns a `device_state` table with
firing, cancelled, bursts_fired and ammunition. Ammunition starts null; capacity,
consumption and reloading are not yet implemented. The factory returns the shared
burst API plus this state table.

The display car now uses the shared single-shot prefab. The Sakura pistol uses
the burst. GrabStart and GrabEnd already exist in the engine; MMS now receives
structured pointer/target references (and GrabStart's raycaster, renderable and
world ray). Grabbing enables the holding hand's trigger, releasing disables it
and pauses/hides all three sub-clips. The parent timeline continues with future
visual shots cancelled, so release/regrab cannot bypass the firing lock. Desktop
holding uses Space.

Every shot requests its own event-driven Raycast under the same muzzle/spread
frame as the beam. `parent_forward(true)` preserves local -Z aim even under a
desktop camera; `nearest_hit_only(true)` selects geometric distance instead of UI
priority. These options and request_raycast use direct RuntimeSpec bindings.
Hand pointers retain their existing behavior. `ray_origin_offset` moves only the
ray origin forward along local -Z; Sakura sets it to 6 cm to clear the barrel.
The returned `raycaster` and hit-filter handle refer to the Raycast component;
its separate `raycaster_mount` Transform supplies the origin offset. Calling
request_raycast on that Transform would fail before the shot animation starts.
The Sakura beam radius is 2.5 cm.

Each laser has a weapon-attached muzzle root and a separate reusable world-root
flight slot. Firing copies the muzzle's complete world TRS into that slot before
requesting its cast and playing the shot. Moving the gun after firing therefore
does not move the existing beam, flash or shot ray. Each of the three slots keeps
its own direction, including its spread offset.

The current RuntimeSpec exposes `transform.world_trs()` as a getter and
`transform.world_trs(pose)` as a setter. `transform.trs()`/`trs(pose)` use local
coordinates. The copied pose is a table of translation, rotation_quat_xyzw and
scale arrays; it contains no reference to the source Transform. These methods
dispatch directly through the configured host. The older `.world.trs()` syntax
is not available through the current RuntimeSpec.

MMS RayIntersected payloads expose raycaster, renderable, distance, origin,
direction and hit_point. A laser filters events by its own raycaster, so hand
pointer hits cannot produce weapon impacts. mini-explosion.mms exports
mini_explosion(world_position, beats_per_second), returning root and animation.
The white/yellow sphere is twice its initial authored size, expands, turns yellow,
dims and shrinks, then removes its
subtree at 300 ms. It uses the existing Toon+Emissive bloom-extraction path.
Impacts are world roots, independent of the held gun. Damage is not implemented.

Implementing the returned API exposed an evaluator bug: table-member calls
returned the function instead of invoking it. Table calls now invoke the stored
function with arguments and captured state, with regression coverage.

Automated coverage checks held/unheld and wrong-hand gating, the three ordered
sub-clips, beam visibility, exactly one sound request per trigger, firing lock,
three distinct shot casts, nearest-hit selection under a desktop camera,
impact placement/fade/removal, copied world poses that stay fixed as the gun moves,
75 fps burst spacing, release cleanup,
and the existing Mittens Corp car fixture. Headset grip/effect/audio acceptance
remains manual.
