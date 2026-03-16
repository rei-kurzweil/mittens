# Transform pipeline

This doc defines a speculative but reusable model for transform processing in the engine.

It is intentionally broader than XR. The same transform pipeline should be able to power:

- XR hand / controller transform stabilization
- gizmo inheritance shaping
- dynamic bones
- hair
- cat ears
- body fat / soft follow motion
- clothing follow / lag
- jiggle-style secondary motion
- stabilized helper transforms

This doc is about the **general transform-processing vocabulary**.

Related docs:

- [docs/spec/vr-input-2.md](docs/spec/vr-input-2.md) for how XR input currently works and how it could feed into this pipeline model.
- [docs/analysis/gizmo-transform-propagation.md](docs/analysis/gizmo-transform-propagation.md) for earlier motivation around gizmo transform shaping.

---

## Goals

- Define a declarative way to process transforms in topology.
- Allow processing only part of a transform.
- Support both spatial filtering and temporal filtering.
- Keep the model compatible with the engine’s topology-first component tree.
- Keep the authored model primitive and explicit around TRS/channel operators.

## Non-goals

- This doc does not fully specify implementation details of numeric integration or spring tuning.
- This doc does not define final serialization syntax.

---

## 1. Core idea

The current engine mostly treats transforms as values that are authored locally and propagated through the hierarchy.

The transform pipeline idea adds a second layer:

- transforms can be treated as **streams** flowing through topology
- intermediate nodes can **process** those streams before they reach the final subtree

So instead of only having:

```text
ParentTransform -> ChildTransform -> Renderable
```

we could have:

```text
Transform source -> Transform pipeline -> Processed transform -> Renderable subtree
```

This lets the engine express:

- channel filtering
- smoothing
- spring follow
- parent-space remapping
- local/world overrides
- TRS splitting and recomposition

---

## 2. `TransformPipeline` vs `TransformPipelineProcessor`

A useful separation is:

- `TransformPipeline`: the **authored topology-side** representation
- `TransformPipelineProcessor`: the **runtime/system-side** evaluator

### `TransformPipeline`

This is the component-tree analog.

It would be represented by nodes/components in the ECS topology, such as:

- `TransformForkTRS`
- `TransformMapRotation`
- `QuatTemporalSmooth`
- `TransformMergeTRS`
- `TransformPipelineOutput`

This keeps the authored structure declarative and inspectable.

### `TransformPipelineProcessor`

This is more of a system/runtime concern.

Its job would be to:

- walk the relevant transform-pipeline topology
- evaluate operators in order
- maintain state for temporal operators
- produce the processed transform stream that downstream nodes use

That means the processor is not really “just another component”; it is a system that interprets authored pipeline nodes.

### Internal parsed shape in `TransformPipelineSystem`

Even if authored topology stays component-tree-shaped, the runtime system should probably parse that topology into a simpler internal node graph before evaluation.

The MVP internal shape can stay small:

```rust
struct TransformPipelineBlock {
  owner_component: Option<ComponentId>,
  input: TransformPipelineInput,
  stages: Vec<TransformPipelineStage>,
  output: TransformPipelineOutput,
}

enum TransformPipelineInput {
  ParentWorld,
}

enum TransformPipelineStage {
  ForkTrs(TransformForkTrsStage),
  Block(Box<TransformPipelineBlock>),
}

struct TransformForkTrsStage {
  translation_ops: Vec<TransformPipelineVec3Op>,
  rotation_ops: Vec<TransformPipelineQuatOp>,
  scale_ops: Vec<TransformPipelineVec3Op>,
  merge_mode: TransformPipelineMergeMode,
}

enum TransformPipelineVec3Op {
  Pass,
  Drop,
  TemporalSmooth { smoothing_factor: f32 },
}

enum TransformPipelineQuatOp {
  Pass,
  Drop,
  TemporalFilter { smoothing_factor: f32 },
}
```

That shape matches the goals you described:

- one pipeline block has an input, an output, and one or more processing stages
- a stage can contain vector/quaternion operators
- a stage can also contain another pipeline block
- the runtime can preserve a compact execution form even if authored topology later grows more expressive

The current MVP parser can stay narrow:

- parse explicit `TransformPipelineComponent` / fork / map / merge / output topology
- synthesize controller-rotation-smoothing blocks for tests or helper construction when useful
- later expand to parse additional dedicated authored pipeline components from topology

