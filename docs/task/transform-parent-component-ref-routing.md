# Transform parent routing via `ComponentRef`

Date: 2026-05-17

Status note: the authored `TransformPipeline` / `TransformPipelineOutput` wrapper/output topology has now been removed. Any references below to those names are historical; current authored transform shaping uses `TransformForkTRS` with downstream content attached directly under the fork root.

This task captures the transform-routing capability needed before armature visualization can live comfortably in a separate helper tree.

It should be approached after the transform-pipeline cleanup pass, not before.

The immediate goal is not a broad transform-pipeline API redesign.

The goal is narrower:

- allow a transform subtree to treat some other transform as its parent/input basis
- make that target durable and serializable via the existing `ComponentRef` model
- support helper trees that need to follow authored transforms outside their own local topology

This should be treated as a prerequisite task for the broader armature-viz split, while keeping transform-pipeline authoring cleanup as a separate later task.

Related task:

- [transform-pipeline-cleanup-checklist.md](transform-pipeline-cleanup-checklist.md)
- [serialize-component-and-armature-viz-save-plan.md](serialize-component-and-armature-viz-save-plan.md)

---

## 1. Problem statement

The current transform-pipeline system can only take input from the local parent-world basis and can only sample alternate transforms by walking upward through ancestors with `TransformSampleAncestor.skip(n)`.

That is enough for splice-style topologies where the interesting source transform is somewhere above the pipeline owner.

It is not enough for helper trees whose transform source lives elsewhere in the world.

The target use case is roughly:

- a glTF armature lives inside one authored tree
- a runtime/editor helper visualization tree lives elsewhere
- helper nodes need to follow transforms from named or otherwise durable targets inside the authored tree

So the missing capability is:

- “treat the transform referenced by this selector or durable component reference as the parent/input basis for this subtree”

---

## 2. Desired authored shape

The desired surface should feel closer to a direct transform-parent override than to a long pipeline ceremony.

Sketch:

```meow_meow
// in the 3d humanoid skinned mesh
let root_to_query_from =
GLTF {
    T { } // bunch of stuff
    T {   // the armature i think
      ...
      T {
         name = "some_component" // part of the actual armature
      }
    }
};

// in the editor's bone viz tree:
T { // the original parent
  TransformParent {
    target = "#some_component"
    root = root_to_query_from
    T {
      // inner transform that behaves differently..
    }
  }
}
```

This is not a final syntax commitment.

It does capture the intended semantic shape:

- a subtree says which transform it wants to inherit from
- that transform can live outside the subtree
- selector resolution may optionally be rooted/scoped if the target is expressed as a query rather than a guid

### Why this shape is attractive

It makes the authored intent obvious:

- “this subtree follows that transform”

rather than encoding that intent indirectly through:

- local topology tricks
- ancestor-skip counting
- a mandatory outer `TransformPipeline` wrapper for cases that are conceptually just parent redirection

---

## 3. What exists today

Current transform-routing capabilities:

- `TransformPipelineInput` only supports `ParentWorld`
- `TransformPipelineOutput` can redirect traversal to explicit output roots
- `TransformSampleAncestor.skip(n)` can replace translation or rotation from an ancestor transform

Current durable reference capabilities:

- `ComponentRef::Guid(uuid)`
- `ComponentRef::Query(selector)`

Current `ComponentRef` users:

- `ActionComponent`
- `IKChainComponent`

Current runtime resolution model for those refs:

- guid lookup through `World::component_id_by_guid`
- selector lookup through `World::find_component`
- deferred resolution by the owning system when needed

So the building blocks already exist, but the transform pipeline does not currently consume them.

---

## 4. Core design question

There are at least two plausible ways to add this capability.

### Option A — explicit `TransformParent` / parent-basis override component

Semantic idea:

- this subtree's transform basis comes from a referenced external transform instead of the immediate local parent-world basis

Possible authored shape:

```text
T
  TransformParent(target=..., root=...)
  T
    ... driven subtree ...
```

Pros:

- matches the way the problem is described in authoring terms
- avoids forcing users to think in terms of channel-level ops for a simple parent-redirection case
- may let many helper-tree use cases avoid a full transform-pipeline wrapper entirely

Cons:

- introduces a transform-routing concept that is not currently expressed in the pipeline runtime
- needs careful definition for whether it replaces full parent TRS, only some channels, or composes with local child transforms afterward

### Option B — ref-based transform-pipeline input or sampling operator

Semantic idea:

- keep the transform pipeline as the routing/evaluation framework, but let a stage or input node pull transform data from a referenced external transform

Possible authored shapes:

```text
TransformPipeline
  TransformInput(target=..., root=...)
  TransformForkTRS
  TransformPipelineOutput
```

or:

```text
TransformMapTranslation
  TransformSampleTarget(target=..., root=...)
TransformMapRotation
  TransformSampleTarget(target=..., root=...)
```

Pros:

- keeps all transform-routing logic inside one subsystem
- reuses the existing pipeline execution model more directly
- naturally composes with smoothing, yaw extraction, and similar channel ops

Cons:

- keeps the current ceremony for cases that conceptually just want a different parent
- may feel clunky for the common “follow this other transform” case

### Current preference

For the authoring problem described here, an explicit `TransformParent`-style abstraction looks closer to the intended semantics.

That said, the implementation may still lower into transform-pipeline runtime machinery internally.

The important distinction is:

- user-facing authoring shape does not have to equal the runtime evaluator shape

---

## 5. Query scoping and `root`

If `target` is a guid-backed `ComponentRef`, then an extra query root is probably unnecessary.

If `target` is a selector-backed `ComponentRef::Query`, then scoping matters.

The authored sketch above suggests:

- `target = "#some_component"`
- `root = root_to_query_from`

That points at an important design question:

- should selector resolution default to a global/world search, or should it be explicitly scoped to a root reference?

Reasonable first-pass rule:

- guid refs ignore `root`
- query refs may use `root` as an explicit search scope
- if no `root` is provided, fall back to the current global/rooted query behavior

This keeps the durable guid path simple while still giving authored scenes a readable scoped-query option.

---

## 6. Local-transform semantics that must be decided

Before implementation, the task needs a crisp answer for how local transform composition works.

Questions:

1. Does `TransformParent` replace the whole parent-world basis for the child subtree?
2. After taking the referenced transform as the basis, is the child `TransformComponent.model` still applied normally on top?
3. Does the first version need full TRS inheritance only, or channel-selective inheritance too?
4. If channel-selective behavior is needed, should that be expressed through transform-pipeline ops layered under the parent override rather than being part of `TransformParent` itself?

The likely first-pass answer is:

- `TransformParent` supplies the parent-world basis
- child local transform still composes on top normally
- channel shaping remains a transform-pipeline concern layered afterward if needed

That keeps the first version small and keeps `TransformParent` conceptually about routing, not filtering.

---

## 7. Relation to the current transform-pipeline boilerplate problem

This task should stay separate from the broader question of simplifying transform-pipeline authoring.

That broader cleanup might later include:

- allowing `TransformForkTRS` without a mandatory outer `TransformPipeline`
- collapsing common one-channel filter patterns into shorter authored forms
- making pipeline input selection more first-class

Those are good directions, but they should not block the immediate routing feature needed for the armature-viz split.

This task only needs to establish:

- an authored way to follow an external transform
- durable reference semantics for that target
- a runtime resolution/evaluation story compatible with the existing ECS and serializer

---

## 8. Proposed implementation order

### Stage 1 — pick the authoring surface

- decide whether the first-class authored concept is `TransformParent`, a ref-based pipeline input, or a thin authoring wrapper that lowers to pipeline internals

### Stage 2 — define the reference payload

- decide how `target` is stored
- decide whether it is literally `ComponentRef`
- decide whether `root` is another optional `ComponentRef`

### Stage 3 — define runtime resolution behavior

- when refs resolve
- where cached resolved ids live
- what happens when the target is not yet present
- whether unresolved refs retry every tick or on specific lifecycle events

### Stage 4 — define transform composition semantics

- referenced parent basis
- local child transform composition
- interaction with optional downstream pipeline filtering

### Stage 5 — use it for separate armature visualization trees

- prove the feature on the bone/armature visualization helper-tree use case before widening scope

---

## 9. Acceptance criteria

This task is complete when:

- a subtree can follow a transform outside its own local topology
- the target reference is durable and serializable
- query-based references can be scoped when needed
- the runtime behavior is defined clearly enough that helper trees can follow authored armature transforms without topology hacks
- the feature can support the later armature-viz split task cleanly

This task is not required to solve:

- general transform-pipeline authoring ergonomics
- the full `Serialize` save-filtering plan
- every future transform-routing or retargeting use case

---

## 10. Open questions

1. Should the first user-facing abstraction be `TransformParent`, or should that just be sugar over a more general pipeline-input operator?
2. Is `root` necessary only for query refs, and should it be ignored for guid refs?
3. Should selector resolution default to whole-world search when `root` is absent?
4. Where should resolved target ids be cached: on the authored component, in the transform system, or in a dedicated transform-routing runtime cache?
5. Does the first version need channel-selective parent inheritance, or is full parent-basis replacement enough?