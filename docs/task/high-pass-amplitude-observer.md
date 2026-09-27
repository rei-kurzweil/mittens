# Task: high-pass RMS observation for voice response

Status: implemented for microphone capture and the shared voice-response panel.
XR hardware and subjective voice tuning still need a performer pass.

## Goal

Let an `Amplitude` observer measure a high-pass-filtered microphone signal so
low-frequency rumble, handling noise, and DC offset do not drive an avatar's
mouth as strongly. Keep this analysis path independent of audible microphone
output and of the unfinished `VolumeNormalization` policy. An unconfigured
`Amplitude` continues to measure the raw rolling RMS.

## Authoring and live controls

Two independent builder methods are required. An author can set only the
cutoff, or set the cutoff and then override the default resonance:

```mms
let simple_voice_level = Amplitude.rolling_window(0.080)
    .highpass(120.0).from(microphone) {}

let tuned_voice_level = Amplitude.rolling_window(0.080)
    .highpass(120.0)
    .highpass_resonance(0.7)
    .from(microphone) {}
```

`highpass(cutoff_hz: f32)` selects a high-pass RMS callback unit in place of
the raw `RollingRms` unit for this observer. It supplies a sensible default
resonance. `highpass_resonance(q: f32)` is a separate, optional builder that
sets the filter's Q: the sharpness and peaking around the cutoff. It requires
`highpass` on the same `Amplitude` and cannot enable filtering by itself.
The default Q is 0.707; authored Q is accepted in `0.1..=10.0`.
Preserve the usual rolling-window and source builders, and serialize both
filter settings. Reject nonfinite, nonpositive cutoffs and Q values. Resolve
the usable cutoff against the active device sample rate and its Nyquist limit
at binding time.

The shared voice-response panel in `assets/components/ui/mouth_response_panel.mms`
shows **high-pass cutoff (Hz)** and **resonance (Q)** when passed a filtered
Amplitude observer. It keeps RMS centre, RMS range, and mouth amount for any
avatar. Slider changes update callback-side coefficients and clear the rolling
window without reopening capture. The cutoff slider covers 40–400 Hz in 5 Hz
steps for voice tuning; the component API accepts any finite positive cutoff
and clamps it below Nyquist at the active sample rate.

## Audio-thread design

`AmplitudeSystem` supplies `InputAmplitudeConsumer` descriptors to
`AudioInputSystem`. A capture callback builds one `RollingRms` per consumer
and pushes frame mean-square values into it. The descriptor carries an
explicit raw/high-pass analysis mode and filter parameters. At stream setup,
the capture path selects the matching mode. The high-pass unit filters each
channel's signed PCM samples **before squaring**, keeps
per-channel filter state across callback buffers, then compute rolling RMS (and
define peak from the same filtered signal). Filtering an already computed
mean-square value cannot reject low frequencies.

The existing `AudioHighPassFilterComponent` belongs to the audible audio graph.
Its high-pass implementation is currently one-pole; resonance needs a defined
filter response and likely a different implementation. Reuse validated filter
math if suitable, but do not route this meter through the audible graph or
imply that its current `resonance` field already affects the capture analysis.
Callback work remains bounded and allocation-free. A shared atomic parameter
block lets live changes update coefficients and generation without reopening
capture; old-generation snapshots are rejected by the retained observation
path. Source/generation validation and the bounded snapshot handoff are shared
with raw `Amplitude`.

## Verification

1. With identical PCM input, raw and high-pass observers can coexist on one
   `AudioInput`. DC and a low-frequency test tone fall substantially in the
   high-pass RMS while a passband voice-frequency tone remains measurable.
2. Cutoff and resonance changes have predictable, bounded responses at several
   device sample rates; silence stays neutral and values remain finite.
3. The mouth follows the filtered observer when wired through AVC, and the
   panel's two filter controls update that observer live without changing the
   microphone's audible samples.
4. Restoring a minimized panel recreates all five sliders with their current
   values; each visible track spans its slider travel inside the narrower panel.
5. Raw `Amplitude` scenes and `VolumeNormalization` consumers retain their
   current behavior unless explicitly configured for this new analysis mode.

## Related work

- [Audio amplitude AVC first slice](audio-amplitude-avc-first-slice.md)
- [Adaptive volume normalization](adaptive-volume-normalization-for-amplitude.md)
- [Performer-facing mouth-response controls](avatar-control-mouth-response-controls.md)
