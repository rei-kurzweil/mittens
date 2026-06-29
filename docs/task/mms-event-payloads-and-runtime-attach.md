# MMS event-payload binding + runtime `attach()`

Goal: let MMS scripts register event handlers that receive event payload fields,
and let those handlers spawn new component subtrees attached to runtime
component ids (not only mutate existing ones). The motivating use case is:
*"when the user selects a bone in the editor, attach a small red marker cube to
that bone."* But the surface area applies generally — any handler that needs to
react to a hit/selection by adding scene content.

## Current state (the three gaps)

### Gap 1 — `SelectionChanged` not in MMS signal-kind parser

`src/meow_meow/evaluator.rs::parse_signal_kind` (≈ line 1474) only recognises:
```
Click, DragStart, DragMove, DragEnd, RayIntersected, ParentChanged,
CollisionStarted, CollisionEnded, Scrolling
```
`SelectionChanged` (already emitted by `EditorSystem` at
`src/engine/ecs/system/editor_system.rs:151`) is not listed, so
`on(scope, "SelectionChanged", ...)` errors out at parse.

### Gap 2 — handler invocation always passes `Value::Null` as the event arg

`src/meow_meow/runner.rs:234-249`:
```rust
HostCallKind::RegisterHandler { scope, signal_kind, handler } => {
    rx.add_handler_closure(
        signal_kind,
        scope,
        move |world, emit, _signal| {                   // <- _signal ignored
            if let Err(e) = eval_mms_fn(
                &handler,
                vec![Value::Null],                       // <- always Null
                ...
            ) { ... }
        },
    );
}
```
The closure ignores the `Signal` envelope and the handler is invoked with a
single `Value::Null`. So `fn(e) { e.selected }` always sees `e == null`.

### Gap 3 — no MMS-runtime `attach(parent, child_expr)`

Existing MMS handlers can call **methods** on existing component objects
(`panel_layout.enable_inspect()`, `status.set_text(...)`), but there's no way
to *spawn* a new component subtree and attach it to an arbitrary parent at
runtime. Handlers can't realise a `ComponentExpression` into the world.

The closest existing mechanism: top-level MMS evaluation produces
`IntentValue::Attach` plus add-component intents that the runtime flushes
before the windowing loop starts. Inside a handler we'd want the same
intent stream emitted via the `emit` parameter that the handler closure
already receives.

## Target design

### Event → MMS Value conversion

Define a conversion `EventSignal → Value` that returns an MMS table/object
exposing the event's fields. Component ids surface as MMS
`ComponentObject { id, component_type }` so the handler can immediately call
methods on them or pass them to other MMS functions.

Per-kind field mapping:

| Event              | Fields exposed                                          |
| ------------------ | ------------------------------------------------------- |
| `Click`            | `renderable`, `raycaster`, `hit_point`, `screen_pos_px` |
| `DragStart`        | `renderable`, `raycaster`, `hit_point`, `ray_dir_world`, `screen_pos_px` |
| `DragMove`         | `renderable`, `raycaster`, `hit_point`, `delta_world`, `screen_pos_px`, `screen_delta_px` |
| `DragEnd`          | `renderable`, `raycaster`, `hit_point`                  |
| `RayIntersected`   | `renderable`, `raycaster`, `t`, `origin`, `dir`         |
| `SelectionChanged` | `editor_root`, `selected` (nullable)                    |
| `ParentChanged`    | `child`, `old_parent`, `new_parent`                     |
| `CollisionStarted` | `a`, `b`, `delta`                                       |
| `CollisionEnded`   | `a`, `b`, `delta`                                       |
| `Scrolling`        | `scroll_component`, `drag_scope`, `delta_world`, `scroll_offset`, `max_scroll`, `viewport_height`, `content_height` |

`ComponentId` fields wrap as `ComponentObject` with `component_type` resolved
via `world.component_name(id)`. Vectors become MMS arrays of numbers.
`Option<ComponentId>` becomes `ComponentObject | null`.

### `attach(parent, child_expr)` runtime API

