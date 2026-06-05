# Shared editor UI root duplicates editor-scoped PaintSystem handlers

## Status

Open bug / investigation note.

## Symptom

When more than one `EditorComponent` exists in the world, `PaintSystem` installs one reducer
instance per editor root, but those reducer instances all subscribe to the same shared runtime UI
tree.

This causes a single UI `SelectionChanged` event from the Paint panel to be reduced multiple
times, once for each installed editor-scoped paint handler.

Observed runtime trace shape:

```text
[PaintSystem][trace] reduce editor_root=ComponentId(10v1) panel_query_root=ComponentId(94v1) ...
[PaintSystem][trace] reduce editor_root=ComponentId(36v1) panel_query_root=ComponentId(94v1) ...
```

Both reducers are consuming the same tool-selection event from the same panel scope.

## Why this is a bug

The current setup mixes two different ownership models:

1. scene/editor input routing is scoped per `editor_root`
2. editor panel UI is currently materialized under one shared `editor_runtime_ui_root`

That means editor-scoped systems which need to observe UI state can accidentally subscribe
multiple times to one global UI source.

For `PaintSystem`, this duplicates:

- tool-selection reduction
- panel-focus reduction
- status recomputation
- any future side effects that depend on those same state transitions

Even when duplicate side effects happen to be harmless today, the event ownership is wrong and
makes traces misleading.

## Repro

- Run a scene that creates more than one `EditorComponent`.
- Ensure the shared editor runtime UI panels are spawned.
- Click Paint panel tools such as `Line` and `Spray Can`.

Expected trace shape:

```text
one promoted event
one reduce
one side_effects pass
```

Actual trace shape:

```text
one promoted UI event fan-outs into multiple reduce/side_effects passes,
one per editor_root
```

## Root cause

`PaintSystem` is installed per editor root:

- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs#L833)
- [src/engine/ecs/system/paint_system.rs](../../src/engine/ecs/system/paint_system.rs#L125)

But the panel query root is effectively singleton/shared:

- [src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs#L86)
- [src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs](../../src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs#L517)

So each editor-root installation attaches handlers to the same `panel_query_root`.

## Constraints

We do need both of these capabilities:

- input / click / drag events should still work for objects inside individual `editor { ... }`
  scene trees
- editor systems also need to listen to selection/focus/tool events emitted from the editor UI tree

So the fix is not "stop listening to UI events" and not "make everything global". The missing
piece is a clean ownership boundary between editor-scoped scene interaction and editor-UI state.

## Likely fix directions

### Option A: runtime UI becomes editor-owned, not global

Spawn one runtime UI root per editor and keep panel selections under that editor's own subtree.

Then editor-scoped systems can safely subscribe to:

- scene events on `editor_root`
- UI selection events on that editor's own UI root

This is the cleanest ownership model if multiple live editors are a real target.

### Option B: keep shared UI, but make UI-driven systems shared too

If the runtime UI is intentionally global, then systems consuming that UI state should not be
installed once per editor root.

Instead:

- one shared paint/UI reducer consumes panel selection state
- a separate routing layer decides which editor scene root receives paint placement input

This avoids duplicate reducers but requires an explicit notion of "active editor" or equivalent.

### Option C: shared UI emits editor-qualified events

Keep one UI tree, but encode which editor a panel/action belongs to and require subscribers to
filter by that owner explicitly.

This is workable, but it is more complex than matching the tree ownership directly.

## Questions to answer

- Is the editor runtime UI intended to be singleton for the whole app, or one instance per editor?
- If singleton UI is intentional, what is the canonical routing rule from UI state to target editor?
- Should `PaintSystem` conceptually be editor-scoped, UI-scoped, or split into both halves?
- Are there other editor systems besides paint that are already double-subscribing to the same
  shared panel tree?

## Related

- [docs/bugs/paint-panel-shell-does-not-take-focus.md](./paint-panel-shell-does-not-take-focus.md)
- [docs/bugs/panel-layout-selection-interaction.md](./panel-layout-selection-interaction.md)
- [docs/task/paint-panel-selection-and-panel-focus.md](../task/paint-panel-selection-and-panel-focus.md)
- [docs/refactor/selection-option-topology.md](../refactor/selection-option-topology.md)
