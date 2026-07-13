# Dense Scene Editor Raycast Scalability

Date: 2026-07-12

Status: open

## Investigation status: 2026-07-12

The initial assumption that 5,184 terrain raycast entries caused the steady-state slowdown was
not supported by profiling.

Confirmed fixes and measurements:

- The BVH pending queue was not flushed at runtime, so raycasts could fall back to brute-force
  scans. `BvhSystem::tick()` now flushes pending work.
- A valid BVH miss incorrectly triggered a brute-force scan. Fallback now occurs only when no BVH
  index exists.
- The terrain produces about 5,370 total BVH shapes in `bisket-vr-demo`.
- Initial BVH builds measured approximately 6-8 ms in the latest release run.
- Steady state refits 35 moving renderables per frame at approximately 0.025 ms average and
  0.094 ms maximum in the captured run.
- The captured XR interval issued zero scene rays, so terrain ray queries contributed no time to
  that run.
- CPU simulation measurements remained small: animation approximately 0.006 ms, skinning
  approximately 0.35 ms, and the combined AVC/body-follow/IK block approximately 1.1 ms per frame.

The dominant measured XR costs were instead:

- per-eye rendering: approximately 8.2-9.2 ms per frame
- OpenXR frame submission: commonly 0.4-1.5 ms, with intervals around 2.4-2.9 ms
- total `render_xr`: approximately 9.2-12.3 ms per frame

Conclusion: the current terrain representation is not the observed steady-state bottleneck. Dense
scene optimization remains useful future work, but it should not be used to explain or fix the
current XR smoothness problem without new evidence.

This clears terrain raycasting, not terrain rendering. The measured per-eye render stage includes
drawing the 5,184 terrain cubes for each eye, and the window camera renders the scene again. The
next controlled comparison should run the same VR scene with only `voxel_terrain()` disabled while
keeping the avatar, XR session, cameras, and editor UI unchanged. Window rendering should also be
timed separately so total frame cost can be divided among simulation, two XR eyes, and the desktop
view.

### 72 by 72 versus 36 by 36 VR-only comparison

`voxel_terrain` now accepts `{ length, width }`; only `bisket-vr-only-example` was changed to
`{ length = 36, width = 36 }`. The original VR and desktop demos retain the 72 by 72 default.

The reduced run confirmed:

- BVH shapes fell from approximately 5,379 to 1,476.
- initial BVH build time fell from roughly 6-8 ms to roughly 2 ms.
- eye rendering improved modestly from roughly 9.4-10.1 ms to 8.9-9.9 ms.
- XR submission remained variable and commonly blocked for 7-12 ms.
- total XR render commonly remained around 17-22 ms.

The comparison also exposed unrelated global ECS scans:

- skinning fell from approximately 0.51 ms to 0.26 ms
- the combined AVC/body-follow/IK block fell from approximately 1.78 ms to 0.70 ms

Terrain has no semantic relationship to skinning or IK. These reductions occur because those
systems enumerate `world.all_components()` every frame and filter by component type. They should
cache registered component IDs, consistent with the engine's caching philosophy. This is a
separate CPU scalability issue from BVH maintenance and XR rendering.

### Related XR correctness finding

`InputXRGamepadSystem` locomotion is not connected for the topology authored by both
`bisket-vr-demo.mms` and `vr-input.mms`:

- authored topology: `Editor -> InputXR -> InputXRGamepad` and `InputXR -> driven Transform`
- current resolver searches for a transform ancestor above `InputXR`
- there is no such transform in either example, so locomotion has no target

This should be tracked/fixed as an XR locomotion topology issue. It is separate from BVH and dense
terrain performance.

## Problem

Editor interaction now reaches raycastable objects outside `Editor {}` trees. This fixes the
semantic boundary: `Editor {}` controls what appears in editor UI, not which world surfaces tools
can use.

In `bisket-vr-demo`, however, making `voxel_terrain()` raycastable exposes a performance problem.
The terrain contains a 72 by 72 grid, or 5,184 independently rendered cubes. Registering all of
them as raycast targets makes the terrain usable by selection and the 3D cursor, but frame rate
drops substantially.

We need dense world geometry to support editor tools without requiring every visible renderable to
be an independently maintained, per-frame raycast target.

## Product distinction

These capabilities should not be treated as identical:

1. **Surface interaction**
   - place the 3D cursor on a surface
   - paint or free-draw onto a surface
   - place objects using a hit point and surface normal
   - does not require the hit renderable to become an inspector/gizmo target

2. **Object selection**
   - resolve a semantic object from a hit
   - update inspector state
   - attach a transform gizmo
   - may require stable object identity and an editable transform

3. **Fine-grained element selection**
   - select one voxel, instance, triangle, or other sub-element
   - requires an explicit editing model and should not follow automatically from surface picking

The first milestone only needs surface interaction for dense terrain. Selecting and attaching a
gizmo to each terrain cube can remain a later capability.

## Current path

