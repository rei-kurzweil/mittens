# Task: Simplify default TOP translation sampling

Date: 2026-10-12
Status: implemented in the working tree; focused automated checks pass.
Headset acceptance in mittens-corp remains pending.

## Implemented contract

`TOP.sample_translation().source(...).axes(...)` copies selected world
translation coordinates from a concrete **structural ancestor T** when normal
transform propagation visits the operator. The source's cached world matrix is
read directly. The sampler neither updates that source nor schedules consumers.

The full incoming linear basis and unselected translation coordinates are
preserved. A downstream T composes its local offset afterward. Sampling changes
the basis supplied to children, not the cached world matrix of its parent T.
The source/axes/root authoring surface and default XYZ mask remain unchanged.

The AVC capsule uses this arrangement:

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

## Ordering and updates

- Ordinary ancestor propagation computes the source before visiting its
  descendants. Locomotion and physics/contact corrections update the capsule
  through that path, including fixed substeps.
- Tracking/body updates visit the sampler with new incoming X/Z and read the
  frame's already-computed Y. HMD Y is replaced on every visit.
- Parent/inverse routing retains its existing behavior. A sampler reads the
  ancestor's current cached world value when those paths visit it; it does not
  traverse their external dependencies or promise to solve routing feedback.
- Direct sample or fork configuration updates rebuild inherited input through
  intervening operators from the nearest ancestor T's cached world matrix.
  Samplers no longer retain a separate input-matrix cache.
- A local write to the first T below a sampler reevaluates that operator path,
  then composes the new local offset. Deeper descendant writes compose from
  their nearest T ancestry as usual.
- Raw local/topology writes without the usual runtime propagation boundary do
  not promise updated world matrices. Callers must trigger the normal update.
  Separate legitimate updates may each visit the sampler; the sampler itself
  produces no settling waves.

## Binding, validity, and compatibility

Keep authored ComponentRef values and a retained resolved handle. GUID sources
use indexed lookup; query sources, including root scopes, must resolve to
exactly one concrete T. World query/topology revision or source/root edits
invalidate the binding. Ordinary typed pose writes retain it; axes-only edits
change evaluation without resolving the source again.

Forward references become observable on a normal evaluation after their source
exists and is attached as an ancestor. Source loss, replacement, or reparenting
invalidates through the world revision/live handle checks. Removal retires the
owner's runtime state. A replacement GUID/query target must satisfy the same
ancestor requirement.

External, sibling, and descendant sources report `UnsupportedSource` and
preserve input. Non-T, missing, or ambiguous sources retain their corresponding
statuses. Selected non-finite coordinates preserve input and can recover on a
later ordinary evaluation. Invalid/pending sampling keeps AVC readiness and
contact disabled so tracked HMD Y cannot become collision Y.

This intentionally removes the original arbitrary external-source contract.
No passive external mode, new builder, generalized scheduler, or cycle solver
is introduced. Ancestor validation is a simple structural walk at binding time;
there is no recursive traversal of sampler/parent/inverse dependencies.

## Removed machinery

- Reverse sample dependency links and changed-translation-source collection.
- Sampling relationship/refresh revisions.
- Observation of unrelated parent/inverse routing settings for sample cycles.
- Recursive sample dependency validation with its 64-level cutoff.
- The 64-wave dependent-refresh loop and its call sites in transform updates.
- Cached sampler input and revision-driven input reuse.
- The three-pass capsule sample settling loop; pending AVC bindings are
  evaluated once before contact gating.

Retained parent/inverse behavior is separate from this simplification. The
[retained source bindings plan](transform-operator-source-bindings-and-invalidation.md)
must keep default sampling outside generalized dependency scheduling.

## Verification

Focused automated tests cover masks and shear-preserving linear basis, retained
queries across pose changes, ambiguity/loss/replacement, scoped and forward
references, unsupported placement, non-finite recovery, configuration edits,
reparented inherited input, downstream offsets, and MMS dump/reload.

Evaluation/resolution counters exist only in test builds. They verify one
sampler evaluation per ordinary ancestor visit, no selector resolution for
unchanged pose updates or mask edits, and no automatic visits for unsupported
external-source motion. A routing fixture exercises parent and inverse-local
boundaries between the ancestor and sampler.

The existing real SystemWorld/contact fixtures cover capsule readiness, floor
and wall correction, gravity/jump support, physical head-height changes,
uniform scale, source recovery, multiple avatars, desktop routing, and repeated
vehicle mount/dismount.

The full library run completed with 982 passed, 51 failed, and 2 ignored.
An isolated checkout of the starting revision had 980 passed, 52 failed, and
2 ignored; every failure in the updated run also occurred in that baseline.
Formatting and executable compilation are checked separately.

Headset acceptance remains with the user: mittens-corp while walking and while
mounted, falling, descending stairs, stepping off the stage edge, and returning
to pedestrian movement. Verify capsule Y stays independent of physical HMD Y
on unchanged support, contact corrects genuine locomotion motion, and vehicle
attachment/contact exclusion remains correct.

## Related work

- [Original axis-selective translation sampling task](transform-sample-translation-axes.md)
- [AVC capsule height independent of HMD tracking](avc-capsule-before-hmd-tracking.md)
- [HMD height / floor-contact feedback](../bugs/mittens-corp-hmd-height-floor-contact-feedback.md)
- [Jump controls and box stacks](mittens-corp-jump-controls-and-box-stacks.md)
- [Runtime mirror resolution panel](runtime-mirror-resolution-info-panel.md)
