# Serialize component and armature-viz save plan

Date: 2026-05-16

Status note: the authored `TransformPipeline` / `TransformPipelineOutput` wrapper/output topology has now been removed. Any references below to those names are historical; current authored transform shaping uses `TransformForkTRS` with downstream content attached directly under the fork root.

This task captures the current serialization direction after the recent merge work.

Current status update: 2026-05-29

- A filtered world save path now exists for the world panel save/load flow.
- A raw exact subtree dump path still exists separately.
- `SerializeComponent` plus MMS `Serialize.on()` / `Serialize.off()` now exist.
- Filtered save currently uses nearest-ancestor visibility, where the nearest ancestor with an immediate `Serialize` child wins.
- The editor runtime UI root is intentionally excluded by attaching `Serialize.off()` under `editor_runtime_ui_root`.
- Several runtime helper roots now attach `Serialize.off()` by default:
  - world-panel runtime UI root
  - text glyph roots and text-shadow glyph roots
  - AvatarControl body `TransformForkTRS`
  - AvatarControl-spawned arm `IKChain`
  - layout-owned `__bg` background quads

However, the current state is still incomplete and has at least one confirmed bad save artifact:

- in `assets/data/world.mms`, a root `BackgroundColor { Color ... overlay { ... cube ... } }` is still being emitted even though the authored root was only:

```text
BGC {
  C.rgba(1.0, 0.65, 0.75, 1.0)
}
```

That means some runtime-owned helper topology is still attaching under the `BackgroundColor` branch and surviving filtered save. This is a real bug in current save behavior and should be treated as unresolved.

There was also a semantic correction during investigation:

- `GLTF` itself should be serialized by default because it is authored content.
- only the runtime nodes spawned by `GLTFSystem` should be excluded by default.
- the intended rule is:
  - authored `GLTF { ... }` remains serializable
  - the nodes realized at runtime from that glTF are wrapped with `Serialize.off()` by default
  - if there is an explicit authored `Serialize.on()` as a direct child of `GLTF`, then those spawned runtime nodes should be allowed back into filtered save

This direct-child `Serialize.on()` rule is the current target semantics. If implementation details disagree with this, the implementation is wrong and should be changed to match the rule above.

Related task:

- [transform-parent-component-ref-routing.md](transform-parent-component-ref-routing.md)

Related bug:

- [../bugs/world-panel-save-freezes-ui-and-degrades-scene-performance.md](../bugs/world-panel-save-freezes-ui-and-degrades-scene-performance.md)

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

### What exists today in the transform pipeline

The current runtime already gives us a few useful pieces, but they are narrower than the desired armature-viz split wants.

What exists now:

- pipeline input is always `ParentWorld`
- a pipeline can redirect traversal to explicit `TransformPipelineOutput` roots
- per-channel ops can already replace translation or rotation by sampling an ancestor transform via `TransformSampleAncestor.skip(n)`

That means the engine already supports one form of “do not just inherit from the immediate authored parent.”

However, the current operator is strictly relative and topology-local:

- `TransformSampleAncestor` walks upward from the pipeline owner
- it samples the `n`-th ancestor `TransformComponent`
- it cannot point at an arbitrary other transform in the world
- it does not use `ComponentRef`

So the current system can express:

- “use the parent bone above this splice instead of the immediate controller-driven transform”

but it cannot yet express:

- “treat the transform referenced by this selector or durable component reference as the input parent/basis for this pipeline”

That missing capability is the main gap for a clean authored-tree / helper-tree split.

### What exists today in `ComponentRef`

The engine now has a durable, serializable `ComponentRef` abstraction.

Current authored forms are:

- `ComponentRef::Guid(uuid)`
- `ComponentRef::Query(selector)`

Current users are:

- `ActionComponent.target_sources`
- `IKChainComponent.target_source`
- `IKChainComponent.end_effector_source`

Current runtime resolution model:

- guid refs resolve through `World::component_id_by_guid`
- selector refs resolve through `World::find_component`
- `AnimationSystem` resolves `ActionComponent` refs when needed
- `IKSystem` resolves IK refs before solving

This is useful because it means the durable authored-reference machinery already exists and already round-trips through MMS cleanly.

### What is missing if transform pipeline ops should use `ComponentRef`

Applying `ComponentRef` to the transform pipeline looks plausible, but it is not a one-line extension.

Today, the transform-pipeline authored components are mostly tiny marker/config components that parse directly into runtime enums and `Copy` data.

They do not currently have a resolution phase analogous to Action or IK.

So if a future pipeline operator should say “sample world transform from this referenced component” or “treat this referenced transform as the parent/input basis,” that likely requires:

- a new authored component or input component that stores `ComponentRef`
- a resolution story for that ref at runtime
- a decision about when unresolved refs are retried
- runtime enum support in `TransformPipelineInput` or the per-channel op enums for ref-based sampling/input selection

