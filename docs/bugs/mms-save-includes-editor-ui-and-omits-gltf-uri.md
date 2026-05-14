# MMS save includes editor-owned content and omits GLTF URI

Date: 2026-05-14

This bug note tracks two regressions in the new MMS save path that uses:

- subtree → `to_mms_ast()`
- AST → unparser
- text file write

The old JSON component codec path is no longer the relevant save path here.

---

## 1. Symptoms

Saving through the World panel's new MMS save logic appears to work, but the emitted scene text
has at least two correctness problems.

### 1.1 Editor-owned content is being saved

The saved scene includes editor/components that belong to the editor tree and should not be part
of the authored scene file.

Expected behavior:

- editor UI
- panel chrome
- gizmos
- other editor-owned helper subtrees

should be excluded from the saved scene.

### 1.2 GLTF component loses its URI

Saved GLTF nodes do not preserve the source asset path/URI, so reloading the saved scene cannot
know which model asset to use.

Expected behavior:

- a GLTF component should round-trip as something like `GLTF.new("...")` or another canonical
  MMS form that includes the asset URI and any other persistent authored fields

Actual behavior:

- the current MMS save path emits a bare/default GLTF component shape with no URI

---

## 2. Current save path

The World panel save button goes through `InspectorSystem`:

- `dump_scene_to_mms(world, Some(editor_ui_root))`
- iterate top-level roots not under the excluded editor subtree
- `component_registry::subtree_to_ce_ast(...)`
- `unparser::unparse_component(...)`

Relevant code:

- `src/engine/ecs/system/inspector_system.rs`
  - `setup_panels_for_editor(...)`
  - `dump_scene_to_mms(...)`
  - `collect_subtree(...)`
- `src/meow_meow/component_registry.rs`
  - `subtree_to_ce_ast(...)`
- `src/engine/ecs/component/mod.rs`
  - `Component::to_mms_ast(...)`

---

## 3. Likely root cause: editor exclusion is too narrow

The current save exclusion logic computes one `editor_ui_root` by walking to the topmost ancestor
of the editor layout root, then excludes descendants of that root when saving.

That assumes all editor-owned content lives under one clean subtree.

That assumption is too weak if some editor-owned helpers or editor-managed components are spawned
outside that subtree, attached elsewhere, or otherwise not reachable from the chosen root.

So the current logic:

- may correctly exclude the panel subtree
- but still serialize other editor-owned nodes/components that should be treated as tooling state

This needs a more explicit ownership rule than “skip descendants of one root”.

Possible fix directions:

1. explicit editor-owned marker / ancestry rule used by save filtering
2. a save-time predicate that excludes editor/tool component kinds even when their ancestry is
   not under the panel subtree
3. a dedicated authored-scene root concept, so save walks only user scene roots rather than
   “all roots except editor subtree”

Current preference: move toward an explicit authored-scene save predicate rather than relying on
one subtree exclusion heuristic.

---

## 4. Likely root cause: GLTFComponent does not implement `to_mms_ast`

`subtree_to_ce_ast(...)` delegates to each component's `to_mms_ast()`.

`GLTFComponent` currently implements JSON `encode()` / `decode()` including `uri`, but it does
not override `to_mms_ast()`.

So the save path falls back to the default `Component::to_mms_ast()` implementation, which emits
a bare component expression based only on `name()`.

That explains why the MMS output loses the URI even though the old JSON codec path knew it.

Expected fix direction:

- implement `GLTFComponent::to_mms_ast()`
- encode the URI in the canonical MMS form for GLTF, likely `GLTF.new("uri")`
- include any other persistent authored fields such as `with_visualized_transforms` when they are
  meant to round-trip through scene save/load

---

## 5. Why this matters

These two issues together mean the new MMS save path is not yet trustworthy as scene persistence:

- saved files include tooling/editor state that should remain runtime-only
- saved files lose enough GLTF data that scene reload is incomplete or wrong

That makes the output look superficially valid while still being semantically incorrect.

---

## 6. Investigation checklist

- identify which editor-owned nodes/components are leaking into the saved output
- confirm whether they are outside the excluded `editor_ui_root` subtree or just not covered by
  the current heuristic
- decide whether save filtering should be ancestry-based, marker-based, or authored-root-based
- implement `GLTFComponent::to_mms_ast()` with a stable MMS constructor shape
- verify save → load round-trip for a scene containing GLTF content

---

## 7. Acceptance criteria

- saving through the World panel excludes editor/tooling subtrees and components
- GLTF components in the saved MMS include the source URI
- save → load reproduces the original GLTF-backed scene content

---

## 8. Relevant code

- `src/engine/ecs/system/inspector_system.rs`
- `src/meow_meow/component_registry.rs`
- `src/meow_meow/unparser.rs`
- `src/engine/ecs/component/mod.rs`
- `src/engine/ecs/component/gltf.rs`