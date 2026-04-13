# Scroll Container Redesign — Analysis & Design

(ﾉ◕ヮ◕)ﾉ*:･ﾟ✧ Replacing `ScrollingComponent` with layout-native, gesture-wired scroll containers.

---

## Problem Summary

`ScrollingComponent` was built before the layout system existed. It implements scroll by:
1. Virtual-windowing: only PAGE_SIZE rows are in the ECS at any time; rows are torn down and rebuilt as the window advances.
2. A `sub_row_y_offset` delta applied to `rows_anchor` for sub-row smooth scroll.
3. Manually wired DragMove handlers per-panel in `InspectorSystem::setup_panels_for_editor`.

This creates several compounding bugs (see `panel-scroll-and-clipping.md`). The correct approach is:

- All rows exist in the ECS simultaneously.
- The container clips via stencil (`overflow: Scroll`).
- Scroll is a simple Y-translation on an inner "scroll track" TC.
- The drag handler is wired automatically by the engine when a `ScrollContainerComponent` is registered — the same way gizmos wire their handlers.

---

## Target Topology

```
container_tc (TransformComponent)
  container_style (StyleComponent { overflow: Scroll, height: GlyphUnits(N) })
  ScrollContainerComponent          ← owns scroll_y, viewport_height, scroll_track ref
  __bg (TC, label "__bg")           ← spawned by LayoutSystem for overflow:Scroll
    ColorComponent                  ← background fill (may be transparent)
    RenderableComponent::square()
    StencilClipComponent            ← already present; clips children to container box
    RaycastableComponent::drag_only()   ← NEW: captures drag for scroll
    RaycastableShapeComponent::Quad2D   ← NEW: hit geometry matches clip quad
  scroll_track (TC)                 ← inner scrollable area; LayoutSystem positions its children
    LayoutComponent { available_width, available_height: None }
    row_0 (TC)
    row_1 (TC)
    ...
```

The outer `container_tc` is the scroll viewport (clipped to `N` glyph units tall).
The inner `scroll_track` is translated by `scroll_y` (always ≤ 0, clamped by content height).
LayoutSystem positions rows inside `scroll_track` without any height constraint.

This mirrors the browser model exactly:
- `overflow: Scroll` on the container = stencil clip + drag-capture on `__bg`
- `transform: translateY(scroll_y)` on `scroll_track` = `UpdateTransform` to `scroll_track`

---

## New Component: `ScrollContainerComponent`

```rust
pub struct ScrollContainerComponent {
    /// Accumulated scroll in world units. Always ≤ 0 (0 = top).
    pub scroll_y: f32,

    /// Total content height in world units — updated by LayoutSystem after each layout pass.
    /// Clamp: scroll_y ≥ -(content_height − viewport_height).max(0)
    pub content_height: f32,

    /// Visible area height in world units — set at spawn time from
    /// LayoutComponent.available_height × unit_scale, or from StyleComponent.height.
    pub viewport_height: f32,

    /// ComponentId of the inner scroll track TC (parent of all scrollable children).
    pub scroll_track: Option<ComponentId>,

    /// ComponentId of the `__bg` TC — set by LayoutSystem when it spawns the clip quad.
    /// Used by SystemWorld to find the scope for handler registration.
    pub(crate) bg_tc: Option<ComponentId>,

    component: Option<ComponentId>,
}
```

`ScrollContainerComponent::init()` emits `IntentValue::RegisterScrollContainer { component_ids: vec![id] }`.

---

## Changes Required

### 1. `layout/block.rs` — `sync_bg_quad`: add raycastable to `__bg` for scroll

When `overflow: Scroll` (i.e. `needs_clip = true`), after spawning/finding `bg_id`:

```rust
// Ensure __bg has RaycastableComponent::drag_only() for scroll.
// Also store bg_tc in the parent TC's ScrollContainerComponent (if present).
if needs_clip {
    sync_scroll_raycast(world, emit, tc_id, bg_id);
}
```

```rust
fn sync_scroll_raycast(
    world: &mut World,
    emit: &mut dyn SignalEmitter,
    tc_id: ComponentId,
    bg_id: ComponentId,
) {
    // Add RaycastableComponent + shape to __bg if not present.
    let children = world.children_of(bg_id).to_vec();
    let has_rc = children.iter().any(|&ch| {
        world.get_component_by_id_as::<RaycastableComponent>(ch).is_some()
    });
    if !has_rc {
        let rc = world.add_component(RaycastableComponent::drag_only());
        let shape = world.add_component(RaycastableShapeComponent::new(RaycastableShapeType::Quad2D));
        let _ = world.add_child(bg_id, rc);
        let _ = world.add_child(bg_id, shape);
        world.init_component_tree(rc, emit);
        world.init_component_tree(shape, emit);
    }

    // Update ScrollContainerComponent.bg_tc if the parent TC has one.
    let tc_children = world.children_of(tc_id).to_vec();
    if let Some(scc_id) = tc_children.iter().find(|&&ch| {
        world.get_component_by_id_as::<ScrollContainerComponent>(ch).is_some()
    }) {
        if let Some(scc) = world.get_component_by_id_as_mut::<ScrollContainerComponent>(*scc_id) {
            scc.bg_tc = Some(bg_id);
        }
    }
}
```

