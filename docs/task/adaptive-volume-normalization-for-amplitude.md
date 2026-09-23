# Adaptive volume normalization for amplitude-driven AVC

Date: 2026-09-19
Status: design sketch

## Naming decision

The canonical component remains **`VolumeNormalization`**. **AGC** is the
short form for its automatic-gain-control behaviour in labels, comments,
diagnostics, and discussion. This is a bounded adaptive control signal for an
avatar, not integrated audio-program loudness normalization:

```mms
let voice_level = VolumeNormalization.from(raw_voice_level) {}
```

Do not rename the Rust component, module/file, MMS registry entry, builders,
tests, or existing example paths. The implementation documents precisely that
this `VolumeNormalization` is AGC-style control, not loudness normalization.

## Working integration scene

`examples/mittens-corp-agc.mms` is the XR history and A/B
scene: it keeps raw input visible while AVC consumes
`VolumeNormalization.from(raw_voice_level)`.

`examples/mittens-corp-agc-desktop.mms` is the desktop-only tuning scene. It
uses the ordinary `Input` pose driver, microphone input, and Bisket's AVC
mouth fallback; no XR runtime is required. Its title-only draggable “Bisket
mouth response” info panel edits the AVC's RMS mapping while leaving the AGC
policy at its current defaults.

### Traffic-light follow-up

`assets/components/traffic_light.mms` contains the first horizontal
red/yellow/green fixture draft: emissive circular lenses, a yellow enclosure
and hood per lens, five inward black planes, and five matching outward yellow
planes. When the asset is first staged, verify plane winding/back-face culling,
close any visible corner seams, tune hood depth/pitch, and decide whether each
signal needs an authored on/off state instead of all three lenses emitting.

## Goal

Add a source-analysis component that adapts microphone level into a useful,
stable range so ordinary quiet speech can animate an AVC mouth without asking
the performer to yell.

The intended authoring shape is:

```mms
let microphone = AudioInput {}
let raw_level = Amplitude.rolling_window(0.080).from(microphone) {}
let voice_level = VolumeNormalization.from(raw_level) {
    // Configuration names and defaults remain to be finalized.
}

AVC {
    mouth_open_from_amplitude(voice_level)
}
```

`VolumeNormalization` is distinct from `Amplitude`:

- `Amplitude` reports source PCM RMS and peak without changing their scale.
- `VolumeNormalization` retains adaptive gain state and reports RMS/peak after
  applying that virtual gain.
- Neither component changes, monitors, or writes audible PCM. This is analysis
  gain for downstream control signals, not an audio graph effect.

In signal-processing terms this is a bounded automatic gain control (AGC), not
integrated loudness/LUFS normalization. The user-facing name is
`VolumeNormalization`; document its AGC behavior precisely.

## Why this is a separate component

The existing path has clean boundaries:

```text
AudioInput callback
  -> preallocated rolling RMS unit
  -> retained AmplitudeSample on AmplitudeComponent
  -> AVC fixed floor/ceiling mapping
  -> viseme_aa morph contribution
```

`AmplitudeComponent` is deliberately an observer, and
`AvatarControlComponent` deliberately owns the fixed mapping from RMS to mouth
weight. Adaptive gain is stateful source conditioning: putting it into either
one would make raw meters misleading or make the behavior AVC-specific.

A wrapper component also lets the same raw amplitude feed diagnostics while a
normalized view feeds AVC. Other future consumers can use the normalized view
without knowing about avatar morphs.

## Proposed component contract

`VolumeNormalization.from(amplitude)` accepts an `AmplitudeComponent` (and,
later, another compatible level provider if there is a concrete need). It is a
host-owned live component handle, not an immediate scalar.

Its authored state should include at least:

- upstream amplitude reference;
- enabled state;
- minimum and maximum gain, preferably authored in dB;
- the normalized RMS range that represents useful activity;
- quiet hold duration before gain may rise;
- gain-up and gain-down rates or time constants;
- sustained-high hold duration;
- peak headroom; and
- a low-level activity/noise gate.

