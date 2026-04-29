# Audio Decoding Thread: Symphonia + rtrb Protocol

Date: 2026-04-18
Status: Draft (Revised v2)

## Overview

To support PCM audio playback (SFX, voice lines, and BGM) alongside synthesized audio, we introduce an **Audio Decoding Thread**, an **AudioAssets** registry, and a unified **Audio Source** model.

This system handles decompressing audio files (WAV, OGG, MP3, etc.) using `symphonia` and ensures that triggering samples is as precise and predictable as triggering synthesized oscillators.

Important separation of concerns:

- **Sample-rate / channel conversion** is a decode-time data-format step.
- **Loudness normalization** is a gain-policy step.

These should be modeled as separate modules/functions, not as one combined post-decode pass.

## 1. Thread Architecture

```text
Main Thread (AudioSystem)
    │
    │ (1) LoadClip { uri, clip_id }
    ▼
Audio Decoding Thread (Symphonia)
    │
    │ (2) PCM Chunks (f32) via rtrb
    ▼
Audio Rendering Thread (fundsp / AudioAssets)
```

### 1.1 AudioAssets (The Registry)
`AudioAssets` lives on the **Audio Rendering Thread** (RT). It is the source of truth for all PCM data used by the DSP graph.

- **Short Clips (SFX):** Fully decoded and stored as `Arc<Vec<f32>>`.
- **Long Clips (BGM):** Caches the first ~5 seconds. Subsequent chunks are pulled from an `rtrb::Consumer<f32>` fed by the Decoding Thread.

`AudioAssets` should be able to hold converted PCM independently of any loudness policy.
That allows the engine to:

- decode and resample once
- analyze loudness separately if desired
- apply gain policy at playback time rather than baking it into the decode path

### 1.2 Audio Decoding Thread
- **Job:** Format detection, decompression, and PCM decode.
- **May include output-format conversion:** Decoded samples may be resampled and remixed to match the engine's playback format (for example 48kHz stereo) before being sent to the RT thread.
- **Not responsible for loudness normalization:** The decoding thread should not change asset loudness. Gain staging, per-voice gain, and any normalization policy belong in a separate loudness/gain stage on the audio rendering thread / DSP side, where they can be applied consistently alongside synthesized sources.

### 1.3 Separate conversion and normalization stages

After decode, the spec should treat these as distinct operations:

1. `convert_sample_format(...)`
   - responsibility: sample-rate conversion, channel remix/downmix/upmix, playback-format adaptation
   - input: decoded PCM in source asset format
   - output: PCM in engine playback format
   - should not apply gain policy

2. `normalize_loudness(...)`
   - responsibility: optional loudness analysis and normalization gain application
   - input: PCM already in engine playback format
   - output: same PCM shape, different amplitude scaling and/or attached loudness metadata
   - policy layer, not correctness layer

Recommended conceptual split:

```rust
fn decode_audio_file(uri: &str) -> Result<DecodedAudio, DecodeError>;
fn convert_sample_format(
    decoded: DecodedAudio,
    target: PlaybackFormat,
) -> Result<ConvertedAudio, ConvertError>;
fn normalize_loudness(
    audio: ConvertedAudio,
    policy: LoudnessPolicy,
) -> Result<ConvertedAudio, LoudnessError>;
```

If normalization remains entirely runtime-side, the architecture should still name the split
explicitly:

```rust
fn decode_audio_file(uri: &str) -> Result<DecodedAudio, DecodeError>;
fn convert_sample_format(
    decoded: DecodedAudio,
    target: PlaybackFormat,
) -> Result<ConvertedAudio, ConvertError>;
// normalization handled later by DSP / playback gain policy
```

The key point is that conversion and normalization are independent layers with different
ownership and different failure modes.

---

## 2. Unified Audio Source Model

We treat **Oscillators** and **Clips** as peers. Both are "Audio Sources" that can be connected to the graph and triggered by the same intent signals.

This authored/world model does not need to match the exact runtime node structs used by
`AudioSystem` or the RT thread. ECS components are scene-facing declarations; the audio
graph compiler may lower them into separate internal node structs/enums.

### 2.1 Hierarchy and Loading
- **Loading:** Adding an `AudioClipComponent` to the world initiates loading/decoding.
- **Connection:** If the component is a descendant of an `AudioOutputComponent`, it is included in the compiled DSP graph.
- **Detached:** If not connected to an output, it remains in `AudioAssets` (cached) but silent.

### 2.2 Triggering (Unified Intent)
We use a common intent for "playing" any source.