**After all items are laid out**, update `content_height` on any `ScrollContainerComponent`:

```rust
// At the end of block::layout, after the cursor_gu loop:
update_scroll_content_height(world, emit, layout_id, cursor_gu * unit_scale);
```

```rust
fn update_scroll_content_height(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    layout_id: ComponentId,
    total_content_world: f32,
) {
    // Walk up to find the parent TC of layout_id, then look for a sibling
    // ScrollContainerComponent.
    if let Some(parent) = world.parent_of(layout_id) {
        // parent is the scroll_track TC; its parent is the container_tc.
        if let Some(container_tc) = world.parent_of(parent) {
            let children = world.children_of(container_tc).to_vec();
            for &ch in &children {
                if let Some(scc) = world.get_component_by_id_as_mut::<ScrollContainerComponent>(ch) {
                    scc.content_height = total_content_world;
                    // Re-clamp after resize.
                    let max_scroll = (scc.content_height - scc.viewport_height).max(0.0);
                    scc.scroll_y = scc.scroll_y.clamp(-max_scroll, 0.0);
                }
            }
        }
    }
}
```

### 2. `rx/signal.rs` — new `IntentValue` variant

```rust
RegisterScrollContainer {
    component_ids: Vec<ComponentId>,
},
```

### 3. `system_world.rs` — intent executor dispatch

In the intent executor's `execute_high_level_intent` or equivalent match:

```rust
IntentValue::RegisterScrollContainer { component_ids } => {
    for &id in component_ids {
        self.register_scroll_container(world, id, emit);
    }
}
```

```rust
pub fn register_scroll_container(
    &mut self,
    world: &mut World,
    scroll_container_id: ComponentId,
    _emit: &mut dyn SignalEmitter,
) {
    // Find bg_tc from the component.
    let bg_tc = world
        .get_component_by_id_as::<ScrollContainerComponent>(scroll_container_id)
        .and_then(|scc| scc.bg_tc);

    let Some(bg_tc) = bg_tc else { return };

    // Scope for the handler is bg_tc itself (DragMove propagates up from renderable).
    self.scroll_system
        .install_scoped_handlers(&mut self.rx, bg_tc, scroll_container_id);
}
```

### 4. New `ScrollSystem`

```rust
pub struct ScrollSystem;

impl ScrollSystem {
    pub fn install_scoped_handlers(
        &mut self,
        rx: &mut RxWorld,
        scope: ComponentId,       // the __bg TC
        scc_id: ComponentId,      // the ScrollContainerComponent
    ) {
        rx.add_handler_closure(
            SignalKind::DragMove,
            scope,
            move |world, emit, env| {
                let Some(EventSignal::DragMove { delta_world, .. }) = env.event.as_ref() else {
                    return;
                };
                // Positive dy (dragging up in world space) → scroll content down → scroll_y decreases.
                let dy = delta_world[1];

                let (new_y, scroll_track) = {
                    let Some(scc) =
                        world.get_component_by_id_as_mut::<ScrollContainerComponent>(scc_id)
                    else {
                        return;
                    };
                    let max_scroll = (scc.content_height - scc.viewport_height).max(0.0);
                    scc.scroll_y = (scc.scroll_y + dy).clamp(-max_scroll, 0.0);
                    (scc.scroll_y, scc.scroll_track)
                };

                if let Some(track) = scroll_track {
                    emit.push_intent_now(
                        track,
                        IntentValue::UpdateTransform {
                            component_ids: vec![track],
                            translation: [0.0, new_y, 0.0],
                            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                            scale: [1.0, 1.0, 1.0],
                        },
                    );
                }
            },
        );
    }
}
```

---

## How Panels Use This (Inspector / World Panel)

After the migration, `spawn_world_panel` and `spawn_inspector_panel`:

1. **Remove** `ScrollingComponent` creation and the manual `DragMove`/`ScrollChanged` handler wiring in `setup_panels_for_editor`.
2. **Remove** `spawn_drag_plane`.
3. Change `content_style` to:
   ```rust
   StyleComponent {
       overflow: Overflow::Scroll,
       height: SizeDimension::GlyphUnits(PAGE_HEIGHT_GU),
       background_color: Some([0.0, 0.0, 0.0, 0.0]),  // transparent — needed for __bg geometry
       ..StyleComponent::new()
   }
   ```