Its retained runtime state should include:

- current linear gain and gain in dB;
- normalized `AmplitudeSample` (RMS, peak, timestamp, sequence, status);
- quiet/high timers or envelopes;
- source/generation identity; and
- an optional reason such as `Holding`, `Raising`, `PeakReducing`,
  `SustainedReducing`, or `Invalid` for diagnostics.

Runtime state is never serialized. Disable, upstream invalidation, source
replacement, discontinuity, or removal starts a new generation and clears the
normalized result instead of retaining stale mouth movement.

### Minimal MMS diagnostics

The first public scalar surface deliberately has only the reads needed by AVC
debugging and the gain-history graph:

```mms
raw_level.value()    // raw retained RMS
voice_level.value()  // AGC-adjusted retained RMS
voice_level.gain_db() // signed gain currently applied by AGC
```

`gain_db()` belongs on `VolumeNormalization`, not `Amplitude`: it reports the
controller's own decision, rather than a changing difference between two speech
measurements. Do not expose peak, sample timestamp/sequence, liveness, reason,
or linear gain until a concrete MMS consumer requires one.

### Live MMS policy updates

The same live component handle accepts validated control updates:

```mms
voice_level.set_gain_limits(-24.0, 24.0)
voice_level.set_target_rms(0.027, 0.030)
voice_level.set_activity_gate(0.003)
voice_level.set_quiet_hold(0.75)
voice_level.set_high_hold(0.75)
voice_level.set_gain_rise(3.0)
voice_level.set_gain_fall(12.0)
voice_level.set_peak_headroom(0.9)
```

These calls validate on the main thread and publish the scalar policy through
a stable lock-free handle. The capture callback reads it at its next control
step; the stream, rolling buffers, AGC timers, gain state, and component
generation are retained. Source, window, enable, and lifecycle changes still
rebuild or reset the unit as appropriate.

## AVC compatibility

AVC currently requires an `AmplitudeComponent` in
`mouth_open_from_amplitude(...)`. The normalized component should be usable in
the same slot:

```mms
AVC {
    mouth_open_from_amplitude(voice_level)
}
```

Avoid making `VolumeNormalization` inherit or literally contain an ECS
`AmplitudeComponent`. Rust/ECS component identity and lifecycle are clearer if
both implement a small internal level-provider abstraction, or AVC uses a
helper that can read a current sample from either concrete component:

```text
read_level_sample(component_id) -> Option<RetainedLevelSample>
```

The durable AVC reference remains a reference to the actual authored
component. Serialization must preserve whether it points to raw or normalized
level. Existing `Amplitude` scenes continue unchanged.

The current AVC floor/ceiling and smoothing still apply after normalization.
That is useful separation:

- normalization adapts differing speaker/microphone level over seconds;
- AVC floor/ceiling determines the desired mouth response curve; and
- AVC smoothing handles short visual motion over frames.

Defaults should make the normalization target agree with AVC's useful RMS
range, but should not silently rewrite AVC calibration.

## Audio-thread ownership and preallocation

Although the MMS surface wraps `Amplitude`, do not implement this as a
main-thread chain that reads the retained raw snapshot and occasionally adjusts
it. The current input callback already owns one preallocated `RollingRms` per
amplitude consumer. Extend registration so one normalized consumer produces a
fused, preallocated analysis unit for the same source and window:

```text
VolumeNormalization
  -> resolves upstream Amplitude
  -> resolves that Amplitude's AudioSource and rolling-window request
  -> registers (normalizer id, generation, source, window, live AGC policy handle)
  -> callback-owned RollingRms + fixed-size AGC state
  -> bounded normalized snapshot queue
  -> main thread validates generation and retains newest result
```

