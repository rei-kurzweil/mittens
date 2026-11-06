# Material contracts, pass dependencies and explicit synchronization

Status: proposed internal design; no explicit-sync backend is implemented.

Our renderer knows much more about its work than an arbitrary sequence of
Vulkan commands communicates to the automatic builder. Use that knowledge to
compile a reusable pass plan and synchronization boundaries. Keep a small live
resource-state record for actual frame/view/image bindings and in-flight work;
a retained plan does not eliminate those runtime responsibilities.

## Start with the contracts that exist

`materials.rs::MaterialBaseKind` currently has `Surface`, `ExtractedEmissive`
and `PlanarReflective`. Resolved material capabilities separately describe
transmission (`None`, `Sharp`, `Rough`), emission and cached deformation. This
proposal uses those existing contracts rather than inventing additional public
MaterialBase constructors.

A MaterialBase alone does not specify a frame's full resource dependencies.
The plan must also resolve material texture bindings, geometry/deformation,
render phase and depth/stencil policy, post-processing configuration, active
view family, runtime texture publications and current target ownership.

| Existing contract or renderer feature | Inputs / prior work | Outputs or attachment effects |
| --- | --- | --- |
| Surface geometry | Mesh/instance/camera/light/material data and bound textures; optional deformed vertices | Scene color; depth tests/writes according to phase; optional stencil test |
| Extracted emissive material | Surface inputs and the material's extraction program; scene occlusion/depth as currently used | Extraction image when the configured post-process path consumes it |
| Planar reflection | Per-view reflected camera capture, or retained frozen image | Reflection texture consumed by the reflective surface |
| Sharp transmission capability | Existing scene color/depth snapshot and ordinary material inputs | Transmission draw into scene color, using the existing snapshot boundary |
| Rough transmission capability | Snapshot and generated rough-transmission levels | Rough transmission draw into scene color |
| Cached deformation capability | Uploaded bones/morph/mesh inputs; dirty compute jobs | Storage-buffer writes consumed as vertex data by scene/capture draws |
| Clip operations and clipped batches | Clip geometry/instance data and ordered stencil state | Stencil updates at enter/exit; stencil tests on intervening color draws |
| Emissive extraction / bloom configuration | Extraction image, depth/occlusion policy, optional blur/pyramid intermediates | Sampled bloom result for the configured composite point |
| Deferred overlay / final composite | Scene color, bloom and relevant attachment contents | Final output image and optional published runtime textures |

Texture reads must be resolved to actual resources. A user-selected/runtime
texture can have a producer elsewhere in the graph. Ordinary immutable asset
textures are imported read-only resources after upload completion. No sampler
or shader code inspection shortcut should be assumed to discover arbitrary
producer dependencies; the existing binding interfaces and renderer contracts
must supply the declarations.

## Structural plan versus actual frame state

Proposed internal concepts, not committed Rust or MMS API names:

- **Pass specification:** purpose, view family, input/output resource roles,
  subresource/range coverage, read/write mode, pipeline stages/access classes,
  required image layout and attachment load/store/resolve policy.
- **Compiled plan:** ordered pass schedule, synchronization edges, compatible
  rendering scopes and reusable command-group boundaries. Rebuild when graph or
  compatibility generations change, rather than once per material draw.
- **Frame bindings:** concrete images/buffers, view/eye, frame slot, acquired
  output image and applicable byte/mip/layer/aspect ranges.
- **Live resource state:** previous access/layout/queue owner and completion
  information needed across submissions and frames, including imported images.
- **Retained recording:** commands plus referenced resources, compatibility and
  dependency generations, slot ownership and pending-use/completion tracking.

The plan can coalesce declarations for stable shared read-only resources. Keep
write ranges and alias relationships accurate; view or handle identity alone is
not proof that underlying memory is independent. Initial implementation can
preserve existing allocations and avoid introducing aliasing.

## Example dependency chains

These are configurations, not a universal fixed sequence for every material:

```text
pose uploads -> deformation compute -> scene geometry reads
mirror capture -> reflection texture -> reflective surface
scene color/depth -> snapshot -> sharp transmission
snapshot -> rough transmission levels -> rough transmission
emissive extraction -> blur/levels -> bloom composite
clip enter -> stencil-tested draws -> clip exit  (inside an ordered rendering scope)
```

The existing recorder can composite bloom before refraction when that path is
active and defer overlays until extraction/composite is ready. The plan must
represent those actual branches; placing every composite after all transparent
draws would change the image. Mirror captures have their own camera/exclusion
and output bindings. A frozen mirror has no capture producer in that frame but
its last completed texture remains an input with retained lifetime/state.

