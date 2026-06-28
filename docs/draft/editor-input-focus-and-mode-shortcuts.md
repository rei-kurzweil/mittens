# Editor Input Focus and Mode Shortcuts

## Why

We want editor hotkeys:

- `1` -> `Select`
- `2` -> `3D Cursor`
- `3` -> `Select + Cursor`

But these must not fire while the user is typing into a `TextInput`.

That requirement exposes a larger modeling problem:

- `InputState` is global raw input state
- `TextInputSystem` owns a private focused text-input target
- editor "focus" today mostly means panel routing / world-click eligibility
- those are not the same thing as "who should receive keyboard shortcuts right now?"

This note works out the shortcut problem first, then frames the broader focus model
needed so we stop conflating:

1. keyboard/text focus
2. panel routing focus
3. selection / active editor / world-click ownership

## Current Facts

### Raw keyboard input already exists

`InputState` already exposes edge-triggered key presses:

- `keys_pressed`
- `key_pressed(&Key)`

See:

- [src/engine/user_input.rs](../../src/engine/user_input.rs)

So there is no technical blocker to detecting `1`, `2`, or `3`.

### `TextInputSystem` already owns text-edit focus

`TextInputSystem` has its own private:

```rust
focused: Option<ComponentId>
```

and only applies `TextInputFrameEvent`s to that focused input.

See:

- [src/engine/ecs/system/text_input_system.rs](../../src/engine/ecs/system/text_input_system.rs)

This is the immediate reason we cannot just bolt `1/2/3` onto editor input handling:
the editor currently has no clean, shared way to ask whether a text input has keyboard focus.

### Editor "focus" is currently routing focus, not keyboard focus

`EditorContextState.focused_panel` is used for things like:

- whether paint tools are active
- which panel should receive world-click-derived behavior
- routing / gating editor-side interaction systems

See:

- [src/engine/ecs/system/editor/context.rs](../../src/engine/ecs/system/editor/context.rs)
- [docs/task/editor-input-routing.md](../task/editor-input-routing.md)

That is not the same as:

- whether keyboard shortcuts should fire
- whether typed characters belong to a text field
- what should happen when the user clicks a plain world object

## The Core Distinction

We need to separate three lanes that are currently too easy to blur together.

### 1. Keyboard Focus

This answers:

- where do `InsertText`, Backspace, arrow keys, and hotkeys go?

Examples:

- a focused `TextInput`
- the editor workspace itself
- nothing

This is the lane that must gate `1/2/3`.

### 2. Panel Routing Focus

This answers:

- which editor subsystems are allowed to react to scene/world clicks and drags?

Examples:

- paint panel focused
- grid panel focused
- world panel focused

This is what `focused_panel` mostly means today.

### 3. Selection / Active Editor Context

This answers:

- which editor tree is active?
- which component is selected?
- which editor root should receive mode changes?

Examples:

- `active_editor`
- `selected_component`
- `interaction_mode`

This is not itself focus. It is editor context.

## Shortcut Problem Statement

The desired semantics are:

1. If a `TextInput` has keyboard focus, `1/2/3` should be treated as text input, not editor mode hotkeys.
2. If no `TextInput` has keyboard focus, `1/2/3` may change editor interaction mode.
3. The target of that mode change should be the current editor context's active editor, not "whichever panel was last clicked".
4. Panel routing focus should remain orthogonal. A mode shortcut should not need to pretend the settings panel was clicked.

That means the shortcut should be modeled as:

- raw input observation from `InputState`
- gated by keyboard focus
- reduced into editor context

not as:

- another settings-panel click path
- another panel-focus side effect
- another `InputComponent` behavior inside `InputSystem`

## What Should Own `1/2/3`?

Not `InputSystem`.

`InputSystem` currently means transform-driving world input:

- WASD
- mouse drag camera rotation
- movement on `InputComponent`

That is a low-level movement/control system, not an editor shortcut coordinator.

The `1/2/3` behavior belongs in an editor-owned layer that consumes `InputState`.

Reason:

- the hotkey is editor-domain behavior
- it depends on editor context (`active_editor`)
- it depends on text-input focus
- it should remain independent of scene movement bindings

So the right architectural direction is:

- keep `InputState` as the raw source
- add an editor shortcut processor on top of it

Possible owners:

1. `EditorContextSystem`
2. a new `EditorShortcutSystem`
3. a broader future `FocusSystem` / `InputFocusSystem` plus editor shortcut reducer

For current scope, `EditorContextSystem` or a dedicated `EditorShortcutSystem` are the pragmatic candidates.

## Minimum Viable Architecture

To support `1/2/3` cleanly, we need three things.

### A. Shared read access to text-input keyboard focus

Today `TextInputSystem` keeps focus private.

We need a shared source of truth that other systems can consult without guessing.

Possible minimal shapes:

1. Expose `TextInputSystem::focused() -> Option<ComponentId>`
2. Mirror text-input focus into a small shared state object
3. Introduce a generic `InputFocusState` and make `TextInputSystem` update that

