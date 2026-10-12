# Shared effective transform-parent basis resolution

Status: proposed implementation plan

Related:

- [Transform component accessors: Mittens engine API](transform-component-accessors-engine-api.md)
- [Transform-stream runtime](../spec/transform-operators.md)
- [VTuber slide-deck detached world-TRS tracker](../task/vtuber-slidedeck-detached-world-trs.md)
- [World-TRS snapshot implementation review](../review/world-trs-snapshot-and-vtuber-slidedeck.md)
- [Transform-camera-specific behavior](../spec/transform-camera-specific.md)

## Goal

Give transform propagation and world-space transform mutation one shared answer to this question:

> What owns this transform's world result, and—when its local TRS is authored—what matrix is the
> effective inherited parent basis applied before that local TRS?

The result must cover:

- a detached/root transform;
- an ordinary transform ancestor separated by zero or more non-transform components;
- a `TransformParentComponent` which redirects inheritance to another target;
- a transform directly owned by a transform-stream boundary;
- a normal transform whose nearest transform parent happens to be stream-owned further upstream.

This is a consolidation of current engine behavior. It must not redesign transform streams, change
authored topology, or approximate matrices which cannot be represented as TRS.

## Why this needs a dedicated slice

`TransformSystem::transform_changed` currently discovers its chain root and effective basis while
building the transform ancestor chain. It has distinct flags for a transform-stream boundary and a
`TransformParent` boundary.

`TransformSystem::world_to_local_trs` now performs a second ancestry walk with conceptually the
same precedence rules. That made the first world setter possible, but it creates a semantic drift
risk: a future boundary type could be added to propagation and omitted from world writes, or vice
versa.

The shared logic should not be an MMS helper. Effective inheritance is an engine invariant used by
any Rust caller which reads, propagates, or writes transforms.

## Terminology

### Structural parent

The nearest ancestor `TransformComponent` reached through ordinary ECS parent links, ignoring
intervening non-transform components.

### Effective parent basis

The matrix which is multiplied by an authored transform's local model matrix:

```text
world = effective_parent_basis * local
```

For a root this is identity. For an ordinary child it is the nearest structural transform's cached
world matrix. For a `TransformParent` redirection it is the resolved target's effective cached
world matrix.

### Stream-owned world result

A transform immediately downstream of a transform-stream boundary is different. Its cached world
matrix is an output owned by that stream operator. Current propagation retains that cached output
as the chain basis and deliberately skips recomputing the transform from its authored local model.

Therefore a stream-owned result is not merely another effective parent matrix. Treating it as one
would imply that changing the owned transform's local TRS can reliably set its world pose, which is
not true under current propagation semantics.

## Required invariant

The resolver must preserve this distinction:

```text
Authored world result:
    world = inherited_basis * local
    -> world setter may invert inherited_basis

Stream-owned world result:
    world = transform-stream output retained in matrix_world
    -> direct authored world setter must reject
```

A descendant below a stream-owned transform is still writable when its nearest parent is that
ordinary transform component:

```text
TransformForkTRS
└── stream_output_transform       stream-owned; direct world write rejects
    └── authored_child_transform  parent basis is stream_output_transform.matrix_world; writable
```

The nearest transform stops the ancestry-gap scan. A boundary above that nearest transform does
not make every descendant stream-owned.

## Proposed engine representation

Use a result which reports ownership as well as the matrix. Names are tentative, but the semantic
split should remain explicit:

```rust,ignore
pub(crate) enum EffectiveTransformBasis {
    Authored {
        source: EffectiveBasisSource,
        matrix: TransformMatrix,
    },
    StreamOwned {
        boundary: ComponentId,
        cached_world: TransformMatrix,
    },
}

pub(crate) enum EffectiveBasisSource {
    Root,
    StructuralTransform {
        transform: ComponentId,
    },
    TransformParent {
        boundary: ComponentId,
        target: ComponentId,
    },
}

pub(crate) enum EffectiveBasisError {
    NotTransform(ComponentId),
    UnresolvedTransformParent {
        boundary: ComponentId,
    },
    TransformParentTargetHasNoWorldBasis {
        boundary: ComponentId,
        target: ComponentId,
    },
}
```

`Root` returns the identity matrix. Keeping it in the same `Authored` branch lets callers use the
same multiplication/inversion path without interpreting absence as an error.

