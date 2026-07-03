# `vr-input.rs`: ControllerXR armature splice stopgap

Date: 2026-03-21

Historical note: references below to `TransformPipeline` / `TransformPipelineOutput` describe the removed authored wrapper/output topology. Current authored controller smoothing uses `TransformForkTRS` as the root operator node with downstream content attached directly under that fork.

This note documents the **current stopgap topology** used in [examples/vr-input.rs](examples/vr-input.rs)
to drive VTuber hand bones from `ControllerXRComponent`.

This is not the final desired authoring model. It is a pragmatic example-level workaround that
lets us:

- insert a `ControllerXR` pose source into an existing imported armature,
- optionally place a transform pipeline under that source,
- attach the original wrist / hand subtree to the **pipeline output leaf** rather than to the
  `ControllerXR` root itself,
- preserve the existing wrist/hand/finger subtree IDs from the imported GLTF hierarchy.

This doc exists because the general splice docs are broader, while `vr-input.rs` now uses a very
specific shape that matters for filtering and for future MMS authoring.

---

## 1. Existing broader docs

Related docs already exist:

- [docs/task/refactor/splice-component-into-topology.md](../task/refactor/splice-component-into-topology.md)
- [docs/task/refactor/controller-xr-armature-targeting.md](../task/refactor/controller-xr-armature-targeting.md)
- [docs/analysis/vr-controller-rotation-filter-ab.md](docs/analysis/vr-controller-rotation-filter-ab.md)

Those docs cover:

- general splice semantics,
- why `ControllerXR` is an awkward fit for an imported armature,
- why the transform pipeline had to be tested on the actual wrist-driving path.

What they do **not** spell out clearly enough is the exact stopgap topology now authored in
`vr-input.rs`.

---

## 2. What the stopgap is trying to accomplish

We want to change this imported armature edge:

```text
LowerArm -> Wrist
```

into something more like:

```text
LowerArm -> ControllerXR(root) -> driven Transform -> optional TransformPipeline -> output leaf -> Wrist
```

The important detail is:

> the **root** of the inserted subtree is what connects to the old parent,
> but the **old child subtree** gets reattached to a **nested leaf/output node** inside the inserted subtree.

So this is not a simple one-edge splice:

```text
parent -> inserted -> child
```

It is a **two-ended splice** where:

- the inserted subtree root attaches at the old parent edge,
- a nested descendant inside that inserted subtree becomes the new parent of the old child subtree.

That distinction is why this example deserves its own document.

---

## 3. Current authored topology in `vr-input.rs`

For the wrist-driving path with filtering enabled, the current shape is conceptually:

```text
J_Bip_*_LowerArm
  ControllerXR(hand=Left/Right, pose=Grip)
    Transform                <-- direct driven child required by OpenXRSystem
      TransformPipeline
        TransformForkTRS
          TransformMapTranslation
          TransformMapRotation
            QuatTemporalFilter
          TransformMapScale
          TransformMergeTRS
        TransformPipelineOutput
          J_Bip_*_Hand
            ... existing finger subtree ...
```

With filtering disabled, the path simplifies to:

```text
J_Bip_*_LowerArm
  ControllerXR
    Transform
      J_Bip_*_Hand
        ... existing finger subtree ...
```

This is implemented in `attach_controller_parent_to_named_wrist(...)` in
[examples/vr-input.rs](examples/vr-input.rs).

---

## 4. Why this shape exists

This topology is a workaround for two engine realities:

### 4.1 `ControllerXR` only drives a direct child transform

`OpenXRSystem` does not drive arbitrary descendants of `ControllerXR`.
It looks for a **direct `TransformComponent` child** and writes the pose there.

So the inserted subtree must begin with:

```text
ControllerXR
  Transform
```

Otherwise the armature wrist path is not actually driven by XR at all.

### 4.2 The transform pipeline output is where the filtered basis lives

If we want filtering to affect the wrist subtree, the wrist cannot stay attached to the raw driven
transform.

It needs to attach **below the pipeline output branch**, because that branch is what receives the
processed transform stream during transform propagation.

So the reattachment target is not the root of the inserted subtree; it is the nested
`TransformPipelineOutput` node.

---

## 5. Why this is a stopgap, not the final model

This pattern works, but it has several drawbacks.

### 5.1 The splice target is not the inserted root

Most splice helpers are naturally described as:

```text
splice root between parent and child
```

But here we really need:

```text
splice subtree-root at parent edge,
then attach old child under subtree-output-leaf
```

That is a more specialized operation.

### 5.2 The example is open-coding topology assembly

