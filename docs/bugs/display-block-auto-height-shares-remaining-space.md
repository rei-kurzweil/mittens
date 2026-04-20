# `display: block` auto-height items share remaining container height

## Status

Open behavior/compatibility bug.

## Symptom

Block-layout children with `height: Auto` currently split the remaining container height evenly when the container height is known.

That means multiple block items without explicit heights behave like flexible fill items instead of intrinsic-height block boxes.

## Expected behavior

For `display: block`:

- `height: Auto` should resolve to intrinsic content height
- block items should stack vertically based on their own measured height
- they should not evenly divide remaining container height just because the parent has a known height

That “share remaining height” behavior belongs to flex semantics, not block semantics.

## Actual behavior

Current measurement logic treats block `height: Auto` items as container-distributed auto-height items:

- auto-height block items are marked `is_auto_height = true`
- when container height is known, the remaining height is divided equally across those items
- each item's content/box height is rewritten from that equal share

## Where this exists

Implementation:

- [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs)

Current behavior:

- [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs#L92-L170)

Current draft docs that describe this behavior:

- [docs/draft/layout-system-impl-plan.md](../draft/layout-system-impl-plan.md#L8-L26)
- [docs/draft/block-layout-two-pass-walkthrough.md](../draft/block-layout-two-pass-walkthrough.md#L71-L110)
- [docs/analysis/world-panel-layout.md](../analysis/world-panel-layout.md)

## Why this matters

This diverges from normal CSS/block expectations and makes layout debugging confusing.

It also hides the intended semantic split:

- `display: block` should use intrinsic/box-model sizing
- `display: flex` should be the mode that distributes available space

## Likely fix direction

Move remaining-space distribution out of block auto-height handling and into flex layout semantics.

Concretely:

- block `height: Auto` should stay intrinsic unless a separate explicit fill/stretch concept exists
- flex containers should own the “share remaining space” behavior
- any docs that currently describe block auto-height as fill behavior should be updated after the code change

## Follow-up questions

- should there be a distinct fill/stretch size mode for non-flex cases?
- should current panel layouts be migrated to `display: Flex` once that behavior exists there?
- do we need regression tests for block intrinsic-height vs flex distributed-height behavior?