`StreamOwned` includes the cached world output because propagation needs to retain it. World
mutation uses the ownership classification to reject the operation and does not invert that
matrix as though it were a parent basis.

The helper should be owned by `TransformSystem` or a small engine-transform module used by it. It
should not become a component method because resolution requires `World`, transform-stream
boundary knowledge, and cached ancestor state.

## Resolve one ancestry gap, not the entire tree

A reusable helper should scan upward from one actual transform until the first meaningful item:

```rust,ignore
fn resolve_effective_basis(
    world: &World,
    transform_stream: &TransformStreamSystem,
    transform: ComponentId,
) -> Result<EffectiveTransformBasis, EffectiveBasisError>;
```

The scan order for every non-transform ancestor is significant:

1. If the parent is an ordinary `TransformComponent`, return its cached `matrix_world` as a
   structural authored basis.
2. Otherwise, if it is a `TransformParentComponent`, resolve its target and return the target's
   current effective cached world matrix.
3. Otherwise, if `TransformStreamSystem::is_transform_stream_boundary` reports a boundary, return
   `StreamOwned` for the original transform.
4. Otherwise continue through the non-transform wrapper.
5. If there is no parent, return authored root/identity.

`TransformParent` must be checked before the general stream-boundary predicate because the current
predicate includes `TransformParentComponent`. Redirection has a resolvable basis and different
failure behavior; it must not collapse into generic stream ownership.

The helper resolves one gap between transforms. It must not recursively recompute ancestor world
matrices or enumerate the whole world.

## Propagation integration

Refactor the chain-building portion of `transform_changed` to consume the shared resolver without
changing propagation order:

1. Start at the changed transform and add it to the leaf-to-root transform chain.
2. Resolve its next ancestry gap.
3. For a structural transform basis, move to that transform, add it to the chain, and repeat.
4. For root/identity, stop and recompute the complete reversed chain from identity.
5. For `TransformParent`, stop and recompute the complete reversed chain from the redirected target
   basis.
6. For `StreamOwned`, stop, retain the owned chain root's cached world matrix, skip applying that
   root's local model, and recompute only its ordinary transform descendants.
7. For unresolved redirection, preserve the current behavior: retain the last effective cached
   world result rather than falling back to structural ancestry.

Each ancestry gap is scanned once, so a depth-N chain remains O(N). Do not implement propagation by
calling a helper which independently walks from every transform all the way to the world root;
that would make deep transform updates O(N²).

The refactor should first be characterized against current behavior. It is not an opportunity to
change light, camera, collision, renderable, transform-dependent, or stream evaluation ordering.

## World-write integration

`world_to_local_trs` should use the same resolver:

```text
Authored { matrix: effective_parent_basis }
    desired_local_matrix = inverse(effective_parent_basis) * desired_world_matrix
    desired_local_trs = strict_decompose(desired_local_matrix)

StreamOwned { boundary, .. }
    reject TransformAccessError::TransformStreamOwned
```

Map basis resolution errors into the public/internal `TransformAccessError` without losing the
boundary and target IDs needed for diagnostics.

Conversion still happens when `SetTransformTrs` executes, not when MMS evaluates the setter. That
preserves execution-time topology and parent matrices.

## Cache and mutation policy

The resolver is read-only. It may read:

- structural parent links;
- component types;
- `TransformParent` references and their resolved target;
- cached `matrix_world` values;
- the transform-stream system's boundary classification.

It must not:

- evaluate a transform stream;
- mutate or refresh cached world matrices;
- silently substitute identity for an unresolved `TransformParent`;
- decompose any matrix;
- walk transform-parent dependents;
- emit intents or events.

Callers remain responsible for ensuring they run at a point where cached world matrices are
coherent. `SetTransformTrs` already executes at the normal command drain and propagation already
owns cache refresh ordering.

## Error behavior

Keep resolution errors different from mathematical conversion errors:

- resolution errors explain why an effective basis is unavailable;
- inversion errors identify a singular effective parent;
- decomposition errors identify a world pose which has no exact local TRS under that basis.

Propagation and setters consume the same resolution result but respond differently:

