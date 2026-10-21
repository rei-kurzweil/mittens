# Host HID providers

`Universe::input_providers` owns zero or more `HidProvider` sources. Hosts call
`Universe::update_with_input_providers(dt)` to poll them and pass one combined
`InputState` to the existing ECS systems. `Universe::update(dt, &snapshot)` remains
available for callers supplying their own snapshots, including replay/tests.

## No providers

A new Universe has no HID providers. Its first provider-driven update prints
this informational message to stdout:

```text
[Input] No HID providers configured; keyboard/mouse input is idle.
```

The message is printed once per period with an empty collection, rather than
every frame. Updates continue normally. `Input` produces no transform updates,
and no gameplay key or text events are generated. OpenXR head/controller input,
animation, physics, and other simulation continue independently.

Removing the last provider clears held controls and latched arrow look on the
next update. Keyboard delivery silently forgets removed sources, so removal
does not fabricate gameplay key callbacks. Mouse release state cancels held
pointer interactions. Other providers' held controls remain active.

## Providers and multiple sources

Each `HidProvider::poll()` returns a current held-state snapshot and the keyboard,
text, button, and wheel transitions collected since its previous poll. A provider
must consume its frame events and clear held controls when focus/device access
is lost. `UserInput` implements the shared event/state processor. A consuming
crate can translate platform events to engine-owned `HidEvent` values and call
`UserInput::handle_event`, then use its `HidProvider::poll` implementation.
`WinitHidProvider` provides a shared handle for desktop window callbacks.

```rust,ignore
let provider_id = universe.input_providers.add(my_provider);
universe.update_with_input_providers(dt);
universe.input_providers.remove(provider_id);
```

- Keys/buttons are held while any provider holds them. One provider's release
  does not release another provider's held control.
- Keyboard records carry a stable source ID internally. Two keyboards can hold
  the same physical key without suppressing each other's down/press/up events.
  Public MMS key/code values retain their existing meaning.
- Provider poll order and event delivery order follow registration order;
  event order within each provider is preserved. This does not reconstruct a
  global hardware timestamp order across different providers.
- Text events are appended in that order, and wheel deltas are summed.
- The shared pointer follows the most recently active provider. Idle cached
  positions do not overwrite it. Changing source does not generate a cursor
  jump or a drag between unrelated device coordinates.

`Windowing::run_app` and the existing custom-input/frame-hook variants register
one Winit provider automatically, alongside providers already on the Universe.
`Windowing::run_app_with_providers` uses only the configured collection, which
can be empty. Window close/Escape remain host lifecycle controls.

The `openxr-only` example registers no HID providers and uses the same update
method. An Android activity adapter can later supply keyboard/touchpad events
through its own provider.

## Build boundary

`InputState`, `Key`, `KeyCode`, `NamedKey`, `PhysicalKey`, `MouseButton`, and
`HidEvent` are engine-owned types under `engine::user_input`. Consumers should
import these instead of Winit types. Physical identity, logical text, native
unidentified codes, and public MMS key/code spellings remain distinct.

The default `desktop` Cargo feature provides `WinitHidProvider`,
`UserInput::handle_window_event`, `Windowing`, and window renderer APIs. Disable
default features to build the library without Winit. Generic `UserInput`, the
provider collection, and XR-only rendering remain available. Android activity
input translation belongs in the consuming Android host and is not implemented
yet. See [Android library builds](../../android/docs/base-library-build.md).

## Validation

Focused input/keyboard/text tests pass with desktop support enabled and disabled.
The public-API consumer test supplies input from an external crate without
Winit. Coverage includes zero sources, actual desktop movement from one source,
overlapping holds from two sources, pointer handoffs, focus loss, and removing
the last source after an arrow key was held. The zero-source test polls three
frames and prints the informational message once. Shared HID-event tests cover
modifier/key identity, logical text, repeat, and focus loss. Desktop adapter
tests cover platform key/native-code conversion and window pointer events.

The desktop cube/bloom example reached the present loop through the Winit
provider path. A bounded XR-only run completed successfully with no HID
providers and printed the idle message exactly once; that run reached the
SYNCHRONIZED session state. Android input delivery remains untested.
