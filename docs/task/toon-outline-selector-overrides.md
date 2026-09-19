# ToonOutline selector exclusions and per-renderable overrides

Date: 2026-09-19

Status: proposed follow-on

Parent task: [ToonOutline component and shared render instance](toon-outline-component-and-shared-deformation.md)

## Goal

Extend `ToonOutline` so one modifier can describe exceptions and variations across the renderables
in its scope. This is primarily GLTF ergonomics: an avatar should keep one authored outline policy
on its `GLTF` while individual imported meshes or node subtrees can be excluded or assigned a
different width/color.

The renderer should still receive only the effective `Option<ToonOutlineParams>` for each existing
`VisualInstance`. Selector matching must not happen while recording frames, create extra visual
instances, duplicate geometry, or add new render operations.

## Proposed MMS API

Use an array only for the simple list accepted by `excluding_renderables`. Use one repeated
`for_matching` builder call per override rule:

```mms
GLTF.new("assets/models/avatar.glb") {
    ToonOutline
        .width(0.012)
        .color([0.015, 0.008, 0.025, 1.0])
        .excluding_renderables([
            "#Head",
            "#Face",
        ])
        .for_matching("#Hair", {
            width = 0.006
        })
        .for_matching(".accessory", {
            width = 0.018
            color = [0.06, 0.015, 0.09, 1.0]
        })
        .for_matching("#Glasses", {
            color = [0.01, 0.01, 0.015, 1.0]
        }) {}
}
```

The names above are illustrative, not asserted Bisket/VRoid node names. A smoke-test scene should
use selectors confirmed against the imported asset.

The same API works as a non-GLTF wrapper:

```mms
ToonOutline
    .width(0.01)
    .excluding_renderables(["#no_outline"])
    .for_matching(".hero", { width = 0.02 }) {
    T { name = "ordinary" R.cube() }
    T { id = "no_outline" R.sphere() }
    T { class = "hero" R.capsule() }
}
```

The target arguments also accept live component values. These are retained as GUID-backed
`ComponentRef` values rather than flattened into selector strings:

```mms
let special_mesh_group = T { class = "hero" R.capsule() }

ToonOutline
    .excluding_renderables([special_mesh_group])
    .for_matching(special_mesh_group, { width = 0.02 }) {
    special_mesh_group
}
```

## Why repeated `for_matching` calls

Prefer repeated builder calls over an array of generic rule tables.

- Each call reads as one ordered rule: target reference first, settings second.
- Authors do not have to assemble a JavaScript-like array of anonymous records.
- MMS already preserves repeated builder calls in authored order. Components such as
  `MorphTargetMap` and `HumanoidBoneMap` use repeated `.slot(...)` calls as precedent.
- The component can append a typed `ToonOutlineMatchRule` for each call.
- Serialization can reproduce the same ordered calls directly.
- Adding another rule creates a small diff without restructuring an array.

Repeated calls are therefore part of the proposed contract, not an accidental parser behavior.
Calling `excluding_renderables` more than once should likewise append references rather than replace
the previous list.

Do not add an alternative `matching_rules([{ selector = ..., settings = ... }])` API in the first
slice. Supporting two equivalent authoring shapes adds parsing and serialization ambiguity without
improving expressiveness.

## Settings-table contract

The second argument to `for_matching` is a deliberately small typed record, not arbitrary data.
For the first slice it accepts only:

```text
width: finite, non-negative number
color: four finite color channels, normalized by the same rules as ToonOutline.color
```

Both fields are optional. An empty table and unknown fields are errors. Alpha remains authored for
future use, but the current outline pass is opaque.

Examples:

```mms
.for_matching("#Face", { width = 0.004 })
.for_matching("#Hair", { color = [0.03, 0.01, 0.05, 1.0] })
.for_matching(".jewelry", {
    width = 0.002
    color = [0.08, 0.06, 0.02, 1.0]
})
```

Exclusion remains a separate operation instead of overloading `width = 0`. This preserves the
difference between “do not participate in the outline phase” and “an override happened to resolve
to zero width,” and gives the system a direct `None` result for excluded renderables.

## Rule resolution and precedence

Resolve one effective value for each renderable using this deterministic cascade:

1. Start with the component's base `width` and `color`.
2. Visit `for_matching` rules in authored order.
3. For each matching rule, replace only the fields present in its settings table.
4. Later matching rules therefore win per field. Selector specificity has no special meaning.
5. After applying overrides, any match from `excluding_renderables` produces `None` and wins
   absolutely.

For example:

```mms
ToonOutline
    .color([0.0, 0.0, 0.0, 1.0])
    .for_matching(".hair", { color = [0.04, 0.01, 0.06, 1.0] })
    .for_matching("#HairFront", { width = 0.004 }) {}
```

