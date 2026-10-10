# Task: reusable spectrum visualization for audio analysis

Status: planned. Parent: [audio spectrum debugging epic](epic/audio-input-spectrum-debugging.md).
Depends on: [FFT buckets from audio analysis taps](audio-input-spectrum-taps.md).

Updated: 2026-10-09. Release: [0.10.0](epic/0.10.0/README.md).
Current backend plan: [shared FFT](shared-fft-audio-node-and-phonemes.md), with
[configurable bucket count and copied MMS snapshots](audio-fft-buckets-and-mms-snapshots.md).
History timing: [time slices and transport-relative resolution](audio-fft-time-slices-and-transport-grid.md).
The first implementation uses ordinary scene geometry and existing UI assets;
custom materials and an FFT-specific renderer pipeline are not prerequisites.

## Goal

Show the retained frequency buckets for microphone mouth-response tuning in a
scene panel. Make raw and post-filter spectra independently selectable, with
a clear both-view comparison. When the performer changes the high-pass cutoff
or resonance, the display should reveal the effect in a few updates without
changing the mouth-response signal or audio monitoring route.

Deliver a raw-microphone manual snapshot first, a small Start/Stop history
second, and exact pre/post-filter comparison when those backend taps exist.
The first coarse bucket view proves the data path; it does not yet resolve
a low high-pass cutoff across the full microphone frequency range.

This is a reusable diagnostic view for a spectrum observer, not a Reimu-only
panel. The existing shared voice-response panel may host or open it, but the
spectrum view should also work without an avatar. Treat the probe's tap choice
as analysis configuration; hiding a graph should not imply that a still-active
probe has stopped doing FFT work unless the UI explicitly disables it.

## Existing example and reusable assets

[mittens-corp-agc.mms](../../examples/mittens-corp-agc.mms), particularly
`make_level_history_graph`, samples raw RMS and gain at 10 Hz and rebuilds
two twelve-sample cube views. It uses retained scalar methods, shared
[info_panel / info_panel_body](../../assets/components/ui/info_panel.mms),
[button](../../assets/components/button.mms), `Click`, and `FrameTick` handlers.
The minimized accordion removes its body; the example pauses reads/rebuilds
and recreates content on `AccordionRestoreRequested`.

Reuse those patterns, including glyph-unit/world-transform conversion, as a
starting point. Do not copy its manually named scalar slots or assume rebuilding
hundreds of cells at 10 Hz has the same cost. The FFT panel uses copied arrays
and bounded rows. Check host-array indexing/iteration/history storage in the
runtime before committing the example implementation.

## First example: Snapshot and 32 frequency columns

Plan `examples/audio-fft-snapshot.mms`, with a microphone and proposed
`FFT.from(microphone).buckets(32)`. Start paused. One Snapshot button reads one
coherent latest retained record and creates or updates one ordinary cube/column
per bucket. Frequency increases left-to-right; column height is the band level
mapped through the documented fixed dB display range. Values are f32-derived
MMS numbers, not scene components returned by the FFT.

Build a reusable planned `assets/components/ui/fft_info_panel.mms` wrapper around
the existing info-panel asset, keeping source binding separate from avatar UI.
Display the source, frame age/time, status, frequency edges/range, level scale,
and bucket count. Snapshot copies the latest completed window; it does not
block to acquire a window starting at click time. Pending/invalid data needs a
clear message rather than a fabricated fresh plot.

Use the existing built-in materials for frequency columns and panel content. No shader
parameter array, custom GPU buffer, shader compilation, or dedicated FFT
pipeline is needed for this proof. Prefer opaque frequency columns/cells on a common plane
to avoid making transparency ordering another dependency.

## Next example: Start/Stop and flat spectrogram

Keep a single latest-spectrum frequency-column view and optionally show a bounded history:
frequency on X, sampled time on Y, level as cell color. All cells have constant
geometry depth; this is a flat spectrogram, not a displaced 3D height field.
It represents the same B-by-T values that a height field would use.

Configure history span separately from time-slice spacing. Use seconds or
beat-duration units for the span, and seconds or a fraction of one beat's
duration for spacing. Derive durations from the selected BPM and run them on
the capture timeline; no output beat-phase alignment is required. Transport
pause/seek alone does not freeze or reset capture history.
The proposed transport example is two beats with one slice every 1/16 beat:
T = 32 slots, B = 32 buckets, at most 1024 history cells plus 32 frequency columns.
At 120 BPM this spans one second and requests 32 slices/second; at 60 BPM it
spans two seconds and requests 16 slices/second. Four UI refreshes/second can
update several slices at once. These are prototype bounds to measure, not a
promised rate. Cap stored arrays and live scene objects explicitly.

The first latest-only prototype can use coarse sampled history. The requested
fine time grid needs the timing adapter and bounded batch/history delivery in
the linked time-slice task; screen refresh must not silently set time resolution.

- **Snapshot:** refresh the frozen frequency-column view once; while stopped, it does not
  resume history or append a fake time row. Disable it while continuously
  running if that simplifies the first UI.
- **Start:** begin reading retained snapshots or bounded slice batches at the
  panel refresh cadence. Add only
  a new `(generation, sequence)`; never append duplicates when polling faster
  than FFT publication. Evict the oldest row when full.
