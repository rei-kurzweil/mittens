# Editor Selection And Paint Perf

Date: 2026-06-05

Status: active investigation.

## Summary

The worst editor-context lockups are reduced, but two performance problems remain:

- selecting a renderable nested under an `EditorComponent` can still freeze the whole main thread
  for roughly `0.5s` and sometimes `1.5s` to `2.0s+` before the gizmo appears
- painting is consistently slow, roughly `0.2s` to `0.6s` before the placed object appears

Both symptoms freeze animation as well, so this is not just camera/input routing. The frame update
path is stalling on main-thread work.

## Current observations

### Selection stall

Current user-observed behavior:

- clicking or dragging a renderable nested under an editor can pause the whole app briefly
- the stall happens before or during transform-gizmo placement
- the slowdown is much better than before, but still large enough to feel broken
- animation pauses during the stall, so the main thread is blocked

Recent code changes already removed some obvious extra work:

- editor selection is now suppressed while the Paint panel is focused
- world-panel selection sync no longer rebuilds the full world-panel model just to find the
  selected row
- inspector row generation is capped so extremely deep selections do not explode indefinitely

That means the remaining selection stall is likely in one or more of:

1. gizmo registration / attachment / visual rebuild work
2. inspector panel rerender after editor selection
3. world-panel rerender after editor selection
4. command / signal churn triggered by editor `SelectionChanged`
5. topology work caused by selecting wrapper transforms instead of authored targets

### Paint latency

Current user-observed behavior:

- painting works, but each placement still feels slow
- object appearance usually lags by roughly `0.2s` to `0.6s`
- this happens even after the focus-steal/gizmo issue was reduced

Recent code changes already addressed one correctness bug:

- freshly painted assets now strip `SelectionComponent` and `OptionComponent` before entering the
  scene, so painted content should no longer inherit panel-style selection behavior

The remaining paint cost is likely in one or more of:

1. `spawn_mms_module_component_uninitialized(...)` evaluating or materializing MMS on every paint
2. bounds measurement for placement pose computation
3. subtree initialization after spawn
4. attach / command processing cost for the new subtree
5. avoidable rework if the asset could be cloned from a pre-materialized prefab tree instead of
   re-evaluated each placement

## Important product-level question

We need to confirm whether paint placement is supposed to:

- re-evaluate MMS exports every time, or
- evaluate/materialize once and clone a prefab tree for each placement

If the intended model is prefab-style cloning, current paint performance may be fundamentally doing
the wrong kind of work.

## Immediate instrumentation targets

Add explicit timing around the selection path first.

### Selection path timings

Measure at least:

- editor `DragStart` handler total time
- `select_editor_target(...)`
- gizmo resolution / spawn / attach time
- editor `SelectionChanged` handling time
- world-panel sync time
- inspector refresh time
- command processing immediately following selection

Suggested logging shape:

```text
[perf][select] target=<cid> phase=gizmo_attach dt_ms=...
[perf][select] target=<cid> phase=inspector_refresh dt_ms=...
[perf][select] target=<cid> phase=world_panel_sync dt_ms=...
[perf][select] target=<cid> phase=selection_total dt_ms=...
```

### Paint path timings

Measure at least:

- paint event handling total time
- asset spawn/materialization time
- placement bounds measurement time
- pose resolution time
- subtree init time
- attach/command time until visible result

Suggested logging shape:

```text
[perf][paint] asset=<name> phase=spawn_mms dt_ms=...
[perf][paint] asset=<name> phase=measure_bounds dt_ms=...
[perf][paint] asset=<name> phase=init_subtree dt_ms=...
[perf][paint] asset=<name> phase=paint_total dt_ms=...
```

## Likely follow-up experiments

### Selection

1. Temporarily disable inspector rerender on editor selection and compare latency.
2. Temporarily disable world-panel rerender on editor selection and compare latency.
3. Temporarily attach no gizmo at all and compare latency.
4. Log whether the selected target is an authored transform or an `editor_auto_raycastable`
   wrapper.

These toggles should identify which subsystem is dominating the stall.

### Paint

1. Time pure MMS spawn without placement.
2. Time placement math without spawn.
3. Prototype one cached prefab/materialized-tree clone path for a simple asset and compare against
   repeated MMS spawn.
4. Check whether paint cost scales mostly with asset complexity or stays flat across simple assets.

## Desired outcome

Short term:

- selection-to-gizmo latency should feel effectively immediate
- paint placement of a simple box/cube asset should feel near-instant

Long term:

- selection and paint should both be cheap enough that editor interactions stay within normal
  frame-time budgets rather than blocking the main thread for visible fractions of a second
