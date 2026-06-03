# Paint Panel Selection: Visualization Breakdown

## The Good (it works!)

The click → selection state pipeline is confirmed functional. When you click a paint
panel item:

1. **Click event fires** → `SelectionSystem::install_handlers` global handler catches it
   (`src/engine/ecs/system/selection_system.rs:24`)
2. **`resolve_selection_click()`** walks up the parent chain from the clicked
   renderable; finds `OptionComponent` marker → finds nearest enclosing
   `SelectionComponent` ancestor (`src/engine/ecs/system/selection_system.rs:122-135`)
3. **`handle_selection_click()`** updates the `SelectionComponent` in-place:
   - Sets `selected_index`, `selected_item`, `selected_component`
   - Clears/pushes `selected_entries`
   - Calls `add_selection_highlight()` / `remove_selection_highlight()`

The `println!` output confirms correct resolution:
```
[selection] text=Some("Fill") index=Some(3)
```

## The Broken (visualization)

`add_selection_highlight()` (`src/engine/ecs/system/selection_system.rs:178-229`)
creates this subtree under the clicked item:

```
item_id (e.g. paint_panel_item)
  └── highlight_id (TransformComponent "selection_highlight")
        ├── style_id (StyleComponent)
        │     position: Absolute
        │     top/left/right/bottom: -0.2 GlyphUnits
        │     background_color: gold [1.0, 0.84, 0.0, 1.0]
        │     background_z: -0.005
        └── emissive_id (EmissiveComponent intensity: 3.0)
```

### Why the gold highlight never renders

**1. No positioned ancestor for absolute anchoring**

The highlight Style uses `Position::Absolute`, which anchors to the *nearest
positioned ancestor* (an element with `position: Relative | Absolute | Fixed`).
The parent `paint_panel_item` does NOT set `position` — it defaults to
`Position::Static` (default in `src/engine/ecs/component/style.rs:16-17`).
Result: the highlight positions relative to the initial containing block
(viewport), making the gold fill the whole screen or nothing sensible.

**2. Dynamic element not processed by layout**

The highlight node is created at runtime via `world.init_component_tree()`
(`selection_system.rs:228`). The layout system has already processed the tree
by this point. Layout produces a list of renderable items per frame —
dynamically-added nodes won't be picked up unless layout re-runs on the
affected subtree. Even if it does, see point 1.

**3. Emissive on a non-renderable node**

`EmissiveComponent` registers the node for emissive rendering
(`src/engine/ecs/component/emissive.rs:init`), but the highlight node is a bare
`TransformComponent` with no mesh, no text, and no renderable. The emissive
pipeline modulates an existing renderable's color — it won't create geometry
from nothing.

### Bonus: `item_owns_layer` quirk

The layout system's `item_owns_layer()` checks for Style background_color on
children (`src/engine/ecs/system/layout/block.rs:266-273`). It does NOT
recursively check deeper than one level. The Style child of the highlight
Transform is two levels down from the item (`item → highlight → style`), so the
layout system's layer detection wouldn't find it anyway.

### Likely fix direction

Replace `add_selection_highlight`/`remove_selection_highlight`'s dynamic tree
approach with one of:
- **Style mutation**: find the existing `StyleComponent` on the item and toggle
  its `background_color` between the item's original color and the gold
  highlight color. No tree surgery needed.
- **Pre-spawned highlight slots**: pre-create a hidden highlight overlay node as
  a sibling/child of each option at MMS-template time (in
  `assets/components/panel_items.mms`), controlled by a flag component.

## REPL Display of Selection State

### Current behavior

`SelectionComponent::to_mms_ast()` returns only:
- `Selection()` in Single mode
- `Selection("multiple")` in Multiple mode

This means `cat <node>` in the REPL shows `Selection {}` regardless of what is
actually selected. All state fields (`selected_index`, `selected_item`,
`selected_entries`) are invisible.

This is purely a `to_mms_ast` serialization choice — the data IS live in the
world, just not exposed through the AST serialization path that the REPL uses.

### How REPL display works

```
SelectionComponent::to_mms_ast()       → ComponentExpression AST
  component_registry::subtree_to_ce_ast() → builds full subtree CE
    unparser::unparse_component()        → MMS text
      REPL "cat" command prints this text
```

(locations: `src/meow_meow/component_registry.rs:277`,
`src/meow_meow/unparser.rs:43`, `src/engine/repl/repl_backend.rs:362-363`)

### What would be needed

Trivial — modify `to_mms_ast` in `src/engine/ecs/component/selection.rs:131-140`
to emit constructor calls for the state fields, similar to how
`TextComponent::to_mms_ast()` emits `font_size(...)`.

Example approach:
```rust
fn to_mms_ast(&self, _world: &World) -> ComponentExpression {
    use ce_helpers::*;
    let mut node = match self.mode {
        SelectionMode::Single => ce_call("Selection", "", vec![]),
        SelectionMode::Multiple => ce_call("Selection", "multiple", vec![]),
    };
    if let Some(index) = self.selected_index {
        node = node.with_call("selected_index", nums([index as f64]));
    }
    if let Some(ref item) = self.selected_item {
        node = node.with_call("selected_item", strs([item]));
    }
    node
}
```

The `ce_helpers` module (`src/engine/ecs/component/mod.rs:343`) provides
`with_call()` for builder chains. ~15 lines of code.

### Risk assessment

- `to_mms_ast` is used for:\
  — REPL `cat` command (read-only display)\
  — `subtree_to_ce_ast` → `unparse_component` (read-only display)\
  — `ce_ast_to_materialized` (`src/meow_meow/component_registry.rs:576`) for
    spawn/re-spawn (but only when the CE is evaluated via `eval_ce`)
- Since `SelectionComponent` is a runtime-only state component that is NEVER
  spawned from MMS (it's created by the Rust system code), enriching its
  `to_mms_ast` output has **zero round-trip risk** — the extra constructors will
  simply be ignored if they don't match any registered builder method.
- To be fully correct, could register no-op builder methods like
  `.selected_index(usize)` on the `Selection` type in the component registry,
  or simply not worry about it since it's never spawned from MMS source.

### Verdict

**~15 lines, 5 minutes, zero risk.**