- `RaycastableComponent` topology determines whether a renderable is eligible.
- `SystemWorld::refresh_raycastable_bindings(...)` revisits descendant renderables when an ancestor
  raycastable is registered.
- `BvhSystem` stores an AABB entry per eligible renderable and refits against current transforms.
- `RayCastSystem` performs broad-phase queries and shape-specific narrow-phase tests.
- `GestureSystem` chooses click and drag targets from the sorted hit list.
- editor selection and cursor systems consume the resulting `Click`.

Relevant code:

- `assets/components/floors/voxel_terrain.mms`
- `src/engine/ecs/system/system_world.rs`
- `src/engine/ecs/system/bvh_system.rs`
- `src/engine/ecs/system/raycast_system.rs`
- `src/engine/ecs/system/gesture_system.rs`
- `src/engine/ecs/system/editor_scene_hit.rs`
- `src/engine/ecs/system/cursor_3d.rs`

## Options

### 1. Static BVH entries

Mark dense terrain branches as static and avoid refitting their entries every frame.

Advantages:

- preserves exact per-cube hits and existing renderable identity
- relatively small change to the current architecture
- useful beyond terrain for other static scenes

Costs:

- still stores and traverses thousands of entries
- needs correct invalidation when a static subtree changes
- does not reduce initial BVH construction cost

### 2. Chunked proxy geometry

Create one raycast proxy per terrain chunk rather than one per rendered cube. Narrow phase can
resolve the exact voxel or surface inside the selected chunk.

Advantages:

- substantially fewer BVH entries
- supports exact hit points and optional voxel identity
- maps naturally to terrain generation and streaming

Costs:

- introduces a separate proxy-to-semantic-target mapping
- requires terrain-specific or extensible narrow-phase logic
- chunk size affects update cost and query precision

### 3. Combined terrain mesh or collider

Raycast against a combined terrain surface/collision representation while rendering cubes or
instances separately.

Advantages:

- surface interaction does not scale with renderable count
- cleanly separates rendering from tool collision
- likely sufficient for cursor, paint, and placement

Costs:

- combined geometry must be generated and updated
- selecting an individual cube requires a secondary coordinate lookup
- collision and editor-raycast representations may need different precision

### 4. Instanced renderable with instance-aware hits

Represent repeated cubes as one instanced renderable and return an instance index from raycast.

Advantages:

- improves rendering and raycast representation together
- retains per-instance identity when needed

Costs:

- larger renderer, BVH, signal-payload, and editor-selection change
- instance transforms and updates need a dedicated data model
- excessive for the cursor-only milestone

### 5. Reuse physics collision queries

Use the terrain's collision representation for editor surface queries.

Advantages:

- avoids maintaining duplicate spatial acceleration data
- terrain already authors static collision shapes

Costs:

- physics queries may not expose the required renderable/semantic identity or surface frame
- tool behavior becomes coupled to physics configuration
- not every editable surface necessarily has collision enabled

## Recommended staged direction

Do not begin terrain proxy/chunking work solely for the current performance report. Preserve the
options below for future scale tests where ray count, BVH maintenance, or memory measurements show
an actual bottleneck.

### Milestone 1: cursor-only dense surfaces

- introduce a surface-query path that can return hit point, normal, and a coarse semantic owner
- allow `Cursor3dSystem`, paint, and placement tools to consume it without selecting a renderable
- use a combined or chunked static terrain proxy in `voxel_terrain`
- keep terrain cubes out of per-renderable editor selection unless explicitly requested

### Milestone 2: static scene optimization

- distinguish static and dynamic raycast targets
- stop refitting unchanged static targets every frame
- measure BVH build, refit, query, and narrow-phase costs independently

### Milestone 3: optional fine selection

- define whether a terrain click selects the terrain owner, chunk, or individual voxel
- add explicit hit metadata such as instance/voxel index rather than inferring identity from a
  renderable component
- only attach a gizmo when the resolved semantic target has an editable transform

## Measurement requirements

Before choosing the final representation, capture:

- frame time with terrain raycasting disabled
- frame time with 5,184 per-cube entries
- BVH entry count and build time
- static/dynamic refit time per frame
- ray query and narrow-phase time for desktop and XR pointers
- memory used by dense raycast entries

Test at minimum:

- cursor movement over terrain while the pointer is idle and active
- free draw across terrain
- object placement on terrain
- ordinary selection and gizmo interaction with non-terrain objects
- terrain regeneration or mutation, if supported

## Acceptance criteria

- Terrain outside `Editor {}` is usable by 3D cursor, paint, and placement tools.
- Dense static terrain does not cause a substantial idle-frame regression.
- World-panel contents remain based on `Editor {}` ancestry.
- Surface interaction does not automatically imply per-voxel inspector or gizmo selection.
- Gizmo handles continue to win interaction priority over ordinary scene surfaces.

## Related tasks

- `docs/task/editor-input-routing.md`
- `docs/task/shared-3d-cursor-and-selection-vs-surface-placement.md`
- `docs/bugs/raycast-and-bvh.md`
