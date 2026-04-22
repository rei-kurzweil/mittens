# Audio Decoding Thread: Symphonia + rtrb Protocol

Date: 2026-04-18
Status: Draft (Revised v2)

## Overview

To support PCM audio playback (SFX, voice lines, and BGM) alongside synthesized audio, we introduce an **Audio Decoding Thread**, an **AudioAssets** registry, and a unified **Audio Source** model.

This system handles decompressing audio files (WAV, OGG, MP3, etc.) using `symphonia` and ensures that triggering samples is as precise and predictable as triggering synthesized oscillators.

## 1. Thread Architecture

```text
Main Thread (AudioSystem)
    │
    │ (1) LoadSample { uri, sample_id }
    ▼
Audio Decoding Thread (Symphonia)
    │
    │ (2) PCM Chunks (f32) via rtrb
    ▼
Audio Rendering Thread (fundsp / AudioAssets)
```

### 1.1 AudioAssets (The Registry)
`AudioAssets` lives on the **Audio Rendering Thread** (RT). It is the source of truth for all PCM data used by the DSP graph.

- **Short Samples (SFX):** Fully decoded and stored as `Arc<Vec<f32>>`.
- **Long Samples (BGM):** Caches the first ~5 seconds. Subsequent chunks are pulled from an `rtrb::Consumer<f32>` fed by the Decoding Thread.

### 1.2 Audio Decoding Thread
- **Job:** Format detection, decompression, and **normalization**.
- **Normalization:** All samples are resampled and mixed to match the engine's output (e.g., 48kHz Stereo) before being sent to the RT thread.

---

## 2. Unified Audio Source Model

We treat **Oscillators** and **Samples** as peers. Both are "Audio Sources" that can be connected to the graph and triggered by the same intent signals.

### 2.1 Hierarchy and Loading
- **Loading:** Adding an `AudioSampleComponent` to the world initiates loading/decoding.
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
- **Sample:** Start playback from beginning. `note.velocity` maps to gain. `note.pitch` maps to playback rate/resampling. If `note.duration` is provided, stop playback after that duration (unless shorter).

---

## 3. ECS Components

### 3.1 `AudioSampleComponent`
The "Source" component.

```rust
pub struct AudioSampleComponent {
    pub uri: String,
    /// If true, the sample is re-triggered on every "Note On" intent.
    /// If false (e.g. for ambient loops), it may ignore triggers once playing.
    pub trigger_mode: AudioTriggerMode,
}
```

### 3.2 `AudioSamplePlayer` (The DSP Node)
The internal `fundsp` node created by `AudioGraphCompiler`.
- It maintains a `cursor` (index into the sample).
- It reads from `AudioAssets`.
- It handles the `Trigger` message by resetting `cursor = 0` and `enabled = true`.

---

## 4. Animation and Lookahead

The `AnimationSystem` already supports **audio lookahead**. It scans upcoming keyframes and emits scheduled intents with a `beat_context`. 

1.  **Keyframe N:** Puppy bark action at beat 10.0.
2.  **Animation System (at beat 9.9):** Detects upcoming bark. Emits `AudioScheduleNote` with `beat_context: 10.0`.
3.  **Audio Rendering Thread:** Receives the intent. At exactly beat 10.0 (sample-accurate), it resets the `SamplePlayer` cursor for the "bark.wav" asset.

---

## 5. Prototype Workflow ("kristi vs puppy 🦴🪦")

- **The Gunshot:**
    - `AudioSampleComponent { uri: "glock.wav" }` under an `AudioOutput`.
    - Initialized once. 
    - Triggered by `ActionComponent` on a "Puppy Death" animation keyframe.
- **Urban Ambience:**
    - `AudioSampleComponent { uri: "wind.ogg" }` attached to the output.
    - Set to `playing: true` on level start.
    - Since it's "Long", only the start is pre-cached; the decoding thread streams the rest.
