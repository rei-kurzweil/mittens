# FrameTick history-visualization benchmark and retained-bar update plan

Status: planned. Follow-up to
[Volume-normalization scalar reads and renderable AGC gain history](audio-analysis-scalar-reads-and-renderable-history-graph.md).

## Purpose

The AGC monitor samples retained `Amplitude.value()` and
`VolumeNormalization.gain_db()` values at 10 Hz. Its current implementation
keeps a twelve-sample logical history, then removes and rebuilds each track's
fixed twelve-bar view on every sampling edge.

This is intentionally straightforward and makes the newest snapshot, colour,
labels, and moving whole-second marker unambiguous. It is also a recurring MMS
`on_global("FrameTick", ...)` pattern, now used by the AGC visualizers in both
desktop and XR scenes. Before replacing it with a more incremental mechanism,
measure its real cost and compare equivalent output.

## Baseline scene arrangement

- `examples/mittens-corp-agc.mms` is the XR example (renamed from
  `mittens-corp-volume-normalization.mms`). It has two independent grabbable
  info panels: **AGC mouth response** settings and **AGC response** history.
- `examples/mittens-corp-agc-desktop.mms` has the same conceptual split for
  desktop input.
- The response accordion owns an `active` flag. On `AccordionMinimized`, its
  `FrameTick` handler must do no elapsed-time update, retained scalar reads,
  history shifts, bar-view rebuilds, numeric text updates, or scene intents.
  On restore, it rebuilds the body, resets elapsed time, and resumes on the
  next ordinary sampling edge.

This suspend-on-minimize behavior is part of the benchmark baseline, not an
optimization to defer: a hidden monitor should not consume its normal update
budget.

## Current work per 100 ms sampling edge

1. Read two already-retained main-thread scalar values; no audio-callback
   work, queue drain, allocation, or audio-unit reconfiguration is allowed.
2. Shift twelve input RMS values and twelve signed gain values in MMS tables.
3. Advance the one-second marker and cap logical sample count at twelve.
4. Find both view layers; remove each old view subtree; attach new fixed trees
   containing twelve bars plus an optional marker and optional twelve labels.
5. If labels are enabled, update the one-line current-value readout.

The non-sampling `FrameTick` path should be limited to an elapsed-time add and
comparison while the panel is expanded. The minimized path should skip even
that.

## Questions to answer before changing the update strategy

- What is the per-frame and per-100-ms-handler time in the MMS evaluator,
  intent servicing, layout, renderable lifecycle, and renderer submission?
- How many `Attach` and `RemoveSubtree` intents, created components, layout
  dirties, and renderables occur per response update, with numeric labels off
  and on?
- Does the result differ materially on desktop versus XR at their normal frame
  rates and GPU workloads?
- Does minimizing the response accordion reduce handler time and generated
  intents to zero after the click event itself? Does restoring cause exactly
  one body rebuild and no catch-up burst?
- Are global selector lookups a meaningful share of the cost compared with
  scene-tree rebuilding, or is persistent scoped identity sufficient only as a
  later cleanup?
- What rate and response pattern exercise gain sign changes, zero gain,
  labels, and the moving one-second marker without requiring live microphone
  variability?

## Measurement plan

1. Add narrowly scoped instrumentation around dispatched `FrameTick` MMS
   callbacks and callback intent servicing. Report count, total, mean, p50,
   p95, and worst duration separately for sampling and non-sampling edges.
   Keep it opt-in and exclude the cost of printing/logging from the measured
   interval.
2. Record frame time / effective FPS alongside those measurements, and collect
   evaluator, layout, and renderer timings where the engine already exposes
   them. Do not infer render cost from scripting time alone.
3. Run a deterministic harness that feeds retained raw RMS and AGC gain values
   through at least several hundred frames. Test expanded-labels-off,
   expanded-labels-on, and minimized states for both scenes.
4. Repeat a manual XR run at the target headset refresh rate and a desktop run
   with the same scene and panel placement. Record hardware, build mode,
   resolution, refresh rate, and any mirror cost.
5. Establish a budget from the baseline data before selecting an implementation
   change. The result should include a small before/after table, not only a
   subjective smoothness report.

## Alternatives to compare after the baseline

| Strategy | Update at each 100 ms edge | Expected trade-off |
| --- | --- | --- |
| Rebuild fixed views (current) | Remove/attach two twelve-bar subtrees | Simplest retained-state rendering; highest scene-tree churn. |
| Retain bars and update transforms/colours | Shift values, set bar transforms; change colour only on gain-sign transition | Fewer components/intents; needs a reliable live transform/material mutation path and careful label lifecycle. |
| Ring-buffer placement | Reuse twelve bars and advance a newest index/marker | Avoids visual shifts; needs chronological labeling and a clear wrap transition. |
| Renderer-special history primitive | Upload compact scalar data | Potentially fastest, but outside this task unless generality is justified. |

All alternatives must preserve: oldest-to-newest order, 100 ms cadence with no
long-frame catch-up burst, signed green/pale-red gain semantics, blue raw input
semantics, optional numeric labels that perform no label updates while off, and
the moving one-second marker.

## Exit criteria

- A reproducible benchmark reports scripting callback cost and frame-rate
  impact for expanded/minimized and labels-off/on states.
- The selected update approach is justified by that data and has equivalent
  visual behavior under deterministic history inputs.
- XR and desktop examples keep separate settings and response panels.
- Automated coverage proves minimization stops response sampling/rebuild
  intents and restoration resumes without a catch-up burst.