`vr-input.rs` manually creates:

- `ControllerXR`
- driven `Transform`
- optional transform pipeline nodes
- output leaf
- reattachment of the wrist subtree

That is fine for debugging, but it is too low-level as a long-term authoring story.

### 5.3 The engine still lacks a first-class concept for “external subtree target”

This example is still using topology surgery to make `ControllerXR` affect an imported armature.

Long term we may want a clearer concept such as:

- pose source,
- pipeline/filter chain,
- explicit destination transform/subtree.

The current stopgap keeps everything inside ordinary topology because it was the fastest path to a
real experiment.

---

## 6. What this means for a future splice API

The existing splice concept in
[docs/task/refactor/splice-component-into-topology.md](../task/refactor/splice-component-into-topology.md)
is still useful, but `vr-input.rs` suggests we likely need a richer variant.

Instead of only:

```rust
splice_between(parent, child, inserted_root)
```

we may eventually want something conceptually closer to:

```rust
splice_subtree_between(
    parent,
    child,
    inserted_root,
    inserted_child_target,
)
```

where:

- `inserted_root` is what gets attached under `parent`,
- `inserted_child_target` is some descendant inside the inserted subtree,
- `child` is reattached under `inserted_child_target`.

That is the real operation being performed by the example.

---

## 7. How this could be expressed in MMS

Today, plain MMS component nesting naturally expresses only **tree construction**:

```text
Parent {
    InsertedRoot {
        InsertedLeaf {
            Child
        }
    }
}
```

That works for newly-authored trees, but it does not directly express:

- “find an existing imported armature node”,
- “insert a new subtree above it”,
- “reattach that existing subtree under a nested output leaf”.

So if we want this pattern in MMS, we likely need a dedicated topology helper concept.

### Option A: explicit `splice` block with `output` target

Conceptually:

```text
splice [name='J_Bip_L_Hand'] with ControllerXR.new(true, Left, Grip) {
    Transform {
        TransformPipeline {
            TransformForkTRS {
                TransformMapTranslation {}
                TransformMapRotation {
                    QuatTemporalFilter.with_smoothing_factor(220.0)
                }
                TransformMapScale {}
                TransformMergeTRS {}
            }
            TransformPipelineOutput as wrist_output {}
        }
    }
    attach_existing_subtree_to wrist_output
}
```

This is explicit, but it introduces new MMS syntax/runtime rules.

### Option B: helper constructor that declares the adoption leaf

Conceptually:

```text
ControllerXR.new(true, Left, Grip) {
    Transform {
        TransformPipeline {
            ...
            TransformPipelineOutput.adopt_existing_subtree()
        }
    }
}
```

paired with a world/query API call like:

```text
avatar.splice_find("[name='J_Bip_L_Hand']", <subtree>)
```

This keeps tree authoring local, but still requires a special “adopt existing subtree here”
concept.

### Option C: query + splice primitive

Conceptually:

```text
let wrist = find_component(vtuber, "[name='J_Bip_L_Hand']")

splice_subtree_at(wrist) {
    ControllerXR.new(true, Left, Grip) {
        Transform {
            TransformPipeline {
                ...
                TransformPipelineOutput.adopt_spliced_child()
            }
        }
    }
}
```

This feels closer to the real semantics:

- resolve an existing node,
- insert a new subtree above it,
- nominate which nested leaf should adopt the displaced subtree.

Of the options above, this is probably the cleanest conceptual match.

---

## 8. The main semantic we should preserve in MMS

Whichever syntax we choose, the important semantic should be explicit:

> A splice can insert an entire subtree, and the old child subtree may be reattached to a nominated
> descendant/output leaf inside that inserted subtree.

That is the real capability we need.

If MMS only models simple `parent -> inserted_root -> child`, it will not be expressive enough for
the transform-pipeline-based wrist-driving case.

---

## 9. Recommendation

Short term:

- keep the current `vr-input.rs` stopgap as a useful experimental implementation,
- document it clearly,
- avoid pretending it is already the final authoring model.

Medium term:

- add a richer splice helper in `Universe` / `World` that supports:
  - inserted subtree root,
  - nominated descendant/output adoption target,
  - reattachment of the old child subtree at that target.

Long term:

- expose that richer operation in MMS with an explicit “adopt spliced child here” concept.

---

## 10. Current implementation pointer

The current stopgap lives in:

- [examples/vr-input.rs](examples/vr-input.rs)

Specifically:

- `attach_controller_parent_to_named_wrist(...)`

That helper is the current reference implementation for this pattern.