---

## 3. Primitive-only authored model

The current direction is to model transform processing directly in terms of primitive authored operators.

That means authored topology is expected to use nodes like:

- `TransformPipeline`
- `TransformForkTRS`
- `TransformMapTranslation`
- `TransformMapRotation`
- `TransformMapScale`
- `TransformDrop`
- `TransformMergeTRS`
- `TransformPipelineOutput`

The runtime evaluator should not need a separate sugar/desugaring path for transform filtering.

Instead, even simple “inherit some channels, drop others” behavior should be authored as an explicit primitive operator subtree.

---

## 4. Transform stream / operator mental model

The important mental model is:

- a transform source produces a transform stream
- operators fork, filter, smooth, or remap that stream
- the processed result is reattached to a visible or interactive subtree

For XR this might look like:

```text
ControllerXRComponent
  T(raw)
    TransformPipeline
      TransformForkTRS
        TransformMapRotation
          QuatTemporalSmooth
      TransformMergeTRS
        T(filtered)
          hand mesh / helper / ray subtree
```

But the same idea also fits:

- dynamic-bone chains
- jiggle follow helpers
- gizmo visual subtrees
- camera-relative attached props

---

## 5. Split into TRS is a fork, not a map

This terminology matters.

Breaking a transform into:

- translation
- rotation
- scale

is not really a “map”. It is a **fork** into channels.

So the cleaner vocabulary is:

- `TransformForkTRS`
- `TransformMapTranslation`
- `TransformMapRotation`
- `TransformMapScale`
- `TransformMergeTRS`

Conceptually:

```text
Transform
  -> TransformForkTRS
    -> T channel
    -> R channel
    -> S channel
  -> per-channel operators
  -> TransformMergeTRS
  -> Transform
```

This is clearer than saying “TransformMap splits TRS”, because splitting is branch creation, not transformation.

---

## 6. Proposed operator taxonomy

If the engine adopts a transform pipeline model, the operators likely fall into a few classes.

### A. Structural / channel operators

- `TransformForkTRS`
- `TransformMergeTRS`
- `TransformCompose`
- `TransformDecompose`

These are about representation changes.

### B. Spatial operators

- `TransformFilterChannels`
- `TransformOverrideTranslation`
- `TransformOverrideRotation`
- `TransformOverrideScale`
- `TransformParentSpaceMap`
- `TransformLocalSpaceMap`

These are about what spatial data flows through.

### C. Temporal operators

- `Vector3TemporalSmooth`
- `QuatTemporalSmooth`
- `TransformTemporalSmooth`
- `TransformSpring`
- `TransformCriticallyDamped`

These are stateful and operate over time.

Important distinction:

- `QuatTemporalSmooth` is the natural fit for rotation channels
- `Vector3TemporalSmooth` is still very useful, but mostly for translation, velocity-like streams, and other physics-style vector targets

### D. Routing / attachment operators

- `TransformPipelineInput`
- `TransformPipelineOutput`
- `TransformPipelineTarget`

These are about where the processed transform comes from and where it is applied.

---

## 7. Example authored shapes

### XR hand / controller stabilization

The more explicit authored shape probably wants to look like this:

```text
ControllerXRComponent {
  T {
    TransformPipeline {
      with_processing(
        TransformForkTRS {
          TransformMapRotation {
            QuatTemporalFilter {
              smoothing_factor = 1.0
            }
          }
        }
      )
      with_output(
        T {
          R {
            CUBE
          }
        }
      )
    }
  }
}
```

Important parts of this shape:

- the input transform is explicit from the parent `T {}` context
- `TransformPipeline` does not need a separate input node for the common case
- a future `with_input(...)` form could still exist for non-parent inputs or multi-source pipelines
- the output subtree is also explicit, which fits the topology-first style of the engine

### Direct no-scale inheritance equivalent

An authored subtree that keeps translation + rotation while dropping scale can be written directly as:

```text
TransformPipeline {
  with_processing(
    TransformForkTRS {
      TransformMapTranslation
      TransformMapRotation
      TransformMapScale {
        TransformDrop
      }
    }
  )
}
```

### Dynamic bone / secondary motion shape

For dynamic-bone-style follow, the pipeline may be attached to a driven authored transform:

```text
HeadBoneTransform {
  EarBaseTransform {
    TransformPipeline {
      with_processing(
        TransformForkTRS {
          TransformMapRotation {
            TransformSpring {
              stiffness = ...
              damping = ...
            }
          }
        }
      )
      with_output(
        T {
          cat ear renderable
        }
      )
    }
  }
}
```

This same pattern can be adapted for:

- hair strands
- tails
- clothing anchors
- body fat / soft tissue follow

### Does fork need merge?

Not always.

There are really at least three useful shapes:

1. **fork only, implicit passthrough merge**
   - useful when only one or two channels are modified
   - unspecified channels inherit the input transform unchanged
   - this is probably the nicest authored UX for common XR smoothing cases

2. **fork + explicit merge**
   - useful when the authored graph wants to make recomposition visible
   - useful when multiple channel branches or multiple intermediate values need to be combined explicitly
   - likely best for more advanced pipeline authoring and debugging

3. **fork to non-transform outputs**
   - useful when a branch drives something that is not immediately recomposed into a full transform
   - plausible for dynamic-bone internals, helper state, or future constraint systems

So the likely direction is:

- `TransformForkTRS` is fundamental
- `TransformMergeTRS` exists, but is not mandatory in every authored shape
- common cases can treat merge as implicit at pipeline output time if only a partial set of channels was processed

For XR controller smoothing, implicit merge is probably fine.

For dynamic bones and richer secondary motion, explicit merge may become more useful.

---

## 8. Temporal smoothing specifically

If the main immediate need is “smooth only part of a transform”, then the most useful first operator is probably a rotation-focused temporal operator.

Examples:

- `QuatTemporalFilter`
- `QuatTemporalSmooth`
- `RotationTemporalFilter`
- `Vector3TemporalSmooth` for translation or other vector-valued streams

For XR controller/hand proxies, practical use cases are:

- keep translation responsive
- smooth small rotational jitter
- optionally smooth translation less aggressively than rotation

For dynamic bones / jiggle-like systems, translation and rotation may both be temporal targets, but they likely want different operators than plain exponential smoothing.

Examples:

- hair strands may want spring / lag behavior on rotation and/or tip position
- cat ears may want head-follow rotation plus delayed secondary motion
- body fat / soft tissue may want translation-oriented follow with damping
- clothing anchors may want filtered transforms that lag behind the driver slightly

So it is useful to think in terms of:

- simple smoothing operators
- spring / lag operators
- eventually constraint-aware operators

rather than one monolithic temporal filter type.

---

## 9. Common runtime machinery for `src/engine/ecs/system/transform_pipeline_system.rs`

There is not a `src/engine/ecs/system/transform_pipeline_system.rs` yet, but the current code already shows what common machinery belongs there.

Today:

- subtree traversal and inherited-world propagation live in `TransformSystem`
- transform-pipeline parsing/evaluation lives in `TransformPipelineSystem`

The common responsibilities likely include:

- reading the input transform for a pipeline boundary
- decomposing a transform into translation / basis / scale channels
- orthonormalizing rotation when scale is removed
- extracting scale magnitudes from basis vectors
- recomposing a final transform from partially processed channels
- storing per-node temporal state for filters, springs, and dampers
- evaluating authored operator subtrees in a deterministic order
- publishing the resulting transform to the output subtree

The transform-pipeline runtime already owns the seed of this shared layer:

- translation passthrough logic
- basis extraction from the inherited world matrix
- rotation-only reconstruction via orthonormalization
- scale-only reconstruction via axis magnitudes

So the likely structure is:

- keep decomposition / recomposition helpers in `src/engine/ecs/system/transform_pipeline_system.rs`
- keep `TransformSystem` focused on general transform propagation while delegating transform-processing nodes to the pipeline system

This is the key architectural benefit: spatial filtering, temporal filtering, and dynamic-bone follow stop being separate ad hoc systems.

### Where temporal state can live

There are a few reasonable options for where filter state should live.

#### Option A: system-owned state keyed by parsed operator path

This is the current MVP direction.

- `TransformPipelineSystem` owns temporal state caches
- each state entry is keyed by the owning component plus the parsed stage/operator path
- authored components stay purely declarative

Pros:

- no runtime state leaks into serialized component data
- easy to reset/rebuild when topology changes
- one place to manage vec3/quat/spring operator history

Cons:

- requires stable path/key generation when parsing authored topology
- reparsing or topology churn has to preserve identity carefully if state continuity matters

#### Option B: runtime state attached directly to operator components