- **Stop:** stop retained FFT reads and geometry/color updates. Freeze the
  latest frequency columns/history and label them Paused with their source times. The FFT
  component continues analyzing while enabled; stopping analysis is a separate
  component lifecycle action.
- **Minimize:** suspend visual reads/updates and release the body as the asset
  already does. Preserve arrays and user Start/Stop intent separately from
  body visibility; restoring a stopped view must not silently start it.
- **Resume/restore:** do not catch up missing history by repeating the latest
  spectrum. Show the elapsed-time gap or reset live history, with a separate
  boundary on source/configuration changes. Rebuild only the bounded body and
  bind exactly one set of control handlers.

Each history row is a **time slice** drawn from a spectral frame, with a PCM
window interval; window length, FFT hop, slice spacing, and screen refresh are
distinct. Latest-only reads make the coarse prototype a sampled diagnostic
history. Fine spacing needs actual produced/retained frames, not duplicated or
interpolated readings. Label time gaps rather than assume uniform audio coverage
from uniform row spacing; old/new rows must not silently mix different bucket edges,
sample rates, or generations. Pre/post comparison requires matched frame
intervals, not merely readings made in the same FrameTick.

Initial manual snapshots may rebuild 32 frequency columns using the AGC pattern. For rolling
history, prefer retaining bounded row/cell handles and updating/recycling only
the incoming row using supported transform/color methods. If the prototype
instead rebuilds the grid, measure attach/remove/query/handler costs and keep
its cadence conservative. Geometry updates, retained reads, and row eviction
run at the ordinary script/main-thread boundary, never in an audio callback.

## Display contract

- Read only main-thread-retained snapshots. Never read callback or worker
  buffers from render code. Show pending, invalid, stale, and dropped data
  clearly; a deliberately stopped historical view remains labelled Paused,
  while a running view must not present a source's old last frame as live.
- Label the source, tap position, cutoff/Q when applicable, sample rate, FFT
  window size, frequency range, and level convention. Raw and post-filter
  traces need distinct, themeable colors and the same axes when compared.
- Start near four visible updates per second and measure whether a different
  rate improves tuning. Interpolation may smooth the display but must not
  invent frequency detail or hide a generation reset.
- First use the backend's explicit linear bucket edges and a fixed dB display
  range. At 48 kHz, 32 full-range bands are 750 Hz wide, too coarse to inspect
  a 120 Hz transition. Log-frequency/narrow-range views are later additions
  with their own aggregation contract; do not relabel linear buckets as log
  data or imply a cutoff is resolved merely because there are 32 frequency columns.
- Reuse the pastel theme vocabulary of the shared voice-response panel, with
  enough contrast to tell pre and post apart in a headset.

## Later rendering choice

A custom spectrum material/shader is a possible later optimization: upload a small
retained bucket array or texture at snapshot cadence, then let a fragment
shader draw frequency columns or traces on one panel surface. Compare it with updating a
bounded procedural mesh or instanced frequency columns at the same cadence. Choose by
measured implementation cost and XR render/update performance, not by an
assumption that a shader is always cheaper.

This follows the [custom-materials epic](epic/custom-materials.md) only if
ordinary geometry becomes a measured bottleneck or a richer view needs it.
Do not add a hard-coded FFT material/pipeline to ship the first diagnostic.

The planned [MMS custom fragment shader first slice](mms-custom-fragment-shader-first-slice.md)
only specifies scalar `f32` parameters. It does not yet support arrays,
storage buffers, or texture parameters. A bucket-driven material therefore
needs a small renderer-owned data binding or an explicit extension to that
contract. Do not send one shader parameter update per FFT bin or compile a
pipeline for each spectrum frame. Keep GPU resources bounded and update them
at the retained snapshot rate with frame-safe lifetime handling.

## Acceptance

1. A standalone raw-microphone Snapshot example renders exactly the configured
   number of ordinary frequency columns from one copied record, without custom material
   infrastructure. Known tones verify bucket labels/scaling; invalid data and
   intentionally frozen data cannot be mistaken for live measurements.
2. The reusable FFT info panel has working Start/Stop, independently configured
   history span/time-slice spacing, and bounded flat history. Seconds and beat
   subdivision modes follow the time-slice task using source-local timing and
   BPM-derived spacing; transport phase alignment is optional later work.
   Duplicate sequences, long frames, pause/resume, source resets, and panel
   collapse/restore never produce fabricated history, growing object counts,
   accumulated handlers, or a stopped view that resumes itself.
3. Once exact filtered taps exist, a test scene displays raw, filtered, both,
   and neither for the same
   microphone observer. The both view aligns timestamps and axes; a stale
   side is visibly marked rather than silently compared.
4. A sufficiently resolved bucket policy shows attenuation below the high-pass
   cutoff; changing cutoff/Q updates the display and mouth-response RMS from
   the same stage. Raw-bin backend tests remain the first low-cutoff proof.
5. Source loss, stream rebuild, panel collapse/restore, and observer removal
   clear or mark the view correctly without leaking probe or GPU resources.
6. Record worker, MMS array-copy/read, scene mutation, main-thread upload, and
   render costs in desktop/XR for snapshot-only and B-by-T history. Stopped/
   minimized views perform no FFT reads or plot updates; enabled FFT compute
   may continue. With all probes disabled, no spectrum compute/handoff remains.
