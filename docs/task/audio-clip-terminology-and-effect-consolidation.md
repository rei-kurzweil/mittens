# AudioClip Terminology and Audio Effect Consolidation

Date: 2026-04-29

This task note records two related audio design decisions:

1. use **`AudioClip`** as the primary term for PCM-backed playable audio at all levels
2. consolidate the current many-audio-effect-component model into a smaller unified effect model

This is a docs/task note only. No `src/` changes are proposed here yet.

Related docs:

- [docs/draft/audio_decoding_thread.md](../draft/audio_decoding_thread.md)
- [docs/task/audio-decode-convert-normalize-split.md](./audio-decode-convert-normalize-split.md)

---

## 1. Terminology direction

The current draft uses "sample" heavily:

- `AudioClipComponent`
- `AudioClipPlayer`
- `LoadClip`

That is workable internally, but it is not the best top-level term for MMS authors or for
engine architecture docs because "sample" is overloaded:

- a decoded PCM asset
- a single PCM sample value
- a runtime playback source
- a verb ("sample a signal")

The preferred top-level term should be:

- **`AudioClip`**

That term is clearer for authored scenes, for animation/event triggering, and for engine
architecture docs.

---

## 2. Terminology table (target for docs/spec)

We should add a terminology table in `docs/spec` later. The intended vocabulary is:

| Term | Meaning | Audience |
|---|---|---|
| `AudioSource` | Any playable sound-producing source | umbrella / architecture |
| `AudioClip` | PCM-backed playable source | MMS + engine |
| `AudioOscillator` | Procedural synthesized source | MMS + engine |
| `AudioOutput` | Sink/root output node | MMS + engine |
| `AudioEffect` | Signal-processing node applied in the audio graph | MMS + engine |
| `AudioClipAsset` or decoded clip data | Decoded PCM asset data in memory | engine-internal |
| `MusicNote` | Trigger payload describing pitch/velocity/duration semantics | engine + animation |

Recommended rule:

- **Clip** = playable PCM-backed source node
- **Sample** = avoid as the primary public/source term
- **AudioSource** = umbrella category that includes `AudioClip` and `AudioOscillator`

---

## 3. Unified play intent

The audio direction should use one scheduled play intent for any audio source.

Current engine state is still oscillator-specific:

- `OscillatorScheduleSetNote`
- `OscillatorScheduleMusicNote`
- `OscillatorSetPitch`
- `OscillatorSetEnabled`

But the planned direction should be:

- one play/schedule intent for any `AudioSource`

Recommended intent naming direction:

```rust
IntentValue::AudioSchedulePlay { ... }
```

Why not `AudioScheduleNote` as the top-level name:

- "note" is natural for oscillators
- "play" is more natural for clips
- the unified concept is "trigger this audio source", not "all audio is a note"

`MusicNote` can still remain the pitch/velocity/duration payload shape when that is the
right trigger model. The top-level intent name should stay source-agnostic.

---

## 4. Current component situation

Current audio graph-related ECS components include:

- `AudioOutputComponent`
- `AudioOscillatorComponent`
- `AudioGainComponent`
- `AudioMixComponent`
- `AudioLowPassFilterComponent`
- `AudioBandPassFilterComponent`
- `AudioHighPassFilterComponent`
- `AudioLimiterComponent`
- `AudioBufferSizeComponent`
- `MusicNoteComponent`

This is already too granular for authored MMS and likely too fragmented for the long-term
audio graph model.

The effect components in particular are overly exploded.

---

## 5. Proposed source model

Source-side terminology should become:

- `AudioOutput`
- `AudioOscillator`
- `AudioClip`

Conceptually:

```text
AudioSource
  ├── AudioOscillator
  └── AudioClip
```

Recommended component direction:

```rust
AudioOutputComponent
AudioOscillatorComponent
AudioClipComponent
AudioEffectComponent
AudioMixComponent   // maybe kept separate; see open questions
MusicNoteComponent  // maybe kept as a trigger/config payload
```

Important boundary:

- in **ECS / World authored topology**, there should be one `AudioEffect` /
  `AudioEffectComponent`
- inside **AudioSystem**, **AudioGraphCompiler**, and the RT thread, the engine can compile
  those authored nodes into whatever internal structs/enums it wants

So this task is about simplifying the authored/component-world model. It does **not**
require the runtime audio thread to use the exact same struct shape.

---

## 6. Proposed effect consolidation

Instead of many effect component types:

- `AudioGainComponent`
- `AudioLowPassFilterComponent`
- `AudioBandPassFilterComponent`
- `AudioHighPassFilterComponent`
- `AudioLimiterComponent`

we should move toward one unified effect component:

```rust
pub struct AudioEffectComponent {
    pub effect_type: AudioEffectType,

    // shared / union-style parameter surface
    pub gain: Option<f32>,
    pub cutoff_hz: Option<f32>,
    pub center_hz: Option<f32>,
    pub bandwidth_octaves: Option<f32>,
    pub resonance: Option<f32>,
    pub threshold: Option<f32>,
    pub attack_ms: Option<f32>,
    pub release_ms: Option<f32>,
}
```

With:

```rust
pub enum AudioEffectType {
    Gain,
    LowPass,
    BandPass,
    HighPass,
    Limiter,
}
```

The user compared this to `Style { ... }`, which is the right mental model:

- one component type
- one broad parameter surface
- only some fields are relevant depending on the selected effect type

That is a good direction for MMS ergonomics.

The compiled/runtime side can still lower that into internal node-specific structs such as:

