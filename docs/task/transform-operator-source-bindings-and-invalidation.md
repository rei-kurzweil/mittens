# Task: Retained transform-operator sources and indexed dependency refresh

Date: 2026-10-12
Status: planned; this task adds documentation only. The existing uncommitted
translation-sampler/AVC changes are the starting implementation, not work
completed by this task.

## Outcome

Give transform operators a consistent runtime mechanism for retaining source
handles, invalidating bindings, and refreshing affected consumers. Remove the
whole-World parent-dependent discovery scan from normal transform updates.
Preserve the TOP authoring API, operator composition, existing fallback policies,
and XR capsule/contact behavior.

First migrate `parent`, `inverse_local`, and `sample_translation`. Then apply the
same lifetime/invalidation rules to contextual ancestor sampling and inspect
camera-settings discovery. Operators without sources do not need artificial
source bindings. This is internal runtime work, not another TOP API migration.

## Current behavior and cost

Relevant code:

- `src/engine/ecs/component/transform_operator.rs`: authored references and
  kind-specific reference resolution.
- `src/engine/ecs/system/transform_operator.rs`: evaluation, retained sampling
  state, inverse-local state/indexes, and routing-change observation.
- `src/engine/ecs/system/transform_system.rs`: propagation and dependent refresh.
- `src/engine/ecs/mod.rs`: component storage, GUID index, and query revision.
- `src/engine/ecs/system/system_world.rs`: mutation/removal and contact scheduling.

| Kind | Source resolution today | Consumer refresh today |
| --- | --- | --- |
| `parent` | Re-resolves ComponentRef during evaluation and other routing paths. GUID lookup is indexed; queries traverse their scope. A non-T target can supply its nearest ancestor T's world basis. | `update_transform_parent_dependents` enumerates all World components, identifies parent operators, re-resolves targets, and checks ancestry. |
| `inverse_local` | Re-resolves ComponentRef each evaluation. Already retains last valid inverse/input and source handles for its reverse index. | Existing source-to-consumers index refreshes consumers of a directly changed source. |
| `sample_translation` | Retains source handle, authored settings, input, revisions, and validity. Reads the source's live cached world matrix after binding. | Retained source-to-consumers index plus changed-source tracking and bounded refresh waves. Broad revisions revisit registered samplers. |
| Fork ancestor sampling | Walks ancestry to select the requested T on evaluation. | Inherits structural propagation; external/contextual changes need an explicit dependency audit. |
| Camera-specific settings | Discovers settings children and retains effective/input matrix state. | Existing specialized anchor/settings handling. |

The parent scan is **transform-change-driven**, not a once-per-frame timer.
Normal T updates reaching the end of `TransformSystem::transform_changed` invoke
it. Direct parent/inverse/sample refresh paths have early returns. Multiple
tracked, animated, physics, or scripted T updates can therefore cause multiple
whole-World parent scans in a frame, even when no parent operators are present.
`TransformOperatorSystem::tick` only records elapsed time; it does not itself
scan/evaluate all operators each frame.

Parsing a selector and retaining its matching handle are different caches.
World already caches parsed MMQ syntax and has an O(1) GUID index. Repeating a
query still traverses the matching scope; repeating GUID resolution does not.

## What current invalidation means

Invalidating a binding means its retained identity/validation may be stale.
Re-resolve/revalidate before treating it as ready. It does not mean delete the
source, reset its pose, or cache an old coordinate instead of reading live data.

The sampler currently uses these triggers:

| Trigger | Current effect |
| --- | --- |
| First evaluation | No retained state; resolve and validate. |
| Sampler `source`, `root`, or `axes` settings differ | Authored-settings comparison requests resolution/validation again, including an axes-only edit. |
| World add/remove/attach/detach/reparent, label/GUID edits | Changes `World::query_revision`; all retained sampler bindings are conservatively stale, including GUID bindings and unrelated scopes. |
| Any `get_component_record_mut` call | Increments query revision immediately, even when only non-query metadata is accessed, nothing is changed, or the requested ID is absent. This is a conservative mutation-access signal. |
| Ordinary typed pose write | Does not change query revision. Propagation marks changed sources and refreshes indexed consumers; evaluation reads current coordinates. |
| Sampler resolved-source relationship changes | Advances a global sampling relationship revision, requiring dependency validation again. |
| Evaluated parent/inverse authored routing settings change | `reference_routes` observes their settings and advances that same relationship revision so mixed dependencies are revalidated. This map does not cache their resolved handles. |
| SystemWorld subtree removal | Removes retained sample/route state and reverse links; source removal invalidates consumers. World removal also advances query revision. |
| A selected coordinate becomes non-finite | Value validation reports `NonFinite` and preserves input. A later finite pose can recover without selector re-resolution. |

These revisions are checked when evaluation/refresh/readiness runs. Low-level
World topology edits invalidate through revision changes but do not themselves
emit runtime transform-refresh intents; callers still need the normal mutation
and propagation boundary.

Currently a relationship revision change also causes the sampler to resolve its
selector again. Separate that from dependency validation in the new design.
The inherited-input cache also uses query revision as a conservative signal to
rebuild its structural basis; distinguish this from a name-only matching change.

## Binding and invalidation contract

Keep authored ComponentRef values on components for MMS/dump behavior. Own
runtime binding state in TransformOperatorSystem, keyed by live operator ID and
source role. A binding should retain the authored reference/scope, resolved
component, effective T when applicable, validity, and relevant versions.

Distinguish four types of change:

