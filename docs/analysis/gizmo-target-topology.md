# Gizmo target topology vs GLTF viz overlay

## Problem statement

When clicking a joint like `J_Bip_L_UpperArm` (via its transform-visualization “viz box”), the editor moves the transform gizmo into the clicked hierarchy, and the viz box + gizmo appear to rotate when dragging.

However, the actual skinned mesh does **not** deform (the arm doesn’t move), even though it looks like the gizmo is “on the upper arm”.

This document explains what’s happening, why the skinning doesn’t update, and some workarounds.

## Observed component topology

With `with_visualized_transforms` enabled, `GLTFSystem` creates extra components for transform-only nodes:

- Under the real glTF node `TransformComponent` (the joint):
  - `OverlayComponent` named `viz_overlay:<node>`
    - `TransformComponent` named `viz:<node>` (used to apply a small constant scale)
      - `RenderableComponent` named `viz_box:<node>`
        - `RaycastableComponent`

So for the upper arm joint you saw a path like:

```
/editor/.../J_Bip_L_UpperArm/
  viz_overlay:J_Bip_L_UpperArm/
    viz:J_Bip_L_UpperArm/
      viz_box:J_Bip_L_UpperArm
```

The transform gizmo is also reparented into that area when you click:

- `editor_transform_gizmo` (component type `transform_gizmo`)

## Key behavioral detail: what the gizmo actually targets

The gizmo system (`TransformGizmoComponent` + `TransformGizmoSystem`) has an explicit contract:

- “Attach this as a child of a `TransformComponent` you want to manipulate.”
- The gizmo’s `target_transform` is resolved by walking **up** ancestry until a `TransformComponent` is found.

The crucial part is how retargeting happens when the editor moves the gizmo:

- The editor selection handler chooses the **nearest Transform ancestor** of the clicked renderable.
- It then *attaches the gizmo under that transform*.
- The gizmo system listens for `ParentChanged` on the gizmo component and rebinds `target_transform` to the nearest Transform ancestor of the new parent.

For a `viz_box:<node>` hit, the nearest Transform ancestor is **not** the real joint transform.

It is the **viz transform**:

- Hit renderable: `viz_box:J_Bip_L_UpperArm`
- Nearest Transform ancestor: `viz:J_Bip_L_UpperArm`
- Gizmo gets attached under `viz:J_Bip_L_UpperArm`
- Gizmo’s `target_transform` becomes `viz:J_Bip_L_UpperArm`

So dragging the gizmo rotates/translates/scales the **viz transform**, not the **real joint transform**.

## Why the skinned mesh doesn’t deform

Skinning (`SkinnedMeshSystem`) computes joint matrices using the joint `TransformComponent`s spawned for the glTF skeleton nodes.

Those joint transforms are the **real** glTF node transforms (e.g. `J_Bip_L_UpperArm`), resolved via the glTF node index → component id map.

Since the gizmo is currently modifying `viz:J_Bip_L_UpperArm` (a helper transform that is *not* part of the skin joint list), the skinning matrices do not change, so the mesh does not deform.

This also explains the “looks like it’s on the joint” confusion:

- The gizmo is visually near/under the joint in the tree.
- The viz box is a child of the real joint.
- Rotating the viz transform rotates the viz box, which is what you see.
- But the skeleton joint used for skinning is unchanged.

## Is the overlay the cause?

The `OverlayComponent` is likely not the direct cause of the *skinning* issue.

The cause is the extra **viz transform layer** and the editor’s “nearest transform ancestor” targeting rule.

That said, overlays do affect what gets clicked and how selection behaves, because the only renderable on transform-only nodes is the `viz_box` subtree.

## How to confirm quickly (no code changes)

1. Enable gizmo debug logs:
   - `CAT_DEBUG_GIZMO=1`
   - Look for logs like:
     - `ParentChanged ... old_target=... new_target=...`
   - When clicking a viz box, `new_target` should be the `viz:<node>` transform.

2. Enable skin apply logs:
   - `CAT_DEBUG_SKIN_APPLY=1`
   - If you rotate a **real** joint transform, you should see skin palette updates.
   - When rotating only the `viz:<node>` transform, you should see no joint-driven binding dirties for the rig.

