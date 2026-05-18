# Audio Sources (｡♥‿♥｡)

Status: Spec draft
Date: 2026-05-16

Unified vocabulary for sound-producing nodes in the audio graph. Covers
oscillators and PCM clips under one umbrella term so triggering, scheduling,
and graph wiring can be described once.

Supersedes the source-naming portion of:

- [docs/draft/audio_decoding_thread.md](../draft/audio_decoding_thread.md)
- [docs/task/audio-clip-terminology-and-effect-consolidation.md](../task/audio-clip-terminology-and-effect-consolidation.md)

---

## 1. Terminology

| Term | Meaning | Layer |
|---|---|---|
| `AudioSource` | Umbrella: any playable sound-producing node | architecture |
| `AudioOscillator` | Procedural synthesized source | ECS + MMS |
| `AudioClip` | PCM-backed playable source | ECS + MMS |
| `AudioOutput` | Sink / root output node | ECS + MMS |
| `AudioEffect` | Signal-processing node in the graph | ECS + MMS |
| `AudioClipAsset` | Decoded PCM data in `AudioAssets` registry | engine-internal |
| `MusicNote` | Trigger payload: pitch / velocity / duration | engine + animation |

Rule: avoid "sample" as a public term. "Sample" means a single PCM value or a
verb, never an authored node.

---

## 2. Source variants

| Variant | Component | Backing data | Notes |
|---|---|---|---|
| Oscillator | `AudioOscillatorComponent` | none (synthesized) | gate-driven |
| Clip | `AudioClipComponent` | `AudioClipAsset` in `AudioAssets` | cursor-driven |

Both connect to an `AudioOutputComponent` ancestor to be included in the
compiled DSP graph. Detached sources stay loaded but silent.

---

## 3. Unified trigger intent

One intent triggers any `AudioSource`:

```rust
IntentValue::AudioSchedulePlay {
    component_ids: Vec<ComponentId>,
    beat_offset: f64,
    beat_context: Option<f64>, // set by AnimationSystem during lookahead
    note: Option<MusicNote>,   // pitch/velocity/duration when meaningful
    gain: Option<f32>,         // generic playback gain
    rate: Option<f32>,         // generic playback rate (clips)
    duration: Option<f64>,     // generic stop-after; overrides note.duration
}
```

Naming rationale: "play" is source-agnostic. "note" is oscillator-flavored.
`MusicNote` remains the payload shape when pitch/velocity/duration apply.

Deprecated (oscillator-specific, to be folded in):

- `OscillatorScheduleSetNote`
- `OscillatorScheduleMusicNote`

---

## 4. Per-variant trigger semantics

| Field | Oscillator | Clip |
|---|---|---|
| `note.pitch` | sets oscillator frequency | maps to playback rate (resample) |
| `note.velocity` | sets gain | scales gain |
| `note.duration` | gate ON, then gate OFF after duration | stop playback after duration |
| `gain` (generic) | overrides velocity gain | overrides velocity gain |
| `rate` (generic) | ignored (use pitch) | playback rate; overrides note.pitch mapping |
| `duration` (generic) | overrides note.duration | overrides note.duration |
| no `note`, no fields | gate ON indefinitely | play from cursor=0 to end |

Trigger always resets clip cursor to 0 unless `AudioTriggerMode` says
otherwise (see clip component).

---

## 5. ECS / runtime boundary

Authored components and compiled runtime nodes are allowed to diverge.

| Authored (ECS / MMS)       | Compiled (RT / DSP) |
|---|---|
| `AudioOutputComponent`     | output sink node |
| `AudioOscillatorComponent` | oscillator DSP node |
| `AudioClipComponent`       | `AudioClipPlayer` (cursor + asset ref) |
| `AudioEffectComponent`     | per-kind internal effect node |

`AudioGraphCompiler` owns the lowering. Authored components are scene
vocabulary; runtime structs are chosen for DSP efficiency.

---

## 6. Trigger payload vs config component (｡•̀ᴗ-)✧

`MusicNote` (the struct) is doing two unrelated jobs today. The spec separates them:

| Role | Carrier | Lifetime | Read by |
|---|---|---|---|
| Trigger payload | `IntentValue::AudioSchedulePlay.note` | per-fire | source at trigger time |
| Default tuning | `AudioOscillatorComponent.frequency` (or similar field) | persistent | DSP at compile/init |

### 6.1 How animation triggers audio (today)

```text
KeyframeComponent
  └── ActionComponent { signal: IntentValue::OscillatorScheduleMusicNote { note, ... } }
                                                                          ▲
                                              MusicNote lives inline here ┘
```

The action carries the note. The keyframe owns the action. `AnimationSystem`
emits the intent at the keyframe's beat (with lookahead for audio). Nothing
in this path reads `MusicNoteComponent`.

