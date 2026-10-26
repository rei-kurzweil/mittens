# Shared laser and held-pistol burst

Date: 2026-10-26

`assets/components/laser.mms` exports `laser(options)` and `laser_burst(options)`.
Both return a table with `root`, `animation`, `stop`, and either `fire_laser` or
`fire_burst`. Attach `root` at the muzzle: local -Z is forward, distances are metres.
Options include `name`, `length`, `flash_size`, `beam_radius` and
`beats_per_second` (default 2, matching Clock.bpm(120)).

Each shot plays an Animation containing two transparent PNG muzzle-flash frames,
an emissive cylindrical beam, and `assets/audio/tool/laser_machine_pistol_C.0.wav`
at its original playback rate. The asset directory is singular `tool`, not
`tools`. Audio has a separate zero-time keyframe for lookahead scheduling; visual
effects disappear after 90 ms. `stop()` pauses the clip and hides its effects.

`AudioClip.play()` is declared in RuntimeSpec and its bound host operation emits
AudioSchedulePlay directly, with no note, pitch, rate or duration override. This
method bypasses the legacy component-method registry and evaluator. It is gated
to the audio phase during deferred animation evaluation so the sound is not
duplicated by visual evaluation. No MusicNote is constructed or invoked.

The burst plays three independent shot clips at 0, 120 and 240 ms. The first is
accurate; later roots have small, increasing deterministic angular offsets.
Independent clips preserve each beam's direction. Calling fire again restarts
the sequence. These are visual/audio effects; hit and damage logic is separate.

The display car now uses the shared single-shot prefab. The Sakura pistol uses
the burst. GrabStart and GrabEnd already exist in the engine; MMS now receives
structured pointer/target references (and GrabStart's raycaster, renderable and
world ray). Grabbing enables the holding hand's trigger, releasing disables it
and stops the burst plus all three sub-clips. Desktop holding uses Space.

Implementing the returned API exposed an evaluator bug: table-member calls
returned the function instead of invoking it. Table calls now invoke the stored
function with arguments and captured state, with regression coverage.

Automated coverage checks held/unheld and wrong-hand gating, the three ordered
sub-clips, beam visibility, exactly one sound request per shot, release cleanup,
and the existing Mittens Corp car fixture. Headset grip/effect/audio acceptance
remains manual.
