# Clipping streams and reusable draw programs

Status: proposed internal improvements; preserve current rendering semantics.

Keeping clipping operations in an ordered render stream is useful: stencil
masks are stateful GPU operations, and the order of mask writes and clipped
color draws matters. The optimization opportunity is to compile/reuse that
program and its compatible command ranges, rather than infer dependencies and
reconstruct everything each frame.

## What exists

`visual_world.rs::RenderOp` contains `EnterClip`, `DrawBatch` and `ExitClip`.
`build_phase_render_stream()` emits per-phase operations and an instance order.
`vulkano_cbb.rs::record_phase_stream_draws()` interprets those operations.

- Enter uses clip geometry with an incrementing stencil pipeline and parent
  stencil reference.
- Clipped batches compare against their assigned stencil reference.
- Exit draws clip geometry with a decrementing stencil pipeline.
- Mask operations invalidate the recorder's binding cache so subsequent color
  draws bind the appropriate state again.
- The current renderer locates the stream-local instance slot with
  `stream_instances.iter().position(...)` at both enter and exit.

The cafeteria capture's `sample_draws` has no `stencil` entry. Therefore no mask
geometry was recorded by that counter in the captured view; the measurements do
not identify clipping as the cause of the 1.990 ms synchronization-analysis cost.
Use an additional clipping-heavy UI fixture before assigning it a performance
priority or comparing alternative representations.

## Keep order; compile structure and lookup

Retain the ordered stream as the semantic source. Add a compiled form only if
it avoids repeated work or enables command reuse:

1. Resolve each mask operation's stream-local instance slot when building the
   stream. Store a direct slot or a compact side table indexed by operation.
   Rebuild on stream order/capacity/instance mapping changes. Removing the two
   linear slot searches is a small focused candidate, with no measured gain yet.
2. Mark balanced operation ranges and their entry/exit state. Derive them from
   the actual existing program, preserving multiple mask sources and current
   overlap/depth behavior. Stencil reference/depth alone is not a unique scope ID.
3. Identify unclipped spans and complete clip ranges as candidate recording
   groups. A group records the required pipeline/descriptors, viewport/scissor,
   stencil references and mesh bindings explicitly.
4. Reuse the compiled structure when transforms/colors/camera data change only
   through stable buffers. Rebuild when topology, mesh/material/pass membership,
   references, buffer identity or stream mapping changes.

This can keep a compact flattened operation vector in the hot path. A separate
hierarchical metadata table for cache ownership does not require a runtime tree
walk or per-draw allocation. Draw batching can continue within compatible spans.

## Clip order and synchronization solve different problems

Enter, clipped draws and exit operate on a shared depth/stencil attachment in
an ordered rendering scope. The pass plan declares the depth/stencil accesses
and attachment load/store policy. Stencil increment/decrement/test behavior,
geometry coverage, sample count and references preserve the actual mask semantics.

Do not turn every clip operation into a separate offscreen pass, image allocation
or rendering-scope break. Do not add a general memory barrier between every
mask and draw simply because the CPU stream has a boundary. Introduce any
barrier based on an actual resource access/layout dependency and the Vulkan
rendering-scope rules. Sampling/copying a stencil/depth aspect elsewhere is a
separate declared consumer with its own dependency.

Moving all enters/exits out of the draw order would lose the relationship
between the masks and the color draws they surround. They can be stored in
separate compiled metadata, but the emitted GPU program still preserves their
ordering. The frame graph does not replace this within-pass program.

## Cache boundaries

Start with either an unclipped stable group or an entire compatible clip range.
An entire range is simpler because its entry/exit stencil contract can be
verified together. A changing child can still require that range to rerecord;
measure how often that occurs before introducing smaller nested caches.

Later partial ranges must declare incoming stencil state, required references,
attachment identity/compatibility, load/clear policy, view/slot and outgoing
state. Command-buffer boundaries do not reset the stencil image. Secondary
recording boundaries also do not imply that pipeline/dynamic state is inherited
as our recorder expects; bind the required state explicitly and respect dynamic
rendering inheritance requirements.

A moved clip shape can remain a data update if geometry identity, slot mapping
and references are unchanged and its transform is read from a stable buffer.
A reparented or reordered clipped subtree changes the program and invalidates
the affected range. Deleting an instance must not let a recycled instance slot
silently reuse an old recording.

## Measurement and validation slice

Count stream rebuilds, operations, enter/exit pairs, mask draws, clipped batches,
lookup time, cache hits, rerecorded groups and pipeline/descriptor binds. Record
which invalidation caused each rebuild. Compare CPU recording and final build
costs, not just the time to interpret `RenderOp`.

Validate current nested and overlapping masks, sibling scopes at equal depth,
rotated/scaled clip geometry, empty content, hidden/streamed-out content, overlay
and supported opaque/cutout/transmission clip streams, both eyes and MSAA.
Maintain the current supported-material restrictions; this optimization does
not add skinned clipping or new clip shapes. Also check stencil clears between
passes, view exclusions, post-process occlusion and lifetime after UI teardown.

Keep existing stream-order tests and add focused coverage only for actual
compiled-slot/range/cache behavior when implementing that slice. A rectangular
scissor shortcut, new clip semantics or a different masking algorithm is separate
work, not a prerequisite for reusing current commands.

Related: [next slices](next-steps.md),
[pass dependencies](pass-dependencies-and-synchronization.md),
[retained resources and commands](persistence-and-command-reuse.md).
