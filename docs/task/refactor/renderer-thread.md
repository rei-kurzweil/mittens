# Renderer thread refactor

Goal: move command buffer recording + queue submission (and associated Vulkano CPU time) off the main/simulation thread, without breaking the current `VisualWorld` + `RenderAssets` architecture.

This is an outline / design scratchpad, not an implementation plan yet.

## Current shape (today)

`Universe` owns:

- `world: ecs::World`
- `systems: ecs::SystemWorld`
- `visuals: graphics::VisualWorld`
- `render_assets: graphics::RenderAssets`
- `renderer: graphics::VulkanoRenderer`

Key coupling points:

- `SystemWorld::prepare_render(...)` flushes pending renderables by:
  - importing glTF meshes/textures
  - uploading meshes/textures via `RenderAssets` using `uploader: &mut dyn RenderUploader`
  - inserting GPU-ready instances into `VisualWorld` (`GpuRenderable { mesh: MeshHandle, ... }`)

- `VisualWorld` is already “renderer-friendly” and contains:
  - `instances: Vec<VisualInstance>`
  - cached per-phase orders + batches (`draw_order`, `draw_batches`, `overlay_*`, `transparent_*`, etc.)
  - shared `bones_palette` (skinning data), with dirty flags

- `RenderAssets` bridges CPU meshes (`CpuMeshHandle`) to GPU meshes (`MeshHandle`) and currently uploads synchronously via a `MeshUploader`.

Implication: today, the simulation thread can trigger **GPU uploads** as part of renderable flushing.

## Non-goals / constraints

- We shouldn’t add implicit backwards-compat shim types.
- Rendering must not read mutable ECS state directly.
- Avoid long-lived locks between threads (prefer single ownership + message passing).
- Avoid per-frame giant clones if possible; but accept a “simple first step” if it’s measurably good.

## What we want after the refactor

- The main thread does gameplay/ECS and produces **render-ready facts**.
- The render thread owns Vulkano objects, GPU resource tables, futures, and swapchain/XR submission.
- Communication is explicit and bounded (backpressure strategy is a design decision).

A useful mental model:

- main thread: *build/simulate + decide what to draw*
- render thread: *turn that into Vulkan work + submit*

## Terminology

- **Render packet**: an immutable, owned message describing a frame (or part of a frame) to render.
- **Render command stream**: a sequence of smaller messages that mutate render-thread-owned state.

## Option A — “Frame packet snapshot” (fastest path to first win)

Main thread keeps owning `VisualWorld` and `RenderAssets`.

Each frame:

1. `systems.prepare_render(world, visuals, render_assets, uploader)` happens on main thread.
2. main thread builds a `RenderFramePacket` snapshot from `VisualWorld`.
3. send packet to render thread.
4. render thread records command buffers + submits.

Packet contents (sketch):

- frame id, camera target(s), extents
- per-view camera matrices (`VisualCamera` / `CameraData`)
- per-phase orders + batches (`draw_*_order`, `*_batches`)
- `instances` (or a tightly packed “GPU instance data” array)
- bones palette (or dirty flag + full palette copy)

Pros:

- minimal architectural change to systems
- render thread becomes a pure consumer

Cons:

- `RenderAssets` still calls into the renderer for uploads, which is incompatible with “renderer lives on another thread” unless:
  - we keep a renderer instance on the main thread (defeats the purpose), or
  - we add a proxy uploader that enqueues uploads and returns handles immediately (see “Asset boundary” below)
- snapshot copying cost can be high if we copy `instances`, `bones_palette`, orders, etc. every frame

When Option A makes sense:

- first milestone: decouple *submission* (`then_signal_fence_and_flush`) and/or waiting from sim thread
- acceptable to do some copying initially to validate the approach

## Option B — “Renderer owns VisualWorld” via command stream (cleanest long-term)

Render thread owns:

- `VulkanoRenderer`
- GPU asset tables
- **the authoritative `VisualWorld`**

Main thread owns ECS/world and sends **commands** describing mutations that should happen to render state.

Example commands:

- `CreateInstance { id, renderable_cpu: Renderable, initial_transform, flags }`
- `UpdateTransform { id, matrix_world }`
- `SetOpacity { id, opacity }`
- `RemoveInstance { id }`
- `UpdateSkinMatrices { instance_id, bones_base, bones_count, matrices }` (or per-rig update)
- `PrepareDrawCache` / `BeginFrame` / `RenderFrame`

The render thread:

- resolves CPU handles to GPU handles (uploads meshes/textures)
- updates `VisualWorld`
- builds cached draw batches/order when dirty

Pros:

