# Secondary Motion System

Status: **implemented retained runtime**. `SpringBone` is the only supported secondary-motion
simulation type today.

## Purpose

`SecondaryMotionSystem` applies procedural motion after the primary pose has been produced by
animation, input, AVC, constraints, and IK. Its output must be propagated into world transforms
before skinned-mesh palettes are refreshed for rendering.

Secondary motion is continuously simulated while enabled. It must not depend on an unrelated
`UpdateTransform` from Input or InputXR to become visible.

## Terminology and authored hierarchy

```text
GLTF
└── SecondaryMotion                  secondary-motion root
    ├── SpringBone.new("hair")       supported simulation child; a spring chain
    │   ├── SpringJoint(...)         ordered joint configuration/reference
    │   ├── SpringJoint(...)
    │   └── SpringJoint(...)
    └── SpringBone.new("bust")       another spring chain
        ├── SpringJoint(...)
        └── SpringJoint(...)
```

- **Secondary-motion root**: a `SecondaryMotionComponent` beneath an authored GLTF. It owns and
  scopes a set of secondary-motion simulations.
- **Simulation child**: a supported direct child of the root. `SpringBoneComponent` is the only
  supported type currently; future secondary-motion types may not be chains.
- **Spring chain**: one `SpringBoneComponent` and its ordered joint configuration.
- **Joint configuration**: one `SpringJointComponent`, containing an authored reference and physics
  values.
- **Bound joint**: the runtime joint configuration after its reference resolves to a concrete
  imported transform `ComponentId` and rest pose.
- **Dirty chain transform root**: the first imported transform modified by a spring chain. This is
  not the `SecondaryMotionComponent`; it is the transform subtree root that must be propagated.

## Required frame order

```text
primary pose writes (animation / AVC / IK)
    → primary transform propagation
    → secondary-motion simulation
    → propagate dirty chain transform roots
    → refresh skinned-mesh palettes
    → render
```

Secondary motion writes final local joint rotations for the frame. World-matrix and skin-palette
consumers must observe those writes in the same frame.

## Retained-runtime architecture

The system retains the complete runtime ownership graph. Frame ticks do not discover roots,
simulation children, chains, or joints by scanning ECS components or enumerating authored children.

An illustrative layout is:

```rust
struct SecondaryMotionSystem {
    roots: HashMap<ComponentId, SecondaryMotionRootRuntime>,
    child_owners: HashMap<ComponentId, ComponentId>,
}

struct SecondaryMotionRootRuntime {
    gltf: Option<ComponentId>,
    children: HashMap<ComponentId, SecondaryMotionRuntime>,
}

enum SecondaryMotionRuntime {
    SpringBone(SpringBoneRuntime),
    // Future supported secondary-motion child types live here.
}

enum BindingState<T> {
    WaitingForDependencies,
    Bound(T),
    Invalid(String),
}

struct SpringBoneRuntime {
    binding: BindingState<BoundSpringChain>,
    enabled_last_frame: bool,
}

struct BoundSpringChain {
    joints: Vec<BoundSpringJoint>,
    previous_tails: Vec<[f32; 3]>,
    current_tails: Vec<[f32; 3]>,
    lengths: Vec<f32>,
    accumulator: f32,
}
```

The precise Rust types may differ, but the ownership and behavior are required:

- Root lookup, child lookup, and child-to-root removal are direct map operations.
- A registered root owns retained runtime entries for all supported direct children.
- A bound spring chain owns resolved joint transform IDs and all simulation state.
- Binding errors and waiting states are stored on the child rather than rediscovered every frame.
- Future simulation types add an enum variant/runtime implementation without broadening the frame
  loop into a full-world or untyped child scan.

## Lifecycle and registration

Registration is event-driven and idempotent.

### Root lifecycle

- `SecondaryMotionComponent::init` registers the root.
- Root registration resolves or records its owning GLTF and creates its retained root entry.
- Root cleanup removes every owned child runtime and reverse-index entry.
- Reparenting a root invalidates its GLTF association and every binding that depends on it.

### Simulation-child lifecycle

- Each supported direct child registers itself during component initialization. For the current
  implementation, `SpringBoneComponent::init` registers the spring chain with its direct
  `SecondaryMotionComponent` parent.