```rust
IntentValue::AudioScheduleNote {
    component_ids: Vec<ComponentId>,
    beat_offset: f64,
    beat_context: Option<f64>, // Set by AnimationSystem during lookahead
    note: MusicNote,
}
```

**Behavior on trigger:**
- **Oscillator:** Gate ON, set pitch/gain, then Gate OFF after `note.duration`.
- **Clip:** Start playback from beginning. `note.velocity` maps to gain. `note.pitch` maps to playback rate/resampling. If `note.duration` is provided, stop playback after that duration (unless shorter).

---

## 3. ECS Components

### 3.1 `AudioClipComponent`
The "Source" component.

```rust
pub struct AudioClipComponent {
    pub uri: String,
    /// If true, the clip is re-triggered on every "Play" intent.
    /// If false (e.g. for ambient loops), it may ignore triggers once playing.
    pub trigger_mode: AudioTriggerMode,
}
```

### 3.2 `AudioClipPlayer` (The DSP Node)
The internal `fundsp` node created by `AudioGraphCompiler`.
- It maintains a `cursor` (index into the clip PCM).
- It reads from `AudioAssets`.
- It handles the `Trigger` message by resetting `cursor = 0` and `enabled = true`.

If loudness normalization is enabled as a policy, `AudioClipPlayer` or an adjacent gain
stage is the natural place to apply it, because that keeps:

- per-voice gain
- note velocity scaling
- normalization policy
- balancing against synthesized sources

in one runtime gain domain.

### 3.3 Proposed helper modules

To keep responsibilities explicit, the design should separate:

#### `audio_decode`

- codec/container detection
- packet decode via `symphonia`
- decoded PCM extraction

Example surface:

```rust
fn decode_audio_file(uri: &str) -> Result<DecodedAudio, DecodeError>;
```

#### `audio_sample_format_convert`

- sample-rate conversion
- channel remix
- playback-format adaptation

Example surface:

```rust
fn convert_sample_format(
    decoded: DecodedAudio,
    target: PlaybackFormat,
) -> Result<ConvertedAudio, ConvertError>;
```

#### `audio_loudness`

- loudness analysis
- normalization policy
- optional gain application

Example surface:

```rust
fn analyze_loudness(audio: &ConvertedAudio) -> LoudnessMetrics;
fn normalize_loudness(
    audio: ConvertedAudio,
    policy: LoudnessPolicy,
) -> Result<ConvertedAudio, LoudnessError>;
```

Whether `normalize_loudness(...)` is destructive, metadata-only, or fully deferred to
playback is a policy choice. It should not be hidden inside decode/resample code.

### 3.4 Authored components vs compiled runtime nodes

The audio ECS/world model and the audio runtime-thread model should be allowed to diverge.

Recommended rule:

- authored scene / ECS world:
  - `AudioOutputComponent`
  - `AudioOscillatorComponent`
  - `AudioClipComponent`
  - `AudioEffectComponent`
- compiled/runtime audio graph:
  - internal node structs/enums chosen for efficient DSP execution

So a single authored `AudioEffectComponent` may compile into different internal runtime
node variants depending on its effect type and configured fields.

This is the same general pattern as other authored-vs-runtime boundaries in the engine:

- authored components are vocabulary for scenes and MMS
- systems are free to compile them into narrower runtime representations

---

## 4. Animation and Lookahead

The `AnimationSystem` already supports **audio lookahead**. It scans upcoming keyframes and emits scheduled intents with a `beat_context`. 

1.  **Keyframe N:** Puppy bark action at beat 10.0.
2.  **Animation System (at beat 9.9):** Detects upcoming bark. Emits `AudioScheduleNote` with `beat_context: 10.0`.
3.  **Audio Rendering Thread:** Receives the intent. At exactly beat 10.0 (sample-accurate), it resets the `AudioClipPlayer` cursor for the "bark.wav" asset.

---

## 5. Prototype Workflow ("kristi vs puppy 🦴🪦")

- **The Gunshot:**
    - `AudioClipComponent { uri: "glock.wav" }` under an `AudioOutput`.
    - Initialized once. 
    - Triggered by `ActionComponent` on a "Puppy Death" animation keyframe.
- **Urban Ambience:**
    - `AudioClipComponent { uri: "wind.ogg" }` attached to the output.
    - Set to `playing: true` on level start.
    - Since it's "Long", only the start is pre-cached; the decoding thread streams the rest.

---

## 6. Design rule

Keep these concerns separate in both naming and code structure:

- `decode_*` = codec/container work
- `convert_*` = sample-rate/channel/data-layout conversion
- `normalize_*` = loudness / gain policy

Do not bury normalization inside conversion code, and do not require normalization in
order for decoded assets to be playable.