### 6.2 What `MusicNoteComponent` actually does

`MusicSystem::apply_music_note_to_oscillator` walks an oscillator's subtree,
finds the first `MusicNoteComponent`, converts the note to Hz, and writes it
into the oscillator's `frequency` field once (gated by `music_note_applied`).

That is a one-shot **initial-tuning setter**, not a trigger. It is also not
parallel to `ActionComponent` — actions trigger over time, this just sets a
default at init.

### 6.3 Recommendation: desugar `MusicNote` to an `Action`

Treat `MusicNote.C(4, 1.0)` (and friends) as **MMS sugar** for an
`ActionComponent` whose signal is `AudioSchedulePlay` targeting the parent
source. Not an ECS component at all.

Desugaring:

```text
AudioOscillator {
    MusicNote.C(4, 1.0)
}
```

becomes:

```text
AudioOscillator {
    Action {
        signal = AudioSchedulePlay {
            targets = [<parent oscillator>]
            note = MusicNote.C(4, 1.0)
        }
    }
}
```

The action fires through whatever drives it (keyframe, `OnInit`-style
trigger, manual emission) — same path as every other action.

Reasons:

- removes the "two roles, same name" confusion — `MusicNote` is *only* a
  trigger payload now
- `AudioSchedulePlay.note` becomes the only `MusicNote` carrier in the runtime
- `MusicNoteComponent` and `MusicSystem::apply_music_note_to_oscillator` both
  disappear (no subtree walk, no `music_note_applied` flag, no init-time
  frequency mutation)
- one fewer ECS component to register, encode, inspect
- preserves trigger semantics: a `MusicNote` always means "play this", never
  "silently retune the oscillator's resting frequency"

### 6.4 Firing model

Three ways an authored `MusicNote` can fire:

| Mode | MMS shape | Fires when |
|---|---|---|
| Play on attach | `MusicNote.C(4, 1.0) { play_on_attach() }` | source attaches to graph |
| Manual | `let note = MusicNote.C(4, 1.0); ...; note.play()` | `.play()` is called |
| Scheduled | `note.play(beat)` (optional `f64`) | transport reaches `beat` |

Default is **silent** — no `play_on_attach`, no `.play()` call → nothing
fires. This matches the "no parent keyframe, no implicit firing" rule and
keeps authored intent explicit.

#### `.play()` signature

```text
note.play(scheduled_beat?: f64, audio_source?: SourceRef)
```

Both params are optional and may be supplied at the call site **or**
pre-authored on the component at registration / definition time (see
§6.4.1). Call-site values override pre-authored defaults.

Resolution order for each param:

1. argument passed to `.play(...)`
2. pre-authored default on the note (`target(...)`, `at_beat(...)` in the
   component body)
3. context / topology default (see §6.6 for `audio_source`)
4. for `scheduled_beat`: `None` → fire immediately (beat_offset = 0)
   for `audio_source`: compile error

#### 6.4.1 Pre-authored defaults

A `MusicNote` can carry its target and scheduled beat as part of its
authored definition, not just at the `.play()` call:

```mms
let kick = MusicNote.C(2, 0.5) {
    target("bass")              // pre-bound voice (resolves via §6.6)
    at_beat(transport.next_bar()) // pre-scheduled default
    play_on_attach()             // optional firing trigger
}

kick.play();                    // uses target=bass, beat=next_bar
kick.play(transport.beat()+2);  // overrides beat only
kick.play(8.0, lead_osc);       // overrides both
```

This matches the broader MMS pattern: builder-body sets persistent
config, methods on the live binding override per-call.

Desugaring sketch:

```text
MusicNote.C(4, 1.0) { target("lead") play_on_attach() }
  ↓
Action {
    trigger = OnAttach
    signal  = AudioSchedulePlay { targets = [<lead from context>], note = C4 }
}

note.play()  // pre-authored target="lead", at_beat=t0
  ↓
emit IntentValue::AudioSchedulePlay {
    targets      = [<lead>],
    note,
    beat_context = Some(t0),
    beat_offset  = 0,
}

note.play(beat, source)  // both overridden
  ↓
emit IntentValue::AudioSchedulePlay {
    targets      = [source],
    note,
    beat_context = Some(beat),
    beat_offset  = 0,
}
```

`beat_context` is the absolute beat — same field `AnimationSystem`
already sets during keyframe lookahead.

### 6.5 Transport access from MMS

For `note.play(beat)` to be useful, MMS needs to read the transport's
current bar/beat so authors can write things like "play on the next
downbeat" or "play 2 beats from now".

Proposed surface (names provisional):

```mms
transport.beat()          // current global beat, f64
transport.bar()           // current bar index, integer
transport.beats_per_bar() // time signature numerator
transport.bpm()           // tempo
transport.next_bar()      // beat number of next bar start
transport.next_beat()     // beat number of next whole beat
```