The callback unit may share a source stream but owns independent fixed-size
window and control state. Construct/rebuild buffers on the main/control thread
before capture starts. Callback work remains fixed arithmetic and bounded queue
pushes: no allocation, ECS access, logging, locks, or blocking. Live scalar
policy reads are atomic; they do not dynamically reconfigure the stream or its
preallocated buffers.

The raw upstream `Amplitude` should remain a live observer as authored, so raw
and normalized meters can be shown together. An optimization that shares its
rolling statistics with the normalizer can come later; it must not couple their
lifecycle or make callback mutation order observable.

Audio clips and oscillators should eventually use the same level-provider
contract, but the first implementation can be explicitly `AudioInput`-only if
their production paths cannot yet host the same preallocated unit.

## Control behavior

Use dB internally for rate decisions and linear gain for applying the result:

```text
raw_rms_db = linear_to_db(raw_rms)
gain_db in [min_gain_db, max_gain_db]
normalized_rms = raw_rms * db_to_linear(gain_db)
normalized_peak = raw_peak * db_to_linear(gain_db)
```

The controller needs hysteresis and separate upward/downward behavior.

### Raising gain

Raise gain only when all of these remain true for `quiet_hold`:

1. the source is valid and current;
2. the raw level is above an activity/noise gate (there is likely voice, not
   digital silence or room noise);
3. normalized RMS is below the lower edge of the useful mouth range; and
4. normalized peak still has headroom.

After the hold, move gain upward slowly, capped by `max_gain_db`. Stop raising
as soon as the useful range is reached. A rate in dB/second makes behavior
independent of callback block size.

The activity gate is important: a literal “quiet for N seconds” rule will turn
silence up to maximum and make the next breath/noise spike open the mouth. A
fixed gate is sufficient for the first slice; adaptive noise-floor estimation
can be explored later.

### Reducing gain

Use two reduction conditions:

1. **Peak safety:** if virtual-gain peak would exceed the configured headroom,
   reduce gain immediately enough to restore headroom. This reacts to a shout
   or transient without waiting for a long RMS window. This is virtual
   clipping prevention; actual ADC clipping in the captured samples cannot be
   repaired after capture.
2. **Sustained mouth saturation:** if normalized RMS remains at or above AVC's
   useful full-open level for `high_hold`, reduce gain at a controlled rate
   until it returns to the target band. This addresses pronounced or
   long-lasting high volume and avoids keeping the mouth pinned fully open.

Gain reduction should be faster than gain increase, but a single loud syllable
that stays below peak headroom should not erase seconds of learned gain. Reset
the quiet timer while high, reset the high timer while quiet, and use distinct
enter/exit thresholds to prevent chatter.

### No-data and silence policy

- Invalid/no-data upstream state publishes invalid/neutral output and does not
  advance adaptation.
- Digital silence publishes zero normalized level.
- During ordinary silence, hold the last useful gain for a configurable time;
  then optionally decay toward a conservative default. Do not increase gain.
- On discontinuity or source/device generation change, reset timers. Start
  from a documented safe initial gain rather than carrying calibration across
  unrelated devices.

## Current tuning defaults

The useful target is expressed relative to the AVC calibration rather than
copied as unrelated magic numbers. The current microphone experiment uses:

- useful normalized RMS band: `0.027 .. 0.03`; the narrow band creates
  hysteresis around the desired `0.03` upper bound;
- activity gate: approximately `0.003` raw RMS (40% below the initial
  calibration, to admit quieter speech while still freezing on silence);
- gain limits: `−24 dB .. +24 dB`, while each fresh source starts at unity
  (`0 dB`) and learns attenuation only after receiving live input;
- quiet hold: `0.75 s`;
- high hold: `0.75 s`; and
- gain rise: `3 dB/s`;
- sustained gain fall: `12 dB/s`; and
- peak headroom: `0.9` linear peak with immediate corrective reduction.

These values need deterministic tests and live measurements. The target band
may later belong directly on `VolumeNormalization` rather than being coupled
to one AVC instance.

