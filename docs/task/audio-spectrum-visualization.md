# Task: reusable spectrum visualization for audio analysis

Status: planned. Parent: [audio spectrum debugging epic](epic/audio-input-spectrum-debugging.md).
Depends on: [FFT buckets from audio analysis taps](audio-input-spectrum-taps.md).

## Goal

Show the retained frequency buckets for microphone mouth-response tuning in a
scene panel. Make raw and post-filter spectra independently selectable, with
a clear both-view comparison. When the performer changes the high-pass cutoff
or resonance, the display should reveal the effect in a few updates without
changing the mouth-response signal or audio monitoring route.

This is a reusable diagnostic view for a spectrum observer, not a Reimu-only
panel. The existing shared voice-response panel may host or open it, but the
spectrum view should also work without an avatar. Treat the probe's tap choice
as analysis configuration; hiding a graph should not imply that a still-active
probe has stopped doing FFT work unless the UI explicitly disables it.

## Display contract

- Read only main-thread-retained snapshots. Never read callback or worker
  buffers from render code. Show pending, invalid, stale, and dropped data
  clearly; do not freeze a bright last frame when capture stops.
- Label the source, tap position, cutoff/Q when applicable, sample rate, FFT
  window size, frequency range, and level convention. Raw and post-filter
  traces need distinct, themeable colors and the same axes when compared.
- Start near four visible updates per second and measure whether a different
  rate improves tuning. Interpolation may smooth the display but must not
  invent frequency detail or hide a generation reset.
- Prefer a log-frequency horizontal axis and a dB vertical axis for voice
  debugging, with a linear option only if it proves useful. Aggregate FFT
  bins into a bounded number of display columns using an explicit rule so a
  low-frequency cutoff remains legible. Do not mistake bin index for Hz.
- Reuse the pastel theme vocabulary of the shared voice-response panel, with
  enough contrast to tell pre and post apart in a headset.

## Rendering choice to validate

A dedicated spectrum material/shader is a reasonable target: upload a small
retained bucket array or texture at snapshot cadence, then let a fragment
shader draw bars or traces on one panel surface. Compare it with updating a
bounded procedural mesh or instanced bars at the same cadence. Choose by
measured implementation cost and XR render/update performance, not by an
assumption that a shader is always cheaper.

The planned [MMS custom fragment shader first slice](mms-custom-fragment-shader-first-slice.md)
only specifies scalar `f32` parameters. It does not yet support arrays,
storage buffers, or texture parameters. A bucket-driven material therefore
needs a small renderer-owned data binding or an explicit extension to that
contract. Do not send one shader parameter update per FFT bin or compile a
pipeline for each spectrum frame. Keep GPU resources bounded and update them
at the retained snapshot rate with frame-safe lifetime handling.

## Acceptance

1. A test scene displays raw, filtered, both, and neither for the same
   microphone observer. The both view aligns timestamps and axes; a stale
   side is visibly marked rather than silently compared.
2. A test tone below the high-pass cutoff drops in the post-filter view, and
   changing cutoff/Q updates the display and the mouth-response RMS together.
3. Source loss, stream rebuild, panel collapse/restore, and observer removal
   clear or mark the view correctly without leaking probe or GPU resources.
4. Record worker, main-thread upload, and render cost in desktop and XR modes
   at the chosen resolution and update rate. With all probes disabled, the
   scene incurs no spectrum compute or bucket uploads.
