# Transform pipeline cleanup checklist

Date: 2026-05-17

Status note: the cleanup described here has now landed for authored topology. `TransformPipeline` and `TransformPipelineOutput` were removed; current authored transform shaping uses `TransformForkTRS` as the pipeline root with downstream content attached directly under the fork.

This task comes before the ref-based transform-parent routing work.

The immediate goal is to simplify and clarify the authored transform-pipeline surface before adding new routing operators such as `TransformParent` or a ref-based transform input.

Conceptually, a transform pipeline should mean:

- one or more transform operators wedged into a transform hierarchy
- sitting between an upstream transform basis and a downstream subtree
- with each operator consuming the transform output of its parent and passing its own output to its children

In particular:

- reduce mandatory wrapper boilerplate where possible
- make each operator's input/output contract explicit
- decide which operators have unambiguous parent/child topology semantics already
- identify which current nodes are overloaded, redundant, or unclear

Related tasks:

- [transform-parent-component-ref-routing.md](transform-parent-component-ref-routing.md)
- [serialize-component-and-armature-viz-save-plan.md](serialize-component-and-armature-viz-save-plan.md)

---

## 1. Current problem

The current transform-pipeline runtime works, but the authored shape is clunky.

The conceptual model we want is simpler than the current wrapper-heavy authored surface:

- upstream transform hierarchy
- one or more transform operators
- downstream transform hierarchy

In other words, “pipeline” should describe the inserted operator chain itself, not a mandatory wrapper component around it.

Common authored pattern today:

```text
TransformComponent
  TransformPipeline
    TransformForkTRS
      TransformMapTranslation
      TransformMapRotation
        QuatTemporalFilter
      TransformMapScale
    TransformComponent
      ... driven subtree ...
```

Even simple cases typically need:

- an outer `TransformPipeline`
- a `TransformForkTRS`
- one or more map nodes
- plus extra routing ceremony in cases where the output really just wants to keep flowing into children

That is more ceremony than the common use cases seem to justify.

Before adding more capability, we should make the operator set and topology rules easier to reason about.

---

## 2. Cleanup goals

### Goal A — remove unnecessary mandatory wrappers

The main candidate is the outer `TransformPipeline` wrapper.

If the runtime can infer the same meaning from a more direct root operator, then authors should not have to add an otherwise-empty boundary component just to say “a transform pipeline starts here.”

The intended end state is that the hierarchy itself makes the boundary obvious:

- the parent side provides the incoming transform basis
- the wedged operator chain transforms it
- the children receive the resulting transform basis

### Goal B — make dataflow contracts explicit

For each operator we should be able to answer:

- what is its input?
- where does that input come from?
- what is its output?
- where does that output go?
- is input/output represented directly by the parent/child topology, or does the operator depend on extra hidden context?

### Goal C — distinguish routing operators from channel operators

Some current nodes are about transform routing or pipeline boundaries.

Others are per-channel transform operations.

Those should be clearly separated in both docs and authoring shape.

But the default case should stay simple:

- ordinary parent/child structure should already define operator composition
- “pipeline” should not imply a second parallel routing language for common cases

### Goal D — identify nodes that should probably disappear

If a node is only present to satisfy the current parser shape, rather than representing a meaningful authored concept, it should be a strong candidate for removal or desugaring.

---

## 3. Operator inventory and checklist

This section lists each current pipeline-related operator/component and records whether its input/output semantics are already clean.

Legend:

- `[x]` = current operator must still be representable after cleanup
- `Topology-clean` = input/output can be understood directly from parent/child structure
- `Ambiguous` = input/output or authored purpose currently depends on hidden context, redundant wrapper shape, or parser-specific behavior

### 3.1 Pipeline boundary and routing operators

- [x] `TransformPipeline`
  - Current role: declares a pipeline boundary; parser root; holds stages and optional output roots.
  - Input today: implicit parent-world basis from the transform ancestor above the pipeline.
  - Output today: processed world transform, optionally redirected through `TransformPipelineOutput` children.
  - Topology-clean: partially.
  - Ambiguous points:
    - mostly exists as a mandatory wrapper/root marker
    - input is implicit rather than authored on the node itself
    - nested `TransformPipeline` blocks are supported structurally, but current use cases do not clearly justify that complexity
  - Cleanup question:
    - can another operator such as `TransformForkTRS` serve as the pipeline root directly?