## Proposed implementation slices

### 1. Shared retained-level seam

- Add an internal `RetainedLevelSample`/reader helper usable by raw amplitude
  and normalized amplitude.
- Generalize AVC's resolver and MMS type check so
  `mouth_open_from_amplitude` accepts either component.
- Keep existing raw-amplitude behavior and serialization unchanged.

### 2. Component and MMS authoring

- Add `VolumeNormalizationComponent` with durable upstream
  `ComponentRef`, validated configuration, generation, and retained sample.
- Register `VolumeNormalization.from(amplitude)` and its builders.
- Reject non-amplitude input, cycles, non-finite values, invalid ranges, and
  an upstream amplitude with no resolvable supported source.
- Round-trip authored configuration while omitting runtime gain/timers/results.

### 3. Fused input callback unit

- Add normalized-consumer registration beside `InputAmplitudeConsumer`.
- Preallocate rolling window and AGC state during stream construction.
- Emit fixed-size generation-tagged snapshots through the bounded ring.
- Rebuild/teardown safely on source, window, enable, and lifecycle changes;
  publish validated scalar policy edits to the existing callback unit.

### 4. Controller policy and diagnostics

- Implement quiet hold, activity gate, hysteretic target band, peak safety,
  sustained-high reduction, rate limits, and gain caps.
- Retain current gain and adjustment reason for main-thread diagnostics.
- Rate-limit logs and report transitions/cap hits rather than each block.

### 5. AVC example and calibration

- Update one microphone-speaking example to show raw and normalized meters.
- Feed only the normalized component to AVC.
- Verify normal speech reaches expressive mouth motion, silence stays closed,
  a shout backs gain down, and recovery does not visibly pump.

## Required tests

- MMS construction, invalid source/configuration, reference replacement,
  round-trip, disable/re-enable, and generation-safe stale rejection;
- deterministic low speech raises gain only after the quiet hold;
- digital silence and below-gate noise never raise gain;
- gain stops at target or maximum cap;
- a virtual peak beyond headroom causes immediate bounded reduction;
- sustained high RMS causes reduction after `high_hold`;
- an isolated non-clipping loud block does not cause sustained reduction;
- hysteresis prevents block-to-block gain chatter;
- behavior is equivalent across callback block sizes and sample rates;
- invalid/no-data/discontinuity clears output and freezes or resets adaptation
  according to policy;
- AVC accepts raw and normalized providers and applies the same mouth mapping;
- primary visemes retain their existing priority over the amplitude fallback;
- callback tests demonstrate no allocation after unit construction.

## Open decisions for live tuning

1. Should minimum gain allow attenuation below unity, or should the first
   slice only boost quiet voices and rely on AVC calibration for loud ones?
2. Should sustained-high detection use the normalizer's target band, the AVC
   full-open threshold, or an explicitly passed calibration profile? Keeping
   the component reusable argues for its own target band.
3. Does gain remain learned through long silence, decay toward unity, or reset
   only on source generation change?
4. Is a fixed activity gate sufficient across microphones, or is a bounded
   noise-floor learner required before enabling gain increase?
5. Should `mouth_open_from_amplitude` keep its name when it accepts both raw
   and normalized level providers, or should a more general alias such as
   `mouth_open_from_level` be added while preserving compatibility?

## Non-goals

- altering audible microphone samples or adding an audio graph gain node;
- recovering information already lost to ADC/hardware clipping;
- LUFS/integrated program loudness normalization;
- compression, limiting, or noise suppression for monitored/recorded audio;
- replacing AVC's visual smoothing or primary viseme arbitration; and
- making `Amplitude` itself adaptive.

## Exit

A preallocated input-thread normalization unit can learn enough bounded gain
for quiet ordinary speech to occupy AVC's expressive mouth range, backs off on
peak danger or sustained saturation, never learns upward from silence, and can
be passed to AVC anywhere a raw `Amplitude` is currently accepted.
