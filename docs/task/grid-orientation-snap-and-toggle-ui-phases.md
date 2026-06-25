# Grid orientation snap and toggle UI phases

Date: 2026-06-25

This task note captures a phased plan for two related editor changes:

- make toggle/state UI consistent across editor settings and grid panel rows
- add grid-level settings UI that can later drive orientation snapping behavior

This should be done in phases, in this order:

1. standardize the existing UI/state language first
2. add a grid settings card / top section to the grid panel
3. add grid orientation snap behavior behind that settings UI

This is intentionally a planning doc only. It is not proposing code changes yet.

## Why this needs a phased plan

Right now there are two separate concerns mixed together:

- the UI is inconsistent about whether a label/button means the current state or the action that will happen if clicked
- the grid panel does not yet have a clear place for grid-wide settings such as orientation snapping

If we add orientation-snap controls before fixing the UI language, we will just extend the same inconsistency into a new settings area.

So the immediate priority is not snapping behavior. The immediate priority is making the existing controls legible and consistent.

## Scope

This task covers:

- editor settings panel toggle presentation for `Show Armature`
- grid panel row toggle presentation for enabled / visible state
- grid panel top-level settings section for grid-wide settings
- a future grid-orientation-snap setting

This task does not yet define the exact runtime data model for persistent grid settings.

This task also does not resolve the broader editor selection issues.

## Current state

### 1. Editor settings uses a checkmark pattern for armature visibility

Today `Show Armature` uses a checkmark-style indicator rather than a button-like on/off state treatment.

That makes it visually different from the grid panel controls and weaker as a reusable toggle pattern.

Relevant files:

- [docs/task/armature-visualization-toggle.md](/home/rei/_/cat-engine/docs/task/armature-visualization-toggle.md)
- [src/engine/ecs/system/editor/settings_panel.rs](/home/rei/_/cat-engine/src/engine/ecs/system/editor/settings_panel.rs:124)

### 2. Grid panel row buttons are action-oriented rather than state-oriented

The current grid panel row controls expose enable/disable and visibility operations, but the labels/icons are easy to interpret as "what clicking does" rather than "what state this grid is currently in".

That is a UX tax because users have to mentally translate:

- "is this grid enabled now?"
- "or does this button mean clicking will enable it?"

We should standardize on the controls describing the current state.

### 3. Grid panel does not yet have a grid settings section

The current grid panel is row-oriented. It is good for:

- selecting a grid
- per-grid actions

It is not yet structured for:

- global grid-tool settings
- active-grid settings
- orientation-snap policy

Relevant docs:

- [docs/task/grid-panel-and-grid-inspector.md](/home/rei/_/cat-engine/docs/task/grid-panel-and-grid-inspector.md)
- [docs/task/grid-panel-select-delete-hide-and-gizmo.md](/home/rei/_/cat-engine/docs/task/grid-panel-select-delete-hide-and-gizmo.md)
- [docs/spec/grid-snapping.md](/home/rei/_/cat-engine/docs/spec/grid-snapping.md)

## UX policy to standardize

Before adding new controls, we should establish one policy and reuse it everywhere:

- stateful labels and toggles should describe the current state, not the action

Examples of the intended reading:

- `Visible` means the thing is currently visible
- `Hidden` means the thing is currently hidden
- `Enabled` means the thing is currently enabled
- `Disabled` means the thing is currently disabled
- `Show Armature` should become an explicit on/off state control, not a separate label with a checkmark ornament

This means the user should be able to read the panel without clicking anything and know the current state directly.

## Phase 1: make toggle/state UI consistent

This phase should happen first.

### Goals

- replace the current `Show Armature` checkmark presentation with an explicit on/off button treatment
- align the visual language of the editor settings panel and grid panel row toggles
- make row labels/buttons describe current state rather than the action to be performed

### Proposed UI direction

- Use a compact colored on/off pill or button beside `Show Armature`
- Reuse the same visual structure for grid row state toggles where possible
- Prefer the control text/icon meaning "current state"
- Use color as reinforcement, not as the only signal

For example:

- `Armature: On`
- `Armature: Off`
- `Visible`
- `Hidden`
- `Enabled`
- `Disabled`

The exact visual wording can still be adjusted, but the semantic policy should be fixed first.

### Phase 1 implementation targets

- editor settings panel authored UI
- editor settings panel sync/update logic
- grid panel row action rendering
- grid panel row action handling, if naming/payload assumptions need cleanup

### Phase 1 done when

- `Show Armature` no longer uses the current checkmark-only pattern
- grid row state controls and editor settings toggles follow the same state-language policy
- labels indicate current state rather than future action
- the UI looks internally consistent before new grid settings are introduced

## Phase 2: add a grid settings card / top section

After toggle consistency lands, the grid panel should gain a top section above the repeated grid rows.

### Goals

- create an obvious home for active-grid settings
- separate grid-wide / active-grid settings from per-row actions
- make future grid snapping controls discoverable

