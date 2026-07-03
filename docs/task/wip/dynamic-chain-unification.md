# Dynamic Chain Unification — WIP

Date: 2026-05-07

Scratch space exploring whether IK chains and spring/jiggle bones should share
primitives. Not a task yet — writing it down so the design space stays visible
while the leash + Y-leash tasks land.

---

## The current split

| Concern | Spring (`QuatSpringComponent`) | IK (`IKChainComponent`) |
|---|---|---|
| Granularity | 1 component per affected TC | 1 component per chain (N TCs) |
| Lives in | `TransformPipelineSystem` | `IKSystem` |
| Output | Inline rotation override during world-matrix propagation | N `UpdateTransform` intents |
| State | Per-stage angular velocity (kept in pipeline system) | Stateless — re-solves from current world matrices each tick |
| Algorithm shape | Local recurrence — each bone reads parent, integrates, hands to child | Global iteration — fwd+bwd passes touch every joint |
| End condition | None (forward integration in time) | Positional constraint at end effector |

Spring is decomposable into per-bone operators because it's a forward-time
integration with no end goal. FABRIK is *not* decomposable because both ends are
constrained and the iterations need to see the whole chain.

So today's "one node per TC" model fits spring and `AimConstraint` (always 1
bone), but **not** `TwoBoneIK` (closed-form, 3 TCs from one component) or
`Fabrik` / `FabrikMulti` (chain solve from one component).

---

## What a unified model would look like

The key insight: spring chains and FABRIK leashes are both *constrained dynamic
chains*, just with different constraint sets.

```
DynamicChain {
    root_constraint:  Pinned                 // always for v1
    end_constraint:   Free | Target { id, weight }
    per_joint_default: { stiffness, damping, mass, rest_local_rot }
    per_joint_overrides: Vec<(ComponentId, JointParams)>
}
```

Behavior table:

| `end_constraint` | `stiffness` | What it is |
|---|---|---|
| Free | 0 | Hair / loose tail (current jiggle preset) |
| Free | > 0 | Antenna spring back to rest |
| Target { weight=1 } | 0 | Classic FABRIK leash |
| Target { weight<1 } | 0 | Soft leash — sags / lags target |
| Target { weight=1 } | > 0 | IK with rest-pose bias (natural arm reach) |
| Target { weight=1 } | per-joint | Skirt panel that follows hip but resists fold-back |

That last group of rows is the genuinely new behavior. Today TwoBoneIK snaps
arms straight and FABRIK has no rest-pose preference; "stiffness-aware IK" gives
a natural elbow / pole bias for free, and lets a leash sag without a separate
spring system.

---

## The hard part: solver fusion

You can't just paste the spring loop and the FABRIK loop together. The
candidates roughly:

### A. PBD / XPBD (position-based dynamics)
- Each tick: predict positions via velocity + gravity, then iteratively project
  positions to satisfy constraints (bone length, end target, rest-angle springs).
- Both ends pinned → end-target projection.
- Free end → no projection at end, position evolves under integration.
- Stiffness becomes an "angular distance from rest" soft constraint.
- Works for arbitrary topology (including Y-branching) the same way FabrikMulti
  does — average candidates at shared joints.

### B. FABRIK + per-joint spring relaxation
- Run FABRIK fwd+bwd as today.
- Between iterations (or after the last one), blend each joint's solved rotation
  toward `rest_local_rot` weighted by `stiffness * dt`.
- Carry angular velocity across ticks for momentum.
- Simpler to retrofit to the existing solver. Loses some physical fidelity vs
  full PBD.

### C. Single forward integrator, optionally pulled toward target
- Pure spring chain by default. If `end_constraint = Target`, add a constraint
  force on the end joint pulling toward target (with `weight` as spring k).
- Does *not* enforce hard length constraints — bones can stretch.
- Cheapest. Not really "IK" — fails when target is closer than chain rest length
  (the chain doesn't fold up nicely).

PBD is the right long-term answer; (B) is the cheapest stepping stone that
keeps the existing FABRIK code load-bearing.

---

## State

Whichever solver wins, this needs **per-joint state across ticks**:
- angular velocity (for momentum / damping)
- previous local rotation (for finite-difference velocity if not stored
  directly)

Spring already has this pattern — keyed by stage path in
`TransformPipelineSystem`. IK doesn't. The unified solver picks one home; the
natural home is `IKSystem` (or a renamed `DynamicChainSystem`) because the
solver writes multi-joint output anyway.

There's also a hook here for the upcoming `VelocityComponent` /
`AngularVelocityComponent` story (see `docs/task/wip/velocity-components.md`) — if
those become a first-class component on TCs, the dynamic-chain solver should
read/write them directly instead of keeping a private state map.

---

## Output mechanism

Spring writes inline during world-matrix propagation. FABRIK emits intents.
Unifying means picking one. Picking intents is the more honest path:

- multi-joint write is naturally an intent batch
- intents already route through the signal pipeline
- spring's inline write is a perf optimization that loses ordering guarantees

The cost: one extra tick of latency for spring chains vs the current path. For
hair / cloth that's invisible. For first-person camera-rig spring (if anyone
does that) it might matter.

---

## What this would replace, in scope order

1. `IKSolver::Fabrik` — current single-chain leash solver
2. `IKSolver::FabrikMulti` — branching leash (per the
   `branching-fabrik-multi-effector.md` task)
3. `QuatSpringComponent` chains — VRM hair / tails / cloth
4. (Stretch) `IKSolver::TwoBoneIK` — though closed-form is genuinely nice; could
   stay as a fast path

Things it would **not** replace:
- `IKSolver::AimConstraint` — pure rotation, no chain
- `QuatSpringComponent` on a single bone (antenna) — degenerate "chain of 1"
  works but is overkill
- `QuatTemporalFilter` and other rotation-shaping ops — those are signal
  filters, not dynamics

So spring and the pipeline don't go away even after unification — they keep the
1-bone shaping ops. The chain stuff moves.

---

## Decision so far

**Don't do this yet.** The leash + Y-leash tasks land first using existing
`Fabrik` / new `FabrikMulti`. They're stepping stones whose internals fold into
this unified solver later. The signal that this design is ready: when the leash
example shows that "chain just sits there frozen between movements" feels
wrong, OR when someone tries to author a skirt with FABRIK and finds there's
no way to express "should resist folding inward".

Until then this stays a wip doc.
