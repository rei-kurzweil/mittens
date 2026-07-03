# Paint panel shell does not take panel focus

## Status

Open bug / investigation.

## Symptom

The Paint panel cannot currently be selected as the focused panel, even though the other
 editor-created panels participate in panel focus selection.

This is distinct from paint-tool selection inside the panel. The issue is with selecting the
outer Paint panel shell itself as the active panel.

## Repro

- Run an editor scene with the standard editor-created panels visible.

Steps to reproduce:
1. Start an editor scene that spawns the world, inspector, assets, and paint panels.
2. Click the Paint panel shell, title bar, or other panel chrome where panel focus should
   resolve to the outer panel option.
3. Compare the result with clicking the world, inspector, or assets panel shells.

## Expected behavior

The Paint panel shell should behave like the other editor panel shells:

- clicking the Paint panel surface should select the Paint panel in the top-level panel
  focus `Selection`
- the Paint panel should receive the same visible focused-panel treatment as peers
- nested tool selection inside the Paint panel should remain a separate inner selection scope

## Actual behavior

The Paint panel does not become the selected/focused panel when clicked.

## Likely investigation targets

- `src/engine/ecs/system/inspector_system_stopgap_mms_adapter.rs`
  Confirm the paint panel shell is spawned as an outer `Option` and attached under the same
  layout-root panel-focus scope as the other panel shells.
- `src/engine/ecs/system/selection_system.rs`
  Confirm click resolution is reaching the Paint panel shell `Option` instead of failing,
  resolving to a nested scope, or being blocked by another hit target.
- `assets/components/panels.mms`
  Confirm the Paint panel subtree shape still matches the intended nested
  `panel shell Option -> content -> inner Selection` structure.
- `docs/task/refactor/selection-option-topology.md`
  Compare the runtime behavior against the intended outer-panel / inner-tool-scope contract.

## Questions to answer

- Is the Paint panel shell actually receiving the click hit, or is another renderable
  intercepting it first?
- Does the outer Paint panel shell `Option` exist at runtime and sit under the top-level
  panel-focus `Selection`?
- Is the Paint panel click incorrectly resolving into the inner tool `Selection` instead of
  the outer panel-shell option?
- Is this related to the known assets/paint panel overlap bug, causing clicks on apparent
  Paint-panel chrome to hit another sibling panel instead?

## Related

- `docs/bugs/panel-layout-selection-interaction.md`
- `docs/task/paint-panel-selection-and-panel-focus.md`
- `docs/task/refactor/selection-option-topology.md`