### Proposed structure

At the top of `grid_panel`:

- a small settings card or section header
- one or more settings rows under it
- below that, the existing repeated list of grids

The important part is the hierarchy:

- top section = settings for the currently active grid / grid behavior
- rows below = specific grid instances and per-grid actions

### Initial contents

The first setting to add here should be:

- `Snap Orientation`

Initially this can be UI-only if needed, but the structure should be designed so additional settings can fit naturally.

Potential future settings:

- orientation yaw policy
- use active grid for gizmo translation
- snap mode / placement policy

Those future settings are not part of phase 2 implementation unless needed to support the first row cleanly.

### Phase 2 done when

- grid panel has a clear top settings section
- the section is visually distinct from the grid instance list
- there is a stable place to host active-grid behavior settings
- the panel no longer implies that every grid-related control must live inside a row action cluster

## Phase 3: grid orientation snap behavior

Only after phases 1 and 2 should we implement the orientation behavior itself.

### Desired semantics

We want an option to snap orientation to a grid, with one important exception:

- preserve free yaw around the grid normal / up vector

That means the grid should constrain tilt/orientation relative to the grid frame, but should not force a single heading around the normal axis.

In other words:

- align the object's local "up" relationship to the grid frame
- do not quantize or overwrite yaw around the grid normal unless a separate policy is later added

### Why this is different from current snapping

Current grid snapping is mostly positional for gizmos, and placement-frame-based for surface placement.

See:

- [docs/spec/grid-snapping.md](/home/rei/_/cat-engine/docs/spec/grid-snapping.md)

Today:

- gizmos snap translation only
- free draw / grid tool use placement frames that affect orientation during placement
- there is no general "grid orientation snap for gizmo rotation" behavior

So this phase would introduce a new behavior class, not just a UI toggle over an existing generic feature.

### Likely design direction

The first version should probably target the simplest meaningful policy:

- when orientation snap is on, use the active grid frame as the orientation reference
- preserve yaw around the grid normal
- do not attempt more advanced angle quantization in the first pass

That keeps the feature coherent with existing surface-frame placement logic without immediately expanding into a full angular snapping system.

## Open design questions

### 1. What does `Snap Orientation` apply to?

Possible scopes:

- gizmo rotation only
- gizmo translation + placement preview orientation
- placement tools only
- all editor placement/manipulation workflows that have an active grid

This should be decided explicitly before implementation.

### 2. Is the setting per-grid, per-editor, or workspace-wide?

Possible homes:

- on `GridComponent`
- in grid-panel runtime/editor context state
- on the editor/workspace state

The UI placement suggests "active grid / grid behavior", but the runtime ownership still needs a clean source of truth.

### 3. What does "preserve yaw" mean exactly for arbitrary object bases?

We need a crisp transform definition for:

- which local axis is treated as the object's up/reference axis
- how we decompose current rotation into "yaw around grid normal" plus the remaining alignment

This should be specified before code work begins.

### 4. Should this apply during preview only, commit only, or both?

Preview and commit should ideally match.

If orientation snapping is added, it should not be preview-only unless that is explicitly intended as a temporary state.

### 5. Does the grid panel settings section configure the active grid or the tool/runtime?

This affects:

- selection changes
- multi-grid workflows
- whether changing the selected grid changes the settings row values

### 6. Do we eventually want separate toggles for position snap vs orientation snap?

That is likely the clean long-term direction, but it may be acceptable to stage the work:

- first add orientation snap only
- keep existing position snapping behavior unchanged

## Recommended execution order

### Step 1

Do the UI consistency pass first:

- editor settings `Show Armature`
- grid row state controls

### Step 2

Add the grid settings top section:

- card/header
- first settings row for `Snap Orientation`

### Step 3

Implement orientation snap behavior:

- define runtime ownership
- define rotation semantics
- wire the new setting into the relevant manipulation/placement path

## Definition of success

This task is successful when:

- existing toggle/state UI uses one clear, reusable state-language policy
- the grid panel has a dedicated settings section above the row list
- there is a clear implementation plan for `Snap Orientation`
- the eventual orientation-snap behavior can be added without reworking the panel structure again

## Related docs

- [docs/spec/grid-snapping.md](/home/rei/_/cat-engine/docs/spec/grid-snapping.md)
- [docs/task/grid-panel-and-grid-inspector.md](/home/rei/_/cat-engine/docs/task/grid-panel-and-grid-inspector.md)
- [docs/task/grid-panel-select-delete-hide-and-gizmo.md](/home/rei/_/cat-engine/docs/task/grid-panel-select-delete-hide-and-gizmo.md)
- [docs/task/grid-tool-and-surface-placement-followups.md](/home/rei/_/cat-engine/docs/task/grid-tool-and-surface-placement-followups.md)
- [docs/task/armature-visualization-toggle.md](/home/rei/_/cat-engine/docs/task/armature-visualization-toggle.md)