- [ ] `TransformPipelineOutput`
  - Current role: marks explicit traversal continuation roots for a pipeline.
  - Input today: the fully processed transform output of the parent pipeline block.
  - Output today: child transforms/subtrees traversed using that processed world basis.
  - Topology-clean: superficially yes, but unnecessary.
  - Ambiguous points:
    - not a math operator, only a routing marker
    - duplicates a composition rule that can be expressed more directly by saying the processed output flows to the operator's children
    - makes ordinary composition look more ceremonial than it is
  - Cleanup question:
    - remove this node and make child traversal the default output rule

### 3.2 TRS stage operator

- [x] `TransformForkTRS`
  - Current role: decompose one transform stream into translation/rotation/scale channels, run per-channel ops, then implicitly recompose.
  - Input today: one transform stream from the parent pipeline block.
  - Output today: one recomposed transform stream that should naturally continue into the fork node's children.
  - Topology-clean: mostly yes.
  - Ambiguous points:
    - recomposition is implicit; `TransformMergeTRS` does not actually carry meaningful authored structure today
    - it is not currently allowed to stand on its own as the natural pipeline root
  - Cleanup question:
    - should `TransformForkTRS` be allowed to act as the root operator without an outer `TransformPipeline`?

### 3.3 Channel map operators

- [x] `TransformMapTranslation`
  - Current role: owns the ordered operator list for the translation channel.
  - Input today: translation channel from the parent `TransformForkTRS`.
  - Output today: transformed translation channel back to the fork stage.
  - Topology-clean: yes.
  - Ambiguous points:
    - none significant beyond whether absent map nodes should always imply pass-through

- [x] `TransformMapRotation`
  - Current role: owns the ordered operator list for the rotation channel.
  - Input today: rotation channel from the parent `TransformForkTRS`.
  - Output today: transformed rotation channel back to the fork stage.
  - Topology-clean: yes.
  - Ambiguous points:
    - none significant beyond ordered-op authoring ergonomics

- [x] `TransformMapScale`
  - Current role: owns the ordered operator list for the scale channel.
  - Input today: scale channel from the parent `TransformForkTRS`.
  - Output today: transformed scale channel back to the fork stage.
  - Topology-clean: yes.
  - Ambiguous points:
    - none significant beyond whether it is often noise when omitted/pass-through

### 3.4 Channel operators

- [x] `TransformDrop`
  - Current role: replace the current channel with the dropped default.
  - Input today: current channel value from the parent map node.
  - Output today: zero translation / identity rotation / unit scale depending on channel type.
  - Topology-clean: yes.
  - Ambiguous points:
    - output depends on channel kind, so the same operator means different defaults under translation, rotation, and scale maps
  - Cleanup question:
    - is the channel-dependent meaning acceptable, or should defaults be more explicit?

- [x] `TransformSampleAncestor`
  - Current role: replace translation or rotation with sampled world values from an ancestor transform.
  - Input today: implicit pipeline-owner ancestry plus current channel context.
  - Output today: replacement translation or rotation channel value.
  - Topology-clean: no.
  - Ambiguous points:
    - depends on the pipeline owner's ancestor chain, not just parent/child dataflow under the operator
    - same node has different meaning under translation and rotation maps
    - `skip(n)` is compact but brittle and topology-relative
  - Cleanup question:
    - keep as a low-level relative-ancestry operator, but treat it as explicitly context-dependent in docs

- [x] `Vector3TemporalFilter`
  - Current role: temporal smoothing for vec3 channels.
  - Input today: current channel value from the parent translation or scale map.
  - Output today: filtered vec3 channel value.
  - Topology-clean: yes.
  - Ambiguous points:
    - state is runtime-owned rather than authored, which is correct, but should be stated clearly

- [x] `QuatTemporalFilter`
  - Current role: temporal smoothing for rotation.
  - Input today: current rotation channel value.
  - Output today: filtered rotation channel value.
  - Topology-clean: yes.
  - Ambiguous points:
    - stateful operator semantics should remain explicit in docs and tests

- [x] `QuatExtractYaw`
  - Current role: project rotation onto pure Y yaw.
  - Input today: current rotation channel value.
  - Output today: yaw-only rotation channel value.
  - Topology-clean: yes.
  - Ambiguous points:
    - none substantial

- [x] `QuatYawFollow`
  - Current role: stateful yaw-follow operator for rotation.
  - Input today: current rotation channel value plus runtime dt/state.
  - Output today: filtered yaw-follow rotation channel value.
  - Topology-clean: mostly yes.
  - Ambiguous points:
    - relies on runtime state keyed by stage path
    - more specialized than the other operators, so it should remain clearly framed as a rotation op rather than a routing primitive

