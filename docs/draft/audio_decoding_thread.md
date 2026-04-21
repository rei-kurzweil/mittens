# Audio Decoding Thread: Symphonia + rtrb Protocol

Date: 2026-04-18
Status: Draft

## Overview

To support PCM audio playback (SFX, voice lines, and BGM) without blocking the main game thread or the real-time (RT) audio rendering thread, we introduce a dedicated **Audio Decoding Thread**.

This thread handles the "heavy lifting" of decompressing audio files (WAV, OGG, MP3, etc.) using the `symphonia` crate and streaming the resulting PCM data to the RT thread via `rtrb` ring buffers.

## 1. Thread Architecture

```text
Main Thread (AudioSystem)
    │
    │ (1) DecodeRequest { uri, sample_id }
    ▼
Audio Decoding Thread (Symphonia)
    │
    │ (2) PCM Chunks (f32) via rtrb
    ▼
Audio Rendering Thread (fundsp)
```

### 1.1 Main Thread (`AudioSystem`)
- Owns the `AudioSampleComponent` lifecycle.
- When a new `AudioSampleComponent` is registered, it generates a unique `SampleId`.
- It sends a `DecodeRequest` to the Decoding Thread via a `crossbeam-channel` or `rtrb`.
- It manages the mapping between `uri` and `SampleId` to avoid redundant decoding.

### 1.2 Audio Decoding Thread
- **Input:** `DecodeRequest` (URI/Path + SampleId).
- **Processing:** 
    - Opens the file using `symphonia`.
    - Detects format and decodes packets into `f32` PCM.
    - Handles sample rate conversion (if necessary) to match the engine's 44.1kHz/48kHz output.
- **Output:** Pushes decoded chunks into an `rtrb` producer associated with the `SampleId`.

### 1.3 Audio Rendering Thread (RT)
- Receives PCM chunks from the Decoding Thread.
- **SFX Mode:** Buffers the entire sample into a `SampleRegistry` for low-latency re-triggering.
- **Streaming Mode (BGM):** Streams directly into a `SampleStreamer` node in the `fundsp` graph, using a small buffer to handle jitter.

---

## 2. ECS Components (Proposed)

### 2.1 `AudioSampleComponent`
Represents an audio asset that needs to be loaded.

```rust
pub struct AudioSampleComponent {
    pub uri: String,
    /// If true, the sample is decoded once and kept in memory on the RT thread.
    /// If false, it is streamed from disk/decoding thread on demand.
    pub preload: bool,
}
```

### 2.2 `AudioSamplePlayerComponent`
A signal-driven component that triggers playback of a sample.

```rust
pub struct AudioSamplePlayerComponent {
    pub sample_uri: String,
    pub volume: f32,
    pub pitch: f32,
    pub looping: bool,
}
```

---

## 3. Communication Protocol

### 3.1 Main -> Decoding (`AudioQueueItem` extension)
```rust
pub enum AudioQueueItem {
    // ... existing variants (SetTransport, ReplaceOscillators, etc.)
    
    /// Request decoding of a file.
    LoadSample {
        sample_id: u64,
        uri: String,
    },
}
```

### 3.2 Decoding -> RT Thread
The Decoding Thread owns an `rtrb::Producer<f32>` for each active stream. The RT thread owns the `rtrb::Consumer<f32>`.

To handle metadata (like "Sample is finished" or "Sample rate is X"), we wrap chunks in a header:

```rust
pub enum DecodingToRtSignal {
    /// Provide metadata before samples start.
    Metadata {
        sample_id: u64,
        channels: u16,
        sample_rate: u32,
    },
    /// PCM data chunk.
    Chunk {
        sample_id: u64,
        data: Vec<f32>, // Or use a pre-allocated pool to avoid RT allocation
    },
    /// Signal end of file.
    EndOfStream {
        sample_id: u64,
    },
}
```

---

## 4. Implementation Notes (Symphonia)

1. **Format Agnostic:** Use `symphonia::default::get_probe()` to support all formats enabled in `Cargo.toml`.
2. **Resampling:** If the file's sample rate doesn't match the output (e.g., 44.1kHz file on a 48kHz output), use `rubato` or a simple linear interpolator in the Decoding Thread. **Do not resample on the RT thread.**
3. **Channel Mixing:** Convert Stereo to Mono (or vice versa) in the Decoding Thread based on the component's spatial requirements.

---

## 5. Integration with "kristi vs puppy 🦴🪦"

- **Gunshots:** `preload: true`. Loaded on boot or level start. Triggered by an `IntentValue::PlaySFX`.
- **Urban Ambience:** `preload: false`. Streamed via the Decoding Thread to keep memory usage low.
- **Nicotine Heartbeat:** A specialized `AudioOscillator` (existing) modulated by the `NicotineComponent` value, layered with a low-pass filtered "Thump" sample.