For the shortcut feature alone, option 1 is the smallest.

For the larger focus model, option 3 is the correct direction.

### B. A keyboard-focus model that is distinct from panel routing

We should add a dedicated shared state, conceptually like:

```rust
struct InputFocusState {
    target: Option<InputFocusTarget>,
}

enum InputFocusTarget {
    TextInput { component: ComponentId },
    EditorWorkspace,
}
```

Important point:

- this is not `focused_panel`

It is allowed for:

- `focused_panel == Some(PaintPanelRoot)`
- `input_focus.target == Some(TextInput { component })`

at the same time.

That is not contradictory. It just means:

- paint still owns world-click routing
- text input still owns keyboard input

### C. An editor shortcut pass that runs after focus is known

The shortcut pass should:

1. read `InputState`
2. check keyboard focus
3. ignore `1/2/3` if a text input owns focus
4. otherwise map keys to editor interaction-mode changes

Conceptually:

```rust
if input_focus.is_text_input_focused() {
    return;
}

if input.key_pressed(&Key::Character("1".into())) {
    set_mode(EditorInteractionMode::Select);
}
if input.key_pressed(&Key::Character("2".into())) {
    set_mode(EditorInteractionMode::Cursor3d);
}
if input.key_pressed(&Key::Character("3".into())) {
    set_mode(EditorInteractionMode::SelectAndCursor);
}
```

This should reduce through the same editor-context mode-change path the settings panel uses,
not create a second ad hoc mutation path.

## Proposed Focus Semantics

### Clicking a `TextInput`

Should do both:

1. set keyboard focus to that `TextInput`
2. leave panel-routing focus alone unless panel-specific logic wants it changed separately

That means:

- text input focus is a user-input concern
- panel focus is an editor routing concern

They may correlate, but they are not the same state.

### Clicking a panel row / panel background

Should:

1. clear text-input keyboard focus if the click is not inside the focused text input
2. possibly update panel-routing focus
3. possibly update selection / active editor context

Again: three separate consequences, not one "focus" concept.

### Clicking the 3D scene / arbitrary world object

Should:

1. clear text-input keyboard focus
2. possibly keep or change panel-routing focus, depending on editor policy
3. update active editor / selected component if the click resolves to an editor-managed target

This is the clearest example of why keyboard focus is orthogonal to panel-routing focus.

The user clicking a world object should usually blur a text field even if the currently
"focused panel" for routing remains, for example, the paint panel.

## Tick / Ordering Implications

`SystemWorld::tick(...)` already has both:

- `InputState`
- `TextInputSystem`
- `EditorContextSystem`

See:

- [src/engine/ecs/system/system_world.rs](../../src/engine/ecs/system/system_world.rs)

So the runtime can support this without threading new global dependencies through `Windowing`.

But ordering matters:

1. click-derived focus changes must be processed before shortcuts that depend on focus
2. text-input-owned text events and editor-owned shortcut events must not both consume the same key press ambiguously

This suggests a future split like:

1. raw input collected into `InputState`
2. click/focus intents resolved
3. shared `InputFocusState` updated
4. text input consumes text-edit events
5. editor shortcut layer consumes non-text hotkeys only when keyboard focus allows it

That sequencing is more important than whether the implementation lives in one system or two.

## Recommended Direction

### Near-term

Do not add `1/2/3` directly to `InputSystem`.

Instead:

1. introduce a draft-level `InputFocusState`
2. define keyboard focus separately from `focused_panel`
3. route editor mode hotkeys through editor context, gated by keyboard focus

### Mid-term

Move toward a shared focus model with explicit semantics:

- keyboard focus
- panel routing focus
- selection / active editor context

This is compatible with the earlier note in:

- [docs/analysis/unifying-selection-and-text-input-focus.md](../analysis/unifying-selection-and-text-input-focus.md)

but narrows the scope:

- this note is about keyboard-focus gating for editor shortcuts
- not full keyboard navigation for every `SelectionComponent`

## Non-Goals

This note does not require:

- unifying panel routing and text input focus into one field
- making editor settings panel own keyboard shortcuts
- making scene clicks and panel focus the same concept
- implementing generalized keyboard navigation for every selection scope yet

## Open Questions

1. Should editor mode hotkeys work whenever no `TextInput` is focused, or only when the editor workspace itself has keyboard focus?
2. Should clicking any non-text-input target always blur text input, or only clicks outside the focused text input subtree?
3. Should number keys be handled as logical characters (`"1"`, `"2"`, `"3"`) only, or should physical key location matter later?
4. Should there be a generic shortcut suppression mechanism for modal UI beyond text input?
5. Should `InputFocusState` live under editor workspace state, or become an engine-wide focus service shared by non-editor UI too?

## Suggested Follow-up Work

1. Add a small shared `InputFocusState` draft and decide ownership.
2. Decide whether `TextInputSystem` exposes focus directly as an interim step or updates shared state immediately.
3. Define the editor shortcut reducer/event path for interaction-mode changes.
4. Only after that, implement `1/2/3` as editor shortcuts gated by keyboard focus.