## Synchronization at declared boundaries

| Producer to consumer | Required responsibility |
| --- | --- |
| Host writes / uploads to shader inputs | Appropriate host completion/flush/coherency or transfer-write visibility, plus slot ownership |
| Compute deformation to vertex fetch | Compute shader storage writes made visible to vertex input attribute reads; use the actual consumer stage if another interface reads the buffer |
| Color attachment to sampled image | Color-write availability/visibility to consuming shader reads, with the correct image layout and subresource range |
| Depth/stencil attachment to sampling/copy | Actual early/late fragment test accesses to shader/transfer consumption, preserving the sampled/copied aspects |
| Resolve/copy/blur level to next consumer | Model the operation actually used (attachment resolve, transfer or shader draw/dispatch), then its next read/write stage and layout |
| Mask enter/color draw/exit within rendering | Preserve stencil operations, references and attachment test ordering; this is not a new offscreen-image edge per mask draw |
| Previous read to overwrite | Execution dependency for the read/write hazard, and memory/layout requirements where applicable |
| Previous write to overwrite | Write/write dependency even when the next pass discards old pixel contents |
| Final output to XR/compositor or presentation | Respect the external image acquisition/release and submission contract for that output path |

Use compatible precise barriers at pass/resource boundaries, with the renderer's
supported Vulkan synchronization facilities. Resolve access modes from actual
operations, not material names alone. Reading a texture repeatedly does not
require a new read/read memory barrier between every draw; prior producers,
layout transitions and overlapping writes still matter.

Dynamic rendering does not infer the inter-pass dependencies for us. Load/store
and resolve choices remain part of the contract. Clearing a target or selecting
`DontCare` does not authorize racing an earlier use of its memory. Preserve
attachment lifetimes and avoid unnecessary scope breaks or intermediate copies.

## Vulkano integration is a separate feasibility slice

The project currently uses automatic primary builders and future-based
submission. Vulkano 0.35.2 offers lower-level `RecordingCommandBuffer` recording,
but this does not automatically integrate an engine-owned dependency graph with
the existing submission/lifetime machinery.

Source inspection shows `PrimaryCommandBufferAbstract` requires state and
`CommandBufferResourcesUsage`; the latter's detailed fields are crate-private.
A raw command buffer is therefore not a drop-in replacement for the current
automatic command buffer/future chain. Determine which integration is viable:

1. Reuse existing auto-built compatible primaries and their retained tracking.
2. Use a supported lower-level recording/submission boundary with engine-owned
   completion, access and lifetime tracking for the scoped experiment.
3. Evaluate a narrowly scoped upstream/integration API if a measured need cannot
   be met through supported interfaces; document it as separate dependency work.

Do not declare an empty resource-usage description while retaining future-based
safety assumptions. For a mixed path, define exactly which side owns transitions,
resource retention, pending use and completion at each handoff. Avoid retaining
full per-draw inference in the parent if the experiment's purpose is to replace
that cost. Any required dependency integration belongs to a separately scoped
implementation slice; this document selects no dependency fork.

## Invalidation and validation

Structural plan changes include pass enablement, material capability changes,
transmission mode, view/capture family, target format/extent/sample compatibility
and clip-scope structure. Concrete recorded-command invalidation also includes
buffer/descriptor identity, capacity/offsets, geometry, embedded constants,
output image and attachment compatibility. Changes to camera/pose/material data
can remain buffer updates only where the interface already permits it.

Check shader-provided emission, extraction with depth occlusion, cutout coverage,
transmission snapshots, rough levels, nested stencil clips, frozen/resumed mirrors,
XR both-eye targets, streaming/destruction, resized resources and multiple frames
in flight. Synchronization validation and output comparisons establish correctness;
non-validation runs establish timing. Measure retained resource memory as well as
CPU time. Keep this work separate from Android renderer feature development.

References:
[Khronos synchronization examples](https://docs.vulkan.org/guide/latest/synchronization_examples.html),
[Vulkan command-buffer lifecycle](https://docs.vulkan.org/spec/latest/chapters/cmdbuffers.html),
[Vulkano recording API](https://docs.rs/vulkano/0.35.2/vulkano/command_buffer/struct.RecordingCommandBuffer.html),
[Vulkano command-buffer submission traits](https://docs.rs/crate/vulkano/0.35.2/source/src/command_buffer/traits.rs).
