# Next optimization slices

Status: proposed implementation order, updated after the build-internals capture.
This document schedules work; it does not add a new renderer or MMS API.

## Objective and measured targets

Reuse stable draw programs, resource bindings and recorded GPU work. Replace
repeated per-command dependency inference with a retained pass dependency plan
where we can express the complete contract. Render the current pose/view every
frame and preserve the existing material, clipping, mirror and post-process
behavior.

The measured inner `build()` contains 1.990 ms synchronization analysis,
0.385 ms resource-usage finalization, 0.748 ms native command emission and
0.176 ms command-metadata cleanup across both eyes. Opaque recording outside
`build()` is another 1.508 ms in the earlier capture. These identify targets;
they are not additive promises of savings or an achievable 2.38 ms speedup.
GPU execution and CPU completion waits are separate from these CPU targets.

## Work order

| Slice | Concrete result | Evidence required before extending |
| --- | --- | --- |
| A. Invalidation and dependency counters | Know which streams/resources change and why | Hit/rebuild/miss counts per eye/pass, topology vs data generations, stencil-op counts |
| B. Retained pass plan | Resolve current material contracts and graph configuration into resource roles and ordered passes | Plan dump agrees with current recorder for bloom, transmission, mirrors and clipping |
| C. Stable bindings for one candidate | Per-slot/view buffer and descriptor identities for a small pass/group | Data still updates; no in-flight overwrite; allocation and descriptor-miss counts |
| D. Command reuse with existing synchronization | Reuse a complete compatible recording and compare retained secondaries | Recordings reused/rebuilt, primary analysis resource count, total CPU/GPU result |
| E. Explicit synchronization feasibility | Prove a bounded lower-level recording/submission path and ownership contract | Correct Vulkano integration, lifetime and pending-use tracking, explicit access/layout boundaries |
| F. Explicit-sync pilot | Small renderer-owned pass chain, with pass-level barriers from the retained plan | Sync validation, output comparison, per-phase CPU and GPU measurements |
| G. Extend to opaque/deformed geometry | Remove repeated access inference from the high-cost stable draw work | Mixed-path dependency tests, correct pose updates, useful measured CPU reduction |
| H. Clip-program improvements | Precompiled slot references and ordered reusable ranges | Nested/overlapping/rotated clip fixtures and independent clip-generation counters |
| I. Parallel dynamic recording | Workers build substantial remaining dynamic groups | Net CPU saving after worker scheduling and main-thread finalization costs |

A and B can be developed independently. C is an enabling step for D and F, not
an allocation optimization justified by a large standalone camera-buffer cost:
that measured setup was only 0.030 ms. H's small slot-lookup improvement can be
considered earlier once a clipping-heavy fixture exists. Major clip redesign
and parallel recording follow measured need rather than block the first reuse
experiment.

## A. Make invalidation observable

Initial instrumentation and captures are recorded in the
[cache invalidation baseline](cache-invalidation-baseline.md). Both stationary
and moving-head baselines rebuilt shared draw structure twice per stereo frame
due to texture updates. The subsequent
[view-local mirror binding fix](view-local-mirror-bindings.md) eliminates those
rebuilds, with two cache hits and seven instance-buffer allocations per frame.
Trace the 36 remaining mutable snapshot allocations before selecting a material-
persistence change. Stable first-eye buffer identities and the retained pass
plan remain enabling work for broader command reuse.

Instrument existing caches before replacing them. `prepare_draw_cache()` returns
early when `dirty_draw_cache` is false; calling it per view does not prove a
rebuild. Count actual rebuilds and the mutation paths that set the dirty flag.

Record topology/material/pass changes separately from transforms, pose, colors,
visibility and per-view transparency sorting. Count instance-buffer allocation,
range updates, capacity changes, descriptor creation and material cache misses.
Include clip enters/exits, clipped batches, slot searches and stream lengths.
Keep counters opt-in or cheap aggregates; add no per-draw logging to normal use.

