# TransformFilter and TransformPipeline (refactor note)

Date: 2026-03-16

Status: completed direction change.

This note is now primarily historical context.

The current engine direction is:

- `TransformFilterComponent` is removed
- `TransformFilterSystem` is removed
- authored transform shaping uses explicit transform-pipeline primitives directly
- gizmo visuals now use explicit `TransformPipeline` / `TransformForkTRS` / map / drop / merge / output topology

For the current canonical model, see [docs/spec/transform-pipeline.md](../spec/transform-pipeline.md).

The remainder of this note captures the reasoning that led to eliminating the filter sugar layer in favor of primitive-only authored topology.

## Executive summary

Today:

- `TransformFilterSystem` no longer contains unique transform-processing logic.
- Its only public behavior is forwarding to `TransformPipelineSystem::evaluate_filter_inherited_world(...)`.
- `TransformSystem` still knows about `TransformFilterComponent` explicitly and applies it before asking `TransformPipelineSystem` to evaluate pipeline nodes.
- `TransformPipelineSystem` also knows about `TransformFilterComponent` explicitly, because it can synthesize a `TransformPipelineBlock` from a filter node.

So the current situation is:

- **runtime math redundancy:** essentially 100% removed already
- **system-level semantic redundancy:** still present
- **authoring redundancy:** intentionally still present, because `TransformFilterComponent` is currently ergonomic sugar and is used by gizmos

The likely end-state is:

- keep `TransformFilterComponent` as authored sugar if we still want the ergonomic shorthand
- desugar it into transform-pipeline primitives at the boundary of `TransformPipelineSystem`
- stop making other systems care about `TransformFilterComponent` directly
- eventually remove `TransformFilterSystem` entirely

## Current code state

### 1. `TransformFilterSystem` is already a compatibility wrapper

Current implementation:

- `src/engine/ecs/system/transform_filter_system.rs`

It now does only this:

- accept `parent_world` + `TransformFilterComponent`
- call `TransformPipelineSystem::evaluate_filter_inherited_world(...)`
- return the result

That means the old filter-specific TRS math has already moved into the transform-pipeline runtime layer.

### 2. `TransformSystem` still special-cases filter nodes

Current implementation:

- `src/engine/ecs/system/transform_system.rs`

During subtree propagation it currently does two separate transform-processing steps:

1. if `node` has `TransformFilterComponent`, call `TransformFilterSystem::filter_inherited_world(...)`
2. then call `transform_pipeline_system.evaluate_pipeline_node(world, node, current_world)`

That means filter nodes are still treated as a distinct pre-pipeline concept in traversal.

### 3. `TransformPipelineSystem` also special-cases filter nodes

Current implementation:

- `src/engine/ecs/system/transform_pipeline_system.rs`

`parse_component_tree(...)` currently recognizes two entry shapes:

- `TransformFilterComponent`
- `TransformPipelineComponent`

For a filter node it synthesizes a `TransformPipelineBlock` via:

- `pipeline_from_transform_filter(...)`

So the transform pipeline runtime already has enough knowledge to treat filters as sugar.

### 4. Current authored use of `TransformFilterComponent`

The main active authored usage is in gizmo visuals:

- `src/engine/ecs/system/gizmo_system.rs`

Examples:

- “inherit translation + rotation, drop scale” for the top-level gizmo filter
- world/local gizmo-space shaping using combinations of translation / rotation / scale inheritance flags

This is a good fit for keeping `TransformFilterComponent` as sugar even if the runtime stops treating it as a separate system concept.

## What is redundant now?

### Fully redundant already

These parts are already redundant in practice:

- the TRS filter math that used to belong uniquely to `TransformFilterSystem`
- any need for a separate filter-only recomposition path
- any need for a separate filter-only temporal/spatial evaluation model

Put differently:

- **all meaningful transform-processing math now lives, or can live, in `TransformPipelineSystem`**

### Still redundantly represented

These parts are still duplicated conceptually:

- `TransformSystem` knows about filters explicitly
- `TransformPipelineSystem` knows about filters explicitly
- `TransformFilterSystem` still exists as a named compatibility layer

That is the actual remaining overlap.

## Can systems stop checking for `TransformFilterComponent` directly?

Yes — that is probably the right direction.

The desired semantic model is:

- systems should ask: “is this node a transform-processing node?”
- not: “is this node specifically a filter?”

In other words, transform propagation should ideally do something more like:

1. ask `TransformPipelineSystem` whether the current node represents a transform-processing boundary
2. if yes, let it parse/desugar/evaluate
3. use the returned processed transform basis and output roots
4. continue traversal without any filter-specific branch

That would let `TransformFilterComponent` exist purely as:

- authored shorthand
- desugared input shape
- not a first-class traversal concern

## What should the desugaring boundary be?

There are three plausible options.

### Option A: parse-time desugaring inside `TransformPipelineSystem`

This is the simplest near-term direction.

Meaning:

- keep authored `TransformFilterComponent`
- when `TransformPipelineSystem` sees one, synthesize a `TransformPipelineBlock`
- return only primitive/operator semantics to the rest of evaluation