3. REPL inspection:
   - After clicking, run:
     - `pwd` (to see where the gizmo ended up)
     - `type ..` and `type ../..` (to see viz/overlay vs real joint)

## Workaround / fix ideas

These are design-level options; pick the one that fits the long-term editor model.

### Option A: “Skip viz transforms” during selection

In the editor selection path, if the nearest Transform ancestor is a viz helper (heuristic by name prefix `viz:` or parent overlay name `viz_overlay:`), then return the next Transform ancestor *above* the overlay.

- Pros: Minimal conceptual change; keeps viz helper architecture.
- Cons: Name-based heuristics are brittle unless we add an explicit marker.

Preferred variant: add a small marker component on viz transforms (e.g. `GltfVizTransformComponent { owner_transform: ComponentId }`) so selection can reliably map from viz → owner.

### Option B: Store a “pick target” reference on the viz renderable

Attach a component to `viz_box:<node>` such as:

- `PickTargetComponent { target_transform: ComponentId }`

Then editor selection uses this component (if present) to decide what transform to attach the gizmo to, instead of choosing the nearest Transform ancestor.

- Pros: Very explicit; avoids ancestry-walking ambiguity.
- Cons: Adds a new concept/component to picking.

### Option C: Don’t insert a viz TransformComponent layer

Instead of `Transform(viz)` + `Renderable(viz_box)`, attach the renderable directly under the real joint transform, and encode the viz scale elsewhere:

- bake scale into the cube mesh vertices
- or use a renderable-specific scale field (if available)

- Pros: Selection becomes unambiguous (nearest transform is the real joint).
- Cons: More invasive to rendering asset setup; mesh baking might be annoying.

### Option D: Decouple gizmo parent from gizmo target

Keep reparenting the gizmo for convenience, but set gizmo `target_transform` explicitly to the desired joint transform (and do not auto-retarget on `ParentChanged`).

- Pros: Makes "gizmo lives under X" independent from "gizmo edits X".
- Cons: Larger behavior shift; requires clear rules for how target is managed.

## Exploring “explicit manipulation targets” (beyond topology)

The root friction here is that we’re using *topology* (nearest `TransformComponent` ancestor) as a proxy for *semantics* (the transform we intend to edit).

If we want gizmos to have explicit rules for what they affect regardless of where they’re attached, we need to introduce (and standardize) an explicit “manipulation target resolution” mechanism.

There are a few directions, from smallest to most general.

### Direction 1: Explicit target on the gizmo (override ancestry)

Add a rule like:

- `TransformGizmoComponent.target_transform_override: Option<ComponentId>`
- Drag logic uses `override.or(resolved_from_parent_changed)`

Then the editor can:

- attach the gizmo anywhere convenient for organization/rendering
- set the override explicitly to the real joint transform (e.g. `J_Bip_L_UpperArm`)

This keeps the gizmo semantics contained and avoids needing global routing.

Open question: how does the editor compute the override from a hit on `viz_box:*`?

- Still needs a mapping from “viz proxy” → “real joint transform” (marker component / pick-target component / naming heuristic).

### Direction 2: Forward/rewrite transform intents for proxy nodes

This is the idea you suggested: “intercept the `UpdateTransform` intent in the viz component and pass it to its parent.”

Important constraint in the current architecture:

- `IntentValue::UpdateTransform { component, ... }` is executed by the mutation executor directly (no handler dispatch for intents).
- A `TransformComponent::set_*` method immediately emits `IntentValue::UpdateTransform` targeting the same component id.

So “intercept in the component” can’t be implemented with *event handlers* today, because intents are not delivered to handler closures.

But we can still model “intercept/forward” in three realistic places:

#### 2A) Rewrite at the mutation boundary (in `MutationExecutor`)

Before applying an `UpdateTransform`, check whether the target component is a proxy.

Example concept:

- `TransformProxyComponent { forward_to: ComponentId, policy: ForwardPolicy }`
- If `UpdateTransform.component` has `TransformProxyComponent`, rewrite the mutation to apply to `forward_to`.

