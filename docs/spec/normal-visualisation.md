# Normal Visualisation Component

A debug component that reads the normals from a parent renderable's `CpuMesh` and spawns
a child subtree of thin cyan cubes, one per vertex, oriented along each normal.

---

## Usage

Attach `NormalVisualisationComponent` as a child of a `RenderableComponent`. On init it
reads the parent mesh and spawns its visualisation subtree automatically. Removing the
component tears down the subtree via `RemoveSubtree`.

```
TransformComponent
└── RenderableComponent          ← parent renderable (existing)
    └── NormalVisualisationComponent
        └── [spawned subtree]
            ├── TransformComponent + RenderableComponent  ← normal 0
            ├── TransformComponent + RenderableComponent  ← normal 1
            └── ...
```

---

## Spawned Geometry (ノ ᵒ ᵕ ᵒ)ノ

Each vertex in the `CpuMesh` gets one child:

- **Mesh**: `MeshFactory::cube()` (builtin)
- **Color**: cyan — `[0.0, 1.0, 1.0, 1.0]`
- **Emissive**: yes — `EmissiveComponent::on()` so normals are always visible regardless of lighting
- **Scale**: `[t, 10t, t]` where `t` is a configurable thickness (default `0.01`).
  The cube is 10× taller along Y than it is wide, making it read as a line/needle.
- **Position**: `vertex.pos + normal * 5t` — base of the cube sits at the vertex
  surface, tip points away along the normal.
- **Rotation**: Y-axis of the cube aligned to the vertex normal. Computed via the
  standard "rotate [0,1,0] onto normal" quaternion (cross + dot, handle the degenerate
  antiparallel case).

---

## Fields

```rust
pub struct NormalVisualisationComponent {
    /// Thickness of each indicator cube (X and Z scale).
    /// Y scale = thickness * 10.
    pub thickness: f32,

    /// ComponentIds of spawned child transforms, for cleanup.
    spawned_roots: Vec<ComponentId>,
}

impl NormalVisualisationComponent {
    pub fn new() -> Self { ... }
    pub fn with_thickness(t: f32) -> Self { ... }
}
```

---

## Data Flow ＼(＾▽＾)／

Spawning is split across two phases because `Component::init` (the intent-handler path) has
access to `World` and `emit` but **not** to `RenderAssets` (GPU-side mesh data). The actual
vertex reads happen later, in the render-preparation path.

### Phase 1 — intent handler (`RenderableSystem::register_normal_vis`)

Triggered by the `RegisterNormalVis { component_ids }` intent emitted in
`NormalVisualisationComponent::init()`.

1. Walk up the parent chain of the `NormalVisualisationComponent` to find the nearest
   ancestor `RenderableComponent`.
2. Read its `CpuMeshHandle` and `thickness` value.
3. Push `(nv_id, renderable_id, base_mesh, thickness)` onto
   `RenderableSystem::pending_normal_vis` — **no mesh data read yet**.

### Phase 2 — render preparation (`RenderableSystem::spawn_pending_normal_vis`)

Called from `RenderableSystem::flush_pending`, which is invoked by
`SystemWorld::prepare_render` (the render path). At this point `RenderAssets` is available.

1. Drain `pending_normal_vis`.
2. For each entry:
   - **Skip** if `NormalVisualisationComponent` no longer exists (entity removed between phases).
   - **Skip** if `spawned_roots` is already populated (double-init guard).
   - **Defer** (re-push to `pending_normal_vis`) if `render_assets.cpu_mesh(handle)` returns
     `None` — the mesh upload may not have completed yet; will retry next frame.
3. For each vertex in the `CpuMesh`:
   - Compute position: `vertex.pos + normal * half_height` (base sits at surface).
   - Compute rotation: quaternion that rotates Y-axis onto the vertex normal.
   - Spawn `TransformComponent + RenderableComponent(cube, scale=[t, 10t, t]) + ColorComponent(cyan) + EmissiveComponent`.
   - Call `world.init_component_tree(root_id, queue)` — queues the subtree's `init` signals
     into the provided `CommandQueue`.
4. Write the collected root IDs back into `NormalVisualisationComponent::spawned_roots`.

The `CommandQueue` is threaded through `SystemWorld::prepare_render` →
`RenderableSystem::flush_pending` → `spawn_pending_normal_vis` and is drained on the next
engine tick, completing the two-phase spawn cycle.

---

## Init / Cleanup

**`init()`**:
1. Emit `RegisterNormalVis { component_ids: vec![self_id] }` intent.
2. `RenderableSystem::register_normal_vis` handles it (Phase 1 above).
3. Actual subtree spawning happens in Phase 2 (next `prepare_render` call).

**`cleanup()`**:
- Emit `RemoveSubtree` for each ID in `spawned_roots`.

---

## Notes

- **Duplicate normals**: flat-shaded meshes have identical normals per triangle face (3
  vertices share one normal). The visualisation will stack three cubes on top of each
  other at those positions — visually fine for a debug tool, no need to deduplicate.
- **No update path**: the subtree is spawned once on init. If the parent mesh changes,
  remove and re-add `NormalVisualisationComponent` to refresh.
- **MaterialHandle**: uses `TOON_MESH` (no vertex colors needed — uniform cyan via
  `ColorComponent`). No new pipeline required.
- **Debug only**: not intended for production use. Could be gated behind a feature flag
  or simply left as an opt-in component.