The important point is that this is feasible with current building blocks, but it is not already implemented.

### Boilerplate / API shape note

The current authored transform-pipeline topology is also somewhat verbose.

Common shape today:

```text
TransformComponent
  TransformPipeline
    TransformForkTRS
      TransformMapRotation
        QuatTemporalFilter
    TransformPipelineOutput
      TransformComponent
        ... driven subtree ...
```

That means even a simple “fork TRS and filter one channel” case requires:

- an outer `TransformPipeline`
- a `TransformForkTRS`
- one or more map nodes
- a `TransformPipelineOutput` node when driving a separate subtree

That does support the current runtime model, but it is a clunky authoring surface.

We should treat the question of reducing that boilerplate as a separate API-design task.

Examples of the kind of future cleanup that may make sense:

- allowing `TransformForkTRS` to act as a pipeline root without an otherwise-empty outer `TransformPipeline`
- allowing more direct authoring of the common “filter one channel, passthrough the others” case
- making input selection feel first-class instead of encoded indirectly through relative ancestor sampling

Those changes should not be bundled into the serialization or armature-viz split task. The immediate prerequisite only needs enough transform-routing power to support the separate helper tree cleanly.

---

## 2. Serialization direction after that prerequisite

Once armature visualization is separated cleanly, the serialization model should be:

- raw subtree dump: exact live tree dump, no filtering
- scene/world save: filtered export path for authored content

The filtered scene/world save path should eventually respect a `Serialize` component with MMS-style semantics.

Desired authored vocabulary:

- `Serialize.off()` excludes a subtree from filtered scene/world save
- `Serialize.on()` re-includes a subtree inside an excluded ancestor subtree

Clarification from current investigation:

- the component that remains authored is still serialized unless an exclusion rule hides it
- for `GLTF`, the authored `GLTFComponent` is not itself the thing to exclude
- instead, the runtime-expanded node tree produced by `GLTFSystem` is the thing that should default to `Serialize.off()`
- the opt-in escape hatch for saving realized glTF runtime nodes is an explicit authored direct child: `GLTF { Serialize.on() ... }`

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
- evaluate whether this needs a new ref-based input/source operator rather than more `skip(n)` ancestor sampling
- use the existing `ComponentRef` durability/resolution model as the starting point if a ref-based operator is added

### Stage 3 — add `SerializeComponent` and MMS `Serialize.on()` / `Serialize.off()`

- define inherited save-visibility semantics
- make the semantics apply only to filtered scene/world save
- keep raw exact dump behavior unchanged

### Stage 4 — make editor/runtime helper spawners attach `Serialize.off()` by default

- editor panels
- gizmos
- editor helper wrappers
- separate armature visualization helper roots

Status:

- partially implemented
- currently covered helper roots include panel runtime UI, text glyph roots, AVC body pipeline, AVC arm IK chains, and layout-owned `__bg`
- still incomplete because at least one runtime-owned subtree is still leaking into filtered save under `BackgroundColor`

### Stage 4.1 — fix currently confirmed leaking runtime helpers

Known confirmed issues from current `world.mms` output:

- root `BackgroundColor` is still picking up runtime helper descendants (`overlay` + `Renderable.cube`) that were not authored there
- this indicates at least one runtime helper path is still missing `Serialize.off()` or is attaching at the wrong boundary
- examples like `bisket-vr-demo` should be used as the regression surface for this work

This should be completed before treating the save-filter work as stable.

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
- authored `GLTF` components remain serializable while their runtime-expanded descendants are excluded by default unless direct-child `Serialize.on()` explicitly opts them back in
- filtered save no longer produces the known bad `BackgroundColor { ... overlay { cube } }` root artifact when the authored scene only provided `BackgroundColor` plus `Color`

---

## 6. Open questions

1. Can the current transform-pipeline output-root model drive separate armature visualization trees directly, or does it need a small extension?
2. Should the new transform-routing operator be:
  - a pipeline input override using a referenced transform as the parent basis, or
  - a per-channel sampling operator using a referenced transform source, or both?
3. How much of the existing `ComponentRef` resolution model can be reused directly for transform-pipeline nodes, and where should that resolution live?
4. Should transform-pipeline authoring boilerplate reduction be designed in the same pass, or explicitly deferred to a separate task after the new routing/input capability exists?
5. Should `Serialize.on()` be implemented immediately, or only after a real use case exists?
6. Which existing user-facing commands should map to filtered scene/world save versus raw exact dump?
7. What exact runtime subtree is currently attaching under root `BackgroundColor` and why is it still surviving filtered save?
8. Should there be a dedicated regression test that asserts a plain authored `BackgroundColor { Color ... }` round-trips without any extra helper descendants?