4. Add a `scroll_track` TC (the former `wpr`/rows_anchor) as a sibling of `content_style` inside `content_slot`:
   ```rust
   // content_slot children:
   //   content_style (StyleComponent with overflow:Scroll)
   //   ScrollContainerComponent { viewport_height, scroll_track: Some(track_tc), .. }
   //   track_tc (TC = scroll track, shifted by scroll_y)
   //     track_layout (LayoutComponent)
   //     [rows added here]
   ```
5. All rows are spawned under `track_layout` unconditionally — no virtual window, no PAGE_SIZE limit.

`SelectionChanged` still triggers a full row rebuild (since the world graph changes). But DragMove no longer triggers rebuild — it only shifts `track_tc`.

---

## Migration Checklist

- [ ] `ScrollContainerComponent` struct + `init()` emitting `RegisterScrollContainer`
- [ ] `IntentValue::RegisterScrollContainer` variant in `signal.rs`
- [ ] `ScrollSystem` struct with `install_scoped_handlers`
- [ ] Add `ScrollSystem` field to `SystemWorld`, instantiate in `new()`
- [ ] `SystemWorld::register_scroll_container` + intent dispatch in executor
- [ ] `block::sync_bg_quad`: add `RaycastableComponent::drag_only()` + shape to `__bg` when `needs_clip`
- [ ] `block::sync_bg_quad`: call `sync_scroll_raycast` + update `ScrollContainerComponent.bg_tc`
- [ ] `block::layout`: call `update_scroll_content_height` after cursor loop
- [ ] `spawn_world_panel`: use `overflow:Scroll` on `content_style`, add `ScrollContainerComponent`, remove `ScrollingComponent` + `spawn_drag_plane`
- [ ] `spawn_inspector_panel`: same
- [ ] `setup_panels_for_editor`: remove manual `DragMove`/`ScrollChanged` handler wiring for both panels
- [ ] Remove `DRAG_MARGIN`, `DRAG_PLANE_*` constants, `spawn_drag_plane` fn (or keep `spawn_drag_plane` for debug toggle only with opacity=0)
- [ ] Update `WorldPanelComponent`/`InspectorPanelComponent`: remove `scroll_offset_rows`, `rows_anchor_base_pos`
- [ ] `rebuild_world_panel` / `rebuild_inspector_panel`: remove `window_start` arg, always render all rows
- [ ] Delete or deprecate `scrolling.rs` + `ScrollingComponent`

---

## Open Questions

### Q1: Where does `LayoutSystem::tick` get `scroll_y` info?

`LayoutSystem` needs to NOT translate `scroll_track` in its layout pass (it should only position `scroll_track`'s children, not move `scroll_track` itself). The scroll Y offset lives in `ScrollContainerComponent` and is applied exclusively by the DragMove handler via `UpdateTransform`. LayoutSystem only sets positions of children of its layout root — `scroll_track` is positioned by scroll, rows are positioned by layout. No conflict.

### Q2: How large is the row list before virtual windowing matters?

For world panel with many components, all rows being live in the ECS simultaneously may be expensive for large scenes (hundreds/thousands of components). The current PAGE_SIZE=30 hard limit was set partly for this reason.

**Short term**: Keep all rows live; optimize later if needed.
**Long term**: Implement DOM recycling (same virtual-window concept but driven by scroll position from `ScrollContainerComponent`, not by rebuild-on-boundary). The `StencilClipComponent` already hides out-of-bounds rows visually; the cost is per-tick transform propagation for non-visible TCs.

### Q3: `DragMove` scope walk — does ancestor dispatch reach `__bg`?

`GestureSystem` fires `DragMove` with scope = the renderable hit at DragStart. `RxWorld.dispatch` walks up ancestors of that scope looking for registered handlers.

The handler is registered at `bg_tc` scope. For the handler to fire:
- The hit renderable must be a descendant of `bg_tc`, OR
- `bg_tc` must be the renderable itself (since `RenderableComponent` is registered as its own scope).

`__bg` has `RenderableComponent` as a direct child. The scope at DragStart = that renderable. Ancestor walk: renderable → bg_tc → container_tc → ... Handler at bg_tc fires. ✓

### Q4: `LayoutSystem` tick signature change?

`LayoutSystem::tick(&mut self, world: &mut World, emit: &mut dyn SignalEmitter)` — no `rx` needed. The raycastable/shape adds go through `emit` (registration intents), not directly via RxWorld. `ScrollContainerComponent::init` fires `RegisterScrollContainer` intent which is processed by `SystemWorld` (which has `rx`). LayoutSystem does not need `rx`. ✓
