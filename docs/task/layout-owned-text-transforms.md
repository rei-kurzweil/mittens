# Layout-owned Text and TextInput Transforms

## Context
Currently, the `LayoutSystem` (specifically `apply_text_align` in [src/engine/ecs/system/layout/block.rs](src/engine/ecs/system/layout/block.rs)) requires an explicit author-provided `TransformComponent` (usually via a `T {}` wrapper) to perform `text_align` and `vertical_align` shifts. 

If an author writes `Style { vertical_align("middle") } Text { "..." }` without a `T` wrapper, the text sits at the top-left (0,0) of the content box because there is no `TransformComponent` for the layout system to target with an `UpdateTransform` intent.

## Goal
Automate the creation of a "layout-owned" transform for `Text` and `TextInput` components so they participate in alignment math even when not wrapped in an explicit `T`.

## Requirements
- **Automatic Wrapping:** When a styled block contains a `Text` or `TextInput` as a direct child, the engine should ensure a `TransformComponent` is available to drive the alignment.
- **Consistency:** Both `Text` and `TextInput` should follow the same alignment rules.
- **Backward Compatibility:** If the user *has* provided an explicit `T`, we should continue to use it rather than double-wrapping.
- **Z-Order:** Ensure the automatic transform maintains correct Z-ordering (usually slightly in front of the background).

## Proposed Changes
1. **Component Registry / Spawning:**
   - Update `spawn_tree` or the `Text`/`TextInput` expansion logic to always ensure a `TransformComponent` is present at the attachment point if one isn't already there.
   - Alternatively, have the `LayoutSystem` spawn a `__layout_text_transform` node if it detects a "naked" text component during the layout pass.

2. **Layout System (`block.rs`):**
   - Modify `find_text_bearing_direct_child` to actually *create* the child if the styled node has a `TextComponent` but no `TransformComponent` child.
   - Update the "text bearing" check to be more robust across both standard and expanded (per-glyph) text trees.

3. **TextInput Specifics:**
   - `TextInput` already spawns internal labels (`__text_input_content`). We should unify how these internal nodes are positioned so they respect the parent's alignment.

## Task Breakdown
- [ ] Audit `apply_text_align` for "naked" component support.
- [ ] Determine if the expansion should happen at `spawn_tree` time (MMS) or later in a System.
- [ ] Update `TextInputSystem` to ensure its internal `__text_input_content` node is a valid alignment target.
- [ ] Verify that `vertical_align("middle")` works on a raw `TextInput` without manual `T` wrapping.