A renderable matching both rules receives the hair color and the narrower width. Authored order is
visible and stable; introducing CSS-style specificity would make the result harder to predict.

The existing component-scope rule still applies before this internal cascade: the nearest local
`ToonOutline` component wins over an inherited or GLTF-projected policy. Rules from two different
`ToonOutline` components are not merged.

## Target references and what they match

Store every exclusion target and match-rule target as `ComponentRef`. A string argument uses the
engine's existing component query syntax and becomes `ComponentRef::Query`. A component-object
argument becomes `ComponentRef::Guid`, following the existing durable-reference convention.

Matching still needs renderable-oriented semantics because useful GLTF names commonly belong to
imported node transforms rather than the primitive `RenderableComponent` itself.

A rule matches a renderable when its resolved target is either:

- that renderable directly; or
- an owning/container node in scope whose descendant primitive is that renderable.

For a normal wrapper, scope is the modifier's authored descendant tree. For a modifier attached to
a `GLTF`, scope is that specific GLTF instance's imported-node set—the same instance boundary used
by `event.gltf.query(...)`. A rule must never leak into another instance of the same asset or into
an unrelated sibling tree.

When a reference resolves to a container with several primitive children, the rule applies to all
of those primitives. When it resolves directly to one primitive, only that primitive is affected.

An unresolved query reference is allowed because optional meshes vary between avatar exports.
Record it in debug diagnostics, preferably once after GLTF initialization, rather than failing
scene loading. Malformed queries, out-of-scope GUID references, and invalid settings are authoring
errors.

## Component data model

