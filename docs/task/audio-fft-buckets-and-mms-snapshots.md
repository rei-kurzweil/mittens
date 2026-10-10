# Task: FFT bucket count and copied MMS snapshots

Date: 2026-10-09
Status: proposed contract; no implementation in this documentation pass.
Parent: [shared FFT](shared-fft-audio-node-and-phonemes.md).
Release: [0.10.0](epic/0.10.0/README.md).
Consumer: [ordinary-geometry spectrum panel](audio-spectrum-visualization.md).
Companion: [time-slice spacing and history delivery](audio-fft-time-slices-and-transport-grid.md).

## First API

Proposed MMS, not implemented syntax:

```mms
let spectrum = FFT.from(microphone).buckets(32) {}
let frame = spectrum.snapshot()
let values = frame.buckets  // copied MMS array; engine values are f32
let edges = frame.edges_hz // B + 1 edges for B values
```

Use one bucket-count builder on the FFT component. Start with 32 as the default
for the diagnostic example. The underlying FFT size/window/hop are independent:
32 display bands can be aggregated from a 4096-frame transform. Increasing
bucket count cannot create frequency resolution absent from the transform.
The setting works for detached input observers and inline output taps alike.
Frequency bucket count is separate from history span and time-slice spacing;
the latter may be expressed in fractions of a transport beat without MIDI.

V1 needs no arbitrary edge lists, plug-in bucketing functions, or live bucket
count setter. Require a finite positive integer and a documented maximum;
benchmark a provisional capacity of 256 buckets. Reject unsupported counts
rather than silently clamp or invent empty interpolated bands. Serialize the
authored count; allocate runtime storage off callbacks. Recreating/reconfiguring
with another count changes configuration generation and invalidates old results.

## Bins, buckets, and levels

Keep raw one-sided FFT bins internal to the FFT core/test seam. Component
results contain the configured number of aggregated buckets, not every raw bin
copied into MMS. Initial diagnostic policy: equal-width **linear-frequency**
bands from DC to the actual Nyquist frequency, `sample_rate / 2`:

```text
edge[i] = i * (sample_rate / 2) / bucket_count, i = 0..bucket_count
bin_frequency[k] = k * sample_rate / fft_size
```

Assign each bin by its center frequency to exactly one half-open band; the last
band also includes Nyquist. Define DC/Nyquist normalization in the shared core.
Counts must leave every band with at least one real bin under this policy.
Do not label bucket index as Hz, pad to a requested count, or claim a bucket's
width is the FFT's frequency resolution.

Recommended aggregation: sum normalized one-sided **power** within each band,
then square-root it to return a nonnegative linear band RMS level. Use the
shared core's window/energy normalization and document the reference. This
preserves represented energy when adjacent bands are regrouped; averaging bin
magnitudes or averaging dB has different semantics. These are spectrum band
levels, not vowel/phoneme confidences or a normalized probability distribution.

The panel may convert levels to dB relative to the documented reference and
clamp to a fixed display floor, e.g. -80 dB. Never normalize each frame to its
own brightest bucket: that makes silence/noise appear as strong as speech and
makes pre/post comparisons misleading. Test on-bin/off-bin tones, band edges,
DC/Nyquist, and summed band power against the documented window convention.

At 48 kHz, 32 linear bands span 750 Hz each. This is a useful first whole-range
picture, but cannot resolve a 120 Hz high-pass transition. More buckets refine
display bands only up to the FFT's actual resolution. Keep low-cutoff acceptance
tests against raw bins; add a narrow frequency range or a fixed log-frequency
preset in a later focused slice before presenting coarse frequency columns as a filter-tuning
instrument. Future policies must be named and publish actual edges, not silently
change the meaning of `.buckets(count)`. Log bands need an explicit DC policy.

## Coherent snapshot

`snapshot()` copies one latest main-thread-retained record without a worker
round trip or waiting for fresh audio. Proposed fields:

- `buckets`: exactly B finite nonnegative levels; `edges_hz`: B + 1 edges
  when the negotiated format is known.
- Status/validity, source/tap identity, source/configuration generation,
  sequence, captured frame interval/time and declared time domain.
- Actual sample rate, FFT size, window/hop, aggregation/level convention,
  channel policy, and drop/discontinuity counters.

Represent this through supported MMS arrays/records and validate the host-value
conversion in the implementation slice. `f32[]` describes engine data, not a
new MMS type annotation prerequisite. Returned arrays are script-owned copies;
mutating them cannot change the component or worker. Verify indexing/iteration,
retained host results, and nested history storage in real runtime callbacks;
the older array-access example contains historical unsupported-syntax comments
and is not evidence that the complete path works today.

Before negotiation, return invalid/pending status, B zero values, and no frequency
edges; do not invent a sample rate. After loss, return neutral/invalid values
with known-format metadata explicitly marked non-live. A valid silence frame
has a real sequence/interval and zero levels. A stale handle is an ordinary host
method error. Sequence is not a script poll counter: multiple reads may return
the same frame. Readers must use source/configuration generation with sequence
to detect changes and resets.

Taking a UI snapshot means **copy the latest completed FFT window**. It is not
a request to capture an instantaneous transform or begin collecting a fresh
window on click. If no window is valid, show pending/invalid and keep the last
deliberately frozen view labelled with its original time. A request/wait-for-next
API can follow only if needed. Pausing the UI does not disable the FFT component.

## Acceptance

The latest-frame `snapshot()` remains the manual-read API. Fine rolling history
requires the companion task's bounded batch read; faster transport subdivisions
cannot be reconstructed from sparse UI polling of a single retained frame.

- [ ] `.buckets(count)` validation, default, serialization, and bounded storage
  are explicit and tested; bucket count changes never alter FFT size silently.
- [ ] Known signals prove edges, assignment, power aggregation/scaling, and
  equivalent results through input/output adapters.
- [ ] One coherent retained record reaches MMS as copied arrays and metadata,
  including pending/silence/loss, same-sequence reads, reconfiguration, and
  old-generation rejection.
- [ ] The manual snapshot example renders 32 frequency columns from that record without
  custom material/pipeline work. UI history uses bounded B-by-T storage.