Acceptance: a stationary run and a moving-avatar run can explain every rebuild.
A pose-only update should be distinguishable from spawn/remove/damage. No new
claim that all pose changes can already avoid structural invalidation.

## B–C. Compile dependencies and stabilize references

Use [pass dependencies and synchronization](pass-dependencies-and-synchronization.md)
as the design inventory. Produce a deterministic internal plan from existing
resolved material capabilities, post-processing configuration, render targets,
view family and clip requirements. Declare resources once per consuming pass
or appropriate range, rather than attach the same declaration to every draw.

Separate the structural plan from its frame bindings and mutable data. Plan
changes include enabling a pass, changing target compatibility or stream
structure; a new camera matrix usually changes buffer contents. Retain suitably
sized per-slot resources and descriptor sets. Keep current pipelines, material
interfaces and shader behavior during the first prototype.

Acceptance: the plan and resource-access dump reproduce the existing recorder's
ordering, including bloom-before-refraction and frozen mirror behavior. Buffer
identity changes have explicit invalidation reasons; content updates obey frame
completion and host-write visibility rules.

## D. Compare two reuse boundaries

Use a small opaque group and stable per-slot bindings. Measure:

1. A reusable complete primary recording for a compatible view/slot/target, when
   its changing data can be supplied through stable buffers. Dynamic deformation
   and output-image selection need an explicit boundary, not accidental omission.
2. Retained secondary recordings inside a changing primary. This can save leaf
   draw recording/emission, but Vulkano 0.35.2 forwards their resource-use entries
   into primary analysis. Measure that cost, not only secondary cache hits.

Changing camera data does not invalidate a recording if the shader sees it via
stable buffers. Embedded push constants, dynamic offsets and attachment handles
are different and must be inventoried. Do not change the XR camera sampling
point or silently introduce another frame of pose latency to gain reuse.

Acceptance: ordinary movement needs data updates while the chosen stable group
keeps its recording; material/mesh/stream/target changes rebuild the correct group.
Report primary and secondary build counts, resource-use entries, CPU recording,
GPU time and live-frame implications separately.

## E–G. Take over synchronization in a bounded area

First prove the recording/submission integration described in the dependency
plan. `RecordingCommandBuffer` omits automatic barrier tracking, but switching
builders is not a complete lifetime/submission implementation in Vulkano 0.35.2.
Decide that boundary before writing a large replacement recorder.

A renderer-owned fullscreen image chain is a useful correctness pilot because
its resource footprint is small and explicit. It validates the mechanism; its
CPU benefit is not a prediction for the opaque scene. Follow with the measured
opaque/deformation path once the pilot preserves synchronization, ownership and
output. Other passes can continue using the current recorder with declared
handoffs at the boundary.

Acceptance: source-to-destination stage/access/layout declarations, frame-slot
reuse, frozen images, upload/deformation producers and submission completion are
all accounted for. Compare the same scene and quality. Use synchronization
validation and rendered output checks; validation captures are separate from
performance measurements. Preserve the current path as an explicit experiment
control until the scoped replacement is verified.

## H–I. Clip programs and parallel work

[Clipping and command reuse](clipping-and-command-reuse.md) preserves the ordered
stream and proposes compiled ranges and direct slot references. The current
cafeteria sample contains no `stencil` mask draws, so it does not establish clips
as a contributor to the 1.990 ms analysis bucket. Use a suitable UI fixture.

Parallel recording should target work still rebuilt after reuse. Command-pool
ownership belongs to each worker; preserve draw/clip order when joining groups.
With automatic primary analysis still present, worker recording alone can leave
the largest serial stage unchanged.

## Completion record for every slice

Record the chosen scope, invariants, changed source modules, invalidation rules,
validation cases, before/after distributions and open limitations in this epic.
No automatic budgeted speedup claims. Keep new modules focused and below the
repository size targets. This epic does not add MaterialBase variants, change
MMS event APIs or introduce Android rendering features.
