# Transform pipeline

This doc describes the current transform-pipeline direction in the engine.

It is broader than XR. The same transform-pipeline machinery should be able to cover:

- XR controller / hand transform stabilization
- gizmo inheritance shaping
- stabilized helper transforms
- dynamic-bone-style follow later
- secondary motion later

The important current design shift is:

- authored transform shaping is primitive-only
- there is no filter sugar/desugaring layer
- the runtime evaluator is `TransformPipelineSystem`

---

## Goals

- Keep transform processing explicit in topology.
- Support per-channel transform operations.
- Support temporal operators without pushing runtime state into serialized authored data.
- Keep the current runtime shape simple enough for the use cases we actually have.

## Non-goals

- This doc does not define a final editor authoring UI.
- This doc does not fully specify future spring / constraint / blending operators.
- This doc does not assume we already need a general multi-input graph runtime.

---

## 1. Current authored model

Authoring is done with explicit pipeline primitives in the component tree.

Current primitive components are:

- `TransformPipelineComponent`
- `TransformForkTRSComponent`
- `TransformMapTranslationComponent`
- `TransformMapRotationComponent`
- `TransformMapScaleComponent`
- `TransformDropComponent`
- `TransformMergeTRSComponent`
- `TransformPipelineOutputComponent`
- `Vector3TemporalFilterComponent`
- `QuatTemporalFilterComponent`

The common authored shape today is:

```text
TransformComponent
  TransformPipeline
    TransformForkTRS
      TransformMapTranslation
      TransformMapRotation
        QuatTemporalFilter
      TransformMapScale
    TransformMergeTRS
    TransformPipelineOutput
      TransformComponent
        ... driven subtree ...
```

Two important current examples:

- `examples/vr-input.rs` uses a pipeline to smooth controller rotation while leaving translation and scale effectively passthrough.
- `src/engine/ecs/system/gizmo_system.rs` uses pipelines to keep or drop inherited translation / rotation / scale for gizmo visual groups.

So the authored model is already doing real work; this is not just speculative vocabulary anymore.

---

## 2. Runtime ownership: `TransformPipelineSystem`

There is no separate `TransformPipelineProcessor` type in the codebase.

That term is redundant for the current architecture.

The runtime evaluator is simply:

- `TransformPipelineSystem`

Today it is responsible for:

- parsing authored pipeline topology rooted at a `TransformPipelineComponent`
- evaluating the parsed pipeline against an inherited world transform
- owning temporal operator state
- returning the processed world transform and any explicit output roots

`TransformSystem` stays responsible for ordinary transform-tree propagation.

Its relationship to the pipeline system is:

1. walk the normal transform subtree
2. when a node is a transform-pipeline boundary, ask `TransformPipelineSystem` to evaluate it
3. continue traversal from the pipeline output roots, or from normal children if there are none

So the clean runtime split is:

- `TransformSystem`: subtree propagation / cached world matrices / side effects
- `TransformPipelineSystem`: transform-processing evaluation

That is enough. We do not need a second “processor” abstraction unless we later introduce a genuinely separate compiled-runtime layer.

---

## 3. What shape pipelines have in memory today

The current in-memory shape is not a general graph.

It is a parsed rooted program-like tree:

```rust
pub struct TransformPipeline {
    pub owner_component: Option<ComponentId>,
    pub input: TransformPipelineInput,
    pub stages: Vec<TransformPipelineStage>,
    pub output: TransformPipelineOutput,
}

pub enum TransformPipelineStage {
    ForkTrs(TransformForkTrsStage),
  Pipeline(Box<TransformPipeline>),
}

pub struct TransformForkTrsStage {
    pub translation_ops: Vec<TransformPipelineVec3Op>,
    pub rotation_ops: Vec<TransformPipelineQuatOp>,
    pub scale_ops: Vec<TransformPipelineVec3Op>,
    pub merge_mode: TransformPipelineMergeMode,
}
```

And channel values are carried in:

```rust
pub struct TransformPipelineChannels {
    pub translation: [f32; 3],
    pub rotation_quat_xyzw: [f32; 4],
    pub scale: [f32; 3],
}
```

The important properties of this runtime shape are:

- one parsed block represents one rooted pipeline subtree
- input is currently only `ParentWorld`
- evaluation is stage-ordered, not edge-scheduled
- the main stage type today is TRS fork + per-channel op lists
- nested pipeline blocks are possible structurally
- output is either:
  - `ImplicitTransform`, or
  - `OutputRoots(Vec<ComponentId>)`

