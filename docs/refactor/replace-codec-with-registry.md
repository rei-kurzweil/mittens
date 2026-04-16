# Replace ComponentCodec with MMS-Native Serialization

## Goal

Delete `component_codec.rs` and the JSON `encode`/`decode` path. Replace with:

1. `fn encode_mms(&self) -> String` on the `Component` trait — emits valid MMS body text
2. Scene save/load via `.mms` files, parsed through the existing `MeowMeowRunner` pipeline
3. `attach_clone` reimplemented as a direct tree-walk clone (no serialize round-trip)

Serialization is temporarily broken during this refactor. That is acceptable.

---

## Current Callers of ComponentCodec (all must be handled)

| Callsite | File | Purpose |
|---|---|---|
| `ComponentCodec::decode_scene` | `main.rs:298` | Load `.json` scene file at startup |
| `ComponentCodec::encode_scene` | `main.rs:344` | Save scene to `.json` on request |
| `ComponentCodec::encode_subtree_node` | `repl_backend.rs:361,380,389` | REPL scene serialization |
| `ComponentCodec::encode_subtree_node` + `decode_subtree_node_with_new_guids` | `intent_executor.rs:190,199` | `AttachClone` intent — clone a prefab subtree |
| `component_codec::Scene` | `repl_backend.rs:389` | Build JSON scene payload |

---

## Step 1 — Replace `attach_clone` (unblocks codec deletion)

`attach_clone` currently: encode subtree → JSON node → decode with fresh GUIDs → spawn.

Replace with a direct tree-walk clone on `World`. Most components already derive `Clone`.

Add to the `Component` trait:

```rust
/// Clone this component's state into a new heap allocation.
/// Default impl panics — components that don't support clone should remain uncloneable.
fn clone_box(&self) -> Box<dyn Component>;
```

Add a blanket helper macro for components that derive `Clone`:

```rust
macro_rules! impl_clone_box {
    ($t:ty) => {
        fn clone_box(&self) -> Box<dyn Component> {
            Box::new(self.clone())
        }
    };
}
```

Add `fn clone_subtree(world: &World, root: ComponentId) -> Vec<(ComponentNode, Option<ComponentId>)>` to `World` (or a free fn). Walk the subtree depth-first, call `node.component.clone_box()` on each, produce a flat list of `(new_node, parent_id_in_new_tree)`. Spawn them into the world with fresh GUIDs.

Remove `AttachClone` intent handler's dependency on `ComponentCodec`.

**What blocks this:** Components that don't derive `Clone` (check: `ActionComponent` has `ComponentId` references inside intent payloads — those are ephemeral, clone is safe but semantically the action target may be stale after clone. Document this, don't block on it).

---

## Step 2 — Add `encode_mms` to `Component` trait

```rust
/// Emit the MMS body for this component — the part inside `{ }`.
///
/// Return value is zero or more newline-separated MMS statements:
///   - `.call(args)` for builder-style configuration
///   - `key = value` for named assignments
///   - empty string for zero-state marker components
///
/// The ctor line (type name + constructor method + args) is NOT included here —
/// that is the responsibility of the tree-walking emitter which knows context.
fn encode_mms(&self) -> String {
    String::new()
}

/// The MMS constructor expression for this component, e.g. `"R.cube()"` or `"Bloom"`.
/// Default: just the component's canonical type name.
fn encode_mms_ctor(&self) -> String {
    // Subclasses override this when ctor carries state (Color, Renderable, Scrolling, etc.)
    // Default: bare type name, no ctor args.
    // The tree emitter will use shortforms from COMPONENT_SHORTFORMS if available.
    String::new() // means: use type name only
}
```

Two methods because MMS separates the constructor line from the body:

```
// ctor line (encode_mms_ctor):    Bloom
// body (encode_mms):              intensity(0.95)\nradius_ndc(0.06)\nhalf_res(true)
```

vs

```
// ctor line (encode_mms_ctor):    C.rgba(1.0, 0.0, 0.5, 1.0)
// body (encode_mms):              (empty — all state in ctor)
```

