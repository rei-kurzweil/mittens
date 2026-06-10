# Unifying Selection and TextInput Focus

This document analyzes the current focus models for `SelectionComponent` and `TextInputComponent` and proposes a unified architecture that supports focus, keyboard navigation (arrow keys), and multi-selection (shift-arrow and click-drag) for both.

## Current State

### `TextInputComponent` Focus
- **Component State**: `TextInputComponent` has a `focused: bool` field.
- **System State**: `TextInputSystem` maintains a private `focused: Option<ComponentId>`.
- **Logic**:
    - `TextInputSystem::tick_with_queue` receives `InputState` and processes `text_input_events` (Insert, Backspace, MoveCaret, etc.) only if the component matches the system's `focused` ID.
    - Focus is set via `IntentValue::TextInputSetFocus` and `IntentValue::TextInputClearFocus`.
    - `TextInputMoveCaret` and `TextInputMoveCaretTo` are used for navigation.

### `SelectionComponent` Focus
- **Component State**: `SelectionComponent` has NO focus concept. It tracks `selected_entries`, `selected_index`, and `mode` (Single/Multiple).
- **System State**: `SelectionSystem` has no tracking of focus.
- **Logic**:
    - `SelectionSystem` installs a global `Click` handler that resolves clicks to `OptionComponent` nodes within a `SelectionComponent` scope.
    - It updates `SelectionComponent` state and emits `SelectionChanged` events.
    - It has no keyboard event handling.

## The Case for Unified Focus

Selection components in the editor (World Panel, Asset Panel, Inspector) currently feel "static" because they cannot be navigated via keyboard. A unified focus model would allow:
1. **Selection Keyboard Navigation**: Arrow keys move the selection index when the selection scope is focused.
2. **TextInput as Selection**: `TextInput` is conceptually a selection over a character sequence. Multi-selection in `TextInput` (selecting a range of text) is equivalent to multi-selection in a list.
3. **Shared Focus Events**: `Focus` and `Blur` events that can be used by any component (e.g., to show a focus ring).

## Refined Event Model: Selection Movement

To support VR, bespoke keyboards, and standard winit events uniformly, we should generalize caret/selection navigation intents.

### 1. Generalized `Focus` Intent
Replace component-specific focus with generic focus:
```rust
enum IntentValue {
    SetFocus { component_id: ComponentId },
    ClearFocus,
    // ...
}
```

### 2. Generalized `SelectionMove` Intents
Instead of `TextInputMoveCaret`, use abstract movement intents that any focusable component can interpret:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionDirection {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
}

enum IntentValue {
    // ...
    SelectionMove {
        direction: SelectionDirection,
        amount: usize,
        /// If true, expand the selection range from the anchor to the new position.
        /// If false, move both the "caret/focus" and the anchor to the new position.
        select: bool,
    },
    SelectionMoveTo {
        index: usize,
        select: bool,
    },
}
```

#### How components interpret `SelectionMove`:
- **`TextInputComponent`**:
    - `Left`/`Right`: Move caret by `amount` characters.
    - `Up`/`Down`: Move caret to previous/next line (if multiline).
    - `Home`/`End`: Move caret to start/end of line or text.
- **`SelectionComponent`**:
    - `Up`/`Down`: Move `selected_index` ±1 in a vertical list.
    - `Left`/`Right`: Move `selected_index` ±1 in a horizontal grid/row.
    - `Home`/`End`: Move to first/last option.

### 3. Multi-Selection Range Logic (The Anchor)

To support `Shift + Arrow` and `Click + Drag`, both components need an "Anchor":
- `SelectionComponent` adds `anchor_index: Option<usize>`.
- `TextInputComponent` adds `selection_anchor: Option<usize>`.

#### Logic for `SelectionMove { ..., select: true }`:
1. Calculate `new_position`.
2. Update the active `caret` or `selected_index` to `new_position`.
3. The selection range is now everything between `anchor_index` and the active position.
4. `anchor_index` remains UNCHANGED.

#### Logic for `SelectionMove { ..., select: false }`:
1. Calculate `new_position`.
2. Update the active `caret` or `selected_index` to `new_position`.
3. Set `anchor_index = new_position`.
4. Selection is now just the single item/position at `new_position`.

## Proposed Architecture

### 1. Focus Tracking
A central system tracks the "Keyboard Focus" node. Only the focused node receives `SelectionMove`, `InsertText`, etc.

### 2. Input Mapping
The `UserInput` system or a dedicated `InputMappingSystem` translates raw `winit` events (or VR controller actions) into `SelectionMove` intents:
- `ArrowDown` -> `SelectionMove { direction: Down, amount: 1, select: false }`
- `Shift + ArrowDown` -> `SelectionMove { direction: Down, amount: 1, select: true }`

### 3. Event Signals for UI
MMS components can listen for generic focus/blur and selection changes:
```mms
Selection {
    on("Focus", { ... })
    on("SelectionChanged", { ... })
}
```

## Summary of Changes

1.  **Enums**:
    - Add `SelectionDirection`.
    - Add `SelectionMove` and `SelectionMoveTo` to `IntentValue`.
2.  **Component Updates**:
    - `SelectionComponent`: add `focused: bool`, `anchor_index: Option<usize>`.
    - `TextInputComponent`: add `selection_anchor: Option<usize>`.
3.  **System Updates**:
    - `FocusSystem`: Manages `SetFocus` / `ClearFocus`.
    - `SelectionSystem`: Implements `SelectionMove` logic.
    - `TextInputSystem`: Implements `SelectionMove` logic, replaces `TextInputMoveCaret`.
4.  **Input Translation**:
    - Update `UserInput` to emit generalized `SelectionMove` intents instead of caret-specific ones.
