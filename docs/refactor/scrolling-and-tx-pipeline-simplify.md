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

## 7. Implementation plan before changing `src/`

This is the concrete rollout plan for moving scroll-track ownership inside `Scrolling`.

The goal is to make this authored shape work directly:

```text
TransformPipelineOutput
└── Scrolling
    └── content...
```

without requiring the current manual helper shape:

```text
TransformPipelineOutput
└── T
    └── T
        └── Scrolling
            └── content...
```

### Phase 1 — lock the runtime contract

Before rewriting the runtime shape, we should explicitly define the new `Scrolling` contract:

- `ScrollingComponent` becomes the semantic owner of the moved content track
- the moved track must always live *inside* the `Scrolling` subtree
- `ScrollingSystem` should stop discovering an ancestor transform as its implicit track
- authored children of `Scrolling` should be treated as content to be wrapped/moved
- explicit `track` support, if kept at all, should become an escape hatch rather than the default model

Deliverables for this phase:

- doc updates only
- agreed invariants for helper naming/ownership
- explicit statement of what existing topology remains temporarily supported during migration

### Phase 2 — introduce runtime-owned helper topology

Add a runtime-managed internal shape for `Scrolling`, conceptually:

```text
Scrolling
└── __scroll_track
    └── authored children...
```

or, if a separate anchor is useful:

```text
Scrolling
└── __scroll_root
    └── __scroll_track
        └── authored children...
```

Key design decisions for this phase:

- `__scroll_track` should be a normal `TransformComponent`
- any helper nodes must be clearly runtime-owned / reserved (`__scroll_*`)
- helper nodes must not be mistaken for user-authored content by layout, clipping, or editor tooling
- the helper shape must work the same in MMS, hand-built trees, and panel/editor subtrees

This is the phase that removes the need for the extra user-authored `T` around `Scrolling`.

### Phase 3 — migrate registration and sync logic

Once the helper topology exists, move `ScrollingSystem` from “discover ancestor track” to “ensure owned track”.

Concretely:

- registration should ensure the helper track exists
- registration should record that helper track in `ScrollingComponent.track`
- `track_base_pos` should be captured from the helper track itself
- `sync_component(...)` should only ever move the helper track
- drag-scope behavior should stay unchanged unless a separate drag-scope cleanup is needed

Important compatibility rule:

- old authored trees that already supply a safe explicit track should continue to work during migration
- but implicit nearest-ancestor-track discovery should be treated as legacy behavior to phase out

### Phase 4 — reparent authored children under the helper track

This is the most topology-sensitive part and should be implemented deliberately.

The runtime needs a clear rule for which children of `Scrolling` are:

- user/authored content that should move with scrolling
- runtime helper nodes that should stay outside the moved content branch
- style/behavior nodes that belong on the `Scrolling` root itself

The simplest target rule is:

- authored/rendered/layout content under `Scrolling` is wrapped under `__scroll_track`
- `ScrollingComponent` itself stays on the `Scrolling` root
- future scroll-related helper nodes also stay on the `Scrolling` root unless they are part of the moved branch

Implementation detail to decide before coding:

- whether reparenting happens eagerly during registration/init
- or whether a dedicated topology-sync step maintains the helper shape over time

The second option is probably safer if children can be attached after init.

### Phase 5 — preserve topology mutations after init

The new model must keep working when content is added later.

That means we should plan for:

- late-added children under `Scrolling`
- editor/runtime mutation paths that insert nodes after initialization
- MMS-evaluated subtrees that attach more content after the scroll helper already exists

So the runtime likely needs one of:

- a parent-change / child-attached maintenance hook for `Scrolling`
- or an idempotent sync pass that re-homes new content under `__scroll_track`

Without this phase, the feature will work on initial trees but drift out of shape during editing/runtime mutation.

### Phase 6 — migrate canonical call sites

After helper ownership works, convert the known call sites to the simpler model:

1. [examples/ui-layout.mms](../../examples/ui-layout.mms)
2. editor panels / inspector-owned scrolling subtrees
3. any hand-built ECS trees that still create a manual scroll track transform

The conversion target is:

- no extra user-authored `T` only for scrolling ownership
- no ancestor transform being implicitly commandeered by scrolling
- pipeline output transforms remain pure output anchors

### Phase 7 — simplify the remaining API surface

After migration, decide what to do with legacy knobs:

- keep `track: Option<ComponentId>` only as an advanced/manual override
- or remove implicit external-track control entirely
- document whether `track_base_pos` remains public state or becomes runtime-owned internals

This phase should also update docs/comments so the public mental model becomes:

> `Scrolling { ... }` owns the moved content branch inside itself.

### Phase 8 — validation and regression coverage

Before calling the refactor done, we should add/keep coverage for:

- `Scrolling` directly under `TransformPipelineOutput`
- `Scrolling` under ordinary authored transforms
- editor panel scrolling
- late-added children appearing under the scroll track automatically
- drag-scope behavior staying stable in clipped/manual viewport trees

This is the phase that proves the new ownership model is actually safer, not just simpler on paper.

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

### 8.2 Should late-added children be auto-rehomed?

Probably yes, if `Scrolling` is going to be a true topology owner.

The key decision is not whether to support that behavior, but where to implement it:

- in `ScrollingSystem` via parent-change hooks
- in a more general topology-sync layer
- or in a small helper owned by `SystemWorld`

We should answer that before implementation so we do not end up with init-time-only behavior.

### 8.3 Should `TransformMergeTRS` remain visible in MMS at all?

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
