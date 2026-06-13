# Inspector Sidebar Selection Explicit Event And Stopgap Reduction

## Status

Planning note only.

No implementation yet.

## Problem

The inspector sidebar currently updates its visual row highlight when a row is clicked, but the detail view does not update to match.

Current behavior indicates that the click path mutates panel-local state in the runtime adapter, while the detail view only refreshes when the panel model is rebuilt and rerendered.

That split is wrong for two reasons:

- the sidebar row focus is an inspector-domain event and should be represented explicitly in inspector reducer state
- the current stopgap MMS adapter is still mutating `focused_row` ad hoc instead of translating UI input into a real inspector event

## Current behavior

Today the relevant behavior is split like this:

- [`src/engine/ecs/system/editor/inspector_panel.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/inspector_panel.rs:1)
  owns:
  - `InspectorWorkspaceState`
  - `InspectorPanelState`
  - `InspectorWorkspaceEvent`
  - reducer logic
  - sidebar row model building
  - detail model building

- [`src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1)
  currently also owns:
  - decoding sidebar click payloads
  - mutating `panel.subtree_selection.focused_row`
  - deciding whether rerender happens after that mutation
  - the actual sidebar/detail rerender calls

The specific broken path is:

1. sidebar row click is decoded in the stopgap adapter
2. the adapter mutates `panel.subtree_selection.focused_row`
3. the adapter intentionally avoids rerender for that row-click path
4. detail model never rebuilds
5. detail slot remains stale

That means the logical event exists, but it is not modeled as a reducer event.

## Goal

Move inspector sidebar row selection into the inspector domain model explicitly.

The intended flow should be:

1. user clicks an inspector sidebar row
2. runtime bridge decodes the clicked row payload
3. runtime bridge emits an explicit inspector event
4. `reduce_inspector_workspace_state(...)` computes the next state
5. rerender logic compares old/new inspector models and refreshes the detail view as needed

The stopgap adapter should translate events and run effects, not own inspector-local mutation semantics.

## Proposed event model

Add an explicit event to [`inspector_panel.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/inspector_panel.rs:1), for example:

- `InspectorWorkspaceEvent::SidebarRowFocused { panel_id, component }`

Exact naming can vary, but the event should mean:

- the active inspector panel sidebar focus changed
- the focused row is panel-local state
- this is not the same as the world-panel semantic selection event

This event should update:

- `panel.subtree_selection.focused_row`

It should not:

- retarget the inspected root by itself
- spawn a new inspector panel
- change pin state

Those remain separate inspector workspace events.

## Required reducer changes

In [`inspector_panel.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor/inspector_panel.rs:1):

- extend `InspectorWorkspaceEvent` with an explicit sidebar-focus event
- update `reduce_inspector_workspace_state(...)` to handle that event
- keep the reducer pure
- make the focused-row semantics explicit and centralized

The reducer should define:

- what happens if the target component is already focused
- what happens if the panel id does not exist
- whether focusing a row also makes that panel the active panel

Recommended behavior:

- focusing a sidebar row should also make that panel active
- if the row is already focused, next state should remain unchanged
- if the target component is missing, ignore the event

## Required model/view consequences

The detail model already derives from:

- `panel.subtree_selection.focused_row`
- falling back to `panel.inspected`

That means the model side is already close to correct.

What needs to become explicit is:

- sidebar row focus is a model-driving event
- detail rerender is a consequence of model change

The intended render contract should be:

- sidebar row focus changes may keep the sidebar subtree alive if the selection visual is already handled locally
- but detail model must still be re-evaluated
- if current infrastructure cannot rerender only the detail slot cleanly, refreshing the whole inspector panel model is acceptable first

## Stopgap adapter changes

In [`editor_inspector_system_stopgap_mms_adapter.rs`](/home/rei/_/cat-engine/src/engine/ecs/system/editor_inspector_system_stopgap_mms_adapter.rs:1), remove inspector-domain mutation logic that should live in the reducer.

Specifically, the stopgap adapter should stop doing this directly:

- finding the clicked sidebar row
- rebuilding sidebar rows locally just to recover the clicked target
- mutating `panel.subtree_selection.focused_row` inside adapter code

Instead, the adapter should:

- decode the clicked inspector row payload
- resolve the represented component
- dispatch the explicit inspector workspace event
- trigger the appropriate rerender/effect pass from reducer-owned state

## Things to remove from the stopgap adapter

After the explicit event exists, delete or shrink logic that is only compensating for missing reducer semantics:

- direct mutation of `panel.subtree_selection.focused_row`
- comments describing sidebar clicks as a special no-rerender mutation path
- local sidebar-row-to-target reconstruction that exists only to mutate state in place

What may remain temporarily:

- click decoding
- calling the reducer
- panel rerender orchestration
- MMS slot rendering

That is acceptable while the stopgap adapter still exists.

## Preferred intermediate architecture

The intermediate clean split should be:

- `inspector_panel.rs`
  - event definitions
  - state transitions
  - panel model building

- runtime bridge / adapter
  - payload decoding
  - event translation
  - effect execution
  - rerender orchestration

This keeps the stopgap adapter thinner even before it is fully removed.

## Suggested implementation phases

### Phase 1: explicit event

- add explicit inspector sidebar-focus event in `inspector_panel.rs`
- handle it in `reduce_inspector_workspace_state(...)`
- document expected active-panel / focused-row semantics

### Phase 2: route sidebar clicks through reducer

- update the stopgap adapter to translate sidebar row clicks into that event
- stop mutating `focused_row` directly in adapter code

### Phase 3: rerender from model change

- after reducer state changes, refresh inspector panel rendering from workspace state
- at minimum, ensure detail slot rerenders
- optionally keep sidebar rerender coarse at first if needed

### Phase 4: remove stopgap-only inspector mutation logic

- delete the ad hoc sidebar row mutation path
- delete comments and branches that exist only because the event was missing
- keep the adapter limited to translation/effects

## Acceptance criteria

- clicking an inspector sidebar row updates `focused_row` through an explicit reducer event
- clicking an inspector sidebar row updates the detail view
- no adapter code directly mutates `panel.subtree_selection.focused_row`
- sidebar row focus semantics are defined in `inspector_panel.rs`
- stopgap adapter responsibilities are reduced, not expanded

## Non-goals

- fully deleting the stopgap MMS adapter in this task
- redesigning inspector layout
- changing world-panel semantic selection rules
- optimizing rerender granularity beyond what is needed for correctness

## Related docs

- [editor-ui-rerender-audit-and-clean-reducer-boundary.md](/home/rei/_/cat-engine/docs/task/editor-ui-rerender-audit-and-clean-reducer-boundary.md:1)
- [inspector-panel-phase0-pin-and-multi-instance.md](/home/rei/_/cat-engine/docs/task/inspector-panel-phase0-pin-and-multi-instance.md:1)
- [shared-editor-ui-routing-and-paint-state-manager.md](/home/rei/_/cat-engine/docs/task/shared-editor-ui-routing-and-paint-state-manager.md:1)
- [nested-reducers-for-panels.md](/home/rei/_/cat-engine/docs/draft/nested-reducers-for-panels.md:1)