| Result | Propagation | World setter |
| --- | --- | --- |
| Root/identity | Recompute from identity | Local equals desired world TRS |
| Structural transform | Recompute ordinary chain | Invert cached parent world |
| `TransformParent` target | Recompute from redirected basis | Invert redirected basis |
| Stream-owned | Retain stream output, skip owned local | Reject without mutation |
| Unresolved `TransformParent` | Retain last cached result | Reject without mutation |
| Singular authored basis | Propagation can still multiply | Setter rejects inversion |
| Local matrix contains shear/reflection | Not a propagation concern | Strict decomposition rejects |

Queued MMS setters currently report execution failures through stderr. Improving asynchronous
mutation diagnostics is related work, but it is not required to centralize basis semantics.

## Test matrix

### Resolver characterization

- Root transform resolves to authored identity.
- Non-transform wrappers between a root and transform do not alter identity.
- Nearest ordinary transform resolves to its cached world matrix.
- Multiple non-transform wrappers between two transforms resolve to the nearest transform.
- A boundary above the nearest transform does not mark the descendant stream-owned.
- `TransformParent` resolves its target basis and records boundary and target IDs.
- A `TransformParent` target which is not itself a transform uses the same effective cached world
  lookup as current propagation.
- Missing or unresolved `TransformParent` targets return a structured error.
- Each transform-stream boundary kind returns `StreamOwned` for its immediate transform output.

### Propagation parity

- Ordinary root/child/grandchild matrices are unchanged by the refactor.
- `TransformParent` followers still update when their target changes.
- Unresolved followers keep their previous cached world matrix.
- Stream-owned roots retain their evaluated cached world output during a direct descendant refresh.
- Ordinary descendants below a stream-owned root recompute from that cached root.
- Existing camera-specific mono/stereo behavior remains unchanged.

### World setters

- Detached root transfer.
- Ordinary translated and rotated parent.
- Uniformly scaled parent.
- Non-uniform scale with an exactly representable local result.
- Non-uniform scale which would require shear and must reject.
- Singular effective parent and no partial mutation.
- Reflected/non-decomposable result and no partial mutation.
- Resolved and unresolved `TransformParent` boundaries.
- Direct stream-owned target rejection.
- Writable ordinary child beneath a stream-owned transform.
- Parent or topology changes after intent creation but before execution use the execution-time basis.

## Implementation slices

### Slice 1: characterization without behavior changes

- Add focused fixtures around the existing propagation and world-write behavior.
- Cover ordinary gaps, redirection, unresolved redirection, stream ownership, and writable
  descendants below stream output.
- Record current camera-specific and transform-fork behavior before moving traversal code.

### Slice 2: introduce the shared resolver

- Add the ownership/source/error types.
- Implement the single-gap ancestry scan.
- Replace the duplicated scan inside `world_to_local_trs` first.
- Keep all current `TransformAccessError` messages stable or improve them deliberately in tests.

### Slice 3: consume it from propagation

- Replace `stream_boundary`, `transform_parent_boundary`, and `transform_parent_basis` discovery in
  `transform_changed` with the shared result.
- Preserve chain construction, cached stream-root handling, propagation side effects, and
  transform-parent dependent refreshes.
- Confirm the walk remains O(depth) and allocation does not increase beyond the existing transform
  chain vector.

### Slice 4: complete boundary and intent-timing coverage

- Add every world-setter case from the test matrix.
- Add the intervening-parent-change intent test.
- Run the slide-deck detached snapshot test as the end-to-end MMS regression.
- Update the implementation tracker and review once the duplicated traversal is gone.

## Acceptance criteria

- One engine helper defines root, structural parent, `TransformParent`, and stream-owned boundary
  precedence.
- `transform_changed` and `world_to_local_trs` both consume that helper.
- No simpler ancestry walk remains in the MMS evaluator or mutation executor.
- Stream-owned targets remain explicit and cannot be mistaken for invertible authored parent bases.
- Unresolved redirection never silently falls back to structural ancestry or identity.
- Propagation remains O(depth) for one changed transform chain.
- Existing slide-deck world-pose snapshot behavior remains unchanged.
- Focused ordinary-parent, redirected-parent, stream-boundary, singular, shear, and intent-timing
  tests pass.

## Non-goals

- Changing transform-stream authoring syntax or operator semantics.
- Renaming all remaining transform-pipeline identifiers.
- Adding approximate matrix decomposition.
- Adding MMS `translation`, `rotation`, or `scale` world accessors.
- Creating a continuous follow constraint between copied transforms.
- Solving asynchronous MMS mutation-result reporting.
