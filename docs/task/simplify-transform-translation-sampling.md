# Task: Simplify default TOP translation sampling

Date: 2026-10-12
Status: investigation planned; documentation only. Do not edit `src/` as part of
this task's creation. The runtime described below is still the current
implementation; the simplified contract has not been implemented.

## Intent

Make ordinary `TOP.sample_translation().source(...).axes(...)` a small,
predictable operation during transform propagation. Sampling should not
implicitly opt an operator into arbitrary dependency-chain refresh, graph cycle
validation, or repeated settling passes.

The first consumer is the AVC collision capsule: retain the incoming tracked
body X/Z, replace Y from the pre-tracking locomotion frame, then apply the
calibrated capsule offset. That requirement does not itself call for a general
constraint/dependency scheduler.

Investigate a simpler default contract before changing code. Richer scheduling
may eventually be a separate opt-in mode or operator. Names such as
`TOP.sample_translation_cyclic` or `TOP.sample_translation().cyclic(...)` are
uncommitted possibilities, not API requirements for this work. Supporting
cross-branch dependencies and actually solving cyclic feedback are different
features; neither is in scope now. Do not preserve the current machinery merely
to reserve a future mode.

## Actual capsule requirement

```text
locomotion frame T                         <- sampled world Y
└─ InputXR / tracking T
   └─ AVC / tracked body model root
      └─ TOP.fork_trs().drop("rotation").drop("scale")
         └─ TOP.sample_translation().source(frame).axes("y")
            └─ T(calibrated capsule offset) -> Zone -> Collidable
```

```text
input translation  = [body.x, body.y, body.z]
source translation = [frame.x, frame.y, frame.z]
output translation = [body.x, frame.y, body.z]
```

Preserve the incoming matrix's linear basis and unselected translation
coordinates. The downstream T composes its offset afterward. Sampling edits
the basis passed to children, not the parent T's cached world matrix.

In this arrangement the sampled frame is a structural ancestor. When that
frame changes, ordinary propagation should compute it before visiting the
sampler. When tracking/body pose changes, propagation visits the sampler with
new incoming X/Z and reads the frame's current Y. This needs one evaluation per
visit, rather than repeated waves caused by the sampler itself.

Verify this ordering through actual update paths, including operator boundaries,
physics corrections, mount/dismount, and direct updates to capsule descendants.
Structural ancestry alone must not be assumed to settle every interaction with
existing parent/inverse routing.

## Why the implementation is larger

The [original sampling task](transform-sample-translation-axes.md) allows
arbitrary external source transforms and requires targeted dependent refresh
and cycle handling. The implementation supports chains across separate branches.
These broadened requirements should be reconsidered for the default operator.

Current responsibilities to audit:

- `TransformOperatorSystem::evaluate_translation_sample`: retained source,
  authored settings, validity, and cached incoming basis.
- `sample_depends_on`: recursive validation through ancestry, other samplers,
  and parent/inverse routing, with a 64-level cutoff.
- `translation_dependents` and `changed_translation_sources`: reverse links
  and pose-change tracking for external consumers.
- Sampling relationship revisions and `reference_routes`: invalidation driven
  by other operators' routing settings.
- `TransformSystem::update_translation_sample_dependents`: up to 64 refresh
  waves, including further changes produced by each refreshed subtree.
- `operator_refresh_basis`: cached-input reuse and rebuilding through
  intervening operator boundaries after changes.

Retaining a resolved source handle is useful independently of scheduling a
dependency graph. Ordinary binding invalidation, source lifetime, and reading
live values should not require the wave machinery.

## Default contracts to compare

| Candidate | Source and ordering contract | What to examine |
| --- | --- | --- |
| Ancestor-only sampling | Resolve a concrete ancestor T; read its world translation during normal propagation. Reject unsupported placement with a clear status. | Strong fit for the capsule. Can a simple ancestry check and existing propagation provide the complete required behavior, including routing boundaries? |
| Passive referenced sampling | Read the referenced T's already-computed cached world translation when visited. The sampler neither updates the source nor schedules consumers when it changes. | More flexible sources, but callers must establish ordering and trigger evaluation. Define freshness clearly; avoid silently promising same-update external-source refresh. |

