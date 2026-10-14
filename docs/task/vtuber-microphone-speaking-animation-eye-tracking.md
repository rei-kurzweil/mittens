# Task: microphone-speaking VTuber HTC eye-tracking demo and info panel

Date: 2026-09-02

Status: interactive microphone-selection slice implemented; `AudioInput.devices()`
enumeration verified live; microphone capture/switching acceptance remains open

## Outcome and stop condition

Add a focused live-input acceptance scene at
`examples/vtuber-microphone-speaking-xr-eye-tracking-htc.mms`. It begins as
a copy of `vtuber-eye-tracking-mirror-eye-stabilize.mms`, keeps eye tracking
and amplitude-driven `viseme_aa`, uses a black clear background, and displays
the available microphone inputs in a world-space panel.

The scene consumes a reusable authored MMS asset,
`assets/components/ui/info_panel.mms`. The asset supplies title chrome, an
optional icon slot, content slot, and the existing accordion minimize/restore
behavior. It must be ordinary scene UI, not editor-owned UI.

The initial interactive slice stops when the new scene materializes, both
relevant examples have black clear backgrounds, and the panel displays one
clickable row per `AudioInput.devices()` name. Clicking a row switches the
scene's existing `AudioInput` to that session-local index.

This is an enumeration/setup panel. It does not make every listed device live
until clicked, and it does not require dynamic hot-plug updates while the panel
remains open.

## Existing API and device numbering

Use the session-local enumerated index with:

```mms
let microphone = AudioInput.device_number(1) {}
```

`AudioInput {}` selects the host default device. `AudioInput.device_number(0)`
selects the first name returned by `AudioInput.devices()`, and `1` selects
the second. Device numbering is intentionally session-local: users should
check the displayed/enumerated list before choosing an index.

`AudioInput.devices()` is a no-argument static host API returning
`string[]`. It can provide the initial list during MMS materialization; it is
not yet a subscription or device-status query.

Live check (2026-09-03): `AudioInput.devices()` successfully returned the
host's input-device list through the demo panel. This verifies the static API
binding and basic enumeration path, not capture success for every listed
device.

An existing source can be switched during a retained MMS session:

```mms
microphone.select_device_number(1)
```

The capture runtime invalidates its old amplitude measurement, tears down the
previous stream, and starts the selected device on its next tick. If the device
cannot start or delivers no samples, that selection stays unavailable rather
than repeatedly reopening its backend. Clicking a device row again is an
explicit retry; choosing another row is a fresh request.

## Public `info_panel` asset

Build a public wrapper around
`assets/components/internal/ui/accordion.mms`, rather than copying accordion
layout, drag, raycast, and toggle behavior into each scene.

Proposed narrow MMS surface:

```mms
import { info_panel } from "../assets/components/ui/info_panel.mms"

let inputs_panel = info_panel({
    root_name = "microphone_inputs_panel"
    width_gu = 32.0
    unit_scale = 0.075
    title = "audio input devices"
    icon = optional_icon_component
    content = rows
})
```

Required options:

- `root_name`: caller-owned stable name, used for event handling and dragging;
- `width_gu` and `unit_scale`: normal layout sizing controls;
- `title`: a string for the standard styled title bar, or a component used as
  the actual title-bar root (implemented 2026-10-14); and
- `content`: one caller-authored body component.

Optional options:

- `icon`: one caller-authored title-bar component placed beside the title; and
- theme values only when necessary to make the generic panel reusable.

The wrapper owns the default accordion appearance and title layout. It should
not expose editor-specific labels, panel registration, docking, or persistence.

Component titles replace the standard header; they are not nested under a Text
or the standard title-label Transform. They own their layout, theme, authored
name, icons, and movement markers. Use a Transform root when attaching
`Grabbable.parent()` so it resolves the panel root; the accordion appends its
retained minimize control after the custom header's authored children. String
titles keep the existing layout and optional `icon` behavior. Runtime
`is_string` chooses the path; `string | Component` here documents the contract
and is not MMS annotation syntax.

