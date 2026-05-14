# Switch editor panels to MMS modules in `assets/components/`

## Context

The World and Inspector panels are built imperatively in rust
(`src/engine/ecs/system/inspector_system.rs`):

- `spawn_panel_title_bar` — header slot + title label + gizmo.
- `spawn_world_panel` / `spawn_inspector_panel` — outer scaffolding.
- `rebuild_world_panel` / `rebuild_inspector_panel` — row repopulation
  on selection / topology change.

This works but the layout is hardcoded in rust. We'd like the *visual
chrome* of these panels to live as MMS modules under `assets/components/`,
the same way `assets/components/button.mms` already exists. That gives
us:

- A single source of truth for panel styling (no rust/MMS drift).
- Iteration on panel UI without recompiling.
- A worked example of "engine code consuming an MMS factory function" —
  the same pattern third-party MMS components will use.

Today's pure-rust build of the World panel title bar is the placeholder.
The Save/Load buttons in that title bar are a hand-rolled mirror of
`assets/components/button.mms` — same colors, same padding intent — so
that switching to the real button factory later is purely a wiring
change.

## What's missing today

The blocker is a single host API: **call an MMS-exported factory function
from rust and receive a live `ComponentId`**. We can already:

- Load + evaluate an MMS module (`MeowMeowRunner::eval_with_world_at_path`).
- Spawn `MaterializedCE` trees synchronously (`spawn_tree`).
- Convert AST → MaterializedCE (`ce_ast_to_materialized`).

What we cannot yet do cleanly:

- Take a `Value::Function` parsed out of a module's `named` exports and
  invoke it with rust-supplied args, getting back the spawned subtree's
  root `ComponentId`. `eval_mms_fn` exists but does not service HostCalls
  (the path that turns `let root = T {...}` inside the function body into
  a live `ComponentObject`).

## Proposed approach

### Phase 1: enable rust → MMS factory calls

1. Extend the MMS evaluator to support a synchronous "service the
   HostCalls in-thread" mode. Today `MeowMeowRunner` services them
   across a channel; for the function-call path we want a direct
   rust-side handler.

2. Add a public helper:

   ```rust
   pub fn call_mms_factory(
       module_value: &Value,         // a Value::Module
       export_name: &str,            // e.g. "button"
       args: Vec<Value>,             // engine-side values
       world: &mut World,
       emit: &mut dyn SignalEmitter,
   ) -> Result<ComponentId, String>;
   ```

3. Module caching: `MmsAssetCache` keyed by file path, holding evaluated
   `Value::Module` values. Editor setup pre-warms with the modules it
   needs (`button.mms`, eventually `panel.mms` etc).

### Phase 2: factor `button.mms` into the title bar

Replace `spawn_titlebar_button` (the hand-rolled mirror) with:

```rust
let save_btn = call_mms_factory(
    &cache.button_module,
    "button",
    vec![Value::String("Save".into())],
    world, emit,
)?;
world.add_child(header_slot, save_btn);
```

Then attach the click handler to `save_btn` (or to its `Raycastable`
descendant, matching `RaycastableComponent::click_only` behavior).

### Phase 3: factor the panel scaffolding into `assets/components/`

```
assets/components/
  button.mms                     (exists)
  panel.mms                      ← new: title bar + flex layout + scroll
  world_panel.mms                ← new: panel.mms + row data binding
  inspector_panel.mms            ← new: panel.mms + property row binding
```

Each `*.mms` factory takes the runtime knobs (title text, content
width, row provider callback?) as args. Rust shrinks to the
state-management role: maintaining `WorldPanelComponent`'s row caches,
firing rebuilds on `SelectionChanged`, etc.

Open design question: how do row-rebuilds plug in? Two options —

- **Callback into MMS**: row builder is an MMS function the panel
  factory invokes. Needs the same factory-call host API as Phase 1.
- **Imperative children**: rust still owns the `rows_layout` subtree
  and adds/removes children directly. MMS only paints the chrome.

Imperative children is simpler and matches how the buttons will work
in Phase 2; recommend starting there.

## Verification

For each phase:

1. `cargo run --example simple-demo` — World panel renders with the
   familiar title bar + Save/Load buttons.
2. Save button click writes the scene to `<exe-stem>.mms`. The status
   text above the panel updates.
3. `cargo run -- load <exe-stem>.mms` — saved scene re-spawns.
4. Visual diff against the rust baseline; matching colors / padding /
   click hitboxes.

## Related

- `docs/task/mms-component-migration-checklist.md` — tracks `to_mms_ast`
  coverage. Some panel sub-components (Style, HtmlElement) need full
  round-trip support before save/load reaches parity.
- `assets/components/button.mms` — the existing factory; the spec we're
  mirroring in rust today.
