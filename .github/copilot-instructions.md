# Copilot instructions for cat-engine

## Big picture (read these first)
- Engine is a **component tree** (not an entity+components map): `engine::ecs::World` stores `ComponentNode { parent, children }` keyed by `ComponentId` (slotmap). Start in `src/engine/ecs/mod.rs` and `src/engine/ecs/component/mod.rs`.
- `engine::Universe` is the main façade: `World + CommandQueue + SystemWorld + VisualWorld + RenderAssets + VulkanoRenderer` (`src/engine/universe.rs`).
- Frame phases are deterministic:
  - Window loop: `Windowing` (`src/engine/windowing.rs`) calls `universe.update(dt, input)` then `universe.render()` every redraw.
  - Update: `SystemWorld::tick` does input → GLTF spawn → `queue.flush(...)` → systems → physics → more flushes (`src/engine/ecs/system/system_world.rs`).
  - Post-tick: `SystemWorld::process_commands` = flush → dispatch signals → flush again.

## Making changes safely (project-specific rules)
- Prefer mutating component state + then queueing the relevant registration/update command on `CommandQueue` (e.g. changing `ColorComponent` requires `queue.queue_register_color(color_cid)`; see `examples/gravity-fields.rs`). Systems read/apply these in `CommandQueue::flush`.
- Use `Universe::add(root)` to initialize a newly built subtree (`Component::init`), and use `Universe::attach(parent, child)` when reparenting so the subtree is auto-initialized and emits `EventSignal::ParentChanged`.
- Signals are scoped by ancestry (handlers fire for `scope` and its ancestors). Register with `Universe::add_signal_handler(kind, scope_root, handler_fn)`; handlers are **fn pointers** (no closures) and run after command flushing (`docs/events.md`, `docs/signals.md`, `src/engine/ecs/rx/rx_world.rs`).

## Serialization / prefabs
- Save/load is JSON via `ComponentCodec` (`src/engine/ecs/component_codec.rs`). The CLI in `src/main.rs` supports `cargo run -- load <file>` and `cargo run -- save <file>`.
- Prefab cloning uses structural encode/decode with **fresh GUIDs**: `Universe::attach_clone` (`src/engine/universe.rs`). Note: component-internal references are currently copied as-is (no fixup pass).

## Rendering + assets (what to know)
- Renderer consumes `VisualWorld` (batched/instanced) and records a single dynamic-rendering scope in several **virtual draw phases** (see `src/engine/graphics/vulkano_renderer.rs` + `src/engine/graphics/vulkano_cbb.rs`):
  - Background (no depth write)
  - Background `with_occlusion_and_lighting` (depth write ON for self-occlusion)
  - Clear depth (so background never occludes foreground)
  - Opaque (instanced)
  - Optional cutout / alpha-to-coverage (`TransparentCutoutComponent`, instanced)
  - Transparent single-layer (instanced, depth write OFF)
  - Transparent multi-layer (sorted back-to-front, drawn one-by-one)
- Most passes have **two pipeline variants** selected by `MaterialHandle`: `TOON_MESH` vs `SKINNED_TOON_MESH` (skinned/rigged). Pipeline selection and vertex-buffer binding live in `VulkanoState::record_*_draws` in `src/engine/graphics/vulkano_cbb.rs`.
- Shaders are compiled at build-time via `vulkano_shaders::shader!` paths in `src/engine/graphics/vulkano_renderer.rs`. Optional offline build: `assets/shaders/compile-shaders` (needs `glslc`).
- Texture pipeline: `assets/prepare_assets` → `assets/convert_images_to_dds` (needs `compressonator`) to generate BC7 `.dds` under `assets/textures/`.

## Useful workflows
- Run engine: `cargo run`
- Run an example: `cargo run --example gravity-fields` (also see `examples/`)
- Special example name (dots): `cargo run --example mindmap_example_0_3` (declared in `Cargo.toml`).
- Tests: `cargo test` (includes topology tests in `src/engine/ecs/world_graph_tests.rs`).

## Where to look for established patterns
- Signals + actions model: `docs/events.md`, `docs/signals.md`
- BVH + picking data flow: `docs/bvh-and-raycast.md`
- Skinned glTF pipeline: `docs/skinned-toon-mesh.md`
- Large end-to-end example with collisions + signals: `examples/gravity-fields.rs`
