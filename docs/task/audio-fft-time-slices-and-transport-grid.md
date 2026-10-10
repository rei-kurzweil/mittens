# Task: FFT time slices and transport-relative resolution

Date: 2026-10-09
Updated: 2026-10-10: tempo-relative duration is required; transport phase alignment is optional follow-up work.
Status: proposed contract; docs only.
Parent: [shared FFT](shared-fft-audio-node-and-phonemes.md).
Release: [0.10.0](epic/0.10.0/README.md).
Consumer: [FFT info panel](audio-spectrum-visualization.md).

## Terminology and independent settings

Use **frequency columns** for the single-spectrum geometry, **frequency buckets**
for band values, and **time slices** for successive observations in a spectrogram.
A **spectral frame** is one FFT-window result. A musical **bar** remains a
transport/time-signature term, not the name of a frequency column.

| Setting | Meaning |
| --- | --- |
| Frequency bucket count | Number of frequency bands in each spectral frame |
| History span | Amount of capture-timeline history displayed, in seconds or BPM-derived beat-duration units |
| Time-slice spacing | Interval between spectral observations on the source timeline, in seconds or fractions of one beat's duration at the selected BPM |
| FFT window length | PCM duration used for each transform; controls frequency resolution and temporal averaging |
| FFT hop | Advance in PCM frames between successive transform windows |
| UI refresh interval | How often copied results update scene geometry; may update several slices together |

Repeated windowed FFTs form a short-time Fourier transform (STFT). Windows can
overlap: slice spacing is not window length, nor does a slice represent an
instantaneous measurement. Keep the shared core in sample/frame units; a timing
adapter resolves seconds/transport requests without making FFT math depend on
MIDI. Transport-relative audio timing does not imply MIDI events or note analysis.

## Suggested view configuration

Illustrative panel configuration, not existing MMS syntax:

```mms
{
    history_beats = 2.0
    slice_beats = 1.0 / 16.0
    refresh_seconds = 0.25
}
```

Alternatively use `history_seconds = 1.0` and `slice_seconds = 0.03125`.
Allow independent choices of span and spacing units, but reject simultaneous
seconds/beat declarations for the same setting. `slice_beats` means fractions
of **one transport beat**, not a musical note denominator: 1/16 beat differs
from a sixteenth note when a beat is a quarter note. Show explicit units in UI.
The scene chooses the transport; do not guess one from microphone audio.
Only its BPM is required for this mode. Capture can start at any output beat
position: slice endpoints do not need to coincide with the transport's beat
boundaries. `history_beats` describes a duration, not a range of output beat
positions or a promise of phase synchronization.

At constant tempo:

```text
slice_seconds = slice_beats * 60 / bpm
nominal_hop_frames = sample_rate * slice_seconds
slice_count = ceil(history_span / slice_spacing) // when units match
```

At 120 BPM, 1/16 beat is 31.25 ms. Two beats span one second and contain 32
time slots. With 32 frequency buckets, that is 1024 colored cells. At 60 BPM,
two beats span two seconds with the same 32 slots. One second at 60 BPM and
1/16-beat spacing instead needs 16 slots. Distinguish a fixed beat-count span
from a fixed-duration span, especially across tempo changes.

A 4096-frame window at 48 kHz spans about 85 ms; a 31.25 ms hop therefore uses
overlapping windows. Their observations are not independent 31.25 ms chunks.
Label the window duration and actual hop as well as the requested spacing.

## Scheduling and actual sample times

Tempo subdivisions should specify actual slice timing, not merely labels on
four-per-second readings. Resolve the requested fraction and BPM into PCM
intervals using the source's sample rate, and schedule successive window
endpoints on that source's own frame timeline. Preserve each window's PCM
start/end interval. Retain a fractional-frame remainder when rounding scheduled
boundaries; repeatedly rounding one fixed hop must not accumulate drift.
Report requested and effective spacing. Reject rates below one PCM frame or
beyond measured transform/storage limits rather than promise unsupported data.

No mapping between capture and output clocks is needed for this initial
contract. Both use the same BPM-derived duration, while phase and physical
clock drift are unconstrained. BPM/configuration updates cross the ordinary
control path; neither the capture callback nor FFT core queries the transport.
Source timestamps remain source-relative, not output beat positions.

Initial tempo/pause/seek behavior:

- Apply a changed BPM to future slice intervals at a defined source-frame
  boundary. Retain original intervals and tempo revision for existing slices;
  never retimestamp them using the latest BPM.
- Output transport pause/seek/restart alone does not stop or reset microphone
  FFT history. Continue with the last valid selected BPM. A BPM/configuration
  change updates spacing; source loss/restart changes the source epoch.
- UI Stop independently freezes the displayed history. Disabling the FFT
  component independently stops its analysis work.

If a seconds span uses beat spacing across tempo changes, evict by actual time
and enforce a hard cell/slice cap. For a beat-duration span, accumulate local
tempo-relative elapsed units from source-frame intervals and the BPM effective
during each interval; evict by that local duration coordinate, not by output
transport position or the latest BPM alone. Preserve timing metadata across
changes. Fix maximum bucket-by-slice storage and batch sizes before allocating;
change configuration through the normal generation/control path.

## Optional later phase alignment

Only a future explicitly aligned mode would place window endpoints on output
transport beat boundaries. That mode needs capture-to-output clock mapping,
phase/drift handling, mapping quality/epochs, and its own pause/seek policy.
Do not make those mechanisms prerequisites for proportional slice durations
or 0.10.0's initial tempo-relative view. Reading an output beat in the UI is
insufficient to establish a microphone window's phase alignment.

## History delivery prerequisite

`snapshot()` remains the latest-frame read for the manual frequency-column view.
It is insufficient for 32 slices/second when the UI refreshes four times/second:
seven of every eight observations could disappear between reads.

Add an optional bounded main-thread-retained spectral-frame history and a
copied batch-read/cursor contract for the rolling view. Proposed naming:
`read_slices(after_sequence, max_slices)` with epoch/generation validation;
freeze the actual method signature with the MMS host-value implementation.
The worker produces the requested cadence and the UI drains bounded batches
without waiting. Capacity covers the supported span/refresh policy, not unlimited
audio recording. Queue/ring overwrites report gaps; duplicate frames do not fill
missing slots. Paused/minimized views can miss history and must show a boundary
or reset on resume. Retire the history consumer when no longer needed.

Do not run extra transforms merely because the renderer redraws. Conversely,
do not synthesize 1/16-beat detail from 4 Hz frames. The coarse latest-only
prototype remains useful, but label it sampled history rather than the complete
requested time grid. A later resampling/reduction mode requires explicit rules.

## Acceptance

- [ ] Seconds and beat-span/subdivision requests yield the expected bounded
  slice counts, with actual PCM intervals and window/hop metadata.
- [ ] Tempo changes, fractional-frame scheduling, transport pause/seek, source
  restarts, and source-local timestamps have explicit tested behavior. Output
  pause/seek leaves capture history running; no cross-clock mapping is required.
- [ ] Four UI updates/second can receive all 32 produced slices/second in bounded
  batches within supported capacity; saturation makes gaps visible.
- [ ] 32-by-32 flat geometry is bounded, controls freeze/resume predictably,
  and no custom material/pipeline is required. Measure transform, transport,
  copying, and geometry costs separately at the selected resolution.
