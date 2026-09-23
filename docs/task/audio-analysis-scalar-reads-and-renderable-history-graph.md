# Volume-normalization scalar reads and renderable AGC gain history

Date: 2026-09-19

Status: design sketch

## Goal

Expose retained audio-analysis values as MMS numbers, then use that seam in
`examples/mittens-corp-agc.mms` for a bounded history of
automatic-gain-control bars.

Each point is ordinary world content: a narrow renderable cube, stacked along X
with a small gap, vertically positioned around zero, and coloured from the sign
of gain. This is not an immediate-mode or renderer-special debug graph.

```mms
let microphone = AudioInput {}
let raw_level = Amplitude.rolling_window(0.080).from(microphone) {}
let voice_level = VolumeNormalization.from(raw_level) {}

let raw_value = raw_level.value()
let normalized_value = voice_level.value()
let gain_db = voice_level.gain_db()
```

`VolumeNormalization` is the canonical component name; **AGC** is its short
form for the component's automatic-gain-control behaviour in UI and prose. MMS
has no type annotations yet. The later intended shape is
`let voice_value: f32 = voice_level.value()`.

## Scalar-read contract

`value()` copies current retained main-thread state synchronously. It is a
Number, not a component or stream, and works in MMS arithmetic, conditions,
colour choices, transform builders, and `FrameTick` callbacks.

| Provider | `value()` | Gain diagnostics |
| --- | --- | --- |
| `Amplitude` | retained raw RMS | none |
| `VolumeNormalization` | retained AGC RMS | `gain_db()` |

The first slice exposes only the values with a current consumer. Register these
live component methods:

```mms
raw_level.value()             // Number, raw RMS

voice_level.value()           // Number, AGC-adjusted RMS
voice_level.gain_db()         // Number, signed applied gain in dB
```

A live component with pending, neutral, disabled, or invalid state returns a
finite retained value (normally zero), never `NaN`, `null`, or an exception. A
stale/destroyed component handle remains an ordinary method error. Further
sample metadata needs a concrete consumer before it becomes public MMS API.

### Gain semantics

Graph `gain_db()`, not `voice_level.value() - raw_level.value()`: an RMS
difference reflects speech level as well as controller state and is not applied
gain.

- Positive dB: virtual boost added; upward green bar.
- Negative dB: attenuation applied; downward red/orange bar.
- Zero dB: unity gain; small amber neutral bar.

The default policy is `−24 dB .. +24 dB`, starting each fresh capture at
unity (`0 dB`). The graph therefore shows learned attenuation below zero as
well as learned boost above it.

## Engine seam

Add a narrow helper beside the existing raw-or-normalized level-provider
resolver. Do not introduce a generic scalar-control graph for this task.

```rust
struct LevelDiagnostics {
    rms: f32,
    gain_db: Option<f32>,
}

fn read_level_diagnostics(world: &World, id: ComponentId)
    -> Option<LevelDiagnostics>;
```

It copies already-retained main-thread state only. It must not touch the audio
callback, drain queues, lock, allocate, or reconfigure audio. The AGC
reads its bounded `current_gain_db` and normalized retained sample.

Register methods in runtime configuration and dispatch them through the live
component-method registry, so top-level and callback calls agree.
`Amplitude.gain_db()` remains unsupported rather than inventing a value.

This complements, rather than replaces,
`docs/task/generic-scalar-signal-providers.md`. Generalize only after another
concrete asynchronous provider and consumer need the same contract.

## AGC example graph

Add a world-space graph near the AGC status panel: black backing, zero line,
concise title, and a `LayoutRoot` bar container. It should be readable in both
desktop camera and XR mirror.

Make sampling and geometry configurable at the graph-factory boundary:

```mms
let agc_gain_graph = make_gain_history_graph(voice_level, {
    sample_period_sec = 0.100 // 0.0 means one sample per FrameTick
    max_samples = 48
    db_extent = 24.0
    column_width = 0.055
    column_gap = 0.012
})
```

At default settings this shows 4.8 seconds of history. Do not hide these
values as callback magic constants.

