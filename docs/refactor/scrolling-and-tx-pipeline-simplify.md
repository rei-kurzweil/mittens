# Scrolling and Transform Pipeline Simplify

Date: 2026-04-17

This note captures a refactor direction that became clearer while debugging the authored scrolling list in [examples/ui-layout.mms](../../examples/ui-layout.mms).

It is adjacent to, but not the same as, the older scrolling/layout ownership note in [docs/refactor/scrolling-component-layout-system.md](scrolling-component-layout-system.md).

That document is about who should own scrolling and clipping at the system/topology level.
This document is about making the authored topology and transform-pipeline semantics simpler, less error-prone, and easier to read in both MMS and hand-rolled runtime trees.

First implementation slice completed:

- `TransformForkTRS` now behaves as the effective merge point by itself.
- omitted `TransformMapTranslation` / `TransformMapRotation` / `TransformMapScale` branches still mean pass-through
- in-tree authored and runtime-built pipelines no longer need `TransformMergeTRS {}` to "activate" the fork stage

---

## 1. Problem statement

The current interaction between `Scrolling` and `TransformPipeline` is too topology-sensitive.

Today:

- `ScrollingComponent` does not create its own internal track transform.
- `ScrollingSystem` picks a transform above the component in the existing ancestry.
- a transform directly under `TransformPipelineOutput` is effectively pipeline-owned for world-matrix purposes.

That combination makes this kind of authored shape fragile:

```text
TransformPipelineOutput
└── T
    └── Scrolling
        └── content...
```

To make scrolling work correctly, we currently need this instead:

```text
TransformPipelineOutput
└── T                    <- pipeline output anchor
    └── T                <- scrolling-owned track
        └── Scrolling
            └── content...
```

That works, but it is not a great authoring model:

- it is easy to forget the second `T`
- the extra node is structural ceremony rather than semantic intent
- the boundary between pipeline-owned and scrolling-owned transforms is implicit
- it is too easy for scrolling to end up driving an ancestor transform that should really be pipeline-owned

The larger issue is not only the extra `T`, but that `Scrolling` currently controls a transform *above itself* in the hierarchy.

---

## 2. Desired direction

### 2.1 `Scrolling` should own its own moved topology

The desired model is:

- `Scrolling` should not control an arbitrary ancestor transform above itself
- `Scrolling` should own the transform(s) that it moves
- children of `Scrolling` should react to that internal moved track
- authoring `Scrolling { ... }` should be enough to get a stable local scroll subtree

Conceptually, this means `Scrolling` should behave more like:

```text
ScrollingComponent
└── __scroll_track_t
    └── authored children...
```

or equivalently some internal/runtime-owned topology with the same semantics.

The key rule is:

> `Scrolling` should move content *inside itself*, not by reaching upward and commandeering a parent transform.

That would make authored usage much more intuitive in MMS and would reduce the current transform-pipeline quirk.

### 2.2 This must preserve existing uses

Any refactor here must keep all three styles working:

1. authored MMS scrolling trees
2. hand-rolled runtime / ECS construction
3. editor panels (`WorldPanel`, `InspectorPanel`)

So this is not just a convenience change for MMS.
It is a topology/ownership cleanup that should unify the model across all call sites.

---

## 3. Transform pipeline simplification goals

While debugging the same subtree, several transform-pipeline ergonomics issues also became clear.

### 3.1 Omit pass-through `TransformMapTranslation` / `TransformMapRotation`

Today, if translation and rotation are just pass-through, authoring still often ends up looking like:

```text
TransformForkTRS
├── TransformMapTranslation {}
├── TransformMapRotation {}
├── TransformMapScale { TransformDrop {} }
└── TransformMergeTRS {}
```

But if translation and rotation are not doing anything special, those pass-through nodes add noise without information.

Desired simplification:

- if translation stream is just pass-through, `TransformMapTranslation` should be omittable
- if rotation stream is just pass-through, `TransformMapRotation` should be omittable
- omitted streams should default to `Pass`

That makes the authored intent much clearer:

```text
TransformForkTRS
└── TransformMapScale { TransformDrop {} }
```

instead of requiring explicit no-op nodes for the other channels.

### 3.2 `TransformMergeTRS` is not pulling its weight

In practice, `TransformMergeTRS` is not adding much value in the current authoring model.

In the common authored shapes we care about, the actual observable effect comes from `TransformPipelineOutput`, not from treating `TransformMergeTRS` as an interesting semantic nesting point.

The current pattern often looks like:

```text
TransformForkTRS
  ...
  TransformMergeTRS {}
TransformPipelineOutput { ... }
```

That means:

- the merge node exists mostly as ceremony
- the output node is what actually matters for attaching visible/runtime-controlled content

Desired direction:

- reduce the semantic importance of `TransformMergeTRS`
- avoid treating it as a meaningful container for user-authored subtree content
- keep `TransformPipelineOutput` as the real attachment/output concept

This does **not** necessarily mean deleting `TransformMergeTRS` immediately.
But it does mean we should stop leaning on it as if it were the interesting part of the tree.