Pros:

- minimal architecture churn
- no mutation of the stored component tree required
- keeps authoring ergonomic
- already close to the current implementation

Cons:

- `TransformPipelineSystem` still needs to know about `TransformFilterComponent`
- repeated parsing/desugaring may become wasteful if done every propagation walk

This is still a good default first step.

### Option B: cached desugaring in `TransformPipelineSystem`

Meaning:

- keep authored `TransformFilterComponent`
- parse/desugar it once into an internal cached pipeline representation
- invalidate cache only when relevant topology/components change

Pros:

- preserves authored sugar
- avoids repeated parsing
- keeps other systems completely primitive-oriented

Cons:

- requires cache invalidation rules
- slightly more runtime bookkeeping

This is probably the most attractive medium-term direction.

### Option C: rewrite authoring into explicit pipeline nodes up front

Meaning:

- `TransformFilterComponent` is immediately expanded into explicit `TransformPipeline` / `TransformForkTRS` / map / drop primitives in the world topology
- systems never see filter nodes again

Pros:

- one authored/runtime representation
- simplest possible evaluator semantics

Cons:

- mutates the authoring tree
- makes editor/debug tooling more complex
- risks losing the ergonomic and inspectable authored shorthand users actually want

This seems too invasive for now.

## Recommended end-state

The strongest target seems to be:

- keep `TransformFilterComponent` as an authored sugar node
- treat `TransformPipelineSystem` as the only transform-processing evaluator
- perform filter desugaring inside `TransformPipelineSystem`
- make traversal and propagation code operate only on primitive pipeline semantics
- delete `TransformFilterSystem` once callsites are migrated

In that model:

- `TransformFilterComponent` remains an authoring convenience
- `TransformPipelineSystem` owns parsing, desugaring, caching, state, and evaluation
- `TransformSystem` just delegates transform-processing questions to `TransformPipelineSystem`

## Concrete code implications

### `TransformFilterSystem`

Likely outcome:

- remove it entirely

Reason:

- it is already only a forwarding wrapper
- it no longer represents a distinct runtime subsystem

### `TransformSystem`

Current shape:

- explicit `TransformFilterComponent` branch
- then explicit pipeline evaluation call

Target shape:

- no explicit filter branch
- one call into `TransformPipelineSystem`
- if that call says “this node processes inherited transform state”, use the returned processed basis/output roots
- otherwise continue with ordinary subtree traversal

This is the most important cleanup.

### `TransformPipelineSystem`

Current shape:

- parses real pipeline nodes
- can also synthesize a block from `TransformFilterComponent`

Target shape:

- keep that responsibility
- potentially add caching so parse/desugar does not happen on every traversal
- expose one coherent entrypoint for transform-processing nodes

This system should become the sole owner of:

- TRS decomposition / recomposition
- pass / drop channel behavior
- vec3 / quat temporal filters
- merge semantics
- nested pipeline block semantics
- filter desugaring

## How much of `TransformFilterSystem` is redundant?

Short answer:

- **probably all of it**

Longer answer:

- 100% of its transform-processing logic is redundant already
- 100% of its runtime math can live in `TransformPipelineSystem`
- the only thing it still provides is a legacy name and one forwarding function

So if the goal is architectural clarity, `TransformFilterSystem` should disappear.

## What remains special about `TransformFilterComponent`?

Only two things need to remain special, and both are authoring-facing rather than runtime-facing:

1. it is a convenient shorthand users/systems can spawn directly
2. `TransformPipelineSystem` needs to know how to desugar it

Everything else can be primitive/pipeline-based.

## Suggested migration steps

### Step 1

Remove the explicit filter branch from `TransformSystem`.

Instead of:

- checking for `TransformFilterComponent`
- then separately evaluating pipeline nodes

just:

- call into `TransformPipelineSystem`
- let it decide whether the node is:
  - filter sugar
  - real pipeline node
  - or not a transform-processing node at all

### Step 2

Delete `TransformFilterSystem`.

All callsites should already be expressible via `TransformPipelineSystem`.

### Step 3

Add cache invalidation if needed.

If transform-processing parsing becomes expensive, cache desugared `TransformPipelineBlock`s keyed by the authored root node.

### Step 4

Optionally migrate gizmo spawning to explicit pipeline primitives.

This is not required.

It may still be better ergonomically for gizmo code to spawn:

- `TransformFilterComponent::inherit_tr()`

rather than a whole explicit operator subtree.

The important point is that this should be an authoring choice, not a runtime-system split.

## Recommendation

Yes:

- avoid checking for `TransformFilterComponent` in systems other than for desugaring
- ideally even that desugaring lives only inside `TransformPipelineSystem`
- after desugaring, handle only the primitive/operator model:
  - TRS fork
  - per-channel pass/drop
  - vec3/quat operators
  - merge/output semantics

That gives the cleanest architecture:

- one authored shorthand layer
- one runtime evaluator
- one primitive transform-processing vocabulary
