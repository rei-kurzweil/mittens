# Component picker

Status: proposed.

## Problem

Mittens scenes are component trees, not game-object graphs. Several editor
features need an authorable reference to one particular scene component: an
explicit pose-apply destination is the first immediate case. The editor can
already ray-select scene geometry, but it has no reusable control that enters a
short-lived pick mode, receives the next scene hit, stores that component as a
reference, shows what was chosen, and lets the user clear it.

The first consumer is the pose panel's single `Apply pose to` override. The
picker itself must remain generic; a consumer supplies the semantic validation
rule (for the pose panel: the picked component must be, or must belong to, one
unambiguous glTF armature).

## Proposed UI

The component picker is a compact panel field, not a `SelectionComponent` or a
row of `Option` controls:

```text
Apply pose to   [ Pick component ]
Apply pose to   Capsule stick figure · Transform · ComponentId(427v1) [ × ]
Apply pose to   unresolved @uuid:…                                 [ × ]
```

- **Pick component** begins pick mode and changes to a concise cancel state,
  such as `Picking… Cancel`.
- Clicking/tapping a valid scene target commits exactly one component and ends
  pick mode.
- The resolved presentation is compact: readable component label when present,
  component type, and the runtime `ComponentId` (the SlotMap key) side by
  side. The inspector's existing label/type/ID presentation is the reference
  for wording and formatting.
- `×` clears the value and ends any active pick mode. It is visible only while
  a value or unresolved reference exists.
- A missing/deleted reference remains visible as an unresolved value until
  cleared or replaced; it must not silently turn into a different component.
- The field is single-value. Multi-target/batch picking is a separate future
  control, not an accidental consequence of the ordinary picker.

**Phase 1 integration:** install exactly one instance at the top of Pose-panel
content, before `New Pose Library` and all capture-library sections. An empty
field is shown as `Auto (editor selection / source)` for the pose panel's
compatibility fallback. This is deliberately the first pose-panel UI change;
do not add picker or apply-target controls to individual library sections.

## Data contract

The durable value is `Option<ComponentRef>`:

- Committing a live component stores `ComponentRef::Guid` using that
  component's stable UUID. A runtime `ComponentId` is a SlotMap allocation and
  must never be the persisted identity.
- A consumer may also author a query `ComponentRef`, preserving the normal MMS
  query semantics. The control displays the resolved component when there is
  exactly one result, otherwise its unresolved/ambiguous state.
- The owning consumer retains its reference in its own state/component; the
  picker is a UI/editor interaction primitive, not a global mutable target.
- Runtime resolution is refreshed before use and after relevant topology
  changes. Removal, reparenting, or ambiguity produces a visible invalid
  state; it never retargets by a label match.

For the Pose panel, store the picked reference as its apply-destination
override. On Apply, resolve it to a glTF root: accept the picked glTF itself or
its owning glTF, then run the same unique-joint preflight currently used for
pose application. The capture-library section remains the source of the pose.

## ComponentPickerSystem

Add a dedicated `ComponentPickerSystem` under the editor systems. This is not
the responsibility of `SelectionSystem`, `TransformGizmoSystem`, or the Pose
panel:

- `SelectionSystem` owns selection of known `Option` entries in bounded UI
  subtrees.
- the ordinary editor scene-selection path owns the editor's
  `selected_component` and transform-gizmo target;
- `ComponentPickerSystem` owns temporary, editor-scoped component-pick
  sessions and their one-shot scene-hit consumption;
- a consumer panel owns its persisted `ComponentRef` and supplies the
  acceptance/resolution policy for its field.

This separation lets the same picker serve future component-reference fields
without coupling them to pose capture, and prevents a generic scene-pick mode
from changing ordinary editor selection as a side effect.

## Pick-mode interaction contract

Starting pick mode asks `ComponentPickerSystem` to register one editor-scoped,
exclusive scene-hit consumer. It has priority over ordinary editor selection
for the next eligible world click/ray hit, consumes that hit, and then exits.
This prevents a picker click from simultaneously moving the transform gizmo or
replacing the editor's ordinary `selected_component`.