For a grip-grabbable custom header, subscribe to the named inner panel root
for restore events: grabbing reparents that root away from its outer layout
slot. The graphics panel already keeps its restore listener on that moving
root. A header instance is attached, not cloned; construct a fresh component
for another panel.

### Restore contract

The underlying accordion removes its body when minimized and emits
`AccordionRestoreRequested` when opened again. It intentionally does not
retain or recreate arbitrary caller content itself.

`info_panel` forwards that event from its named root. Its caller therefore
owns the reload policy:

1. The demo initially calls `AudioInput.devices()` and creates one row per
   returned name.
2. The next data slice will define the panel's content-reload callback, so on
   `AccordionRestoreRequested` the demo can call `AudioInput.devices()` again
   and let `info_panel` remount freshly built rows.
3. The rows are replaced only at restore time; no polling or live hot-plug
   observer is introduced in this slice.

This is deliberate: it gives the caller a clean place to reload data and keeps
the asset compatible with arbitrary one-shot or dynamic content.

## Demo shape

The new example should retain the source scene's avatar, mirror, lighting,
eye tracking, hand tracking, colliders, shirt physics, `AudioInput`,
`Amplitude`, and explicit AVC mouth-open settings.

Changes:

- rename the header/comments for microphone speaking-animation acceptance;
- change `BGC` to opaque black (`BGC.rgba(0.0, 0.0, 0.0, 1.0)`);
- choose and document either `AudioInput {}` or
  `AudioInput.device_number(1) {}` as the active test microphone;
- import and place the `info_panel` near the mirror without obstructing the
  avatar; and
- render an ordinal and device name in each input row, clearly marking the
  currently authored selected index if the example uses `device_number(1)`.

The visual rows are an inspection/setup aid. Enumerating a device must not
open a capture stream; only the authored microphone that is consumed by
`Amplitude` activates capture.

## Implementation order

1. Add `assets/components/ui/info_panel.mms`, delegating shell/toggle behavior
   to the internal accordion asset and documenting its forwarded event.
2. Add a focused MMS materialization/event test proving the wrapper initially
   mounts content, minimizes, emits restore, and accepts caller-reloaded body
   content.
3. Copy the mirror example into
   `examples/vtuber-microphone-speaking-xr-eye-tracking-htc.mms`; set the
   black clear color and preserve its existing amplitude/AVC setup.
4. Build device rows from `AudioInput.devices()` using the public panel.
5. Add the narrow `info_panel` content-reload callback, then register an
   `AccordionRestoreRequested` handler in the example to rebuild the body from
   a freshly enumerated device list.
6. Verify scene materialization and run the normal focused scripting tests.
7. Perform the intended live HMD/default-mic/device-1 acceptance pass.

## Acceptance checks

- `AudioInput.device_number(1) {}` selects the second currently enumerated
  device and invalid indices fail clearly.
- The new scene has an opaque black clear background and retains eye tracking,
  mirror rendering, and amplitude-driven mouth opening.
- `info_panel` works outside editor UI and accepts title, optional icon, and
  caller content.
- The title bar is draggable and its accordion toggle minimizes body content.
- Restoring emits `AccordionRestoreRequested` from the panel root.
- The example handles that event and recreates its microphone device rows from
  a new `AudioInput.devices()` result.
- Every initial enumeration result appears as exactly one readable row.
- Device listing alone never starts capture for all listed devices.

## Deferred

- stable persisted device identities and name-based `AudioInput.device(...)`;
- device picker controls that mutate the active `AudioInput` at runtime;
- capture level/status/error indicators per device;
- live hot-plug refresh while the panel is expanded;
- scrolling/virtualization for unusually long device lists; and
- a general reactive data-binding or arbitrary component cloning API.
