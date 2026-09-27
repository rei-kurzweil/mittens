# Epic: inspect the spectrum of audio analysis inputs

Status: planning. This epic records the proposed architecture and delivery
sequence; it does not add FFT processing or a renderer.

## Performer and developer use case

While tuning microphone-driven mouth motion, show which frequencies reach the
rolling RMS observer. Changing the high-pass cutoff or resonance should make
its effect visible. For a filtered `Amplitude`, independently select the raw
input before its filter, the PCM after its filter and before RMS, both aligned
views, or neither. The visualization is diagnostic and opt-in; ordinary mouth
response should carry no FFT cost.

The same retained spectrum format should eventually work for a playing clip,
oscillator, or output graph node. Phase one is a microphone analysis tap. A
clip's one-time decode worker prepares an asset; a live spectrum of playback
would need a tap on the samples actually rendered at playback time, not a
one-time FFT during asset decode.

## Architecture direction

Today `AudioInputSystem` builds an independent `RollingRms` for each amplitude
consumer. A filtered unit owns its high-pass biquad, so its post-filter signed
PCM is private to that unit. The capture callback also runs separate fused
RMS/normalization units. The rendering audio graph is a different runtime and
does not provide the input observer's filter output.

Introduce a small, compiled **analysis plan** per active input source: signed
PCM enters a raw tap, optional filters process it, and named taps fan out to
RMS and spectrum consumers. The first plan needs only one filter stage per
filtered amplitude observer and two tap positions. It is not a general audio
mixing graph. Make source identity, tap identity, filter parameters, sample
format, and generation explicit so future source runtimes can provide the same
tap contract. Keep the existing audible graph separate until a concrete
post-effect observer needs integration.

For the first slice, factor the high-pass PCM stage out of `RollingRms` enough
to let the RMS unit and a diagnostic spectrum probe consume the same output.
An unfiltered amplitude continues to use raw PCM. Do not make a second filter
with merely matching coefficients for a post-filter probe: independent state
could disagree during live changes. Avoid rebuilding the CPAL stream when a
probe is enabled or a filter parameter changes if the provisioned callback
plan can be swapped safely. The backend task resolves the exact plan handoff.

## Tickets and order

- [ ] [Capture spectrum buckets at analysis taps](../audio-input-spectrum-taps.md):
  microphone first; source and filter boundaries, optional pre/post probes,
  bounded worker handoff, retained buckets, and deterministic signal tests.
- [ ] [Visualize retained spectrum buckets](../audio-spectrum-visualization.md):
  a reusable diagnostic panel/material, live updates, pre/post comparison, and
  measured render/update cost.

The backend can first expose and test retained buckets without a scene. The
visualizer then consumes that stable result. A later source-runtime ticket can
add playback and graph-node taps after the microphone contract is proven.

## Integrated acceptance

1. A microphone-driven high-pass `Amplitude` and RMS measurement continue to
   work when no spectrum observer exists.
2. Selecting pre, post, both, or neither changes only diagnostic work. Paired
   pre/post views use the same source frames and report their tap positions.
3. A live cutoff or Q change produces a fresh post-filter spectrum and RMS
   from the same filter state. Old buckets cannot survive a source, format, or
   filter generation change as if they were current.
4. A reusable scene view shows the cutoff's effect at roughly four updates
   per second without pushing FFT or GPU work into the capture callback.
5. CPU time, queue drops, memory, and render updates are measured with the
   visualizer disabled, enabled once, and showing both taps.

## Related work

- [High-pass amplitude observer](../high-pass-amplitude-observer.md)
- [Audio amplitude observation](../audio-amplitude-observation.md)
- [Audio-node metering](../audio-node-metering.md)
- [MMS custom fragment shader first slice](../mms-custom-fragment-shader-first-slice.md)