Pros:

- Centralized, deterministic; doesn’t require new “intent handler” infrastructure.

Cons / sharp edges:

- You need a policy for *which* fields forward (translation/rotation/scale) and whether the proxy should remain unchanged.
  - For viz transforms we generally want the viz scale to stay constant, so forwarding scale blindly would be wrong.
- Risk of loops if `forward_to` also forwards.
- Harder to reason about when debugging (“I updated A but B moved”).

This can work as a tactical bridge, but it’s easy to accumulate special cases.

#### 2B) Add a semantic intent stage (rewrite before mutation)

Today we already have a semantic intent executor that expands higher-level intents into canonical mutations (e.g. `SetPosition` → `UpdateTransform`).

We could extend that layer to also rewrite `UpdateTransform` itself based on components present.

This is like “a programmable router for intents”, without changing the mutation executor.

Pros:

- Cleaner separation: “semantic intent routing” vs “apply mutation”.

Cons:

- Still needs explicit proxy metadata and loop-avoidance.
- Still doesn’t solve the root mapping problem unless the proxy metadata encodes the mapping.

#### 2D) ECS-attached signal pipeline on the target node

Another way to make forwarding feel “ECS-native” (and avoid hard-coding a GLTF/viz special case) is:

- Treat intent forwarding as *middleware attached to the target component graph*.
- When executing an intent, resolve its effective target(s), look for a pipeline attached to those targets, and rewrite/forward before applying the mutation.

In this engine’s ECS model, each `ComponentId` stores exactly one concrete component type, so “the target component has a pipeline” can’t mean “the same id stores both Transform + Pipeline”. Practically it means:

- The target node has a child component like `SignalPipelineComponent` (or a forwarding component directly).
- The executor scans the target node’s children for pipeline/ops and applies them.

Sharp edge to be aware of: intents do *not* have a single uniform `target: ComponentId` field today.

- Some intents have `target: Vec<ComponentId>` (e.g. `SetTransform`).
- Some have a singular `component: ComponentId` (e.g. `UpdateTransform`).
- Some have multiple participant ids (`Attach { parents, child }`).
- Some are effectively “untargeted” payloads (`Print`, `Noop`, `ReplExec`).

So a pipeline processor needs a helper like “extract/iterate the effective target component ids for this intent” (and define which participants are eligible for rewriting).

For the viz case, a minimal first rule could be:

- If the intent is `UpdateTransform { component: viz_transform, .. }` and `viz_transform` has a forwarding op, rewrite it to target the owner transform (e.g. parent’s `TransformComponent`, or nearest non-viz ancestor).

Finally, placement matters: the rewrite needs to run *before* any layer that applies transforms (semantic executor or mutation executor), otherwise the wrong component will be mutated and downstream systems (skinning) won’t observe joint changes.

#### 2E) Standardize intent “subjects” (recipients) for pipeline lookup

If we want “any recipient of an intent may have a signal pipeline” to be a first-class concept, it helps to standardize where targeting lives.

Today, `IntentValue` encodes targets in several shapes:

- `target: Vec<ComponentId>` (many variants)
- `component: ComponentId` (register/update/schedule variants)
- multi-party ids like `Attach { parents, child }`
- untargeted (`Print`, `Noop`, `ReplExec`)

This makes a pipeline processor awkward because it must match on every variant just to find “which component ids should be checked for pipeline ops?”.

There are two good options:

**Option 1 (minimal / keeps payload shapes): define a canonical recipients helper**

- Add a single helper (conceptually) `intent_recipients(intent: &IntentValue) -> Vec<ComponentId>`.
- The pipeline processor calls this and checks each recipient node for pipeline/ops.
- This does not require changing intent storage; it just centralizes the mapping.

**Option 2 (stronger standardization): introduce `subjects: Vec<ComponentId>` on all intents**

- Standardize an addressing field name and shape across intents, even when it has 0 or 1 entries.
- For future-proofing (“subject/object split later”), treat `subjects` as “all recipients that are eligible for pipelines”, not necessarily “the grammatical subject”.
- Later, we can add roles (subject/object) without breaking the pipeline lookup concept.

