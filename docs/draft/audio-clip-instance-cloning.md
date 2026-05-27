# AudioClip Instance Cloning (｡♥‿♥｡)

Status: Draft
Date: 2026-05-22

Extends [`docs/spec/audio-sources.md`](../spec/audio-sources.md) with a
multi-instance authoring model: one decoded clip, many independent
playheads, each with its own beat-windowed start/stop (and later pitch,
rate, warp).

Related:

- [docs/spec/audio-sources.md](../spec/audio-sources.md) — parent spec, terminology + trigger intent
- [docs/draft/audio_decoding_thread.md](./audio_decoding_thread.md) — decode pipeline + asset registry
- [docs/draft/mms-records-and-rust-interop.md](./mms-records-and-rust-interop.md) — mms ↔ rust binding mechanics

---

## 1. Motivation (◕‿◕)

Today `AudioClipComponent` is 1:1 with a single runtime playhead. The
RT side keys both the asset *and* the per-voice cursor by the
component's id, so authoring "two voices of the same kick" requires
either re-decoding the file as a separate component or splitting time
on a single voice. Neither matches how authors think:

- a drum machine fires one decoded kick many times per bar, overlapping
- a sliced loop wants the same buffer with different in/out windows
- ambient beds want a second voice offset N beats for thickness

What's missing is an explicit split between the **shared decoded
asset** and the **per-voice playhead** at the authoring layer.

---

## 2. Terminology delta vs `audio-sources.md` §1

| Term | Layer | Role |
|---|---|---|
| `AudioClipAsset` | engine-internal | decoded PCM, keyed by URI. Already exists |
| `RtClipSource` | RT | shared handle over the PCM buffer. Refcounted by live instances |
| `RtClipInstance` | RT | per-voice playhead (cursor, playing, gate). Many-per-source |
| `AudioClipComponent` | ECS + MMS | authored voice. Owns a URI **and** per-instance config. Cloning produces another component sharing the same `RtClipSource` |

A clone at the ECS layer is just another `AudioClipComponent` with the
same URI plus a `source_component` back-reference. At the RT layer, the
asset is reused (URI-deduped); only a fresh `RtClipInstance` is
allocated.

---

## 3. MMS surface

```mms
let kick      = AudioClip.wav("assets/kick.wav");
let kick_late = kick.instance() {        // shares RtClipSource
    start_beat(0.25)                     // begin playhead 1/4-beat in
    stop_beat(1.0)                       // stop at beat 1.0
};
```

- `kick.instance()` returns a new `AudioClipComponent`-typed binding.
  Spawn flows through `component_registry::spawn_tree`
  (`src/meow_meow/component_registry.rs:62`) using a ctor variant that
  takes the source binding instead of a URI string.
- The instance inherits the URI from its source; trigger mode and
  beat window are per-instance defaults.

### Body / chain methods on the instance

| Version | Method | Effect |
|---|---|---|
| v1 | `start_beat(f64)` | initial PCM cursor (in beats relative to trigger) |
| v1 | `stop_beat(f64)` | hard cutoff (in beats relative to trigger) |
| v1 | `trigger_mode(...)` | reuses existing `AudioTriggerMode` (Retrigger / OneShot / Latched) |
| v2 | `pitch(semitones)` | resample-pitch the playback |
| v2 | `rate(f32)` | playback speed multiplier |
| v3 | `warp(algo)` | time-stretch algorithm (Repitch / Elastique-style / PaulStretch / …) |

`start_beat` / `stop_beat` are expressed in **transport beats** —
same scale as `transport.beat()` and `AudioSchedulePlay.beat_context`
(see `audio-sources.md` §6.5). They describe a window *relative to the
trigger's fire beat*, not absolute transport time. The trigger itself
fires via the existing `AudioSchedulePlay` intent — no new intent
needed.

---

## 4. Beat-windowed playback semantics

Per-trigger evaluation:

```
fire_at  = beat_context + beat_offset                  // from AudioSchedulePlay
play_at  = fire_at                                     // trigger fires immediately
cursor0  = beats_to_samples(start_beat, bpm, sample_rate)
stop_at  = fire_at + (stop_beat - start_beat)          // when stop_beat is set
```

- `start_beat` shifts the **initial cursor** into the buffer; it does
  **not** delay the trigger. Authors who want delayed playback use the
  trigger's existing `beat_offset` field.
- If both `stop_beat` and the trigger's `duration` are set, the
  effective stop is the **earlier** of the two — authored windows act
  as floors:

  ```
  effective_duration = min(stop_beat - start_beat, duration)
  ```