Ancestor-only sampling is the initial candidate to prove against the capsule.
The final decision should state which sources are supported and when changes
become observable. Keep the current source/axes authoring surface where it fits;
do not invent a new builder or rename solely to perform a masked copy.

If passive external references remain supported, specify whether stale cached
values are an accepted part of that explicit contract or whether unsupported
ordering is diagnosed. Do not add general graph traversal to enforce the
passive contract. If freshness cannot be explained or verified cheaply, prefer
the narrower default.

Neither candidate automatically follows external consumers, discovers arbitrary
dependency chains, or iterates to convergence. Minimal source/placement
validation is still appropriate; removing graph cycle checks does not mean
accepting invalid handles or unsupported source relationships.

## Investigation and proposed implementation boundary

1. Trace the generated AVC source selection and all relevant transform-update
   paths. Establish when the locomotion source's world matrix becomes current
   and when the capsule sampler executes, including fixed physics substeps.
2. Inventory authored and generated sampler consumers. Identify which genuinely
   require external-source refresh; separate those from tests introduced solely
   to support the expanded behavior.
3. Select and document the default contract. Record the behavior of forward
   references, queries, source loss/replacement, reparenting, live configuration
   edits, and direct descendant writes. Specify any compatibility changes.
4. Map which state and call sites can be removed under that contract. In
   particular, examine removing reverse sample links, changed-source collection,
   relationship revisions, route observation solely for samples, recursive
   dependency validation, and bounded refresh waves. Preserve responsibilities
   needed by other operator kinds.
5. Retain only the source binding, validity, and incoming-basis state that the
   chosen contract actually needs. Prove that re-evaluation honors intervening
   fork/routing stages rather than reconstructing an incorrect parent basis.
6. Present the proposed code changes and focused verification before beginning
   the implementation phase. No richer opt-in mode is required to complete the
   simplification.

Keep masked world-coordinate copying, finite-value handling, retained reference
resolution outside normal pose evaluation, source lifetime cleanup, and AVC
readiness/contact gating on invalid sources. Invalid sampling must not enable
contact with HMD-driven Y. Review broad revision handling for configuration and
topology edits separately from ordinary pose propagation.

## Acceptance evidence for a later implementation

- Y/XZ/XYZ masks preserve unselected coordinates and the full incoming linear
  basis; calibrated offsets and rotation/scale drops still compose correctly.
- Locomotion translation, gravity/contact correction, and tracked body X/Z
  update the capsule through ordinary propagation. Physical HMD Y does not
  change capsule Y on unchanged support.
- Startup, source loss/recovery, reparenting, mount/dismount, multiple avatars,
  and desktop routing keep the existing required capsule behavior.
- Direct operator/configuration changes and descendant transform updates honor
  the chosen source-freshness and inherited-basis contract.
- Normal sample evaluation does not run recursive dependency validation,
  observe unrelated routing settings, or enqueue sample refresh waves. Pose
  changes do not re-run unchanged source queries.
- Unsupported source relationships have documented, testable behavior; tests
  for arbitrary external chains/cycles are revised according to the new scope.

Use focused runtime tests and the existing real SystemWorld/contact fixtures to
verify the chosen design. Headset acceptance remains part of the capsule work.
Repeated visits caused by separate legitimate transform updates are distinct
from sampler-driven settling waves.

## Relationship to existing plans

This task narrows the intended default behavior described by
[axis-selective translation sampling](transform-sample-translation-axes.md).
Its implementation notes remain an account of current code, not a requirement
to preserve generalized scheduling.

The [retained source bindings task](transform-operator-source-bindings-and-invalidation.md)
also proposes shared scheduling for parent/inverse/sample chains. Revisit its
sampler scope after this investigation: reuse binding/lifetime mechanisms where
useful, but do not make the simple sampler participate in a general scheduler
by default. Existing parent/inverse behavior is a separate concern.

- [AVC capsule height independent of HMD tracking](avc-capsule-before-hmd-tracking.md)
- [HMD height / floor-contact feedback](../bugs/mittens-corp-hmd-height-floor-contact-feedback.md)
- [Transform operator API cleanup](transform-operator-api-cleanup.md)
