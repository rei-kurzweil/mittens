# Task: high-pass RMS observation for voice response

Status: planned. The panel layout and reuse cleanup is separate; no high-pass
audio processing or filter sliders are implemented yet.

## Goal

Let an `Amplitude` observer measure a high-pass-filtered microphone signal so
low-frequency rumble, handling noise, and DC offset do not drive an avatar's
mouth as strongly. Keep this analysis path independent of audible microphone
output and of the unfinished `VolumeNormalization` policy. An unconfigured
`Amplitude` continues to measure the raw rolling RMS.

## Authoring and live controls

Proposed MMS shape:

```mms
let voice_level = Amplitude.rolling_window(0.080)
    .highpass(120.0)
    .highpass_resonance(0.7)
    .from(microphone) {}
```

`highpass(cutoff_hz: f32)` selects a high-pass RMS callback unit in place of
the raw `RollingRms` unit for this observer. `highpass_resonance` is a proposed
second builder; settle its name and whether the number means Q or another
precisely defined damping parameter before implementing it. The first builder
has the requested one-float API. Preserve the usual rolling-window and source
builders, and serialize both filter settings. Reject nonfinite, nonpositive
cutoffs and resonance values. Resolve the usable cutoff against the active
device sample rate and its Nyquist limit at binding time.

The shared voice-response panel in `assets/components/ui/mouth_response_panel.mms`
should gain two optional, clearly labelled live controls when the backend is
ready: **high-pass cutoff (Hz)** and **resonance (defined unit)**. Show their
current numeric values. The panel should only show these controls for an
observer configured with high-pass analysis, while retaining RMS centre, RMS
range, and mouth amount for any avatar. Slider changes must update the running
observer safely and reset or transition filter state deliberately; no stale
samples from the prior configuration may be applied. Make the cutoff control
use a range or mapping that gives useful resolution in voice frequencies,
rather than spending most travel near Nyquist.

## Audio-thread design

`AmplitudeSystem` currently supplies `InputAmplitudeConsumer` descriptors to
`AudioInputSystem`. A capture callback builds one `RollingRms` per consumer
and pushes frame mean-square values into it. Extend that descriptor with an
explicit raw/high-pass analysis mode and validated filter parameters. At stream
setup, choose the matching accumulator unit for each observer. The high-pass
unit must filter each channel's signed PCM samples **before squaring**, keep
per-channel filter state across callback buffers, then compute rolling RMS (and
define peak from the same filtered signal). Filtering an already computed
mean-square value cannot reject low frequencies.

The existing `AudioHighPassFilterComponent` belongs to the audible audio graph.
Its high-pass implementation is currently one-pole; resonance needs a defined
filter response and likely a different implementation. Reuse validated filter
math if suitable, but do not route this meter through the audible graph or
imply that its current `resonance` field already affects the capture analysis.
Keep callback work bounded and allocation-free. Preserve source/generation
validation and the bounded snapshot handoff used by raw `Amplitude`. Decide
whether live changes can update a callback-side parameter block without
reopening capture; if they require rebinding, prevent an audible or visual
glitch and reject old-generation snapshots.

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