Two flavours, pick one (probably both, since they compose):

1. **`attach(parent, child)` where `child` is already a built `ComponentObject`** —
   emit `IntentValue::Attach { parents: vec![parent.id], child: child.id }`
   through the handler's `emit`. Simplest.

2. **`attach(parent, ComponentExpression)`** — evaluate the expression to a
   subtree of intents (mirror what top-level evaluation does), then emit them
   through the handler's `emit`. Lets handlers author
   `attach(bone, T.scale(0.02, ...) { R.cube() { C.rgba(1,0,0,1) } })` inline.

The second is the ergonomic win. It needs the evaluator's
`ComponentExpression`-to-intent-stream realisation path to be callable from a
handler context (not only top-level). The `eval_mms_fn` site at runner.rs:239
already passes `emit` into the handler — the intent stream just needs a
destination.

### Putting it together — example

```mms
on(editor_root, "SelectionChanged", fn(e) {
    if e.selected != null {
        print("selected: " + e.selected.name)
        attach(e.selected, T.scale(0.02, 0.02, 0.02) {
            R.cube() {
                C.rgba(1.0, 0.0, 0.0, 1.0)
                EM.on()
            }
        })
    }
})
```

## Work breakdown

### Step 1 — bind `SelectionChanged` (and audit other missing kinds)
- [ ] Add `"SelectionChanged"` to `parse_signal_kind` (evaluator.rs:1474).
- [ ] Audit `EventSignal` variants vs. `parse_signal_kind` for other gaps.

### Step 2 — event payload → MMS Value
- [ ] Add `fn event_signal_to_value(world: &World, event: &EventSignal) -> Value`
  in a new module under `src/meow_meow/` (e.g. `event_payload.rs`).
- [ ] Update runner.rs:234-249 to call the conversion and pass the resulting
  `Value` into `eval_mms_fn` instead of `Value::Null`.
- [ ] Verify MMS field access (`e.selected`, `e.selected.name`,
  `e.hit_point[0]`) works for each variant — add unit tests in
  `src/meow_meow/tests.rs` mirroring the existing Click-handler tests.

### Step 3 — `attach()` runtime function
- [ ] Decide form (built-object vs expression vs both). Recommendation: support
  both, with the expression form layered on the object form.
- [ ] For the **object form**, add `attach(parent, child)` to evaluator.rs's
  builtin call dispatch (alongside `query`, `on`, etc.). Resolves to
  `IntentValue::Attach` and emits through `ctx.emits`.
- [ ] For the **expression form**, factor the
  `ComponentExpression → intent stream` realisation out of the top-level
  evaluator into a function reusable from handler context. Then make
  `attach(parent, T.scale(...) { ... })` work by realising the expression
  into intents whose root child is then attached to `parent`.
- [ ] Tests: handler that spawns a cube under a clicked node; verify topology
  via world inspection.

### Step 4 — docs + example
- [ ] Update or add an example demonstrating the workflow
  (`examples/signal-handler.mms` is the obvious place to grow).
- [ ] Document the available event fields per kind in
  `docs/spec/signals.md` (or a new `docs/spec/mms-handlers.md`).

## Out of scope (don't conflate with this task)

- Per-camera mesh culling (separate doc; covers the avatar head-mesh
  visibility in XR view).
- FABRIK spine chain for avatar (covered in
  `docs/task/avatar-control-head-driven-redesign.md`).
- Generalised reactive-data-binding in MMS (handlers as the only mechanism is
  fine for this scope; bigger redesign is a separate conversation).

## Motivating context

This is being written while debugging the AVC head-driven redesign
(`docs/task/avatar-control-head-driven-redesign.md`). The immediate need is
visualising bone positions on the bisket avatar by selecting them at runtime
and attaching marker cubes. The workaround for the *avatar-debug* iteration is
to read the bone name from the REPL's auto-`cd` on selection
(`editor_system.rs:159-167`) and add a static marker in the .mms, then iterate.
The full feature unblocks any future MMS scene that wants interactive
authoring or in-engine debugging tools.
