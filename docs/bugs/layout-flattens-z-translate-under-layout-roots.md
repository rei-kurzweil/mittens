# Layout flattens child Z translation instead of preserving float z ordering

## Status

Open bug / investigation note.

No source changes made yet.

## Symptom

In the DIY panel repro, text/content that is authored with a positive local Z offset is still visually clipped against the yellow scroll container background instead of reliably layering in front of it.

Examples from the repro:

- row wrappers are authored with `T.position(0, 0, 0.2)`
- nested text wrappers are also authored with `T.position(0, 0, 0.2)`
- an authored child text wrapper uses `T.position(0, 0, 0.1)`

Expected intent is that these offsets survive layout as float layering information.

## Repro

- [examples/diy-panel.mms](../../examples/diy-panel.mms)
- [examples/diy-panel.rs](../../examples/diy-panel.rs)

Relevant authored nodes in the repro:

- the yellow scroll viewport: `container`
- routed rows with local Z offsets
- nested text wrappers with local Z offsets
- `authored_child` text wrapper with local Z offset

## Expected behavior

The layout system should own transforms under the layout root, but preserve authored local Z translation as layout-managed float z ordering.

That means:

- layout still computes X/Y placement
- layout still owns subtree transforms under the layout root
- authored Z translation survives as a float ordering value
- content can layer in front of layout-owned backgrounds without requiring ad hoc exemptions

## Actual behavior

Current block layout overwrites layout item transforms with a translation whose Z is always `0.0`.

Relevant code:

- [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs#L58-L76)

The same pass explicitly preserves only scale:

- existing scale is read from the item's `TransformComponent`
- X/Y are recomputed from layout cursor math
- Z is reset to `0.0`

Meanwhile, spawned background quads preserve a separate `background_z` style field:

- [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs#L131-L149)

So today the layout pass already has a notion of layout-managed Z for backgrounds, but not for general item transforms.

## Likely root cause

The block layout pass currently treats transform ownership as:

- preserve scale
- recompute X/Y
- discard item Z

That is probably too destructive for authored layout content that wants stable float layering.

## Investigation notes

Current behavior appears consistent with the user's direction:

- do not exempt arbitrary transforms under a layout root
- let layout own those transforms
- preserve local Z translation as `z_index`-like ordering
- keep that ordering as `f32`, not coerced to an integer

Questions to answer before code changes:

- should preserved Z live directly on `Transform.translation[2]`, or on a separate layout-time field that writes back into translation?
- should the same preservation rule apply to all layout-managed item transforms, nested layout items, and generated helper nodes?
- how should preserved item Z interact with `Style.background_z` for generated `__bg` quads?
- are there any layout passes besides block layout that currently flatten item Z in a similar way?

## Relevant files

- [src/engine/ecs/system/layout/block.rs](../../src/engine/ecs/system/layout/block.rs)
- [src/engine/ecs/system/layout/measure.rs](../../src/engine/ecs/system/layout/measure.rs)
- [examples/diy-panel.mms](../../examples/diy-panel.mms)
- [examples/diy-panel.rs](../../examples/diy-panel.rs)