- no per-frame cloning of big `VisualWorld` vectors
- renderer thread is the only place that touches Vulkano/GPU
- asset uploads become natural (renderer thread owns uploader)

Cons:

- bigger refactor: systems currently write directly into `VisualWorld`
- we need stable instance ids across threads (existing `InstanceHandle` can be that)

When Option B makes sense:

- we want the renderer thread to truly be independent
- we’re willing to route “visual mutations” through a queue (similar to how intents are routed)

## Option C — Hybrid: double-buffered snapshots (reduce copying)

If we want a snapshot model but avoid per-frame allocations/copies:

- keep `VisualWorld` (and/or just the heavy arrays) in a double buffer
- produce `Arc<[T]>` slices for:
  - instance GPU data
  - per-phase draw order
  - bones palette (or per-slot bones buffer data)

Main thread writes into buffer A while render thread reads buffer B.

Pros:

- keeps consumer model
- lower copying / allocation churn

Cons:

- trickier invariants (what is allowed to mutate when?)
- still leaves the `RenderAssets` upload problem unless paired with the proxy boundary below

## The hard part: the asset upload boundary (`RenderAssets` + `MeshUploader`)

Today:

- `RenderAssets::gpu_mesh_handle(uploader, cpu_mesh)` uploads synchronously and returns a `MeshHandle`
- `VisualWorld` stores `GpuRenderable { mesh: MeshHandle }`

If the renderer is on another thread, there are 2 broad approaches:

### Boundary approach 1: allocate handles immediately + upload asynchronously

Because `MeshHandle`/`TextureHandle` are lightweight `u32` ids, we can:

- allocate a new handle on the main thread the first time a CPU mesh is referenced
- enqueue `UploadMesh { handle, cpu_mesh_data }` to render thread
- return the handle immediately

Render thread fills in `handle -> actual GPU buffers` map.

Key design decision: what happens if something tries to draw a handle that hasn’t uploaded yet?

- block render thread until uploaded (can stall frame)
- render a placeholder mesh/texture
- skip draw until ready

### Boundary approach 2: move RenderAssets (or at least GPU caching) to render thread

Split `RenderAssets` into:

- CPU registry (meshes decoded/imported): can live on main thread
- GPU cache (uploads + `CpuMeshHandle -> MeshHandle`): lives on render thread

Main thread sends `EnsureGpuMesh { cpu_mesh_handle }` commands; render thread resolves.

This pairs naturally with Option B (renderer-owned VisualWorld).

## Skinning considerations (SkinnedMeshSystem)

Skinned mesh updates currently flow through `VisualWorld::bones_palette` and dirty flags.

Threading-friendly packets/commands should avoid “copy full palette every frame” if it’s large.

Possible shapes:

- send full palette only when `take_bones_palette_dirty()` is true (still can be big)
- track dirty ranges per rig and send per-range updates
- move skinning palette ownership to render thread and send only joint matrices per rig

Note: even if the CPU time hotspot was previously “skinning meshes”, the bigger win is often removing GPU sync (`wait(...)`) and reducing submit count. The threading design should keep those knobs open.

## Render-phase parallelism (thread pool)

Once the renderer is off-thread, there are two separate forms of parallelism:

1) **frame-level pipelining**: sim thread and render thread overlap
2) **recording parallelism**: multiple workers record command buffers

A reasonable progression:

- Phase 1: single render thread, no pool
- Phase 2: record secondary command buffers per phase in a small thread pool
  - e.g. opaque/background/cutout/transparent/overlay
  - primary CB becomes small “execute secondary buffers” + present

Caveat: Vulkano allocators/caches may introduce lock contention; record-per-phase helps only if command recording is genuinely large.

## Backpressure strategy

If the render thread falls behind, what should happen?

- **Block** sim thread until packet is consumed (stable but can tank responsiveness)
- **Drop** older packets and keep only the newest (keeps input responsive, can skip frames)
- **Queue** up to N packets then block (bounded memory)

For an editor, “drop old frames, keep latest” is usually acceptable.

## Suggested next step (minimal invasive)

1) Decide between Option A (snapshot) vs Option B (command stream) as the target.
2) If we want a quick experiment, implement Option A with a deliberately small packet (camera + already-built per-phase draw lists + instance GPU data), and leave asset uploads on main thread for the first milestone.
3) Then tackle the asset upload boundary (proxy uploader or move GPU cache).

## Open questions

- Should the renderer thread own swapchain/XR swapchain objects end-to-end?
- Do we need render-thread → sim-thread feedback (e.g. surface resize, XR session state)?
- What is the minimum set of data from `VisualWorld` we can snapshot without copying everything?