---

## Step 3 — Tree-walking MMS emitter

A free function (not on `ComponentCodec`):

```rust
pub fn encode_subtree_mms(world: &World, root: ComponentId, indent: usize) -> String
```

Algorithm:
1. Get `ComponentNode` for `root`
2. Get type name → look up shortform via `shortform_for_component(type_name)`
3. Call `component.encode_mms_ctor()` — if non-empty, emit `TypeName.ctor_result` else `TypeName`
4. If node has `name` set, emit `name = "label"` inside body
5. If node has `classes`, emit `class = "foo bar"` inside body
6. Call `component.encode_mms()` for body lines
7. Recurse into children with indent+1
8. Wrap in `{ }` if body non-empty

GUID embed: add `guid = "550e8400-..."` as a named assignment in the body. The MMS parser currently ignores unknown named assignments (they fall through `apply_named_assignment` with a warning). We need to handle `guid` there — store it on `ComponentNode` after spawn. This is how scene identity is preserved across save/load.

---

## Step 4 — MMS scene save/load in `main.rs`

Replace:
```rust
ComponentCodec::decode_scene(&mut universe.world, filename)  // JSON
ComponentCodec::encode_scene(&world, &root_ids, filename)    // JSON
```

With:
```rust
MeowMeowRunner::eval_file(filename)  // load .mms scene (already exists)
// + feed intents into universe

encode_scene_mms(&world, &root_ids)  // new: emit .mms string, write to file
```

