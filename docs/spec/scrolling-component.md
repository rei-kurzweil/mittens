# ScrollingComponent — reusable virtual scroll container

## Problem

The world panel currently hard-caps at `MAX_ROWS = 30` items. Scroll needs to be
reusable across world panel, inspector panel, file tree, and any future list UI.
The scroll mechanism should be a first-class ECS component, not logic scattered
through each panel's rebuild function.

---

## Design principle: component as state, system as behaviour

`ScrollingComponent` holds *scroll configuration and state*.
It does not hold item data and does not create children itself.

The owning system (e.g. `InspectorSystem`) is the authority on what items exist.
When the scroll position crosses a row boundary the component emits a
`ScrollChanged` event; the owning system's handler rebuilds only the visible
window of ECS children in response.

This keeps the component minimal and side-effect-free while making the behaviour
composable: anything that needs a scrollable list anchors a `ScrollingComponent`
and registers a `ScrollChanged` handler.

---

## ⊹ Component definition

```rust
pub struct ScrollingComponent {
    /// Height of each item in overlay world units.
    pub item_height: f32,
    /// Maximum number of items rendered at once.
    pub page_size: usize,
    /// Total number of logical items (set by the owning system).
    pub total_items: usize,

    // --- scroll state ---
    /// Continuous scroll position. 0.0 = top. Unit = items (rows).
    pub scroll_offset: f32,
    /// The integer row index of the first visible item last time a
    /// `ScrollChanged` event was emitted. Used to detect boundary crossings.
    pub(crate) last_window_start: usize,
}
```

`item_height` is in the same world-unit coordinate system the panel uses (overlay
space). For the current world/inspector panels: `ROW_HEIGHT = 0.045`.

`page_size` is the number of item *slots* the owning system will render.
The owning system controls what goes in each slot.

`total_items` is updated by the owning system whenever the logical item list
changes (e.g. the component tree grows/shrinks). ScrollingComponent uses it to
clamp `scroll_offset`.

### Derived helpers

```rust
impl ScrollingComponent {
    /// First visible item index (inclusive).
    pub fn window_start(&self) -> usize {
        self.scroll_offset.floor() as usize
    }
    /// Last visible item index (exclusive).
    pub fn window_end(&self) -> usize {
        (self.window_start() + self.page_size).min(self.total_items)
    }
    pub fn max_scroll(&self) -> f32 {
        (self.total_items.saturating_sub(self.page_size)) as f32
    }
}
```

---

## ⊹ `ScrollChanged` event signal

Emitted by `ScrollSystem` (or the gesture handler registered by the system that
owns the `ScrollingComponent`) whenever `window_start()` changes.

```rust
EventSignal::ScrollChanged {
    /// The ScrollingComponent that changed.
    scroll_component: ComponentId,
    /// New first-visible item index.
    window_start: usize,
    /// New last-visible item index (exclusive).
    window_end: usize,
}
```

Scope: the `ScrollingComponent` itself.

The owning system's handler rebuilds the visible item subtree in response.

---

## ⊹ How the owning system uses it

### Setup

The owning system:

1. Creates a `ScrollingComponent` and attaches it in the panel hierarchy.
2. Registers a `DragMove` handler on the scroll anchor to call
   `scroll.apply_drag(delta_world_y)`.
3. Registers a `ScrollChanged` handler to trigger a content rebuild.

```
panel_anchor
  └── ScrollingComponent          ← scroll state lives here
        └── rows_anchor            ← only page_size row children live here
              ├── row_0 (Transform + Text + ...)
              ├── row_1
              └── ...              (up to page_size rows at a time)
```

### Content rebuild

On `ScrollChanged`, the owning system:

1. Reads `window_start` / `window_end` from the event (or directly from
   `ScrollingComponent`).
2. Runs its item-collection logic (e.g. `collect_visible_nodes` for the world
   panel) to get the full ordered item list.
3. Slices `items[window_start..window_end]`.
4. Tears down old row children (`detach_from_parent` + `RemoveSubtree` intent).
5. Rebuilds row children for the visible slice.
6. Calls `init_component_tree(rows_anchor, emit)`.

Row `i` in the visible window is positioned at:

```
y = -(i as f32) * item_height      // panel-local index, always 0..page_size
```

The owning system maps panel-local index `i` → global item index
`window_start + i` when fetching labels/data.