- Registration must tolerate root and child intents arriving in either order because the complete
  ECS topology already exists before lifecycle intents drain.
- Child cleanup removes its runtime state in O(1) through the reverse ownership index.
- A child attached outside a valid secondary-motion root remains explicitly unbound and may emit a
  one-time diagnostic; it is not searched for every frame.

### Joint/configuration lifecycle

- Spring joints are collected and resolved when their owning chain binds, not every frame.
- Adding, removing, reordering, or editing a `SpringJointComponent` invalidates only its owning
  spring chain.
- A joint lifecycle/topology notification must make dynamic edits observable without requiring
  periodic child enumeration.
- Removing or respawning an imported GLTF joint invalidates affected bindings before their IDs can
  be reused.

Subtree removal should ultimately use the unified lifecycle described in
[`unify-subtree-removal-and-component-system-cleanup.md`](../task/unify-subtree-removal-and-component-system-cleanup.md).
Defensive missing-ID checks may remain during that migration, but they are not the ownership model.

## Binding and invalidation

Binding a `SpringBone` performs the expensive structural work once:

- Resolve the owning GLTF instance.
- Validate supported GLTF instance scale.
- Resolve the optional center reference.
- Collect ordered `SpringJoint` configuration.
- Resolve every joint reference within the owning GLTF instance.
- Read immutable imported rest poses.
- Validate ancestry, chain overlap, and virtual endpoint requirements.
- Compute segment lengths and initialize simulated tail positions.

A bound chain is invalidated only by a relevant event:

- owning GLTF respawn/reload or root reparenting;
- referenced imported transform removal;
- spring-chain or joint configuration/topology change;
- explicit system reset.

Waiting or failed bindings must not retry expensive selector resolution every frame. They retry when
a relevant dependency event occurs, such as GLTF initialization/respawn or configuration topology
change. Diagnostics should be emitted once per distinct failure until invalidation makes a retry
eligible.

## Per-frame behavior

For each retained root and retained simulation child:

1. Skip children that are waiting for dependencies or invalid until an event makes them retryable.
2. For an enabled bound spring chain, reset after first enable, invalid frame time, or an excessive
   frame gap.
3. Accumulate frame time and advance at the fixed simulation step, subject to the catch-up cap.
4. For each spring joint, apply inertia, drag, stiffness, gravity, and length constraint in chain
   order.
5. Convert simulated tail directions into parent-local corrections relative to imported rest
   rotations.
6. Write local rotations/model matrices to the imported transforms.
7. Add the first modified imported transform to `dirty_chain_transform_roots`, deduplicated for the
   frame.
8. Return those roots to `SystemWorld` for synchronous transform propagation before skinning.

The steady-state frame path must not call `World::all_components`, enumerate a root's authored
children, resolve component selectors, rebuild joint vectors, or allocate discovery collections.

## SpringBone simulation semantics

The existing fixed-step behavior remains the baseline:

- fixed step: `1 / 60` seconds;
- accumulated time capped to four fixed steps;
- momentum derived from current versus previous simulated tail position;
- drag applied to momentum;
- stiffness pulls toward the current primary/rest direction;
- gravity is world-space;
- each tail is constrained to its segment length;
- the previous segment's simulated tail becomes the next segment's simulated head;
- local rotation is the shortest-arc correction from rest direction to desired direction, composed
  with the imported rest rotation.

Center-relative inertia, colliders, angular limits, and non-spring secondary-motion child types are
not yet implemented.

## Diagnostics and performance expectations

- Debug output reports retained roots, registered children, bound/waiting/invalid simulations, and
  maximum correction without triggering discovery work.
- With no registered roots, the tick returns immediately.
- Steady-state CPU cost scales with active retained simulations and their bound joints, not total
  ECS component count.
- Registration, binding, invalidation, and cleanup should be separately measurable from simulation
  time when system profiling is enabled.

## Remaining lifecycle dependency

Secondary motion participates in the current subtree-removal coordinator and also accepts explicit
component cleanup intents. The separate unified subtree-removal project remains responsible for
making that cleanup contract common to every retained engine system. Cached-record existence checks
in the frame path are temporary defensive protection for callers that bypass the coordinator; they
do not perform discovery or binding retries.
