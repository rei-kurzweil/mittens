# MMS Shared Closure Captured Env

Date: 2026-06-19

Status: follow-up implementation task from `docs/task/gltf-in-editor-startup-memory-trace-followup.md`.

## Problem

MMS startup memory blow-up in `assets/components/panels.mms` is now traced to recursive
closure-env deep copies, not GLTF payloads or editor panel materialization.

Current representation:

```rust
Value::Function {
    params: Vec<String>,
    body: BlockStatement,
    captured_env: HashMap<String, Value>,
}
```

Current capture path:

1. `Expression::Function` calls `snapshot_visible()`
2. `snapshot_visible()` deep-clones all visible bindings into a fresh `HashMap<String, Value>`
3. many of those bindings are already `Value::Function`
4. each later closure therefore recursively clones prior closure graphs
5. exporting the closure into `named_exports` clones the whole graph again

The startup trace now shows the recursive chain explicitly:

- `world_panel` captures `panel_button:46`
- `inspector_panel` captures `world_panel:47`
- `asset_panel` captures `inspector_panel:56`

Representative retained steps from the trace:

- `world_panel` closure snapshot: about `+269 MiB`
- `world_panel` export copy: about `+269 MiB`
- `inspector_panel` closure snapshot: about `+538 MiB`
- `inspector_panel` export copy: about `+538 MiB`
- `asset_panel` closure snapshot: about `+1.05 GiB`
- `asset_panel` export copy: about `+1.05 GiB`

This is enough to justify a representation change.

## Goal

Make MMS closure copies shallow for captured environments, so:

- creating a closure does not recursively duplicate prior closure graphs
- exporting a closure does not duplicate the full captured graph again
- function calls still preserve lexical scope semantics

## Intended direction

The minimal viable fix is to share captured environments instead of storing them by owned value.

Recommended representation:

```rust
Value::Function {
    params: Vec<String>,
    body: BlockStatement,
    captured_env: Arc<HashMap<String, Value>>,
}
```

The important property is:

- cloning `Value::Function` should clone an `Arc`, not deep-clone the entire env graph

## Required code changes

### 1. Change closure storage

In `src/meow_meow/object.rs`:

- change `Value::Function.captured_env` from `HashMap<String, Value>` to
  `Arc<HashMap<String, Value>>`
- add the required `Arc` import

### 2. Change closure creation

In `src/meow_meow/evaluator.rs`:

- wrap `snapshot_visible()` with `Arc::new(...)` at `Expression::Function`
- keep current tracing, but expect the large RSS jumps at closure creation/export to collapse

### 3. Avoid re-expanding the env graph at call time

This is the important companion change.

If function calls immediately clone the `Arc<HashMap<...>>` back into an owned `HashMap`, the
startup export problem improves but call-time memory behavior still stays unnecessarily expensive.

So function-frame setup should also be updated to avoid eagerly deep-copying the captured env.

Likely direction:

- change `ObjectWorld::push_function_frame(...)`
- change `Frame` storage if needed
- allow function frames to hold a shared captured env view instead of an eagerly copied map

The exact frame shape is an implementation choice, but the fix is incomplete if call setup still
materializes a full owned env copy for every closure clone/call path.

### 4. Update all function-call entry points

Audit all places that destructure `Value::Function` and seed function execution:

- normal `eval_call(...)`
- `eval_mms_fn(...)`
- handler registration / invocation paths
- any tests that manually construct `Value::Function`

## Semantics to preserve

The change should preserve:

- closures still capture the visible lexical bindings at definition time
- caller locals are still hidden past the function barrier
- rebinding inside a function still behaves like today
- captured scalar values still behave as copied lexical snapshots

This task changes ownership/storage, not lexical semantics.

## Verification

Minimum verification:

1. existing MMS evaluator tests still pass
2. `assets/components/panels.mms` module load no longer shows near-identical
   snapshot-cost and export-copy-cost GiB-scale jumps
3. the hotspot trace should still show the same captured binding counts, but RSS deltas should
   drop sharply

Helpful trace checkpoints to compare before/after:

- `world_panel`
- `inspector_panel`
- `asset_panel`

## Non-goals for this task

This task does not require:

- free-variable-only capture
- changes to MMS syntax
- heap-backed arrays or larger env/object-model redesign

Those may still be worthwhile later, but they are not required to remove the current
closure-graph duplication blow-up.

## Likely follow-up

If shared captured envs remove most of the memory blow-up but some large retained cost remains,
the next likely refinement is:

- narrower free-variable capture instead of full `snapshot_visible()`

But that should only happen after the shared-env change is measured.
