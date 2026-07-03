# Task: Leash / Dynamic Chain via Existing FABRIK

Expose the existing `IKSolver::Fabrik` chain as a general-purpose authoring primitive
for dynamic chains with two constrained ends — primarily a leash between a character's
hand and a dog's collar, but also strings, ropes, and similar pose-driven (non-physics)
chains.

This is the **single-leash** unlock. It does not introduce a new solver; it makes the
existing one usable from outside `AvatarControlSystem`.

## Research Findings

- `IKSolver::Fabrik { max_iterations, tolerance }` already exists
  (`src/engine/ecs/component/ik_chain.rs:31`) and is fully implemented in
  `solve_fabrik` (`src/engine/ecs/system/ik_system.rs:336`).
- The implementation is dual-pin out of the box: the forward pass snaps the end to
  `target_id`'s world position, the backward pass re-pins the root to its
  pre-solve world position (`ik_system.rs:360`, `ik_system.rs:367`). So if the chain
  root TC is parented under (or otherwise tracks) one moving anchor, and `target_id`
  is the other moving anchor, both ends follow.
- `IKSystem` discovers chains generically — every `IKChainComponent` in the world ticks
  (`ik_system.rs:18-25`). `AvatarControlSystem` is just one current author of these
  components; nothing about IK is AVC-specific.
- `IKChainComponent` requires the chain to be a contiguous TC parent/child path from
  the root TC down to `end_effector_id` (`collect_tc_chain`, `ik_system.rs:78`).
- `Fabrik` only handles the *positional* constraint. The current solver does not
  apply orientation constraints at endpoints, gravity, slack/taut behavior, or
  rest-pose bias.

## Proposed Changes

### 1. Authoring documentation

Add a short section to `docs/spec/` (new file `dynamic-chain.md`, or a section appended
to a relevant existing spec) describing the authoring pattern:

- Build a TC chain N segments long (N ≥ 2 TCs, so N-1 bones).
- Parent the chain root TC under one anchor (e.g. the character's hand bone).
- Pick another TC (e.g. dog collar TC) as `target_id`.
- The last TC in the chain is `end_effector_id`.
- Place an `IKChainComponent { solver: Fabrik { max_iterations, tolerance }, target_id,
  end_effector_id }` as a child of the chain root TC.

Document the over-extension behavior: when `distance(root, target) > sum(bone_lengths)`,
the chain goes taut and the end falls short of the target by the deficit. This is
correct leash behavior — it is what stops the dog from teleporting.

### 2. Example: `examples/leash-demo.rs` + `examples/leash-demo.mms`

A minimal scene:

- A draggable "hand" TC (cube) and a draggable "collar" TC (cube), both controllable
  in the editor.
- A chain of ~8 TCs between them, each rendering a small segment mesh.
- An `IKChainComponent { Fabrik }` wiring root → end → target as above.

This example is the verification surface for the authoring pattern. It also exercises
`IKSystem` outside `AvatarControlSystem` for the first time, which is the part of this
change most likely to surface latent assumptions.

### 3. Optional: `LeashComponent` thin wrapper (defer unless needed)

If the example shows that authoring is awkward (e.g. computing bone segmentation by
hand each time), introduce a `LeashComponent { length, segments, anchor_a, anchor_b }`
that, on `init`, spawns the TC chain and the `IKChainComponent` with reasonable
defaults. Until that pain is concretely felt, do not add the wrapper — the raw
authoring path is the source of truth.

### 4. Known gaps (do not address in this task; record for follow-up)

- No gravity / rest-shape bias. A slack leash will hold whatever shape the previous
  tick left it in, not droop. Acceptable for v1; revisit when needed.
- No collision against world geometry.
- No twist control along the chain (FABRIK only constrains positions).
- No cross-tick state — IK re-solves from current world matrices each tick. Combined
  with the lack of gravity bias, slack chains can look "frozen" between movements.
  This intersects with the temporal IK state idea in
  `docs/task/wip/avatar-control.md` §4.

These are deliberately out of scope. The point of this task is to land the basic
two-end-pinned authoring pattern.