If we adopted Option 2, a rough mapping from current variants would look like:

- `SetColor/SetText/SetPosition/SetTransform`: `subjects = target`
- `Detach/RemoveSubtree/AudioGraphRebuild/RequestRaycast`: `subjects = target`
- All the `Audio*` and `Oscillator*` variants that have `target`: `subjects = target`
- `Register*/Remove*/*DirtyImmediate/MakeActiveCamera`: `subjects = vec![component]`
- `UpdateTransform`: `subjects = vec![component]`
- `ScheduleAudio*`: `subjects = vec![component]`

Multi-party intents need a convention. For pipeline *lookup* (not semantics), the simplest rule is:

- `Attach { parents, child }`: `subjects = parents + [child]`
- `AttachClone { parents, prefab_root }`: `subjects = parents + [prefab_root]`
- `RemoveChild/RemoveChildren`: `subjects = parents`

Untargeted intents stay valid:

- `Print/Noop/ReplExec`: `subjects = []`

Practical note: if we do later add a subject/object split, it’s still reasonable for pipeline lookup to consult the union of both lists. What matters for the viz problem is: the `viz` transform (and/or its parent) can be a recipient for pipeline discovery when a transform mutation is about to be applied.

#### 2C) Emit an event on transform mutation, and handle it

Another pattern is:

- When a transform changes (during mutation), emit an *event* like `TransformChanged { component }`.
- Allow handler-based reactive rules that listen to that event and enqueue follow-up intents.

This is closer to a “reactive pipe” model (see below), but it changes the mental model:

- The forwarded update would occur in a later drain (events emitted by handlers are next tick), unless you introduce special semantics.

### Direction 3: A general reactive “pipe” primitive (beyond REPL)

The REPL already has a `pipe` feature (command-line pipes) that moves *component objects* through transformations like `ls | grep`.

That’s not reactive, but it hints at a useful missing primitive: a declarative way to express “when X happens to component A, apply Y to component B”.

If we wanted something like that in the ECS, the design space looks like:

- **Intent pipes**: allow registering a rule that rewrites intents matching a predicate.
  - Example: `UpdateTransform(component = viz) -> UpdateTransform(component = owner)`
  - This is effectively Direction 2B formalized.

- **Event pipes**: emit structured events for important mutations (transform changed, parent changed, etc.) and let scoped handlers react.
  - Works well with the drain-point model but is often “next tick”.

- **Component-level proxies**: attach a component that declares an aliasing relationship, and core systems consult it.
  - Example: `PickTargetComponent` for selection; `TransformProxyComponent` for transform updates.

Tradeoff summary:

- Pipes/routers are powerful but can become invisible control flow.
- Proxy components keep intent routing local and inspectable (`ls`/`type` can show the proxy).

## Practical takeaway for viz overlays

For the specific viz-overlay issue, intent-forwarding is likely overkill.

The least surprising model is:

- “Viz nodes are pick proxies; manipulation targets are the real glTF transforms.”

So prefer an explicit mapping at selection time (marker component / pick-target reference), or explicit gizmo target overrides, rather than forwarding transform mutations after the fact.

## Sketch: signal “stages” / pipeline-attached forwarding

You’re right to call out the architectural mismatch: forwarding based on the *emitter* doesn’t compose well with how things are executed.

- Emitters are an implementation detail used while dispatching handlers.
- Executors (intent/mutation) consume **signals**.
- Therefore, if we want “multiple RxOps like `RxForwardToParent`” to apply reliably, the forwarding metadata has to live on the **signal** (or be derivable from world state at execution time).

This section sketches what a signal-attached pipeline could look like.

### What problem the pipeline solves

We want a rule like:

- “If a transform update is targeting a viz proxy, rewrite it to target the owner transform.”

And we want it to be:

- opt-in (only when visualization mode is on)
- local (only for the viz subtree)
- inspectable (debuggable)
- composable (more than one op)

### Where to attach it

Conceptually the right payload carrier is the runtime `Signal` itself (the envelope that holds either an event or an intent):

