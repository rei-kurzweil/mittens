# Text Input

This document records the first-pass text input architecture.

## Scope

Version 1 is intentionally narrow:

- global app-wide focused `TextInput`
- explicit `TextInput` component authoring in MMS, with no shortform
- generic text-edit intents rather than raw keyboard-only behavior
- basic insertion, backspace, delete-forward, and left/right caret movement
- no selection, clipboard, IME composition, or tab-order yet

The global focus policy is a temporary implementation shortcut. Future multi-user or agent-driven focus should move focus ownership out of global runtime state and into per-user focus state.

## Signals

The engine boundary is generic edit intents:

- `TextInputSetFocus { component_id }`
- `TextInputClearFocus`
- `TextInputInsertText { text }`
- `TextInputBackspace`
- `TextInputDeleteForward`
- `TextInputMoveCaret { direction, amount }`

Observable events:

- `TextInputFocusChanged { old, new }`
- `TextInputChanged { component_id, text, caret }`

Desktop windowing is only one producer of these intents. VR keyboards, MMS widgets, and signal handlers should emit the same text-input intents directly.

## Ownership

`TextInputComponent` owns the editable string and caret index.

The visible glyphs still come from an ordinary descendant `TextComponent`. `TextInputSystem` resolves the nearest descendant text node and forwards text mutations through the existing `SetText` path so live editing reuses the current text rebuild behavior.

## Focus Routing

`TextInputSystem` installs a global click handler on the signal graph.

- Click on a renderable inside a `TextInput` subtree: emit `TextInputSetFocus`
- Click anywhere else: emit `TextInputClearFocus`

This keeps focus routing at the signal layer, not in layout or windowing code.

## Desktop Bridge

`UserInput` buffers frame-local `TextInputFrameEvent` values from winit keyboard events.

`TextInputSystem` translates those buffered platform events into generic text-input intents after gesture focus changes are processed for the frame. That ordering allows click-to-focus and typing to share the same runtime path.