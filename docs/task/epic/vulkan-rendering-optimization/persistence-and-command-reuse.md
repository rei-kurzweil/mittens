# Persistence and command reuse plan

Status: design inventory, not an implemented command-cache architecture.

The goal is to stop repeatedly constructing the same work on the CPU. The GPU
must still execute draws for each new view. A retained command buffer can be
submitted again without rerecording it, provided its referenced resources,
recorded state, synchronization and pending-use rules remain valid.

## Layers to retain independently

| Layer | Current behavior / evidence | Candidate persistence | Changes requiring work |
| --- | --- | --- | --- |
| Mesh buffers and pipelines | Renderer retains uploaded meshes and pipelines | Keep current caches | Upload/replacement, destruction, shader or target format changes |
| Material descriptors | `get_or_create_material_set` and material-instance caches exist | Retain stable sets and identify dependencies | Texture/view/sampler replacement, material layout or buffer identity changes |
| Draw topology and batch streams | `prepare_draw_cache()` is called per view; it can reuse its cache | Keep topology independent of moving instance data | Spawn/remove, material/mesh/pass changes, streaming and destruction |
| Transparency order | Multi-layer transparency prepares a per-view order | Retain only when view/order inputs permit | Camera/object movement, visibility and mirror view changes |
| Instance data | Several passes have cached instance buffers; other paths build buffers while preparing a view | Stable per-slot buffers, dirty range uploads | Transforms, colors, poses, capacity/stream changes |
| Camera and light data | Per-view `Buffer::from_data` calls and global descriptor creation are present | Per-frame/view buffers and stable descriptor sets | Data updates each frame; recreate on capacity/layout/resource changes |
| Rig resources | Persistent deformation output exists; rig setup also creates placeholder lighting buffer/set | Retain invariant placeholder resources and appropriate sets | Output buffer replacement, layout changes, slot ownership |
| Deformation | Dirty jobs are recorded before graphics | Keep dynamic compute separate from reusable graphics commands | Pose/morph changes, capacity and mesh ranges |
| Attachments and post-processing | Target-management helpers already retain compatible targets | Keep target reuse and classify per-pass descriptor churn | Extent, sample count, format, graph and output-image changes |
| Draw commands | Scene commands use a fresh primary builder and `OneTimeSubmit` | Reusable secondary commands for a small stable pass first | Any recorded handle, push constant, offset, viewport or other embedded state changes |
| Frame orchestration | XR chooses current output target and submits each eye | Small per-frame primary commands around reusable work | Acquired image, barriers/layouts, dependency graph and frame slot |

This is a starting inventory from source inspection. Measure cache hits,
allocations and invalidations before asserting a cache is effective or wasteful.

## Data updates versus command changes

A camera or transform change does not intrinsically require a changed draw
command. If commands reference stable buffers/descriptors, the data can change
without changing that reference. Updating the memory must still obey CPU/GPU
synchronization. Use frame slots or explicit completion tracking; never overwrite
a buffer while an earlier frame reads it.

Values recorded directly into a command buffer are different: push constants,
dynamic descriptor offsets, buffer handles, draw counts and dynamic state are
baked into that recording. Reusing those commands preserves those values. Move
only suitable changing values into shader-readable buffers as a deliberate
interface change, rather than assuming an old recording will read new constants.

An acquired XR output image also varies. Either keep recordings for applicable
image/slot combinations or keep output attachments and per-frame transitions in
the dynamic primary command buffer. Secondary commands must meet dynamic
rendering inheritance/compatibility rules; they do not bypass resource tracking.

The [build internals capture](build-internals.md) finds synchronization analysis
and finalization consume about 2.376 ms of the 3.311 ms inner build total.
Vulkano 0.35.2 forwards a secondary's buffer/image usage entries into its parent's
analysis. A retained secondary can therefore save draw emission without removing
that dominant parent analysis. Measure both levels and their resource-use counts
when selecting a reuse boundary; also evaluate a compatible complete per-slot
recording where practical. No secondary-cache performance claim is established yet.

## Incremental migration

The updated [next slices](next-steps.md) define acceptance criteria and ordering.
Use the [pass contract design](pass-dependencies-and-synchronization.md) for
explicit synchronization and the [clip program design](clipping-and-command-reuse.md)
for within-pass command groups. The inventory below remains a reusable checklist;
stable buffers/descriptors are enabling work, not the dominant measured CPU cost.

1. Add measurements without changing rendering behavior.
2. Choose one resource family based on measured cost; retain it with explicit
   capacity, dirty-data and in-flight ownership rules.
3. Add cache-hit/rebuild counters to draw streams and separate instance-data
   invalidation from structural invalidation where needed.
4. Select one stable opaque batch group and list every recorded dependency.
   Prototype retained commands there; other passes continue recording normally.
5. Track generations for geometry, pipelines, descriptor/resource identity and
   attachment compatibility. Rebuild only the affected group when those change.
6. Extend to other passes only after CPU/GPU evidence and correctness checks.
   Transparency, mirrors, refraction, streaming and damage need explicit treatment.
7. Establish the lower-level recording/submission integration before piloting
   engine-owned synchronization at declared pass boundaries. Extend from a small
   renderer-owned chain to opaque/deformation work based on evidence.
8. Consider parallel recording for remaining dynamic work. Each worker gets its
   own command pool/allocator ownership; use coarse batches and preserve ordering.

Do not adopt one giant cache whose smallest change rerecords the entire scene.
Do not create one tiny command buffer per object either: recording, execution and
resource-tracking overhead can outweigh reuse. Determine grouping from captures.

## Questions the measurements should answer

- Is most CPU time spent issuing draw-builder calls or in final `build()`?
- How much occurs in allocations, descriptor updates, material sync and uploads?
- Which resources are recreated despite unchanged identities/content?
- Are the same draw streams or instance buffers regenerated for the second eye?
- What fraction of batches remain structurally stable while avatars and trucks move?
- Can retained buffer identities coexist with the current Vulkano resource tracking?
- Would worker recording reduce the dominant stage, or merely move small work?

References: [Vulkan command buffer lifecycle](https://docs.vulkan.org/spec/latest/chapters/cmdbuffers.html),
[Khronos multithreaded recording sample](https://docs.vulkan.org/samples/latest/samples/performance/command_buffer_usage/README.html).
