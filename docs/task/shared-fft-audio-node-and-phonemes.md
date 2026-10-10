# Task: shared FFT audio node, microphone taps, and phoneme boundary

Date: 2026-10-09
Status: investigation complete; implementation slices proposed. Docs only.

Release target: [mittens-engine 0.10.0](epic/0.10.0/README.md).

## Goal and priority

Define FFT processing once and expose it through an MMS component that can
observe signed PCM in both the input processing path and the output rendering
graph. Start with the microphone spectrum, then the PCM entering existing
input analysis units. Output graphs should eventually permit explicit FFT
insertion at arbitrary supported PCM boundaries, including the final mix.

This task updates the direction in [input spectrum taps](audio-input-spectrum-taps.md)
and the [spectrum debugging epic](epic/audio-input-spectrum-debugging.md).
Those notes already establish optional pre/post-filter observation and bounded
handoff. They stop short of a shared graph node and a phoneme/viseme split.
The present note is the planning entry point for that expanded scope; it does
not claim any of its proposed MMS forms are implemented.

Speech analysis should produce **phonemes** (initially a small vowel-oriented
set). Avatar control maps phonemes to **visemes**, then `MorphTargetMap` maps
visemes to the avatar's morph targets. Frequency bins are useful input features
and diagnostics; they are not phoneme probabilities by themselves.

## Where implementation actually stands

The following describes checked source, rather than older task checkboxes.

| Area | Implemented state and relevant source |
| --- | --- |
| Capture authoring | [AudioInput](../../src/engine/ecs/component/audio_input.rs): default or session-local numbered device, enabled state, selection retry generation. CPAL resources stay outside ECS. |
| Input runtime | [AudioInputSystem](../../src/engine/ecs/system/audio_input_system.rs): owns CPAL streams, provisions amplitude/normalization consumers, drains bounded `rtrb` snapshot queues, handles failure and no-data timeout. |
| RMS/filtering | `CaptureCallback::process` fans frames to `RollingRms`. Filtered RMS owns a per-channel `HighPassBiquad`; signed filtered samples become squares/peaks immediately. Cutoff/Q updates use live atomics and reset accumulated state without reopening capture. |
| Normalization | `NormalizingRms` separately measures raw capture and applies learned gain to retained RMS/peak values. It does not output gain-adjusted PCM, and does not run the upstream amplitude observer's high-pass filter. |
| Main-thread analysis | [AmplitudeSystem](../../src/engine/ecs/system/amplitude_system.rs) resolves references and validates source/generation; [Amplitude](../../src/engine/ecs/component/amplitude.rs) and [VolumeNormalization](../../src/engine/ecs/component/volume_normalization.rs) retain measurements. Source validation accepts input/clip/oscillator, but input provisioning is microphone-specific. |
| Output graph | [AudioGraphCompiler](../../src/engine/ecs/system/audio_graph_compiler.rs) supports oscillator, clip, and `InputSource`, plus separate gain/filter/limiter node kinds. [AudioSystem](../../src/engine/ecs/system/audio_system.rs) lowers graphs into bounded RT node arrays. |
| Output evaluation | [audio_system_fundsp](../../src/engine/ecs/system/audio_system_fundsp.rs) evaluates source graphs for oscillators and playing clips, sums them, clamps the final mono sample, then writes output channels. `InputSource` is an enum/compiler placeholder: no capture-to-render PCM queue/render loop is connected yet. |
| FFT / input worker | No FFT component, unit, retained spectrum, or dedicated input processing worker exists in `src`. RMS and normalization currently execute in the CPAL capture callback. The clip decode worker is a different runtime and is not live playback analysis. |
| Speech/avatar | No `Visemes` or `Phonemes` component/system is implemented. [MorphTargetMap](../../src/engine/ecs/component/morph_target.rs) already accepts canonical viseme slots; [AvatarControlSystem](../../src/engine/ecs/system/avatar_control_system.rs) has an amplitude mouth-open fallback targeting `viseme_aa`, but no phoneme routing. |
| Consolidation | Audio effects remain separate ECS components and MMS names. [Effect consolidation](audio-clip-terminology-and-effect-consolidation.md) is a proposal, not a completed migration. |

Older [viseme spec](../spec/audio-input-and-visemes.md),
[first-slice checklist](audio-input-and-visemes-first-slice.md), and
[visemes epic](epic/visemes.md) describe a proposed worker and direct speech-to-viseme
component. They must be revised during the phoneme slice; their claims about
audible microphone routing and missing mouth slots are not current implementation
status. There is no live `Visemes` implementation to mechanically rename.