1. **Identity/resolution:** reference or scope changes, relevant query metadata,
   deletion, GUID reassignment, and a missing target becoming available.
2. **Structural/dependency validation:** attachment changes, effective-frame
   changes, and operator dependency/configuration changes that can create cycles.
3. **Value update:** the bound T's world matrix or authored local matrix changes.
   Refresh consumers without resolving an otherwise valid binding again.
4. **Kind-specific evaluation policy:** masks, selected camera mode, smoothing
   coefficients, etc. Recompute the result or appropriate state; an axes-only
   edit should not rerun a source query.

Start with safe coarse query/topology versions if needed. Batch reconciliation
of registered bindings once for a changed version, rather than doing the same
query work repeatedly for every pose event in that batch. Narrow GUID and
effective-frame invalidation first. Scoped-query invalidation must account for
targets entering/leaving the scope and ancestor/sibling-sensitive selectors;
do not assume that watching the current target alone is sufficient.

Audit World record mutation call sites before changing revision semantics.
Prefer explicit metadata/topology mutation signals over invalidating on raw
mutable access, but retain a safe conservative path while raw record mutation
remains available. Typed non-pose configuration changes must still invalidate
the relevant owner/dependencies. Do not lose invalidation through boxed/raw
component access, imports, live MMS edits, or direct World APIs.

## Shared dependency refresh

Index consumers by their effective input T and the data they consume:
world pose for parent/sampling; authored local pose for inverse-local. Preserve
parent's non-T-to-nearest-T normalization and scope behavior. Reparenting a
non-T target may change its effective T even when the target ID stays the same.

Mark changed sources during propagation and visit indexed consumers. Ancestor
motion must refresh descendants' world-pose consumers, even when those sources
are not independently passed to `transform_changed`. Local-pose consumers must
refresh on authored local edits even if the resulting world matrix is unchanged.
Input changes still propagate through ordinary child/output topology.

Order/settle mixed parent/inverse/sample chains before contact, camera-dependent
queries, or rendering consumes their results. Detect cycles without repeatedly
resolving query strings through the graph. Deduplicate affected work and avoid
refresh loops or repeated advancement of temporal stages within one update.
Keep explicit bounds/diagnostics for unsupported dependency depth.

Unchanged existing policies are part of the first migration:

- Parent unresolved routing retains existing follower behavior; audit its
  first-match query selection, non-T targets, and unresolved-root-scope fallback.
- Inverse-local retains its last valid inverse; a never-valid source suppresses
  output propagation. Preserve singular-source recovery.
- Sampling requires a unique concrete T and preserves input on invalid sources.
  AVC must keep contact/readiness disabled for invalid or pending sampling.
- Contextual ancestor skip counts T ancestors, not operator nodes.
- Camera-specific settings retain their existing specialized child contract.

Record any proposed policy tightening separately with explicit compatibility
tests. Cache unification must not silently change query selection or fallbacks.

## Testable slices

1. **Baseline and parent index:** instrument source-resolution counts,
   whole-World dependent-discovery counts, and propagated consumers. Record a
   release-build baseline with many unrelated components and repeated T updates.
   Register/retire parent bindings and index their effective T; replace the
   whole-World discovery scan while proving current parent behavior.
2. **Shared retained binding state:** migrate parent, inverse-local, and sampler
   to common binding/invalidation machinery. Preserve kind-specific value and
   fallback policies; remove duplicated indexes/revision trackers only after
   their responsibilities are covered. Prove pose updates do not rerun queries.
3. **Lifecycle and mixed scheduling:** cover metadata/scope edits, reparenting,
   deletion/recreation with reused slots, GUID changes, missing-target recovery,
   operator removal/configuration edits, and dependency chains/cycles. Refine
   broad invalidation without allowing stale handles or inputs. Settle at normal
   mutation/contact boundaries, including all fixed physics substeps.
4. **Contextual source audit:** retain ancestor-selected handles across pose
   updates, rebind on topology/skip changes, and cover sampled ancestor motion
   across intervening operators. Inspect camera-settings discovery costs and
   cache their handles if justified; mode changes select live retained settings.

Verification must include scoped and GUID sources, forward references, existing
non-T parent targets, canonical dump/reload, multiple consumers/avatars, direct
source and ancestor motion, inverse-local singularity/recovery, and invalid
sampler contact gating. Exercise the real SystemWorld phase order.

Measure a release workload with e.g. 1,000 parent consumers, 10,000 unrelated
components, and repeated updates to one source and to unrelated sources. Report
binding-resolution counts, World discovery scans, refresh counts, allocations,
and update time. Normal steady pose updates should produce zero parent-dependent
whole-World scans and zero selector resolutions for valid unchanged bindings;
unrelated updates must not visit every registered operator. Existing child/output
vector allocation is a separate cost; do not claim the whole transform path is
allocation-free merely because bindings are retained.

Smoke-test existing helper/gizmo followers and mittens-corp: physical HMD Y,
horizontal wall contact, jump/landing, and mount/dismount. Keep temporal state
identity, desktop behavior, live camera settings, and filtered runtime-helper saves.

## Related work

- [Transform operators spec](../spec/transform-operators.md)
- [Transform parent ComponentRef routing](transform-parent-component-ref-routing.md)
- [Axis-selective translation sampling](transform-sample-translation-axes.md)
- [Transform operator API cleanup](transform-operator-api-cleanup.md)
- [AVC capsule height independent of HMD tracking](avc-capsule-before-hmd-tracking.md)