Backed by the existing audio/clock transport (see `audio_transport` /
`Clock` systems). Read-only from MMS for now; transport mutation stays in
engine-driven systems.

Example:

```mms
let kick = MusicNote.C(2, 0.5);
kick.play(transport.next_bar());   // schedule on next downbeat
kick.play(transport.beat() + 4.0); // schedule 4 beats from now
```

Open sub-question: does MMS access transport as a global (`transport.x()`)
or via an injected handle bound at scene load? Global is simpler;
injected handle is friendlier to multi-transport setups if those ever
exist.

### 6.6 Associating a note with an `AudioSource` — `MusicContext` wrapper

Rule: **a `MusicNote` must always resolve to a source**. There is no such
thing as a free-floating note. The question is just where the binding
lives.

Primary mechanism is a **`MusicContext`** wrapper that declares the
available sources for any scope containing notes. Bare construction with
an explicit source ref is the escape hatch.

#### a) `MusicContext` — voices/tracks model

```mms
MusicContext {
    voices {
        bass  = bass_osc
        lead  = lead_osc
        kick  = kick_clip
    }

    Animation {
        Keyframe(0.0) { Action { signal = MusicNote.C(2, 0.5, "bass") } }
        Keyframe(0.5) { Action { signal = MusicNote.E(4, 0.25, "lead") } }
        Keyframe(1.0) { Action { signal = MusicNote.C(2, 0.5, "kick") } }
    }
}
```

Resolution rules inside a `MusicContext`:

| Note construction | Resolves to |
|---|---|
| `MusicNote.C(4, 1.0)` | voice `0` (first declared) |
| `MusicNote.C(4, 1.0, "lead")` | named voice from `voices {}` |
| `MusicNote.C(4, 1.0, 2)` | voice index `2` (shorthand) |
| `MusicNote.C(4, 1.0, some_ref)` | explicit component ref (overrides context) |

Named refs are preferred over integer indices — reordering `voices {}`
won't silently reassign every note. Integer is shorthand for quick
prototyping.

The wrapper isn't `Animation`-specific. It wraps **any keyframe- or
note-bearing scope**, including manual `note.play()` chains. A
`MusicContext` may contain animations, raw actions, or `let`-bound notes
played later by other code.

#### b) Inline child of source (no context)

```mms
AudioOscillator {
    MusicNote.C(4, 1.0) { play_on_attach() }
}
```

Still works. Desugar walks up: nearest `AudioSource` ancestor wins, no
context needed. This is the "one sound, one source" shortcut.

#### c) Bare construction with explicit ref (no context)

```mms
let note = MusicNote.C(4, 1.0, my_osc);
note.play();
```

Third argument is required when there's no enclosing `MusicContext` and
no source ancestor. Compile error otherwise.

#### Resolution precedence

When multiple sources of truth could supply the target:

1. explicit ref in note constructor — always wins
2. enclosing `MusicContext` voice (named or index)
3. nearest `AudioSource` ancestor in parent topology
4. compile error

#### Error case

```mms
let bare = MusicNote.C(4, 1.0);  // no context, no ancestor, no ref
bare.play();                     // compile error: unresolved voice
```

#### Fan-out

`AudioSchedulePlay.targets` stays `Vec<ComponentId>`. Authoring sugar for
fan-out can come later (`MusicNote.C(4, 1.0, ["bass", "lead"])` or a
voice group declared in the context).

#### Why a context wrapper

- matches how musicians/composers already think (tracks/voices)
- removes per-note source plumbing in the common case
- one place to swap instruments (change the voice binding, every note
  follows)
- inspector/editor has a natural unit to show: "this context's voices"
- `let`-bound notes work without `.on()` ceremony

### 6.4 Payload shape lock for `AudioSchedulePlay`

Given §6.3, the intent payload stays hybrid as drafted in §3:

- `note: Option<MusicNote>` when pitch/velocity/duration semantics apply
- generic `gain` / `rate` / `duration` for source-agnostic playback overrides

No `MusicNoteComponent` lookup is involved in trigger dispatch.

---

## 7. Remaining open items (deferred)

- `AudioEffectComponent` consolidation — see terminology task doc §6
- loudness normalization policy — see decode/convert/normalize split task doc
- MMS sugar shape for oscillator default tuning (post §6.3)

---

## 8. Related docs

- [docs/draft/audio_decoding_thread.md](../draft/audio_decoding_thread.md) — decode thread + AudioAssets
- [docs/task/audio-clip-terminology-and-effect-consolidation.md](../task/audio-clip-terminology-and-effect-consolidation.md) — terminology + effect consolidation
- [docs/task/audio-decode-convert-normalize-split.md](../task/audio-decode-convert-normalize-split.md) — decode/convert/normalize layering
- [docs/spec/signals.md](./signals.md) — intent/event model