### Driving scroll from drag

The `DragMove` handler for the panel anchor updates the component and fires
`ScrollChanged` when the window changes:

```rust
// Inside DragMove handler registered on the scroll anchor:
if let Some(scroll) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_id) {
    let prev_start = scroll.window_start();
    scroll.scroll_offset -= delta_world_y / scroll.item_height;
    scroll.scroll_offset = scroll.scroll_offset.clamp(0.0, scroll.max_scroll());
    let new_start = scroll.window_start();
    if new_start != prev_start {
        scroll.last_window_start = new_start;
        emit.push_event(scroll_id, EventSignal::ScrollChanged {
            scroll_component: scroll_id,
            window_start: new_start,
            window_end: scroll.window_end(),
        });
    }
}
```

The sign convention: dragging **up** (positive `delta_world_y`) reveals items
lower in the list → `scroll_offset` increases → `window_start` increases.

---

## ⊹ Click vs drag disambiguation on the panel

With `ScrollingComponent` in place the gesture split is clean:

| Gesture | Handler | Effect |
|---|---|---|
| `DragMove` on scroll anchor | ScrollingComponent logic | update `scroll_offset`, emit `ScrollChanged` |
| `Click` on row | row's `Click` handler | select the item (emit `SelectionChanged`) |

`Click` is the new event from `docs/spec/click-and-panel-scroll.md`. Row selection
switches from `DragStart` → `Click` so that a drag-scroll gesture on a row doesn't
also trigger selection.

---

## ⊹ Scroll anchor placement

The `ScrollingComponent` sits *above* the rows anchor in the hierarchy so the
`DragMove` handler scoped to it captures drags across the entire panel surface,
not just on individual rows.

```
panel_anchor  (SelectableComponent::off)
  └── overlay
        └── panel_component  (WorldPanelComponent / InspectorPanelComponent)
              └── ScrollingComponent     ← DragMove + ScrollChanged handlers here
                    └── rows_anchor      ← only live rows here
```

`SelectableComponent::off` on `panel_anchor` still prevents the world panel from
treating clicks on itself as scene-object selection.

---

## ⊹ `total_items` update

The owning system must keep `scroll.total_items` in sync with the actual item
count (e.g. after a `SelectionChanged` that causes a full tree rebuild):

```rust
if let Some(scroll) = world.get_component_by_id_as_mut::<ScrollingComponent>(scroll_id) {
    scroll.total_items = nodes.len();
    scroll.scroll_offset = scroll.scroll_offset.clamp(0.0, scroll.max_scroll());
}
```

If `scroll_offset` is clamped downward (tree shrank), the owning system should
also trigger a `ScrollChanged`-equivalent rebuild.

---

## ⊹ MMS integration

```
ScrollingComponent {
    item_height(0.045)
    page_size(30)
}
```

Methods: `item_height(f32)`, `page_size(usize)`. `scroll_offset` is runtime state
only — not authored.

---

## Non-goals

- **Horizontal scrolling** — out of scope for now.
- **Variable item heights** — all items are `item_height` tall. Non-uniform
  heights require a different layout model.
- **Smooth fractional scrolling** — `scroll_offset` is continuous but the content
  rebuild fires only at integer boundaries (row-snapped scroll). Sub-row smooth
  animation (shifting row transforms by the fractional part each frame) is a
  follow-up.
- **Inertia / momentum scroll** — follow-up.

---

## Implementation checklist

- [ ] Add `EventSignal::ScrollChanged` + `SignalKind::ScrollChanged` to `signal.rs`
- [ ] Add `ScrollingComponent` to `src/engine/ecs/component/`
- [ ] Register `ScrollingComponent` in `component/mod.rs`
- [ ] Register in `meow_meow/component_registry.rs`
- [ ] Add `ScrollChanged` event handling in `InspectorSystem`
- [ ] Refactor `rebuild_world_panel` to accept `window_start: usize`
- [ ] Attach `ScrollingComponent` in `spawn_world_panel` / `spawn_inspector_panel`
- [ ] Wire `DragMove` on scroll anchor → `scroll_offset` update + `ScrollChanged`
- [ ] Switch world panel row selection from `DragStart` → `Click`
  (depends on `EventSignal::Click` from `click-and-panel-scroll.md`)