### 3.3 Nothing should ever be nested inside `TransformMergeTRS`

A concrete topology rule to enforce:

> `TransformMergeTRS` should not become a user-content container.

If content is nested under `TransformMergeTRS`, the authored tree becomes harder to reason about because merge semantics and output attachment semantics get conflated.

Desired invariant:

- `TransformMergeTRS` is a structural/operator marker only
- user/content/runtime attachment should happen through `TransformPipelineOutput`
- no arbitrary subtree content should be parented under `TransformMergeTRS`

This should be true in MMS and in hand-built ECS trees.

---

## 4. Default behavior rules

### 4.1 Default op for each forked TRS stream is `Pass`

When `TransformForkTRS` is attached/initialized, each stream should default to `Pass`:

- translation default = `Pass`
- rotation default = `Pass`
- scale default = `Pass`

This should be true whether the defaults are represented:

- implicitly in parsing/runtime behavior
- explicitly in internal pipeline data structures
- or both

The important authoring consequence is:

- missing `TransformMapTranslation` means pass-through translation
- missing `TransformMapRotation` means pass-through rotation
- missing `TransformMapScale` means pass-through scale

This matches user expectation much better than requiring explicit empty pass-through nodes.

### 4.2 Missing map node should not mean “stream absent”

A missing stream node should not mean the stream vanishes.
It should mean:

```text
stream operation = Pass
```

This is especially important if we want slim authored shapes like:

```text
TransformForkTRS
└── TransformMapScale { TransformDrop {} }
```

That shape should clearly mean:

- translation = pass
- rotation = pass
- scale = drop

---

## 5. Refactor target shape

Putting the two sides together, the desired authored shape should become simpler and more semantic.

Something conceptually like:

```text
StencilClip
└── TransformPipeline
    ├── TransformForkTRS
    │   └── TransformMapScale { TransformDrop {} }
    └── TransformPipelineOutput
        └── Scrolling
            └── content...
```

with runtime behavior ensuring that:

- omitted TRS maps imply pass-through
- scrolling owns its own moved track inside its subtree
- pipeline output remains the output anchor, not the scroll track itself

That would eliminate the current need for:

- explicit pass-through map nodes
- a largely ceremonial merge node in common cases
- author-written extra `T` just so scrolling has something safe to move

---

## 6. Compatibility constraints

We should preserve current working content while moving toward this.

Important constraints:

- do not break existing editor panels while refactoring `Scrolling`
- do not make old authored `TransformForkTRS` shapes silently change meaning
- do not make `TransformMergeTRS` nested-content shapes valid if we plan to forbid them
- do not make transform-pipeline parse/runtime defaults ambiguous

This suggests a staged migration rather than a single rewrite.

---

## 7. Suggested refactor phases

### Phase 1 — document and enforce topology rules

- document that `TransformMergeTRS` is not a content container
- document that omitted TRS map streams mean `Pass`
- audit existing authored trees for content nested under `TransformMergeTRS`
- add validation / warnings for invalid nesting if practical

### Phase 2 — simplify pipeline authoring defaults

- make `TransformForkTRS` treat missing translation/rotation/scale stream nodes as pass-through
- allow authored trees to omit no-op translation/rotation maps
- verify current examples still parse and evaluate identically

### Phase 3 — refactor `Scrolling` ownership model

- move `Scrolling` away from controlling ancestor transforms
- make `Scrolling` own an internal scroll track topology
- keep drag-scope semantics and scrolling events stable
- migrate editor panel creation and authored MMS shapes to the new model

### Phase 4 — simplify common MMS/reference examples

Once phases 2 and 3 exist, convert the canonical examples to the simpler shape:

- no explicit pass-through translation/rotation maps unless they do real work
- no user content under `TransformMergeTRS`
- no extra manual `T` only for scrolling track ownership

---

## 8. Open questions

### 8.1 Should `Scrolling` literally spawn child transforms?

Possibly, but the important requirement is semantic ownership, not a specific implementation detail.

The runtime could satisfy the goal by:

- spawning an internal helper track node
- reparenting children under a helper track
- or otherwise maintaining an equivalent internal topology

The essential rule is still:

- scrolling-owned motion should happen inside the `Scrolling` subtree
- not by mutating a parent transform outside the component

### 8.2 Should `TransformMergeTRS` remain visible in MMS at all?

Maybe, maybe not.

Even if it remains in the runtime representation, we may want MMS authoring to de-emphasize it heavily or eventually make it implicit in the common case.

This note does not require deleting it now.
It only records that it is currently not helping much in the common authored shapes.

---

## 9. Success criteria

This refactor is successful when:

- `Scrolling { ... }` no longer needs a manually authored parent track transform just to be safe
- `Scrolling` never needs to drive a transform above itself in the hierarchy
- `TransformForkTRS` with omitted stream nodes behaves as intuitive pass-through
- simple pipeline authoring can omit no-op translation/rotation maps
- no user/content subtree is nested under `TransformMergeTRS`
- editor panels, hand-rolled trees, and MMS examples all keep working under the same conceptual model
