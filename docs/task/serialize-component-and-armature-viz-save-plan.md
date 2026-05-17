# Serialize component and armature-viz save plan

Date: 2026-05-16

This task captures the current serialization direction after the recent merge work.

The main save/export goal is:

- preserve a raw exact subtree dump path for debugging and tooling
- add a filtered scene/world save path that respects a future `Serialize` component
- let editor/runtime helper trees opt out of scene save without hard-coding a growing list of special cases

Before that, there is one prerequisite that should be handled first because it affects the shape of the save problem:

- armature visualization should stop being entangled with the authored glTF tree and should instead live in a separate helper tree
- transforms then need to propagate correctly across that split, and the transform pipeline is the most plausible place to support that cleanly

---

## 1. Prerequisite first: separate armature visualization from the authored armature tree

### Current problem

Today, editor-induced armature or bone visualization is too entangled with authored state.

There are two related issues:

- helper visualization nodes are spawned as part of the live glTF realization work
- editor ancestry currently forces `GLTFComponent.with_visualized_transforms = true`, which contaminates authored serialized state

That makes the save problem messy because the thing we want to exclude is not only an obvious helper subtree; it also leaks into the authored `GLTFComponent` flags.

### Desired model

Armature visualization should become a separate runtime helper tree rather than a mode that mutates the authored glTF component state.

High-level shape:

```text
authored_gltf_root
  ... imported armature / meshes / authored descendants ...

editor_or_runtime_viz_root
  ... viz bones / boxes / overlays / helper routing nodes ...
```

The authored glTF subtree should remain the thing that scene save serializes.

The visualization subtree should be treated as runtime/editor scaffolding.

### Why this is a prerequisite

If visualization remains mixed into the authored glTF tree, then the later `Serialize.off()` plan has to compensate for both:

- spawned helper descendants
- authored component state that was only turned on because the glTF happened to sit under an editor

That is the wrong abstraction boundary.

The first task should therefore be:

- move armature visualization into its own helper tree
- stop mutating authored glTF serialization state merely because an editor ancestor exists

### Transform propagation question

Once visualization is split into a separate tree, the remaining problem is how that tree follows the armature correctly.

The transform-pipeline direction looks like the right place to investigate this because the existing model already supports:

- transform-processing boundaries
- explicit output roots
- continuing traversal from output roots instead of only ordinary child relationships

That means the prerequisite subtask is not just “spawn the viz elsewhere.”

It is:

- split the topology so the viz helpers are no longer authored descendants of the glTF tree in the save sense
- make transform propagation across that split explicit
- check whether the current transform-pipeline/output-root model is sufficient for this without inventing another bespoke routing mechanism

This doc does not lock in the exact transform-pipeline design yet. It only records that this prerequisite should be solved before the serialization policy work, because it makes the later save rules much simpler and more defensible.

---

## 2. Serialization direction after that prerequisite

Once armature visualization is separated cleanly, the serialization model should be:

- raw subtree dump: exact live tree dump, no filtering
- scene/world save: filtered export path for authored content

The filtered scene/world save path should eventually respect a `Serialize` component with MMS-style semantics.

Desired authored vocabulary:

- `Serialize.off()` excludes a subtree from filtered scene/world save
- `Serialize.on()` re-includes a subtree inside an excluded ancestor subtree

This matches existing MMS naming patterns better than inventing a one-off serializer-only API.

### Why keep two paths

Even if most user-facing save behavior should be filtered, it is still useful to preserve an exact dump path for:

- debugging live runtime state
- REPL inspection
- clone or round-trip tooling that wants the literal current tree
- future cases where we explicitly want to see helper/runtime topology

So the split should remain:

- raw serializer ignores `Serialize`
- scene/world save honors `Serialize`

---

## 3. How the editor flag should fit into this

The existing editor flag should remain a save-policy control, but its effect should be indirect.

Desired behavior:

- when editor helper trees are spawned in the default mode, they get `Serialize.off()`
- when the editor is configured to serialize those helpers, the editor simply does not put `Serialize.off()` on them

That gives one consistent mechanism for later save filtering instead of a hard-coded editor-only exclusion path.

This should apply to editor/runtime helper trees such as:

- panel roots
- gizmo roots
- editor auto-wrapper/helper roots
- armature visualization helper roots once those exist as a separate tree

The important scope rule is:

- do not mark the whole editor subtree off if that would also hide nested authored content we want to keep
- instead, mark only the helper roots that are truly runtime/editor scaffolding

---

## 4. Proposed implementation order

### Stage 1 — split armature visualization into a separate helper tree

- stop treating editor-induced armature visualization as authored glTF state
- stop forcing serialized glTF flags on merely because a glTF has an editor ancestor
- spawn visualization as a distinct runtime helper tree

### Stage 2 — make transforms work cleanly across the authored tree / helper tree split

- verify whether the existing transform-pipeline output-root model can drive the separate helper tree cleanly
- prefer an explicit transform-routing solution over re-entangling helper topology back into the authored subtree

### Stage 3 — add `SerializeComponent` and MMS `Serialize.on()` / `Serialize.off()`

- define inherited save-visibility semantics
- make the semantics apply only to filtered scene/world save
- keep raw exact dump behavior unchanged

### Stage 4 — make editor/runtime helper spawners attach `Serialize.off()` by default

- editor panels
- gizmos
- editor helper wrappers
- separate armature visualization helper roots

### Stage 5 — add `Serialize.on()` support for inner re-inclusion

- this is not required for the first pass if no current topology needs a hole punched back into an excluded tree
- but the traversal model should be designed so that adding it later is straightforward

---

## 5. Acceptance criteria

The prerequisite is complete when:

- editor-induced armature visualization no longer depends on mutating authored glTF serialization state
- armature visualization lives in a separate helper tree
- there is a clear, explicit transform propagation story across the authored tree / helper tree split

The serialization task is complete when:

- there is still a raw exact subtree dump path
- there is a filtered scene/world save path
- filtered save can exclude helper trees via `Serialize.off()`
- filtered save can later support `Serialize.on()` re-inclusion without redesigning the traversal model
- editor/runtime helper trees are excluded by composition rather than by a brittle list of save-time name checks

---

## 6. Open questions

1. Can the current transform-pipeline output-root model drive separate armature visualization trees directly, or does it need a small extension?
2. Should `Serialize.on()` be implemented immediately, or only after a real use case exists?
3. Which existing user-facing commands should map to filtered scene/world save versus raw exact dump?