## Audio graph API refresher and a topology gap

Current public names include `AudioOutput`, `AudioOscillator`, `AudioClip`,
`AudioInput`, `AudioGain`, `AudioLowPassFilter`, `AudioBandPassFilter`,
`AudioHighPassFilter`, `AudioLimiter`, `AudioMix`, and `AudioBufferSize`.
`AudioSource` is an architectural category, not a device-selection wrapper.

The actual compiler starts at a source and follows **effect children**. One
child continues a chain. Multiple children receive the same processed PCM and
their returned results are summed with `AudioMix` weights (default 1). Child
order is sorted `ComponentId` order, not an explicit port/order API. Mix metadata
is excluded from the effect list; unknown component children are skipped.
Effects process their input before recursing into their children.

An example consistent with that compiler is:

```mms
AudioOutput {
    AudioOscillator.saw() {
        frequency(110)
        amplitude(0.25)
        AudioGain.new(0.35) {
            AudioLimiter.new(5.0, 80.0, 0.85)
        }
    }
}
```

By contrast, [audio-graph-example.mms](../../examples/audio-graph-example.mms)
nests limiter/gain/filter **above** its oscillator. Source discovery finds the
oscillator, but compilation does not include those ancestors. Do not copy that
shape into new FFT examples without resolving the mismatch. A focused topology
fixture should settle the supported authoring convention before graph insertion
is presented as working.

Today output graphs are per source. There is no shared compiled bus node for
the final sum. An FFT after one source's chain is therefore different from an
FFT of all audio sent to the device. Final-output observation needs its own
explicit render boundary or an output-bus extension.

Existing microphone analysis authoring is separate from this topology:

```mms
let microphone = AudioInput {}
let level = Amplitude.rolling_window(0.080).from(microphone) {
    highpass(120)
    highpass_resonance(0.707)
}
let normalized_level = VolumeNormalization.from(level) {}
let rms = level.value()
let gain_db = normalized_level.gain_db()
```

These are source references and retained scalar reads, not an authored chain
of PCM-processing nodes. Normalization's `.from(level)` resolves provenance
and window configuration; it does not consume filtered PCM from `level`.

## Shared unit and component contract

Use **audio node** for a source, PCM processor, mixer, or analysis tap; reserve
**device** for CPAL input/output selection. FFT is a PCM observation node. It
passes the original PCM onward unchanged and publishes spectrum results through
a separate bounded path. RMS and phoneme analysis are terminal measurement
consumers unless explicitly wrapped with pass-through behavior.

Proposed layering:

```text
ECS / MMS FFT configuration + latest retained spectrum
    -> source/tap binding and runtime generation
    -> preallocated PCM tap adapter (capture or output callback)
    -> bounded blocks/windows -> analysis worker
    -> shared FftUnit: window, transform, spectrum normalization
    -> bounded spectrum snapshots -> main-thread retention -> MMS/UI
```

There is one FFT algorithm/configuration/result contract and separate unit
instances for separate signals. Never share mutable FFT history or an SPSC
consumer between threads. The capture and output adapters use the same unit
implementation via worker-owned instances. If a later phoneme worker needs
continuous FFT features, it can own a local `FftUnit`; diagnostic snapshots
need not make an extra worker round trip into inference.

The component stores authored configuration and retained data, not plans,
scratch buffers, CPAL streams, or callback PCM. Runtime planning/preallocation
occurs off callbacks. Callbacks only collect bounded signed PCM; FFT execution
stays on a worker. Optional diagnostic taps incur no copying or transform work
when disabled. Bound probe count, window size, queued work, and result storage.

Working MMS vocabulary, **proposed and subject to the first binding slice**:

```mms
let spectrum = FFT.from(microphone).buckets(32) {} // raw capture, detached/inaudible
let filtered = FFT.from(level).tap("pre_rms") {} // exact PCM feeding RMS

// Source -> FFT -> gain -> FFT, using current compiler direction:
AudioOutput {
    AudioOscillator.saw() {
        FFT {
            AudioGain.new(0.35) { FFT {} }
        }
    }
}

// A separate binding to the complete rendered output:
// let output_spectrum = FFT.from(output).tap("output") {}
```