Existing `resolve_world_scene_hit` behavior is the starting filter: it already
rejects runtime editor UI, gizmo subtrees, and `Selectable.off()` helpers, and
normalizes a renderable hit to a preferred or nearest scene `Transform`. The
picker consumes that normalized component. A consumer can reject it after the
generic pick—for example, the Pose panel rejects a component outside any
glTF—then keeps pick mode active with an explanatory status rather than
recording an invalid value.

While mode is active:

- the initiating field has a clear active treatment and cancellation control;
- panel clicks and excluded editor helpers are never candidates;
- cancel, Escape where available, deactivation of the owning panel/editor, or
  successful commit ends the session cleanly;
- only one picker session is active per editor workspace. Starting another
  picker cancels the old session rather than leaving two consumers racing for a
  hit.

`ComponentPickerSystem` publishes explicit lifecycle results to the initiating
field/consumer: started, candidate-rejected with reason, committed, cancelled,
and invalidated. The panel turns those into its field state and status text;
the system must not contain pose-panel-specific labels or glTF rules.

`SelectionComponent` remains useful for bounded, authored option lists. It is
not the implementation vehicle for this control: its scope is a known subtree,
whereas component picking is an ephemeral selection of arbitrary world scene
geometry.

## Visual feedback

On a successful commit, render a non-interactive, runtime-only wireframe box
around the picked component's visual subtree for a short, defined duration
(initial target: 1.5 seconds). This gives immediate spatial confirmation even
when the picker field is distant from the object in XR.

- Derive the box from the same descendant bounds collection used by editor
  selection/highlight and glTF bounds visualization; do not guess mesh size or
  alter the selected model's transform.
- Parent helper visuals beneath a dedicated editor/runtime helper root, mark
  them non-serializable and `Selectable.off()`, and remove them at expiry,
  cancel, clear, or target removal.
- Use an emissive/high-contrast wireframe box. If the target has no visual
  bounds, show a small non-interactive pivot marker instead and retain the
  textual field value.
- This feedback is independent of the ordinary editor-selection highlight and
  must not replace, clear, or move the transform gizmo.

## Implementation slices

1. Add `ComponentPickerSystem`, picker state, and an editor-scoped pick-session
   coordinator, with a deterministic scene-hit priority/consumption rule.
2. Add reusable compact field rendering: idle, picking, resolved, unresolved,
   and clear states.
3. Add reference storage/resolution and lifecycle handling for GUID/query
   values.
4. Add the runtime-only wireframe/pivot feedback helper with a testable expiry
   mechanism.
5. **Phase 1:** Integrate exactly one picker at the top of the Pose panel. A
   non-empty valid value overrides Apply's implicit selection/source
   resolution; empty retains the legacy fallback. Do not add controls to pose
   library sections.
6. After the interaction is proven there, make the field usable by other
   component-reference editors without inheriting pose-specific validation.

## Acceptance criteria

- [ ] A user can start pick mode from the Pose panel, ray-pick the capsule
  figure in `capsule-stick-figure.mms`, and see its label/type/runtime ID plus
  a brief bounds highlight.
- [ ] The pick neither changes global editor selection nor moves the gizmo.
- [ ] Picking Bisket, a Bisket joint, or the capsule root resolves the expected
  owning glTF destination; picking stage geometry reports why it is rejected.
- [ ] Clear immediately restores the visible `Auto` pose-destination state and
  removes any active picker feedback.
- [ ] Deleted/unresolved/ambiguous references cannot cause Apply to emit pose
  updates.
- [ ] Existing panel selection, inspector selection, toggles, and gizmo
  interaction continue to work with no picker session active.
- [ ] Unit tests cover reference persistence/resolution, pick-session
  exclusivity, scene-hit consumption, cancel/clear/removal, and highlight
  cleanup; UI tests cover the compact field states.

## Out of scope

- A hierarchy browser, asset picker, or fuzzy text search for components.
- Picking several components in one ordinary field.
- Creating game-object abstractions or replacing `ComponentRef`.
- Cross-skeleton pose retargeting itself; the picker selects the destination,
  while the mapping/basis work remains in the capsule retargeting task.