The factory owns `elapsed`, bar count, and root. Its bar
helper makes exactly one cube per sample:

```mms
fn make_gain_bar(gain_db, config) {
    let magnitude = min(abs(gain_db), config.db_extent)
    let height = max(magnitude * config.units_per_db, config.min_bar_height)
    let is_boost = gain_db > 0.0
    let is_cut = gain_db < 0.0
    let y = if is_boost { height / 2.0 } else { -height / 2.0 }
    let colour = if is_boost {
        [0.20, 1.00, 0.48, 1.0]
    } else if is_cut {
        [1.00, 0.30, 0.18, 1.0]
    } else {
        [0.95, 0.62, 0.16, 1.0]
    }

    return T.position(0.0, y, 0.0).scale(
        config.column_width, height, config.column_depth,
    ) {
        R.cube() { C.rgba(colour[0], colour[1], colour[2], colour[3]) }
    }
}
```

Use the MMS math helpers available when implementing it. The essential geometry
is a real renderable/colour subtree, compressed X width, signed Y extent
centred on zero, and a tiny visible neutral column.

Make every bar an `inline-block` child in the layout root, with container
width encoding its gap. When `remove_child(0)` evicts the oldest bar, layout
reflows the row. Do not use an ever-increasing X coordinate: a bounded graph
would otherwise leave holes or overlap bars.

## Sampling rules

Use `on_global("FrameTick", fn(event) { ... })`, never the audio callback.

1. Accumulate `event.dt_sec`.
2. At positive `sample_period_sec`, append at most one bar when the interval
   is reached and retain only the remainder. A long frame must not create a
   catch-up burst of duplicate historical bars.
3. At `sample_period_sec = 0.0`, append one bar per frame.
4. Read `gain_db()` each interval; zero is the safe visual result before a
   live sample arrives.
5. Use `bars.attach(make_gain_bar(...))`; after capacity, remove the oldest
   logical record and call `bars.remove_child(0)`.

This follows the bounded live-tree pattern in
`examples/data-viz-http-rolling-window.mms`. If later requirements need exact
100 ms *audio-time* bins rather than visual sampling of retained state, add a
bounded AGC history ring; FrameTick resampling cannot reconstruct missed
callback updates.

## B-button A/B behaviour

B keeps switching AVC between raw and normalized amplitude. It must not stop or
remove the AGC: the graph stays live in both modes, showing what the AGC would
do while the operator compares AVC's raw path.

```text
AGC = ON  — AVC uses normalized level
AGC = OFF — AVC uses raw amplitude; AGC graph remains live
```

## Implementation and tests

1. Add the retained scalar reader and finite-value sanitation.
2. Register/dispatch `Amplitude.value()`, `VolumeNormalization.value()`, and
   `VolumeNormalization.gain_db()` as Number methods in evaluator and live
   callback paths.
3. Add `make_gain_history_graph` / `make_gain_bar` to the example, including
   backing, zero line, configurable cadence/capacity, and bounded children.
4. Tune at 100 ms / 48 samples; temporarily author a negative minimum gain to
   exercise attenuation.

Required tests:

- Raw/AGC RMS reads, AGC gain reads, and finite pending/neutral/invalid
  results.
- Clear unsupported `gain_db()` error on `Amplitude`.
- A live handler uses readings in arithmetic and an `if`.
- Simulated FrameTicks create one 100 ms bar, cap/evict children, and avoid
  long-frame catch-up bursts.
- Positive/negative/zero gains create upward green, downward red/orange, and
  neutral amber renderable bars.
- B changes AVC's source without removing the AGC or graph.

## Non-goals

- a generic reactive scalar graph or scalar type annotations;
- full audio-sample history, waveform rendering, or a renderer-special graph;
- unbounded runtime component creation; and
- changes to PCM, AGC ownership, or AVC mapping.

## Exit

MMS can read finite retained audio-analysis values and signed AGC gain in a
callback. The AGC example renders a bounded, configurable
history of signed-gain cube renderables while retaining raw-versus-normalized
AVC comparison.