`FFT.from(level)` is provenance/tap selection, never FFT of `level.value()`.
An inline FFT with children must forward PCM exactly once. A detached probe
does not add an audible branch. Adding an FFT sibling to existing effect
children would otherwise add another summed branch; explicitly validate or
reject that ambiguous shape. Distinguish node input, own processed output,
and downstream branch sum when exposing future arbitrary-node bindings.

Input authoring can remain fixed: raw source and a specific unit's pre-RMS
boundary first, named intermediate/final PCM boundaries later. It does not need
the output graph's full topology API. An input chain's final FFT goes before
terminal RMS/phoneme reduction. Current virtual normalization has no PCM output
to inspect; a real gain stage would be a separate change.

Keep linear-frequency bins in the shared core; publish configured aggregated
buckets according to the [bucket-count and MMS snapshot contract](audio-fft-buckets-and-mms-snapshots.md).
Use `.buckets(32)` as the first example, independently of FFT window size.
Use [time slices](audio-fft-time-slices-and-transport-grid.md) for successive
spectral observations, with history span and slice spacing independent of
bucket count, window length, and UI refresh. Beat-fraction spacing is an audio
timing adapter, not MIDI processing; the core still operates on PCM frames.
Include actual sample rate, FFT size, window/hop,
level convention, channel policy, source/tap identity, frame interval, sequence,
generation, status, bucket edges, and drop information. Bucket aggregation is
separate from the transform; the worker applies the component's count/policy
before bounded publication. Main-thread reads return copied snapshots, following
the existing [retained analysis reads](audio-analysis-scalar-reads-and-renderable-history-graph.md)
pattern. Prefer one coherent `snapshot()` over several reads that can observe
different frames. Define pending/invalid/stale semantics before exposing arrays.

## Delivery slices and gates

### 1. Shared FFT core and spectrum protocol

- [ ] Add a runtime-independent `FftUnit` with signed PCM input, explicit
  channel policy, preallocated window/history/scratch, and reset semantics.
- [ ] Select transform library and verify its plan/scratch ownership locally.
  This investigation does not select a dependency or benchmark a backend.
- [ ] Use the earlier 4096-frame Hann / roughly 4 Hz diagnostic proposal as a
  starting experiment: at 48 kHz that is about 85 ms and 11.7 Hz bin spacing.
  Four Hz is only the initial latest-frame diagnostic cadence. Fine history
  must produce/retain actual frames at its requested time-slice spacing and
  can deliver them in batches at a slower UI refresh rate. Keep those settings
  separate from window length and eventual phoneme feature cadence.
- [ ] Specify one-sided bin scaling, DC/Nyquist handling, dB floor/reference,
  and mono/per-channel policy. Mono averaging can cancel antiphase channels;
  it is not equivalent to RMS's per-channel mean energy.
- [ ] Verify silence, DC, known-amplitude on-bin/off-bin tones, multiple rates,
  reset, and bounded storage with deterministic PCM fixtures.

Gate: known PCM produces interpretable bins without ECS or device access.

### 2. Raw microphone FFT component

- [ ] Register `FFT` construction/builders, reference validation, serialization,
  `.buckets(count)`, coherent copied-array snapshot reads, and main-thread
  result handling in the MMS registry/catalog. Validate exact array shape,
  frequency edges, same-sequence reads, and configured memory bounds.
- [ ] Make enabled FFT a capture consumer independently of `Amplitude` and
  `AudioOutput`; stopping the final consumer stops capture.
- [ ] Add a bounded preallocated PCM handoff and worker lifecycle. Preserve
  existing RMS behavior for this first slice; do not move it merely to add FFT.
- [ ] Separate device stream signature from probe-plan changes so toggling
  FFT need not reopen capture. Define safe plan ownership/swap/reclamation.
- [ ] Distinguish capture liveness from scalar snapshot arrival: current timeout
  logic watches the measurement queue and needs to support an FFT-only source.
- [ ] On queue gaps, device changes/failure, disable, removal, and shutdown,
  reset windows and reject old-generation results; never bridge missing PCM
  into an apparently continuous window. Keep capture-relative frame time
  explicit rather than assuming it is main/output clock time.

Gate: a detached microphone produces readable live buckets with bounded cost;
no FFT work runs in the capture callback, and enabling a probe does not alter
audible routing or RMS results.

Prove this gate with the [manual Snapshot/32-column example](audio-spectrum-visualization.md)
using the existing info-panel asset and built-in materials. Add Start/Stop and
a small flat bucket-by-time grid afterward. The diagnostic view does not depend
on custom shaders/materials or an FFT-specific renderer pipeline.