Scene files change extension from `.json` to `.mms`. Old `.json` files are not migrated (they're gone with the codec).

---

## Step 5 — REPL backend

`repl_backend.rs` uses `encode_subtree_node` + `Scene` to package component trees for the REPL wire protocol. Replace with `encode_subtree_mms` output. The REPL protocol changes from JSON payload to MMS text payload — update both ends.

---

## Step 6 — Delete `component_codec.rs`

Remove:
- `src/engine/ecs/component_codec.rs`
- `pub mod component_codec` from `src/engine/ecs/mod.rs`
- `pub use component_codec::ComponentCodec` from `src/engine/ecs/mod.rs`
- `pub use component_codec::ComponentNode` if present (it isn't — `ComponentNode` is in `component/mod.rs`)

Remove from `Component` trait:
- `fn encode(&self) -> HashMap<String, serde_json::Value>` — delete
- `fn decode(&mut self, data: &HashMap<String, serde_json::Value>)` — delete

Remove from all component files the `encode`/`decode` implementations (they become dead code after the trait methods are removed). This can be done mechanically with a script — all impls of the old signature.

---

## Step 7 — Per-component `encode_mms` / `encode_mms_ctor`

Implement for all components. Priority order matches what's used in `.mms` examples today:

**Ctor-carries-state (need `encode_mms_ctor`):**
- `RenderableComponent` — `R.cube()`, `R.sphere()`, `R.plane()`, etc. (derive from `base_mesh` u32 → mesh kind string; add a `mesh_kind: MeshKind` field to make this lossless)
- `ColorComponent` — `C.rgba(r, g, b, a)`
- `ScrollingComponent` — `Scrolling.new(vh, ch)`
- `GLTFComponent` — `GLTF.new("uri")`
- `ControllerXRComponent` — `ControllerXR.new(enabled, hand, pose)`
- `TextureComponent` — `Texture.uri("path")` / `Texture.render_image("name")`
- `AnimationComponent` — `A.looping` / `A.playing` / `A.paused`
- `KeyframeComponent` — `KF.at(beat)`

**Body-only (just `encode_mms` body calls):**
- `TransformComponent` — `.position(x, y, z).scale(sx, sy, sz).rotation(rx, ry, rz)` (fix: encode TRS fields not baked matrix)
- `BloomComponent` — `intensity(v)\nradius_ndc(v)\nhalf_res(bool)`
- `DirectionalLightComponent` — `intensity(v)`
- `PointLightComponent` — `intensity(v)\ndistance(v)`
- `StyleComponent` — full flex/size state (currently partial — make complete)
- `AvatarControlComponent` — all builder fields
- `InputComponent` — `speed(v)`
- `InputTransformModeComponent` — `fps_rotation()\nroll_axis_y()` etc.
- ... (all others with non-trivial state)

**Zero-state markers (empty default is correct):**
- `OverlayComponent`, `TransformMapTranslationComponent`, `TransformMapRotationComponent`, `TransformMapScaleComponent`, `StencilClipComponent`, `BackgroundComponent`, `EmissivePassComponent`, `TransformPipelineComponent`, `TransformForkTRSComponent`, `TransformMergeTRSComponent`, `TransformPipelineOutputComponent`, `TransformDropComponent`, `PointerComponent`, `InspectorPanelComponent`, `WorldPanelComponent`, `SelectableComponent`

---

## TransformComponent TRS Fix (Required for Correctness)

Current `encode` stores `model` matrix. The matrix is baked from TRS but TRS is not recoverable from it in general (well, it is via decomposition, but we shouldn't have to). The fix:

Store TRS on `TransformComponent`, not the baked matrix:

```
encode_mms: .position(tx, ty, tz).scale(sx, sy, sz)
            + .rotation(rx, ry, rz)  if rotation != identity
```

On decode: `apply_call` already handles `.position`, `.scale`, `.rotation` — this path exists in `component_registry.rs` `apply_call`. So decode goes through the existing registry, no new code.

---

## RenderableComponent Mesh Kind Fix (Required for Correctness)

Current encode stores raw `CpuMeshHandle(u32)`. This is a runtime slot index, not stable.

Fix: add `mesh_kind: Option<MeshKind>` to `RenderableComponent`:

```rust
pub enum MeshKind { Cube, Sphere, Plane, Square, Triangle, Circle2d, Tetrahedron }
```

Set it in the constructors (`cube()`, `sphere()`, etc.). `encode_mms_ctor` emits `R.cube()` from `mesh_kind`. Dynamic meshes (GLTF-spawned) have `mesh_kind = None` and emit a comment `// dynamic mesh — not serializable` or are omitted.

---

## GUID Handling for Scene Identity

For scene round-trip (save→load preserves node identity for cross-references like `Action.update_transform`):

- `encode_mms` on each component emits `guid = "uuid-string"` in the body
- `apply_named_assignment` in `component_registry.rs` handles `"guid"` — stores it on the `ComponentNode` after creation
- Fresh user-authored MMS (no `guid` field) gets a new UUID as today

This is the only mechanism needed. No other changes to `ComponentNode` or `World`.

---

## What Is Temporarily Broken

Between Step 1 completion and Step 7 completion:

- `--load-scene` / `--save-scene` CLI flags non-functional (comment out or gate behind a feature flag)
- REPL scene serialization non-functional
- `attach_clone` works from Step 1 onward (not blocked by the rest)

---

## Files Changed

| File | Action |
|---|---|
| `src/engine/ecs/component_codec.rs` | **Delete** |
| `src/engine/ecs/component/mod.rs` | Remove `encode`/`decode` from trait; add `encode_mms`, `encode_mms_ctor`, `clone_box` |
| `src/engine/ecs/component/*.rs` | Remove old impls; add `encode_mms`/`clone_box` per component |
| `src/meow_meow/component_registry.rs` | Add `guid` handling in `apply_named_assignment` |
| `src/engine/ecs/mod.rs` | Remove codec re-exports |
| `src/engine/ecs/rx/intent_executor.rs` | Replace `AttachClone` codec path with tree-walk clone |
| `src/engine/universe.rs` | `attach_clone` delegates to new tree-walk clone |
| `src/main.rs` | Replace `decode_scene`/`encode_scene` with MMS runner + emitter |
| `src/engine/repl/repl_backend.rs` | Replace `encode_subtree_node` with `encode_subtree_mms` |
| `src/engine/graphics/primitives/mod.rs` | Add `MeshKind` enum to `RenderableComponent` |
