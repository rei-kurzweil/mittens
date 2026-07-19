# Asset preview layout resolution

## Context

Asset preview tiles (`asset_item.mms`) show a scaled preview of each exported function from `assets/components/`. Exports that return raw geometry (like `pencil_icon()` in `icons.mms`) render correctly — their `RenderableComponent` bounds are computed by `BoundsSystem::calculate_subtree_local_bounds`, auto-scaled to fit within 0.2 GU, and centered in the tile.

Exports that return UI components — styled transforms containing `Text` — fail to produce usable bounds. The preview shows giant uncentered "Preview" text instead.

## Root cause

`BoundsSystem::calculate_subtree_local_bounds` (`src/engine/ecs/system/bounds_system.rs:37`) only looks for `RenderableComponent`:

```rust
if let Some(r) = world.get_component_by_id_as::<RenderableComponent>(node) {
```

Text components are not `RenderableComponent`. They get their visual size from the **layout system** — a styled transform's width/height is resolved by the `LayoutSystem` only when the component is inside a subtree attached to a `LayoutRootComponent`.

When an asset like `button(label)` is spawned as a preview, it returns a styled transform with a `Text` child. Since it is **not attached to a layout root**, layout never runs on it. The styled transform's resolved size stays at its initial/default value, and the `BoundsSystem` finds zero `RenderableComponent`s → returns `None` → fallback `scale = 0.5` with `offset = [0, 0, 0]` — no centering and insufficient shrink.

## Approach

The `LayoutComponent` has to stay attached to the preview subtree permanently — not just for a temporary measure pass — because the preview needs to keep rendering so we can see the asset's actual visual output. Layout must continue to tick on it every frame.

The core challenge is a **timing problem**: `BoundsSystem::calculate_subtree_local_bounds` runs during setup, before the `LayoutSystem` has had a chance to tick. Styled transforms have no `RenderableComponent` at spawn time — the layout system generates background quads (which DO carry `RenderableComponent`) when it runs. So we can't measure styled content until after layout has resolved it.

### Hierarchy

```
preview_slot  (styled, from asset_item.mms)
  └── preview_shell  (transform, offset + scale)
        └── layout_root  (LayoutComponent)    ── stays permanently
              └── preview_root   (the spawned asset)
```

The layout root must be parented **below** `preview_shell` and **above** `preview_root`. The layout system's `measure_container_items` only walks **direct children** of the `LayoutComponent` that pass `is_layout_item` (have a `TransformComponent` + a `StyleComponent` child). If `preview_shell` (a plain transform with no `StyleComponent` child`) were between the layout root and the styled content, layout would never descend to the actual styled elements.

### Build phase (during `build_asset_item_shell`)

1. Spawn the preview (`button("Preview")` etc.)
2. Check if it needs layout via `subtree_needs_layout_root` (walks for `StyleComponent` without `LayoutComponent` ancestor)
3. Try `BoundsSystem::calculate_subtree_local_bounds`:
   - **Has bounds** (icons, geometry assets) → compute scale/offset to fit within 0.2 GU, set on `preview_shell` immediately. No layout root needed. Done.
   - **No bounds AND needs layout** → create `preview_shell` with **identity transform** (no scale/offset yet), insert `LayoutComponent` between `preview_shell` and `preview_root`. Push `preview_shell` onto a `pending_remeasure` list.
   - **No bounds AND doesn't need layout** → fallback scale 0.5, done.

### Remeasure phase (after `LayoutSystem::tick`)

The `AssetSystem` holds a `pending_remeasure: Vec<ComponentId>` — the ids of `preview_shell` transforms that were created for styled-content previews.

After `LayoutSystem::tick()` completes in the main tick sequence (`SystemWorld::tick`), call:

```rust
asset_system.remeasure_pending_previews(world, render_assets, emit);
```

This method:
1. For each pending `preview_shell`, find its child layout_root → preview_root chain
2. Run `BoundsSystem::calculate_subtree_local_bounds(world, render_assets, preview_root)` — layout-generated background quads now exist, so bounds should return a real AABB
3. Compute scale/offset from the bounds (same math as the build phase: fit within 0.2 GU)
4. Emit `IntentValue::UpdateTransform` on `preview_shell` to set the real position and scale
5. Remove from pending list

### Why this works

Layout-generated background quads are children of the styled transform node, created during `LayoutSystem::tick()`. They have a `RenderableComponent` with a proper mesh and AABB. After layout runs, `BoundsSystem::calculate_subtree_local_bounds` can finally see them and compute accurate aggregate bounds, giving proper auto-scaling and centering for the preview.

## Detecting whether a preview needs layout

Not every asset needs a `LayoutComponent` wrapper — only those whose spawned subtree contains **styled transforms** (`StyleComponent`) without an existing `LayoutComponent` ancestor.

Detection (`subtree_needs_layout_root` in `asset_system.rs`): walk the spawned preview subtree with a stack + visited set. For each node with a `StyleComponent`, walk up ancestors looking for a `LayoutComponent`. If none found, return `true` (needs layout root).

The scale fallback uses the same detection result: if bounds are None and the subtree needs layout, use a very small scale (0.05) since glyph-unit dimensions are huge in world space. This is a temporary placeholder until the remeasure pass runs.

## Affected files

- `src/engine/ecs/system/asset_system.rs` — `build_asset_item_shell()`: deferred remeasure flow, `subtree_needs_layout_root()` detection, `pending_remeasure` vec + `remeasure_pending_previews()` method
- `src/engine/ecs/system/bounds_system.rs` — `calculate_subtree_local_bounds()`: only considers `RenderableComponent` (layout-generated quads appear after layout ticks, which is why remeasure is needed)
- `src/engine/ecs/system/system_world.rs` — tick sequence: call `asset_system.remeasure_pending_previews()` after `LayoutSystem::tick`

## Related

- `assets/components/asset_item.mms` — the tile template with `preview_slot`
- `docs/spec/file-tree-panel.md` — panel prefab pattern
- `docs/how_to/guide/signals.md` — signal/intent pipeline (layout runs during the tick)
