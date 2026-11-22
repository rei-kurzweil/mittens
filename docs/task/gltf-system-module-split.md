# GLTF system module split

Status: proposed 2026-11-22 (AGENTS.md review signal; no split performed).

`src/engine/ecs/system/gltf_system.rs` is about 1.9k lines, roughly 600 of
them tests. It mixes four responsibilities:

1. **Resource import** (`load_gltf_resources`, `import_with_fallback`,
   accessor readers): parses a file once into cached CPU meshes, textures
   and skins.
2. **Instance spawn** (`spawn_node_recursive`, skin/morph binding in
   `tick_with_queue`): builds the per-instance ECS node tree.
3. **Upload** (`flush_imports`, `flush_mesh_imports_only`): registers cached
   meshes and uploads textures.
4. **Debug dumps and audits.**

Proposed split by responsibility: `gltf_import.rs` (1, plus the height
normalization already in `gltf_height.rs`), `gltf_spawn.rs` (2), and keep
`gltf_system.rs` as the system with upload and scheduling (3), with the debug
dumps beside import. Tests move with their responsibility.

Performance constraints: import stays once per cache key; spawn keeps
reading node transforms from the cached document without re-reading
buffers; uploads stay batched in `flush_imports`. Validation: the existing
GLTF system tests and a scene load of a skinned avatar (twilight rooftop) with
identical mesh/texture cache counts before and after.
