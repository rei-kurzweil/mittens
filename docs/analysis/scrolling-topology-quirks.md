# Scrolling Topology Quirks

Date: 2026-04-17

Historical note: this debugging note refers to `TransformPipelineOutput`, which has been removed from the authored API. The current authored shape is `TransformForkTRS` with the driven `T` attached directly under the fork root.

Notes from debugging the authored scrolling list in [examples/ui-layout.mms](../../examples/ui-layout.mms).

The forward-looking refactor plan that should remove this quirk is tracked in
[docs/task/refactor/scrolling-and-tx-pipeline-simplify.md](../task/refactor/scrolling-and-tx-pipeline-simplify.md).

This is not quite a bug report. The runtime behaved consistently with its current ownership rules, but those rules were easy to violate when combining `Scrolling`, `TransformPipeline`, and manually authored clip/view hierarchies.

---

## Short version

`ScrollingComponent` does not create its own transforms.

It only stores scroll state and asks `ScrollingSystem` to move an existing track transform.

The track is chosen like this:

- explicit `track` if one was already assigned
- otherwise the nearest ancestor `TransformComponent`

That means authored topology matters.

---

## The quirk

A `TransformComponent` that sits directly under `TransformPipelineOutput` is effectively pipeline-owned.

`TransformSystem` treats that transform as a pipeline boundary and preserves its cached `matrix_world` from pipeline evaluation instead of recomputing it from local transform changes.

This is intentional. The relevant logic is in [src/engine/ecs/system/transform_system.rs](../../src/engine/ecs/system/transform_system.rs).

The consequence is:

- if `ScrollingSystem` picks that pipeline-output child transform as its track
- and then emits `UpdateTransform` to move it
- the scroll state changes, but the visual world transform does not update the way a normal authored transform would

So the list can receive drag events and show changing `scroll_offset`, while appearing visually stuck.

---

## The failing shape

This authored pattern is fragile:

```text
TransformPipelineOutput
└── T                       <- nearest ancestor transform for Scrolling
    └── Scrolling
        └── items...
```

In that shape, `ScrollingSystem` will usually choose the `T` directly under `TransformPipelineOutput` as its moved track.

That was the shape used during debugging of [examples/ui-layout.mms](../../examples/ui-layout.mms).

Symptoms:

- drag delivery works
- `ScrollingSystem` logs show `scroll_offset` changing
- content does not visibly move as expected

---

## The safe shape

Keep a separate pipeline-owned output root and a separate scrolling-owned track transform below it:

```text
TransformPipelineOutput
└── T                       <- pipeline-owned output root
    └── T                   <- normal authored transform used as scroll track
        └── Scrolling
            └── items...
```

In MMS, that looks like:

```c
TransformPipelineOutput {
    T {
        T {
            Scrolling.new(1.0, 100.0) {
                // content
            }
        }
    }
}
```

This gives `ScrollingSystem` a normal inner `T` to move, while the outer `T` remains the pipeline output anchor.

---

## What `Scrolling` does not do

`Scrolling.new(...)` does not spawn a container transform or a track transform.

It only registers a `ScrollingComponent` with:

- `viewport_height`
- `content_height`
- `scroll_offset`
- optional `track`
- optional `drag_scope`

So when authoring MMS, the moved transform must already exist in the tree.

See [src/engine/ecs/component/scrolling.rs](../../src/engine/ecs/component/scrolling.rs).

---

## Related drag-scope note

For manually authored clip trees, the drag surface and the scrolling drag scope also need to line up.

In the `ui-layout` case, the viewport plane needed a raycastable surface so drag events would actually land in the same clipped branch.

That issue was separate from the transform-pipeline quirk:

- drag-scope mismatch prevents scrolling input entirely
- pipeline-owned track mismatch allows input, but prevents visible motion

Both can happen in the same authored subtree.

---

## Practical guideline

When combining `Scrolling` with `TransformPipeline`:

- treat the `TransformPipelineOutput` child transform as an output anchor
- add one more inner `T` if scrolling should move that content
- attach `Scrolling` under the inner `T`, not directly under the first transform below `TransformPipelineOutput`

If scrolling appears to work numerically but not visually, check whether the chosen track is sitting directly under `TransformPipelineOutput`.