- gain node state
- low-pass node state
- band-pass node state
- high-pass node state
- limiter node state

The important point is:

- **World/ECS surface:** one `AudioEffectComponent`
- **Compiled/runtime surface:** internal effect-node structs/enums are fine

---

## 7. Recommended MMS surface

Recommended authored shape:

```mms
AudioEffect.gain() {
    gain(0.8)
}

AudioEffect.low_pass() {
    cutoff_hz(1200.0)
    resonance(0.6)
}

AudioEffect.band_pass() {
    center_hz(1800.0)
    bandwidth_octaves(1.2)
    resonance(0.4)
}

AudioEffect.limiter() {
    threshold(0.8)
    attack_ms(5.0)
    release_ms(80.0)
}
```

This preserves:

- explicit effect kind
- familiar builder/body call style
- one conceptual effect component in the language

An alternative style would be:

```mms
AudioEffect {
    type = "low_pass"
    cutoff_hz(1200.0)
    resonance(0.6)
}
```

But constructor-selected variants are probably cleaner and more aligned with existing MMS
component vocabulary.

---

## 8. Why consolidate

Benefits:

- simpler MMS vocabulary
- smaller component registry surface
- easier docs and examples
- easier future query/editor tooling
- easier to build a unified audio graph UI/inspector

Potential downside:

- one component carries many fields that are irrelevant for some effect kinds

This downside applies at the authored/ECS layer only. It does not mean the RT thread must
carry one giant union-like runtime struct. The runtime can and probably should compile the
authored effect into a smaller typed internal node representation.

That downside is acceptable if:

- irrelevant fields are clearly documented
- encode/decode/MMS emission only includes relevant fields
- runtime validation is explicit

---

## 9. Phase split

### Phase 1

Focus on:

- `AudioClip` terminology
- unified source model
- decode / convert split
- one play intent for any audio source
- effect component consolidation design

### Phase 2

Focus on:

- loudness normalization policy
- where normalization gain lives
- whether normalization is destructive or metadata-only

Normalization should not block Phase 1 source/effect cleanup.

---

## 10. Open questions

### 10.1 `AudioClip` everywhere, or only in MMS?

Recommended answer:

- use `AudioClip` everywhere user-facing and in engine architecture docs
- reserve more technical/internal names only for lower-level PCM data structures if needed

### 10.2 Should the internal asset/data structure also be called `AudioClip`?

Possible choices:

- `AudioClip`
- `AudioClipAsset`
- `DecodedAudioClip`

Recommendation:

- scene/ECS/playable node: `AudioClipComponent`
- cached decoded asset data: `AudioClipAsset` or `DecodedAudioClip`

That keeps node-vs-data distinction visible.

### 10.3 Should `AudioMixComponent` also be consolidated into `AudioEffectComponent`?

This is not obviously the same kind of thing.

`AudioMix` is graph-topology / summing metadata, not a normal one-input effect.

Likely answer:

- keep `AudioMixComponent` separate

### 10.4 Should `AudioBufferSizeComponent` remain separate?

Likely yes.

It configures output/runtime behavior, not a source/effect node.

### 10.5 Should `MusicNoteComponent` remain separate?

Open question:

- is `MusicNoteComponent` a useful authored component in the long run?
- or should note payloads live only in intents/actions/animation triggers?

This can stay separate from clip/effect consolidation for now.

### 10.6 Constructor-selected effect type or `type = ...` field?

Options:

- `AudioEffect.low_pass() { ... }`
- `AudioEffect { type = "low_pass" ... }`

Recommendation:

- constructor-selected type is cleaner for MMS

### 10.7 Union-style fields or nested per-type config?

Options:

- one flat union-style parameter surface
- enum with typed payload structs internally

Recommendation:

- MMS surface can be flat / style-like
- Rust runtime can still use a typed enum internally if that is cleaner

So we should not force the ECS storage layout to mirror the authored surface exactly.

### 10.8 One unified `AudioEffectComponent` in ECS/world, or only a front-end alias?

Two implementation paths:

1. real ECS/world consolidation
   - replace many authored world components with one `AudioEffectComponent`
   - compile that into internal audio-system node structs/enums later
2. MMS/front-end consolidation only
   - keep many effect components in the world
   - present one authored `AudioEffect` abstraction only at the registry layer

Open question:

- do we want actual ECS/world simplification, or only MMS simplification?

Recommended answer:

- **yes to ECS/world consolidation**
- **also yes to separate internal runtime node structs**

So "one `AudioEffectComponent` in world" does not imply "one mega effect struct in the RT
thread".

### 10.9 What should the unified play intent payload be?

If we unify trigger/play across oscillators and clips, we need to decide whether the payload is:

- generic play params (`gain`, `rate`, `duration`, maybe `pitch`)
- `MusicNote`
- a hybrid (`MusicNote` plus generic playback flags)

This should be resolved together with the source model.

---

## 11. Recommended next steps

1. Add a terminology table to `docs/spec` using `AudioClip` as the primary PCM-backed source term.
2. Keep the decoding draft aligned on `AudioClipComponent` / `AudioClipPlayer` naming.
3. Decide whether Rust-side consolidation is real ECS consolidation or authored-surface-only consolidation.
4. Write a dedicated effect-model draft if needed, covering:
   - effect kinds
   - parameter vocabulary
   - graph topology rules
   - authored `AudioEffectComponent` vs compiled internal audio node boundary
5. Keep normalization explicitly deferred to Phase 2.
