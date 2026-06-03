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

The `LayoutRootComponent` has to stay attached to the preview subtree permanently — not just for a temporary measure pass — because the preview needs to keep rendering so we can see the asset's actual visual output. Layout must continue to tick on it every frame.

### Steps

1. **Insert a `LayoutRootComponent` between the `preview_slot` and the `asset_preview_shell`** so the preview subtree has a layout root to resolve against.

   Current hierarchy:
   ```
   preview_slot  (styled, from asset_item.mms)
     └── asset_preview_shell  (transform, offset + scale)
           └── preview_root   (the spawned asset)
   ```

   New hierarchy:
   ```
   preview_slot  (styled, from asset_item.mms)
     └── layout_root  (LayoutRootComponent)    ── stays permanently
           └── preview_root   (the spawned asset)
   ```

   The `asset_preview_shell` transform (offset + scale) stays on `preview_slot` or is folded into a transform on the `layout_root` itself — the key change is that `preview_root` is parented directly to the layout root so its styled children resolve properly.

2. **Extend `BoundsSystem::calculate_subtree_local_bounds`** to handle `TextComponent`-only subtrees by estimating bounds from the font metrics (glyph cell dimensions) of the text. This gives a fallback AABB even when no `RenderableComponent` exists, enabling proper auto-scaling and centering.

3. **Alternatively**: run a layout measure pass on the preview before computing bounds, since with a `LayoutRootComponent` present the styled transforms will have resolved sizes. The bounds walk can then use those resolved sizes.

## Affected files

- `src/engine/ecs/system/asset_system.rs` — `build_asset_item_shell()` (lines 308–428): fallback behavior for bounds = None
- `src/engine/ecs/system/bounds_system.rs` — `calculate_subtree_local_bounds()`: only considers `RenderableComponent`
- `src/engine/ecs/system/layout/` — layout resolution pass (may need a standalone measure API)

## Related

- `assets/components/asset_item.mms` — the tile template with `preview_slot`
- `docs/spec/file-tree-panel.md` — panel prefab pattern
- `docs/spec/signals.md` — signal/intent pipeline (layout runs during the tick)
