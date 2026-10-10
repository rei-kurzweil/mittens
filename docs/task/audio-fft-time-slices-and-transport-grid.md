# Task: FFT time slices and transport-relative resolution

Date: 2026-10-09
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
| History span | Amount of history displayed, in seconds or transport beats |
| Time-slice spacing | Interval between displayed spectral observations, in seconds or fractions of one transport beat |
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
Label the window duration and actual hop as well as the requested grid.

## Scheduling and actual sample times

Tempo subdivisions should specify actual slice timing, not merely labels on
four-per-second readings. For an aligned mode, place window endpoints on the
selected transport grid and preserve each window's PCM start/end interval.
Round scheduled absolute boundaries to device frames while retaining fractional
timing remainder; repeatedly rounding one fixed hop must not accumulate drift.
Report requested and effective spacing. Reject rates below one PCM frame or
beyond measured transform/storage limits rather than promise unsupported data.

Capture sample time and output transport time currently have different origins.
Define their mapping/epoch and measure its quality before claiming microphone
beat-phase alignment. Until then, a BPM-derived interval may provide matching
spacing with explicitly **unaligned** phase. Reading the current engine beat in
the UI cannot retrospectively timestamp a microphone FFT window accurately.

Specify tempo edits, pause, and seek as part of the adapter contract:

- New beat boundaries follow tempo changes; retain original frame intervals,
  tempo/transport revision, and beat metadata for existing slices.
- Seeking/restarting creates a marked history boundary and rejects stale epoch
  data. Tempo changes must not reinterpret old capture timestamps silently.
- Audio capture can continue while transport is paused. Seconds mode can still
  advance; transport-aligned mode suspends new beat-grid slices and labels that
  condition. UI Stop independently freezes the displayed history.

If a seconds span uses beat spacing across tempo changes, evict by actual time
and enforce a hard cell/slice cap. Do not derive the whole retained interval from
only the latest BPM. Fix maximum bucket-by-slice storage and batch sizes before
allocating; change configuration through the normal generation/control path.

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
  restarts, and unmapped capture clocks have explicit tested behavior.
- [ ] Four UI updates/second can receive all 32 produced slices/second in bounded
  batches within supported capacity; saturation makes gaps visible.
- [ ] 32-by-32 flat geometry is bounded, controls freeze/resume predictably,
  and no custom material/pipeline is required. Measure transform, transport,
  copying, and geometry costs separately at the selected resolution.
