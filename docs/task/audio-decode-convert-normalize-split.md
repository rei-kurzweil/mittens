# Audio Decode / Convert / Normalize Split

Date: 2026-04-29

This task note records the current design direction for PCM clip preparation and turns it
into an implementation-facing checklist.

The key change is to stop treating:

- audio decode
- sample-rate / channel conversion
- loudness normalization

as one blended responsibility.

These should be modeled as separate stages with separate ownership.

Related draft:

- [docs/draft/audio_decoding_thread.md](../draft/audio_decoding_thread.md)

---

## 1. Current design direction

The current draft now distinguishes three conceptual layers:

1. `decode_*`
   - codec/container detection
   - packet decode via `symphonia`
   - decoded PCM extraction
2. `convert_*`
   - sample-rate conversion
   - channel remix/downmix/upmix
   - playback-format adaptation
3. `normalize_*`
   - loudness analysis
   - normalization policy
   - optional gain application

The intended separation is:

- decode/conversion are about "can the engine play this asset format?"
- normalization is about "how loud should this asset be relative to other sources?"

Those are not the same concern and should not be hidden in the same function/module.

---

## 2. Recommended module/function split

Recommended conceptual API:

```rust
fn decode_audio_file(uri: &str) -> Result<DecodedAudio, DecodeError>;

fn convert_sample_format(
    decoded: DecodedAudio,
    target: PlaybackFormat,
) -> Result<ConvertedAudio, ConvertError>;

fn analyze_loudness(audio: &ConvertedAudio) -> LoudnessMetrics;

fn normalize_loudness(
    audio: ConvertedAudio,
    policy: LoudnessPolicy,
) -> Result<ConvertedAudio, LoudnessError>;
```

Equivalent module split:

- `audio_decode`
- `audio_sample_format_convert`
- `audio_loudness`

This note does not require those exact names, but it does require the responsibilities to
stay separate in code structure.

---

## 3. What should happen in each stage

### 3.1 Decode

Should handle:

- file/container probing
- codec selection
- compressed packet decode
- extraction into a decoded PCM representation

Should not handle:

- engine playback-format assumptions
- loudness policy

### 3.2 Convert

Should handle:

- sample-rate conversion to engine playback rate
- mono/stereo/multichannel remix rules
- PCM layout adaptation required by the renderer/player

Should not handle:

- loudness normalization
- artistic gain staging

### 3.3 Normalize

Should handle:

- optional loudness analysis
- optional gain calculation
- optional offline/baked normalization, if the chosen policy wants that

Should not be required for:

- correctness of decode
- correctness of resampling
- basic playability of the clip

---

## 4. Recommended ownership

Current recommended ownership is:

- decode thread owns decode
- decode thread may also own sample-format conversion
- render/DSP side owns loudness policy by default

Why:

- conversion is deterministic and target-format-driven
- normalization interacts with runtime gain staging, note velocity, synth/sample balance,
  and limiter/headroom policy

So the default recommendation is:

- decode → yes, on decoding thread
- convert → yes, likely on decoding thread
- normalize → no, not by default on decoding thread

---

## 5. Implementation tasks

No `src/` changes are proposed here yet. This is the task breakdown.

### 5.1 Data model

- define `DecodedAudio`
- define `ConvertedAudio`
- define `PlaybackFormat`
- define `LoudnessMetrics`
- define `LoudnessPolicy`

### 5.2 Decode path

- isolate codec/container decode into an `audio_decode` layer
- ensure the output representation is independent of engine playback format
- decide whether decode emits planar or interleaved PCM internally

### 5.3 Conversion path

- isolate sample-rate conversion into a dedicated `convert_*` layer
- isolate channel remix rules into the same layer or a narrow helper beneath it
- make the target playback format explicit in the function boundary

### 5.4 Loudness path

- define whether loudness analysis exists at load time, playback time, or both
- define whether normalization is destructive, metadata-only, or purely runtime gain
- define where normalization gain is applied in the playback graph

### 5.5 AudioAssets integration

- decide whether `AudioAssets` stores:
  - converted PCM only
  - converted PCM plus loudness metadata
  - converted-and-normalized PCM for some asset classes
- make that choice explicit in the registry API

---

## 6. Open questions

These are the main unresolved design questions before implementation.

### 6.1 Where should normalization live by default?

Options:

- decoding thread
- asset registry post-process step
- playback/DSP side gain stage

Current recommendation:

- playback/DSP side by default

Reason:

- it keeps normalization in the same gain domain as note velocity, per-voice gain, mixing,
  and headroom limiting

### 6.2 Should normalization be destructive or metadata-only?

Options:

- destructive: rewrite stored PCM amplitudes
- metadata-only: store recommended gain alongside PCM
- fully deferred: analyze or estimate at playback/use sites only

Open question:

- for short SFX, destructive normalization may be convenient
- for long streamed assets, metadata-only is likely cleaner
- mixing both policies may be the right design

### 6.3 What is the canonical target format for converted PCM?

Need to decide:

- sample rate: fixed engine rate or backend-dependent rate?
- channel count: fixed stereo, or preserve multichannel until later?
- sample layout: interleaved or planar inside `AudioAssets`?

This affects:

- conversion cost
- memory layout
- `AudioClipPlayer` implementation shape

### 6.4 Should pitch playback reuse the conversion layer?

The current draft says sample playback may map pitch to playback rate/resampling.

Open question:

- is "asset load conversion" strictly one-time to engine rate, while note pitch uses a
  separate playback-rate path?
- or should both share the same SRC implementation in different modes?

Likely answer:

- same algorithm family may be reused
- but load-time format conversion and runtime pitched playback should still be treated as
  different call sites with different performance constraints

### 6.5 Should loudness analysis happen eagerly or lazily?

Options:

- eager: analyze when asset loads
- lazy: analyze on first use
- offline/import-time only

Tradeoffs:

- eager improves determinism and caching
- lazy reduces load-time cost for unused assets

### 6.6 Do short and long assets follow the same normalization policy?

This may need separate answers for:

- short one-shot SFX
- voice lines
- long streamed ambience/music

Because:

- destructive normalization is cheap for short cached assets
- streamed assets may need metadata + runtime gain rather than rewriting large buffers

### 6.7 What should the normalization policy actually target?

Potential policies:

- `Off`
- peak normalization
- RMS normalization
- LUFS/integrated loudness normalization

Open question:

- what is the minimum viable policy for the engine?
- do we need policy-per-asset, policy-per-output, or one global project policy first?

### 6.8 Where is the policy configured?

Possible locations:

- global audio engine setting
- `AudioOutputComponent`
- per-clip asset metadata
- `AudioClipComponent`

This should be explicit before implementation to avoid burying normalization behavior in
ad hoc defaults.

---

## 7. Recommended near-term decisions

To unblock implementation without overdesign, the most pragmatic first pass would be:

1. Separate decode and sample-format conversion unconditionally.
2. Keep loudness normalization out of the decoding thread.
3. Store converted PCM in `AudioAssets`.
4. If needed, store loudness metadata next to the PCM rather than destructively rewriting
   samples at first.
5. Apply any normalization gain in `AudioClipPlayer` or an adjacent playback gain stage.

That gives a clean architecture now and leaves room for stronger offline normalization
later without entangling it with decoding correctness.