Suggested authored representation:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ToonOutlineOverride {
    pub width: Option<f32>,
    pub color: Option<[f32; 4]>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToonOutlineMatchRule {
    pub target: ComponentRef,
    pub settings: ToonOutlineOverride,
}

pub struct ToonOutlineComponent {
    pub color: [f32; 4],
    pub width: f32,
    pub excluded_renderables: Vec<ComponentRef>,
    pub matching_rules: Vec<ToonOutlineMatchRule>,
    source_component: Option<ComponentId>,
}
```

The projected GLTF copies retain the full ordered policy and their existing `source_component`
link. Every imported primitive may still receive a projected `ToonOutlineComponent`, including an
excluded primitive. Exclusion means its effective visual parameter is `None`; keeping the
projection preserves source-update fan-out and makes later policy changes able to re-enable it.

Do not place selector strings or rule tables in `VisualInstance`. They are ECS authoring policy,
not renderer state.

Do not store resolved `ComponentId` values in the serialized component either. Component IDs are
runtime-local and imported GLTF targets do not exist when the authored component is initially
materialized. Keep durable `ComponentRef` values in the component and keep any resolved target
sets in `RenderableSystem` runtime state.

## Reference-resolution plan

`ComponentRef` is the right stored type, but the existing `resolve_component_ref` helper returns
only one component. `for_matching` and `excluding_renderables` intentionally allow one query to
match several nodes, so add a dedicated internal resolver:

```rust
fn resolve_outline_targets(
    world: &World,
    scope: ToonOutlineScope,
    target: &ComponentRef,
) -> Result<Vec<ComponentId>, OutlineTargetError>;
```

Resolution behavior:

- `ComponentRef::Guid` performs the normal O(1) GUID lookup, then verifies that the result belongs
  to this modifier's ordinary-wrapper or GLTF-instance scope.
- `ComponentRef::Query` runs an all-results query within the scope. It must not call the existing
  first-result-only helper.
- A GLTF scope uses that component's `scripting_query_roots`, which includes the imported roots
  tracked by `GLTFComponent::spawned_node_transforms` without leaking into another asset instance.
- An ordinary wrapper scope uses the modifier's authored subtree/root.
- Results are deduplicated while preserving deterministic DFS order.
- A matched renderable is a direct target. A matched container expands to only the descendant
  renderables belonging to the same policy scope.
- A GUID resolving outside the scope is an error. A query with zero results is allowed and may
  produce a once-per-initialization diagnostic.

At registration or source-policy update, resolve the policy in one pass:

1. Collect the renderables governed by this authored source component.
2. Resolve every exclusion/reference rule to scoped component IDs.
3. Expand container targets to a deduplicated set of governed renderable IDs.
4. Initialize each governed renderable with the base outline parameters.
5. Apply ordered match-rule field overrides to the matching renderable IDs.
6. Replace excluded renderables with `None` after all overrides.
7. Compare these effective results with the current visual values and update only changed
   instances.

The first implementation can hold the temporary target sets only for the duration of this pass.
If repeated live policy editing later makes resolution measurable, cache a compiled mapping keyed
by the source component and invalidate it on policy or relevant topology changes.

## GLTF integration

The current GLTF projection point remains the right ownership boundary:

1. Resolve the authored `ToonOutline` policy at the `GLTFComponent`.
2. Spawn the imported node/primitive tree.
3. Add a non-serialized, source-linked projection below every generated renderable as today.
4. Resolve the policy's `ComponentRef` targets within this GLTF instance's query scope and evaluate
   that primitive against the resulting target sets.
5. Send either `Some(ToonOutlineParams)` or `None` to its one `VisualInstance`.

Do not resolve references inside `spawn_node_recursive`: at that point the complete imported-node
set has not yet been written to `GLTFComponent::spawned_node_transforms`. Resolution should happen
from the queued projection-registration intents after the GLTF tick has completed that metadata,
or from one explicit post-spawn policy-application step after the metadata assignment.

Selector evaluation should occur when projections/renderables are registered, not every frame.
Cache the effective result on the visual instance through the existing `toon_outline` field.

When the authored source policy changes, copy the new policy to its projections, reevaluate each
associated target, and dirty outline membership/instance data only when the effective result
changes. The first implementation may use the current source-linked scan; a source-to-consumer
index is a later optimization if profiling justifies it.

Imported-node queries need an explicit GLTF-instance boundary. Do not rely on walking from an
imported renderable back to the authored `GLTFComponent`: imported transforms are attached beneath
the GLTF's transform anchor and are not ordinary descendants of the `GLTFComponent` itself.

## Renderer impact

No renderer architecture change is required. After ECS resolution:

```text
excluded target        -> VisualInstance.toon_outline = None
included base target   -> Some(base params)
matching override      -> Some(resolved params)
```

The existing outline order and ordinary `DrawBatch` construction consume those effective values.
Different colors and widths remain per-instance attributes, so they do not split batches. All
targets continue to share their existing mesh and, when skinned, their existing deformation-cache
range.

This feature must not add per-frame selector queries or per-rule draw lists.

## First implementation slice

Implement one vertical slice:

1. Add ordered exclusion references and match rules to `ToonOutlineComponent`.
2. Register repeatable MMS builders:
   - `excluding_renderables(component-ref-or-array)` using `arg_component_ref_vec`;
   - `for_matching(component-ref, settings-table)` using `arg_component_ref`.
3. Validate and normalize rule settings through the same helpers used by base `width` and `color`.
4. Serialize exclusions and repeated rules without losing authored order.
5. Resolve direct renderable and container-node matches for ordinary wrapper scopes.
6. Resolve the same semantics inside one GLTF instance and recompute projected effective values.
7. Keep excluded projections but set their visual outline value to `None`.
8. Extend the Bisket outline example with at least one confirmed exclusion and two visibly
   different matching settings.

The first slice does not need live mutation methods for individual rules. It must, however, keep
the source/projection representation compatible with later whole-policy updates.

## Tests

Add focused tests for:

- query-backed and GUID-backed component references materialize and serialize correctly;
- repeated `for_matching` calls are preserved in order;
- repeated `excluding_renderables` calls append;
- omitted fields inherit the previously resolved value;
- later matching rules win per field;
- exclusion wins regardless of builder-call position;
- selecting an imported container affects all primitive renderables beneath it;
- references do not cross between two instances of the same GLTF;
- unmatched query references do not fail GLTF initialization;
- excluded primitives retain their projected component but have
  `VisualInstance.toon_outline == None`;
- included and overridden primitives still use one visual instance, mesh handle, and deformation
  range each; and
- serialization/materialization round-trips ordered rules and exclusion references.

## Follow-up roadmap

- Live methods to append, replace, remove, or clear rules with source-linked projection updates.
- Editor presentation of the resolved rule and source reference for a selected primitive.
- Once-per-load diagnostics listing selectors that matched no renderables.
- A source-to-projected-consumer index if policy updates become expensive on large scenes.
- Optional named rules if targeted live editing proves awkward with order-only identity.
- Per-camera visibility remains a separate feature and should not be folded into outline matching.

## Decisions captured by this draft

- Store targets as `ComponentRef` and use repeated `.for_matching(target, settings)` calls, not an
  array of rule records.
- Preserve call order and use later-match-wins, per-field cascading.
- Keep `excluding_renderables` separate and absolute.
- Treat matched imported container nodes as selecting their descendant primitives.
- Resolve references on ECS/GLTF changes, never during rendering.
- Keep one projected component and one visual instance per primitive; store only effective outline
  parameters in the renderer.