### 3.5 Merge operator

- [ ] `TransformMergeTRS`
  - Current role: nominally the recomposition point for TRS.
  - Input today: not actually modeled as a meaningful child-driven input in the parser.
  - Output today: effectively none as a distinct authored step; recomposition is already implicit in `TransformForkTRS` evaluation.
  - Topology-clean: no.
  - Ambiguous points:
    - parser currently ignores child content under it
    - recomposition already happens without it carrying real authored structure
    - likely a vestigial or placeholder concept rather than a necessary operator
  - Cleanup question:
    - should this disappear entirely from the authored surface?

### 3.6 Nested pipeline block support

- [ ] nested `TransformPipeline` inside `TransformPipeline`
  - Current role: allows recursive pipeline blocks.
  - Input today: processed transform stream from the parent pipeline stage chain.
  - Output today: processed transform stream back to the parent stage chain.
  - Topology-clean: partially.
  - Ambiguous points:
    - more general than current authored needs appear to require
    - contributes to parser/runtime complexity
    - may be unnecessary until a concrete use case exists
  - Cleanup question:
    - should nested pipeline blocks remain a supported authored concept in the first cleaned-up API?

---

## 4. Proposed cleanup decisions to make

### Decision 1 — can `TransformForkTRS` be the natural root?

This is the main cleanup target.

If yes, then a common pipeline could become:

```text
TransformComponent
  TransformForkTRS
    TransformMapRotation
      QuatTemporalFilter
```

instead of requiring an outer wrapper with no extra authored meaning.

### Decision 2 — `TransformPipelineOutput` should disappear

Current direction:

- processed transform output should flow directly to the operator's children
- ordinary operator composition should therefore be expressed by parent/child structure alone
- a separate output marker node is unnecessary ceremony

That means the default authored meaning should be:

- node consumes its parent's transform output
- node produces a transform output
- that output becomes the input basis for its children

This is the core conceptual rule for the cleaned-up API.

Once that rule is in place, “a transform pipeline” just means one or more such operators wedged between two parts of the transform hierarchy.

### Decision 3 — does `TransformMergeTRS` disappear?

Current evidence points to yes.

Recomposition is already implicit in the fork stage evaluator, and `TransformMergeTRS` does not currently express meaningful authored structure.

### Decision 4 — do nested pipeline blocks stay in v1 cleanup?

If there is no strong current use case, it may be better to simplify the authored API first and defer recursive block support until needed.

### Decision 5 — which operators are explicitly context-dependent?

`TransformSampleAncestor` is the clearest case.

The cleanup doc and future API should state clearly that some operators do not derive all semantics purely from parent/child topology; they also depend on the pipeline owner's position in the wider transform tree.

---

## 5. Minimal supported operator set after cleanup

The cleaned-up API should still support at least:

- one transform-routing/root concept
- one TRS fork stage
- translation map
- rotation map
- scale map
- channel drop
- ancestor sampling
- vec3 temporal smoothing
- quat temporal smoothing
- yaw extraction
- yaw-follow
- ordinary composition by child traversal

Strong candidate removals or deferrals:

- `TransformMergeTRS`
- `TransformPipelineOutput`
- mandatory outer `TransformPipeline` wrapper
- nested pipeline blocks unless a real authored use case requires them now

---

## 6. Acceptance criteria

This cleanup task is complete when:

- the transform-pipeline authored surface has a documented operator inventory
- each operator has an explicit input/output contract
- it is clear which operators are topology-clean and which depend on extra transform-tree context
- there is a concrete decision on whether the outer `TransformPipeline` wrapper remains mandatory
- there is a concrete decision on whether `TransformPipelineOutput` remains in the authored API
- there is a concrete decision on whether `TransformMergeTRS` remains in the authored API
- the resulting surface is simpler enough that the later `TransformParent` or ref-based routing task does not have to build on unnecessary ceremony

---

## 7. Open questions

1. Should `TransformForkTRS` become the default pipeline root and replace the mandatory outer `TransformPipeline` wrapper?
2. After removing `TransformPipelineOutput`, do we need any separate routing node at all, or is parent/child composition sufficient for the current operator set?
3. Should `TransformMergeTRS` be removed entirely from the authored API?
4. Should nested `TransformPipeline` blocks remain supported, or be deferred until a real use case exists?
5. Should channel-dependent operators like `TransformDrop` stay polymorphic by context, or become more explicit?
6. Which current examples or systems should be rewritten first as proof that the cleaned-up surface is actually better?