- [ ] Add the time-slice/history sub-slice for configurable seconds/beat spans
  and beat-fraction spacing: timing/clock mapping, bounded spectral history and
  copied batch reads, effective hop metadata, gaps, and tempo/pause/seek handling.

### 3. Input processing boundaries

- [ ] Factor signed high-pass processing out of fused `RollingRms` sufficiently
  for the exact same filtered samples to feed RMS and FFT. Do not duplicate a
  filter using merely matching parameters/state assumptions.
- [ ] Provide raw and pre-RMS taps; pair their windows by frame interval.
  Live cutoff/Q updates invalidate partial filtered windows without opening
  another capture stream. Preserve per-channel filter behavior.
- [ ] Introduce a small typed input plan for PCM stages and terminal observers.
  Add intermediate/end taps only at actual PCM boundaries. If stages move to
  an input worker, preserve temporal state, bounded queue-gap semantics, and
  measured mouth-response latency; treat that move as an explicit sub-slice.

Gate: a low tone attenuates after high-pass while a passband tone remains;
RMS and spectrum observe the same filter output. RMS/virtual AGC have no
mislabelled output spectrum.

### 4. Output node insertion and final output

- [ ] Lock and test source/effect authoring direction, branching, and identity
  before adding FFT recognition to compiler and RT lowering.
- [ ] Add a pass-through FFT tap node backed by the same bounded adapter/core.
  Test source, between-effects, and end-of-chain insertion on oscillators and
  playing clips. Analyze rendered playback, not decoded asset PCM.
- [ ] Add an explicit final-output tap after the mono sum/clamp and before
  device sample-format conversion, matching what is written to output channels.
  Pre-clamp headroom inspection can be a separately named later boundary.
- [ ] Handle skipped/disabled sources, graph swaps, removed probes, silence,
  and output stream replacement without indefinitely retaining old buckets.
- [ ] For microphone output tests, first implement capture-to-render transport,
  consumer provisioning, sample-rate/channel conversion, and underrun resets.
  The existing `InputSource` enum does not satisfy that prerequisite.

Gate: bypass/insertion leaves PCM unchanged; deterministic two-source output
shows both frequencies at the final tap, and each source tap shows its own
signal. Queue-full FFT work cannot stall rendering. Measure callback worst-case
time and worker cost with probes disabled, one active, and multiple active.

### 5. Phonemes first, visemes downstream

- [ ] Revise the older speech spec/checklists to `Phonemes.from(microphone)`:
  worker-owned acoustic analysis, timestamped phoneme/vowel weights and
  confidence/status retained on an audio-analysis component.
- [ ] Define the initial vowel labels/order, normalization/confidence semantics,
  silence/noise handling, feature window/hop, and speaker calibration. Revisit
  [backend evaluation](viseme-detection-backend-evaluation.md) in phoneme terms;
  an FFT visualization alone does not establish vowel-recognition quality.
- [ ] Keep speech recognition independent of whether a diagnostic FFT component
  is visible/enabled. Reuse FFT computation where compatible; otherwise give
  recognition its own configured unit instance.
- [ ] Add AVC-side phoneme-to-viseme mapping, then resolve canonical `viseme_*`
  channels through existing `MorphTargetMap`. Keep avatar policy out of capture
  and FFT processing; define silence/stale/removal release and driver priority
  alongside the existing amplitude fallback.
- [ ] Reserve a separate `Visemes` component for reusable mapping configuration
  and/or retained mouth-pose output if that becomes useful. It is not the audio
  recognizer. Settle its attachment/reference API in this slice rather than
  inventing a complete avatar API as a prerequisite for microphone FFT.

Gate: recorded vowels/speech/noise fixtures verify the phoneme outputs; AVC
mapping drives the expected existing mouth slots and releases on stale input.
No inference or avatar work runs in an audio callback.

## Scope boundaries

Effect consolidation and terminology cleanup are related, but a wholesale
`AudioEffect` migration should not block microphone FFT. Add FFT as its own
analysis component/node; reuse runtime contracts without forcing all authored
components into a single type. Track consolidation separately once actual
graph semantics are settled.

The [spectrum visualization task](audio-spectrum-visualization.md) remains a
consumer of retained results: ordinary-geometry snapshot first, bounded history
next, a custom shader only if later justified. Speech model selection,
general graph redesign, and actual phoneme/viseme implementation are later
slices. The first reviewable implementation target is slices 1–2: shared core
plus a working, inaudible, raw-microphone spectrum component.