In this model, each temporal operator component stores non-serialized runtime state.

Pros:

- state identity is naturally tied to the operator node itself
- reparsing becomes simpler because the state is already located on the authored node

Cons:

- mixes authored config and runtime state inside component payloads
- makes serialization/lifecycle boundaries less clean
- gets awkward if one authored node is reused in multiple evaluation contexts later

#### Option C: state attached to output transforms or driven targets

In this model, the driven transform stores the temporal state that produced it.

Pros:

- easy to think about from the perspective of “this output is smoothed”

Cons:

- poor fit for pipelines that fork into multiple channels or multiple outputs
- awkward for nested blocks or non-transform intermediate values
- couples filter state to targets instead of operators

The strongest current direction is still Option A: system-owned state keyed by parsed operator identity.

### How temporal filtering should scale with frame rate

There are also a few reasonable update models.

#### Option 1: raw per-frame alpha

Example:

```text
output = lerp(previous, input, smoothing_factor)
```

Pros:

- very simple

Cons:

- frame-rate dependent
- the same authored value feels different at 72 Hz vs 144 Hz

#### Option 2: exponential decay using `dt`

Example:

```text
alpha = 1 - exp(-lambda * dt)
output = lerp(previous, input, alpha)
```

Pros:

- much more frame-rate stable
- one authored smoothing parameter can behave consistently across different runtimes
- works well for both vec3 and quat filters

Cons:

- authored parameter semantics need to be documented clearly (`lambda`, half-life, or time constant)

#### Option 3: fixed-step simulation with an accumulator

Example:

- accumulate frame time
- step the filter at a fixed simulation rate (for example 120 Hz)
- optionally interpolate presentation output

Pros:

- best determinism
- often a better fit for spring/dynamic-bone operators

Cons:

- more machinery
- may be overkill for the first smoothing/filter MVP

The current MVP implementation uses Option 2 when `dt` is available:

- state lives in `TransformPipelineSystem`
- vec3/quat filters are keyed by parsed stage path
- alpha is computed as `1 - exp(-smoothing_factor * dt)`
- if `dt` is unavailable, the implementation falls back to a clamped raw alpha as a best-effort path

That is a good starting point for XR smoothing.

If dynamic bones and springs become a primary use case, a later fixed-step path may make sense for those operators specifically.

## 10. Integration with the current topology model

There is one architectural constraint that matters here:

- this ECS is topology-first and node-oriented
- it is not a free-form “many arbitrary components on one entity” model

So whatever transform pipeline exists likely needs to be representable as a subtree of nodes/components.

That is why the operator-tree framing is appealing.

It matches the style of:

- source node
- processor node(s)
- output node
- actual renderable / interaction subtree

That is also why the transform filter proposal in the gizmo docs naturally ended up as a node in the topology.

So the likely direction is:

- **transform processing as subtree topology**, not hidden magical flags on `TransformComponent`
- **runtime execution as `TransformPipelineProcessor`**, not ad hoc logic scattered across unrelated systems

---

## 11. Practical recommendation

The strongest current recommendation seems to be:

1. define a broader **transform pipeline** concept in the spec
2. treat TRS split as a **fork**
3. allow both implicit merge and explicit `TransformMergeTRS`, depending on authored complexity
4. use explicit primitive operator subtrees for channel inheritance/drop behavior
5. introduce temporal smoothing as one operator family inside that model
6. keep `TransformPipeline` as the component-tree analog
7. keep shared channel-processing math in `TransformPipelineSystem`
8. keep `TransformPipelineProcessor` as the runtime/system that evaluates it

This avoids painting the engine into a corner where:

- `TransformFilter` is one system
- temporal smoothing is another unrelated system
- dynamic-bone follow is a third unrelated system

Instead, they become different uses of the same transform-processing vocabulary.

---

## 12. Summary

The broader transform-pipeline model should:

- provide a reusable transform-processing vocabulary
- unify spatial filtering and temporal filtering under one authored topology model
- allow simple pipelines to omit explicit merge when recomposition can be implicit
- keep quaternion-first smoothing for rotation channels
- keep vector smoothing available for translation and other physics-style vector streams
- scale from XR stabilization to dynamic bones and secondary motion

That broader model could cover:

- XR hands/controllers
- gizmo inheritance shaping
- stabilized helper transforms
- dynamic bones
- jiggle / follow motion
- hair / ears / clothing / soft secondary motion
