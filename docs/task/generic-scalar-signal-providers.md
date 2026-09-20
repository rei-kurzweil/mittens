# Generic scalar signal providers

Date: 2026-09-19
Status: exploration / not an implementation commitment

## Why this exists

Several components expose a current numeric control value that is produced on
its own schedule and consumed later by another system. The immediate example
is microphone amplitude:

```text
AudioInput callback -> Amplitude -> VolumeNormalization -> AVC mouth weight
```

`Amplitude` and `VolumeNormalization` both ultimately provide a finite `f32`
level sample. AVC currently needs only one such current value to drive the
mouth-open fallback. Other plausible sources include a viseme confidence,
speech activity detector, OSC float, animation output, gamepad axis, sensor
measurement, or a scripted control source.

The current raw-or-normalized level helper is intentionally a small local seam,
not the final general abstraction. It lets the volume-normalization work land
without prematurely imposing a graph model on all engine signals.

## Important distinction: this is not the existing Signal bus

Mittens `Signal`/`IntentSignal` values are typed events and mutation requests
with explicit drain points and routing scope. A scalar provider is a retained
latest-value observation. It is pull-read by a consumer, may be updated by a
different thread, and does not imply that every change must emit an event.

It is also not an audible audio graph. A scalar `0.2` may have originated from
audio analysis, but it is a control value only; passing it to AVC must never
alter PCM or capture routing.

## The useful common contract

If this becomes real, the common item should be more than a bare `f32`. A
consumer needs enough provenance to reject stale or invalid asynchronous data:

```rust
struct RetainedScalarSample {
    generation: u64,
    sequence: u64,
    timestamp_sec: f64,
    value: f32,
    status: ScalarStatus, // Pending | Live | Neutral | Invalid
}
```

`Live` requires finite data. `Neutral` is an intentional current value, often
zero. `Pending` has no accepted sample for the current generation, and
`Invalid` records a source, configuration, lifecycle, or discontinuity failure.
Those states must not be encoded with a magic float such as `NaN`.

The specific domain is also part of compatibility. A mouth driver usually
wants a non-negative normalized activity/confidence value. A gamepad axis can
be signed, and a distance is measured in metres. A single unlabelled float
type must not silently connect semantically incompatible values.

## Candidate integration shape

Do not use a Rust trait object stored in ECS as the first public design. ECS
component identity, serialization, and lifecycle are clearer if the authored
reference remains a `ComponentRef` to the concrete provider. A narrow resolver
can then identify supported providers:

```text
read_scalar_sample(component_id, expected_kind) -> Option<RetainedScalarSample>
```

Initially, `expected_kind` can be an internal enum such as `MouthOpenLevel` or
`NonNegativeLevel`, not a fully general unit system. A caller checks enabled,
generation, status, freshness policy, and domain before using the `value`.

Concrete components retain their own configuration and runtime ownership:

- `Amplitude` owns raw rolling PCM RMS/peak measurement.
- `VolumeNormalization` owns callback-side AGC state and emits a normalized
  level snapshot.
- a future OSC provider owns packet parsing and bounded handoff.
- AVC owns the mapping and smoothing from a valid level to a morph weight.

This preserves a diagnostic advantage: raw and normalized meters remain
separate authored components even if AVC can consume either one.

## Asynchrony and retention rules

A provider is a stream of observations, but most visual consumers do not need
to process every element. They read the newest valid retained sample once per
frame. Therefore the transport should be bounded and allowed to drop older
samples, while the retained sample has monotonically increasing `sequence`.

Every producer that can be rebuilt or replaced must use a generation boundary:

```text
source change / disable / discontinuity
  -> increment generation; clear retained output
  -> reject queued snapshots from older generations
```

The producer thread must retain ownership of callback state and preallocated
buffers. The main thread validates and retains snapshots; it does not run a
stateful audio controller from intermittent reads of a raw retained sample.

Freshness is consumer policy, not a reason to erase producer provenance. AVC
may treat an old live sample as neutral; a diagnostic meter may continue to
show it as stale along with its timestamp.

## A possible later graph, deliberately deferred

The word “graph” is useful as a mental model, but no generic graph should be
introduced merely to avoid a small match over current providers. A future
scalar-control graph would need explicit answers for all of these:

- Pull reads, pushed snapshots, or both?
- Which thread owns each node and each edge?
- Are values held, interpolated, sampled at frame time, or resampled at a
  declared rate?
- How are units/domains declared and converted?
- How are cycles detected and resolved?
- What are serialization, query, replacement, and generation semantics?
- Can a node allocate, log, lock, or execute MMS on a real-time callback?
- Which operations are generic (map, clamp, smooth, combine) versus
  domain-specific (audio RMS, mouth calibration, pose solve)?

Until those questions have concrete consumers, retain provider-specific
components and add only narrow resolver categories where needed.

## Relationship to adaptive volume normalization

The current implementation sequence should remain:

1. Finish callback-owned `VolumeNormalization` for `Amplitude` / `AudioInput`.
2. Let AVC accept raw and normalized non-negative level providers through the
   narrow retained-level resolver.
3. Exercise it in the microphone-speaking scene with raw and normalized
   diagnostics.
4. Revisit a generic scalar-provider contract once at least one non-audio
   asynchronous source and one additional consumer need the same semantics.

This avoids making the AGC task wait for a general control-signal runtime while
leaving a documented compatibility path open.

## Questions to revisit

1. Should `AmplitudeSample` evolve into a shared scalar sample, or should a
   new generic type be introduced while amplitude keeps RMS/peak-specific data?
2. Is one `value` enough, or should scalar providers also expose optional
   quality/confidence and peak metadata?
3. Which domain categories are useful before a full unit system exists?
4. Do consumers need timestamp freshness limits as authored policy?
5. Should generic scalar transforms be ECS components, MMS expressions, or a
   separate control runtime when a real graph is justified?