So the current runtime model is much closer to:

- a small compiled pipeline program

than to:

- a graph of arbitrary nodes and edges

---

## 4. What `TransformPipeline` does in memory

`TransformPipeline` is the runtime representation of one authored pipeline root.

Conceptually it means:

- take one input transform stream
- run an ordered list of processing stages on it
- produce one processed transform stream
- optionally redirect traversal to explicit output roots

In other words, a block is not “a transform node” in world topology.

It is an internal executable description for:

- one pipeline boundary
- one input
- one ordered body of work
- one output routing decision

That means the in-memory `TransformPipeline` type is not the authored ECS component of the same name.

It is a parsed runtime pipeline description whose semantics are closer to:

- pipeline program
- compiled subtree
- executable transform-processing block

than to a reusable graph block with free-form incoming and outgoing edges.

### Why blocks exist at all

Blocks let the runtime separate two concerns:

- authored topology shape
- execution shape

The authored topology is a component subtree.

The runtime does not want to evaluate that raw topology directly every time at the level of individual ECS component checks while also applying math.

So it parses the topology into a smaller internal representation that says, in effect:

- here is the input source
- here are the stages
- here are the channel ops
- here is where traversal should continue afterward

That is what the block is for.

### Why the current block shape is a bit awkward

The current parsed `TransformPipeline` shape works, but it is slightly more abstract than what we actually need today.

Current reality:

- we have one input source
- we have ordered stages
- we mostly have one real stage family: TRS fork with per-channel ops
- we are not doing graph scheduling
- we are not doing multi-input composition
- we are not sharing subgraphs across pipelines

So the current `Pipeline(Box<TransformPipeline>)` recursion is more general than our current authored use cases require.

That does not make it wrong, but it does mean:

- the runtime shape is currently “tree of blocks/stages”
- not “graph of nodes/edges”
- and not “flat instruction list” either

---

## 5. How evaluation works today

At runtime, evaluation is:

1. parse the rooted component subtree if the current node is a `TransformPipelineComponent`
2. decompose the inherited world matrix into channels:
   - translation
   - rotation quaternion
   - scale
3. apply stages in order
4. recompose a world matrix
5. return the processed world matrix plus output roots

The main evaluator functions are:

- `parse_component_tree(...)`
- `parse_pipeline_block(...)`
- `parse_fork_trs(...)`
- `evaluate_pipeline_node(...)`
- `evaluate_block(...)`
- `evaluate_stage(...)`
- `evaluate_fork_trs(...)`

Current operator behavior is intentionally simple:

- vec3 ops: `Pass`, `Drop`, `TemporalSmooth`
- quat ops: `Pass`, `Drop`, `TemporalFilter`

Current merge behavior is also simple:

- `ImplicitPassthrough`
- `Explicit`

But today both modes reassemble the same channel set in practice; explicit merge is mostly an authored/runtime marker, not a distinct recomposition algorithm yet.

---

## 6. Temporal state shape

Temporal state currently lives in `TransformPipelineSystem`, not on authored components.

It is keyed by:

- `owner_component`
- parsed `stage_path`

via:

```rust
struct TransformPipelineStageKey {
    owner_component: Option<ComponentId>,
    stage_path: Vec<usize>,
}
```

And the system stores separate state maps for:

- vec3 temporal filters
- quat temporal filters

This is a good match for the current goals:

- authored components stay declarative
- runtime state is not serialized
- state identity follows the parsed operator location inside a pipeline root

The tradeoff is that reparsing identity has to remain stable enough for temporal continuity.

---

## 7. What our current use cases actually need

The current real use cases are narrower than a general graph system.

### A. XR controller rotation smoothing

Needed behavior:

- single input transform
- decompose to TRS
- smooth rotation only
- pass translation through
- pass scale through
- drive an output subtree

This is exactly what the current block + fork + per-channel-op shape is good at.

### B. Gizmo inheritance shaping

Needed behavior:

- single input transform
- keep or drop selected channels
- optionally produce multiple authored visual groups with different inheritance behavior
- continue traversal from explicit output roots

This is also a strong fit for the current shape.

### C. Likely near-future use cases

Still likely to fit the current shape:

- translation smoothing
- no-scale attachment helpers
- world/local visual-space shaping
- simple follow or damping operators

### D. Not required yet

Not actually needed yet:

- multiple input streams blended into one output
- arbitrary fan-in between operators
- arbitrary fan-out between internal operators
- edge-level routing semantics
- shared reusable subgraphs
- cycle handling
- topological scheduling of arbitrary node graphs

That distinction matters. It suggests we should not over-design the runtime around graph problems we do not have yet.

---

## 8. Should this be `TransformPipelineNode` / `TransformPipelineEdge` instead?

Probably not yet.

A node/edge runtime becomes attractive when at least one of these becomes real:

- multiple upstream sources can feed one operator
- one operator’s result can feed multiple downstream operators independently
- subgraphs are shared or instanced internally
- evaluation order is no longer just rooted-tree traversal
- operators need explicit typed ports and edge routing

That is not the current system.

Current reality is:

- rooted authored subtree
- one inherited input transform
- ordered stage evaluation
- per-channel op lists
- optional nested pipeline blocks

So a `Node` / `Edge` runtime would mostly add ceremony right now.

It would likely complicate:

- parsing
- temporal-state identity
- debugging
- authored/runtime correspondence

without clearly helping the current use cases.

### When a graph model would start making sense

If we later add real features like:

- blend between parent world and another transform source
- mix multiple constraints into one output
- branch one channel stream into multiple independent consumers
- explicit retargeting and recombination flows

then a graph runtime may become the better abstraction.

At that point, a shape like this could make sense:

```rust
struct TransformPipelineNode {
    id: TransformPipelineNodeId,
    op: TransformPipelineOp,
    inputs: Vec<TransformPipelinePort>,
    outputs: Vec<TransformPipelinePort>,
}

struct TransformPipelineEdge {
    from_node: TransformPipelineNodeId,
    from_port: usize,
    to_node: TransformPipelineNodeId,
    to_port: usize,
}
```

But that should be introduced when the use cases demand it, not in advance.

---

## 9. What would be a better flexible shape if we want to evolve carefully

The best near-term direction is probably:

- keep the authored topology exactly as it is now
- keep `TransformPipelineSystem` as the sole runtime evaluator
- keep the internal representation rooted and tree-shaped
- make the internal names a little more honest about what they are

In practice there are two reasonable choices.

### Option A: keep the current shapes, just clarify semantics

That means:

- keep the current parsed `TransformPipeline` type
- keep `TransformPipelineStage`
- keep `TransformForkTrsStage`

and document that:

- a block is an executable rooted pipeline description
- not a general graph block

This is the smallest change and is probably good enough today.

### Option B: rename toward “program” / “plan” terminology later

If names start feeling misleading, a future rename could be cleaner:

- parsed `TransformPipeline` -> `TransformPipelineProgram` or `TransformPipelinePlan`
- `TransformPipelineStage` -> `TransformPipelineOpGroup` or `TransformPipelineStep`

That would better reflect current semantics:

- one input
- ordered execution
- rooted subtree
- no explicit edges

This may read more clearly than “block inside block inside stage”.

---

## 10. Recommended direction

The strongest current recommendation is:

1. keep the authored model primitive-only
2. treat `TransformPipelineSystem` as the runtime evaluator; do not introduce a separate `TransformPipelineProcessor` concept
3. treat the current internal shape as a rooted parsed execution plan, not a general graph
4. keep `TransformPipeline` for now unless the parsed/runtime-vs-authored name overlap starts getting in the way
5. only move to `Node` / `Edge` if we gain real multi-input or shared-subgraph use cases

This keeps the architecture aligned with what we actually need today:

- XR smoothing
- gizmo inheritance shaping
- simple future follow/damping operators

without prematurely paying graph-runtime complexity costs.

---

## 11. Summary

Today, transform pipelines are:

- authored as explicit topology primitives
- evaluated by `TransformPipelineSystem`
- represented in memory as rooted parsed blocks/stages
- executed as ordered channel-processing steps over one inherited world transform

The parsed in-memory `TransformPipeline` currently means:

- one compiled pipeline boundary
- one input
- ordered processing stages
- one output routing decision

That is a good fit for the real use cases we have now.

`TransformPipelineNode` / `TransformPipelineEdge` is a plausible future direction only if we start needing true graph semantics such as multi-input blending, shared subgraphs, or explicit port routing.