- `Signal { event: Option<EventSignal>, intent: Option<IntentSignal>, ... }`
- add `pipeline: Vec<RxOp>` or `stages: RxStages`

Why not attach to the emitter?

- the mutation executor never sees the emitter
- intents can be generated from components directly (e.g. `TransformComponent::set_*`), not only from user handlers

### What an op would look like

Think of each op as a pure-ish transformer:

- input: `(world, signal)`
- output: modified `signal` (or a sequence of signals)

Example ops:

- `RxForwardUpdateTransformToParent`
- `RxMapComponent { from: ComponentId, to: ComponentId }`
- `RxRewriteTargetByComponentMarker { marker_type: ..., field: ... }`

For viz overlays you probably want something more specific than “to parent”, because the parent chain is:

`viz_box(renderable) -> viz(transform) -> viz_overlay(overlay) -> joint(transform)`

So “forward to parent transform” might forward to `viz` (wrong) rather than to the real joint transform.
The op needs either:

- “skip overlays and proxy transforms”, or
- an explicit mapping stored somewhere (see below).

### Who applies the pipeline (and when)

There are two natural application points:

1) **Before semantic intent expansion** (Intent stage)
  - rewrite high-level intents (`SetPosition`, `SetTransform`, etc.)
  - pros: keeps semantics consistent

2) **Right before mutation** (Mutation stage)
  - rewrite canonical mutations (`UpdateTransform`, `Attach`, etc.)
  - pros: single choke point for correctness

If you do both, you need a clear ordering rule.

Pragmatic rule of thumb:

- apply target-resolution ops as late as possible (mutation stage), because by then you have a concrete `component` id.

### How GLTF visualization mode would “install” the pipeline

This is the part that tends to surprise people: GLTFSystem can’t attach a pipeline to *all future signals globally* unless there is a global registry.

So you generally need one of these:

- A per-subtree registry component (“pipeline roots”) that executors consult.
- A per-component marker that makes rewriting discoverable at execution time.
- Or literally attaching a pipeline to each emitted signal at the time of emission.

Attaching to each emitted signal is awkward because many intents originate outside glTF code.

So the most inspectable approach is usually **component-level metadata** plus a simple executor rule.

Example:

- When GLTF spawns `viz:<node>`, attach `TransformProxyComponent { owner_transform: <joint> }` on that viz transform.
- Then, mutation stage does:
  - if applying `UpdateTransform` to a component with `TransformProxyComponent`, rewrite the target to `owner_transform`.

This achieves “pipeline semantics” without literally storing a pipeline on each signal.

### Reconciling “pipeline on signals” vs “pipeline derived from components”

You can combine both ideas:

- Signals can carry an optional `pipeline` (useful for explicit, one-off routing).
- Executors can also consult component metadata to *derive* additional ops to apply.

That yields a model like:

1) Start with `signal.pipeline` (explicit)
2) Extend with ops derived from the current target component (implicit)
3) Apply ops in a defined order

This gives you both:

- explicit routing for special interactions (e.g. editor gizmo)
- implicit routing for systemic proxy behavior (e.g. viz transforms)

### Why this is still a design decision (not free)

Pipelines are powerful, but they’re also easy to overuse.

If forwarding is only needed for selection/manipulation, doing it at selection time (pick-target mapping) is usually simpler and more debuggable than a general signal pipeline.

If you do introduce pipelines, the most important thing is to keep them discoverable in tooling (REPL) so debugging isn’t “invisible magic”.

## Recommendation

If transform-only node viz is meant to be a *pure view/pick proxy* for the underlying glTF transform, the cleanest approach is:

- Add an explicit marker/reference from the viz subtree back to the real transform (Option B), or
- Add a marker on the viz transform indicating it’s a proxy and which owner it represents (Option A, marker-based).

Avoid relying on name prefixes alone unless it’s strictly internal/debug.

## Notes for future editor semantics

Long-term, it may be useful to standardize a “selection target resolution” policy:

- Renderable hit → resolve selection target transform
- Selection target transform → resolve manipulation target (may differ if editing constraints apply)

That makes it easier to support proxy objects, overlays, and editor-only handles without accidentally manipulating view proxies.
