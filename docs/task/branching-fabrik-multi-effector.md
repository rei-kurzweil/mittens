# Task: Branching FABRIK (Multi-Effector)

Add a new `IKSolver` variant that solves a **tree-shaped** chain — one shared root,
multiple end effectors — using multi-effector FABRIK. Primary use case: a single
leash that branches in a Y to two dogs (one root anchor at the character's hand,
two collar targets).

Secondary priority. Lands after `leash-dynamic-chain-fabrik.md` validates the
single-chain authoring path.

## Research Findings

- The current FABRIK solver assumes a linear chain (`collect_tc_chain` walks one
  TC-child path; `ik_system.rs:78`). It cannot represent a tree.
- `IKChainComponent` carries exactly one `target_id` and one `end_effector_id`
  (`src/engine/ecs/component/ik_chain.rs:53-62`). A branching solver needs N targets
  and N effectors.
- Multi-effector FABRIK is a known extension: forward pass runs from each end
  effector toward the shared root independently, producing one candidate position
  for each shared joint per branch; the backward pass averages those candidates
  at branch joints before continuing toward the leaves. See e.g. Aristidou & Lasenby
  2011 §4 "FABRIK with multiple end effectors".
- The TC topology already supports trees natively (`children_of`); the IK side is
  the only place that flattens to a linear chain.

## Proposed Changes

### 1. New solver variant in `src/engine/ecs/component/ik_chain.rs`

Add:

```rust
IKSolver::FabrikMulti {
    max_iterations: u32,
    tolerance: f32,
}
```

This variant ignores the existing `target_id` / `end_effector_id` fields on
`IKChainComponent`. Multiple targets/effectors require a new component (next
section), because an enum-only change cannot carry N pairs.

Update `encode` / `decode` for the new variant.

### 2. New component for branching topology

Add `IKBranchComponent` in a new file `src/engine/ecs/component/ik_branch.rs`:

```rust
pub struct IKBranchComponent {
    /// Solver config. Currently always FabrikMulti.
    pub solver: IKSolver,

    /// One entry per end effector: (end_effector_tc, target_tc).
    pub effectors: Vec<(ComponentId, ComponentId)>,

    pub weight: f32,
    component: Option<ComponentId>,
}
```

Placement convention mirrors `IKChainComponent`: place as a child of the shared
**root joint TC**. The system walks down the TC subtree from the root, collecting
all paths that terminate at one of the `effectors[i].0` IDs, to discover the tree.

### 3. New solver in `src/engine/ecs/system/ik_system.rs`

Add `solve_fabrik_multi`. Algorithm sketch:

1. From the root TC, BFS/DFS down the TC subtree. For each effector
   `(end_id, target_id)`, find the TC path root → end. Record the union of all path
   nodes as the **tree node set** and the per-node **branch membership**
   (which effector paths pass through this node).
2. Compute current world positions for every node, and bone lengths along each
   parent→child edge.
3. Iterate up to `max_iterations`:
   a. **Forward pass**: for each effector path independently, snap the end to its
      target and walk up the path computing candidate positions for each ancestor.
      Accumulate candidates per node.
   b. At each node, set its position to the **average of its candidates** across
      branches. (Nodes on only one path carry one candidate; the shared root and
      branch-junction nodes average across multiple.)
   c. **Backward pass**: pin the root to its original world position; walk down the
      tree (BFS), enforcing each parent→child bone length.
   d. Break early if every effector is within `tolerance` of its target.
4. Convert solved positions to local rotations and emit one `UpdateTransform` per
   non-leaf node, mirroring the existing `solve_fabrik` post-pass
   (`ik_system.rs:376-426`).

### 4. System dispatch

In `IKSystem::tick`, add a parallel discovery pass for `IKBranchComponent` and
dispatch to `solve_fabrik_multi`. The `IKChainComponent` path is unchanged.

### 5. Example: `examples/y-leash-demo.rs` + `.mms`

Three draggable anchors (one hand, two collars) and a Y-shaped TC tree
(stem from hand, splitting into two arms after some segments). One
`IKBranchComponent { FabrikMulti, effectors: [(end_a, collar_a), (end_b, collar_b)] }`
under the stem root.

The example is the acceptance criterion: dragging either collar pulls only that
arm of the Y plus the shared stem; the unaffected arm stays put modulo shared-stem
movement.

### 6. Out of scope

- Joint-angle limits / cone constraints at branch points.
- Asymmetric weighting between effectors (e.g. dog A pulls harder than dog B).
- Self-collision between branches.
- Trees with internal nodes that are themselves targets (only leaves are
  effectors in this design).

These are reasonable v2 extensions but not required for the Y-leash use case.