- If `start_beat` lands past the end of the PCM, the trigger is silent
  (no underflow, no wrap).

---

## 5. ECS / RT implementation sketch (not normative)

Just enough to confirm the surface is implementable; full spec when
this graduates.

### `AudioClipComponent` field additions

```rust
pub struct AudioClipComponent {
    pub uri: String,
    pub trigger_mode: AudioTriggerMode,
    pub load_state: AudioClipLoadState,
    // new:
    pub source_component: Option<ComponentId>, // Some => clone of another clip
    pub start_beat: Option<f64>,
    pub stop_beat: Option<f64>,
    component: Option<ComponentId>,
}
```

### RT-side wiring

- `RegisterAudioClip` (handled in `audio_system.rs` / `audio_system_fundsp.rs`):
  - If `source_component` is `None`: same as today (decode if URI not
    already in `clip_assets`, allocate fresh `RtClipInstance`).
  - If `source_component` is `Some(src)`: look up the source's asset by
    URI, **skip decode**, allocate a fresh `RtClipInstance` keyed by
    *this* component's FFI id.
- `clip_assets: HashMap<u64, RtClipAsset>` (`audio_system_fundsp.rs:441`):
  the existing per-URI dedup is what makes the source shared. Confirm
  the key is URI-derived (not component-id-derived) and tighten if not.
- `render_sample_from_clips` (`audio_system_fundsp.rs:938-997`): already
  iterates per-instance. Only change is honoring `start_beat` when
  initializing the cursor on trigger, and `stop_beat` when checking
  end-of-window.

### Lifetime

- `RtClipSource` is refcounted by the number of live `RtClipInstance`s
  pointing at it, **not** by an "original" component. Removing the
  original `AudioClipComponent` does not stop the clones; the asset
  stays loaded until the last instance is gone.

---

## 6. Authoring examples

### Slice loop, two voices

```mms
let loop_src = AudioClip.wav("assets/break.wav");
let slice_a  = loop_src.instance() { start_beat(0.0) stop_beat(2.0) };
let slice_b  = loop_src.instance() { start_beat(4.0) stop_beat(6.0) };
```

### Drum machine — one decoded kick, N steps

```mms
let kick = AudioClip.wav("assets/kick.wav");
for step in [0, 1, 2, 3] {
    kick.instance() {
        play_on_attach()
        // trigger system handles the per-step beat scheduling
    };
}
```

### Ambient bed thickening

```mms
let bed   = AudioClip.wav("assets/pad.wav").latched();
let bed_b = bed.instance() { start_beat(8.0) trigger_mode(latched) };
```

---

## 7. Versioning

| Version | Adds |
|---|---|
| v1 | `.instance()`, `start_beat`, `stop_beat`, per-instance `trigger_mode` |
| v2 | `pitch(semitones)`, `rate(f32)` |
| v3 | `warp(algo)` — time-stretch enum |

v1 is implementable on top of the current decode + RT clip
infrastructure with only the field/registration changes in §5. v2/v3
require resampling and warping in the RT render path.

---

## 8. Open questions

- **Chained clones.** `bed_b.instance()` — does it work? Recommend:
  yes, all flatten to the same `RtClipSource` (URI is the join key);
  cloning is structurally flat at the RT layer regardless of authoring
  chain depth.
- **Naming.** `.instance()` vs `.clone()` vs `.voice()`. Recommend
  `.instance()` — `.clone()` collides with Rust semantics in author
  mental models; `.voice()` reads well but conflicts with
  `MusicContext.voices {}` in `audio-sources.md` §6.6.
- **Trigger ergonomics.** Should `kick.instance().play(beat)` work
  directly, or must it go through a `MusicNote` / `AudioSchedulePlay`?
  Leaning toward direct `.play()` on the clip instance as sugar that
  desugars to `AudioSchedulePlay { component_ids: [self], beat_offset, ... }`.
- **Per-instance gain / pan.** Out of scope here; lives on the existing
  `AudioGainComponent` topology. Worth a separate note if attach-order
  ergonomics get awkward.
- **`start_beat` past EOF.** Spec'd as silent (§4); confirm this matches
  what `OneShot` / `Latched` expect (probably yes — the cursor never
  advances so latched state is "still waiting").

---

## 9. Non-goals

- Streaming clips (decode-as-you-play). Whole-buffer assumption holds.
- Per-instance file overrides. An instance is always the same PCM as
  its source — different file means a different `AudioClip`.
- Crossfading / fade envelopes. Belongs in a separate effect or
  envelope spec.
