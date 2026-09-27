# Task: FFT buckets from audio analysis taps

Status: planned. Parent: [audio spectrum debugging epic](epic/audio-input-spectrum-debugging.md).

## Goal and first slice

Retain a bounded, main-thread-readable frequency spectrum for live microphone
PCM. A spectrum probe attached to a filtered `Amplitude` selects raw PCM
before its high-pass stage, signed PCM after that exact stage and before RMS,
both taps, or no active tap. This shows what the RMS calculation receives;
the FFT is never computed from the RMS scalar or from squared samples.

The first slice supports one microphone source, its existing per-observer
high-pass stage, and independent probes at its raw and filtered boundaries.
Multiple observers may coexist. A probe is inactive unless enabled; neither
the default amplitude path nor the audible input path changes merely because
the feature exists.

## Current boundary and options

`AudioInputSystem` provisions a CPAL callback from `InputAmplitudeConsumer`
descriptors. `CaptureCallback` converts input frames and fans them out to
`RollingRms` units. Each filtered `RollingRms` owns a `HighPassBiquad` and
reduces its output to mean square and peak immediately. Snapshots cross an
`rtrb` queue; `AmplitudeSystem` validates source and generation, then retains
the newest result. `VolumeNormalization` currently has another fused unit.

| Approach | Benefit | Cost / decision |
|---|---|---|
| Add an FFT to each raw/high-pass RMS variant | Smallest patch | Creates another fused permutation and obscures a shared post-filter PCM tap. Do not use for this task. |
| Copy raw PCM and run a duplicate filter in an FFT worker | Keeps capture callback simple | Filter state and live parameter timing can diverge from RMS; not suitable for claiming this is the RMS input. |
| Compile a small analysis plan with named taps | RMS and FFT can observe the same filtered samples; extends to later sources | Needs explicit stage/tap ownership, bounded handoff, and reconfiguration rules. Preferred direction. |

The plan may initially be a typed linear stage with fan-out, rather than a
public graph API. Avoid adopting the audible `AudioGraphCompiler` as a shortcut:
its node output and the capture-side amplitude filter currently have different
ownership and semantics. Decide whether generalizing the plan is justified
only after the microphone implementation and a second source are understood.

## Proposed data flow and ownership

```text
CPAL capture frames
  -> finite/clamped signed PCM, with sample rate and channel count
  -> raw tap --------------------------> optional raw spectrum window
  -> amplitude observer's high-pass
  -> post-filter tap ----+-------------> rolling RMS / peak
                        +-------------> optional filtered spectrum window

bounded window handoff -> analysis worker FFT -> bounded bucket snapshots
                         -> main-thread retained spectrum -> UI / renderer
```

The raw tap may be shared by probes of the same source. The post-filter tap is
identified by the exact filter stage/observer, not just equal cutoff values.
Channel policy must be explicit: start with a documented mono downmix for the
spectrum while the high-pass/RMS stage keeps per-channel filter state. Build
matched pre/post windows from the same frame interval so comparisons are
meaningful. If filter reset causes a discontinuity within that interval,
discard or mark the pair rather than displaying a misleading difference.

Provide an authored observer or probe descriptor bound to the `Amplitude`
handle and a tap choice. A possible spelling is
`Spectrum.from(amplitude).tap("pre_filter")`; this is illustrative, not a
committed MMS API. `.from(amplitude)` resolves PCM provenance and filter stage,
not the retained scalar RMS. Define validation for a raw amplitude with no
filter, duplicate probes, disabled sources, and a removed upstream observer.
The ECS component should retain configuration and a latest result, not PCM
buffers or FFT plans.

## FFT and real-time boundary

Choose a fixed, preallocated window size and hop before implementation; a
4096-frame Hann window at 48 kHz gives about 11.7 Hz bin spacing and an
85 ms window, adequate to inspect a cutoff near 120 Hz. This is a starting
point to benchmark, not a fixed public contract. Publish around four times
per second. State the actual sample rate, FFT size, window function, bin
frequency, and level convention with every result; frequency is `k * fs / N`
for bin `k`. A consistent magnitude or dBFS convention must be selected and
tested with known tones. Preserve enough bins to compare the low-frequency
cutoff; display-side aggregation may reduce the number of bars.

Do not run an FFT, allocate, lock, log, access ECS, or wait in the capture
callback. It may write preallocated sample windows into a bounded, nonblocking
handoff. A worker owns FFT plans and scratch storage and publishes fixed-size
or pooled bucket snapshots through another bounded handoff. Prefer dropping
stale diagnostic windows over increasing callback latency or building a queue
of old pictures. Measure copy cost and callback worst-case time with raw,
filtered, and both probes active. If the worker falls behind, expose drop and
staleness counts. Avoid sending full-rate continuous PCM to the main thread.

## Reconfiguration and lifecycle

- Enabling or disabling a probe changes the analysis plan without silently
  changing the microphone's audible route. Decide a safe callback-plan swap
  boundary and preserve the active RMS result where possible.
- Live cutoff/Q changes update the filter shared with RMS and invalidate any
  partly collected post-filter FFT window. Tag windows and snapshots with
  source, observer/tap, sequence, timestamp, sample rate, and generation.
- Stream/device/format changes and removal clear retained buckets or publish
  a fresh neutral/invalid state. Reject queued results from older generations.
- With neither probe active, allocate no FFT worker buffers for this source
  and perform no spectrum window copying.

## Future source boundary

Keep the tap/result contract independent of CPAL so a clip, oscillator, or
compiled audio node can later publish the same spectrum type. The decode
thread currently decodes/converts clip assets ahead of playback; sampling
that thread would describe file contents, not necessarily what a playing
voice sends downstream. A live clip probe belongs at the playback/render
stage or a separately defined offline-asset analysis feature. Likewise a
post-effect tap must identify a compiled node boundary, consistent with
[audio-node metering](audio-node-metering.md).

## Acceptance

1. Deterministic silence, DC, and single-/multi-tone PCM fixtures verify bin
   placement, magnitude convention, sample-rate metadata, and Nyquist limit.
2. A low tone attenuates after high-pass while a passband tone stays visible;
   paired spectra and RMS use the same filtered stage and frame interval.
3. Changing cutoff/Q live refreshes post-filter buckets without a stale
   spectrum or capture reopen; raw buckets remain representative of raw PCM.
4. Raw, post, both, and neither have bounded CPU/memory; queue-full behavior
   never stalls the callback. Probe removal and stream failure clear retained
   state and reject late snapshots.
