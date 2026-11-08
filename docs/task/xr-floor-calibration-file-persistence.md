# Shared XR floor calibration persistence

Status: proposed; persistence is not implemented.

## Current API

`XrSettings.floor.correction_meters()` reads runtime state;
`XrSettings.floor.set_correction_meters(meters)` updates it. Neither accesses disk.
`XrSettingsSystem` defaults to zero and resets with the scene. Values must be
finite and within [-0.5, +0.5] metres; positive correction lowers tracked head and
hands. A changed value emits `XrSettingsChanged`.

Implementation: `src/scripting/builtin_api_config.rs`,
`src/scripting/host.rs`, `src/engine/ecs/system/xr_settings_system.rs`.

## Proposed contract

Allow an optional file parameter on the existing getter/setter, rather than a
component builder. Illustrative syntax, pending API review:

```mms
let floor = XrSettings.floor.correction_meters({file = calibration_file})
XrSettings.floor.set_correction_meters(floor)
// Slider updates use the ordinary in-memory setter.
// On commit, write the chosen value to the shared file:
XrSettings.floor.set_correction_meters(floor, {file = calibration_file})
```

A getter with `file` reads the saved default; it does not silently replace runtime
state. Scene initialization explicitly applies the returned value. The ordinary
getter remains a cheap memory read. An optional setter file writes a versioned
calibration record and applies the validated value. No disk access in frame
handlers or repeated UI refresh; save on commit or debounce interaction.

Accept local filenames and local `file://` URLs with correct percent decoding;
reject remote URLs. Provide one documented shared user-config location for the
examples so calibration survives scene changes and app restarts. Do not store
user calibration in tracked example assets. A later convenience option may select
that default location; no per-avatar copy of calibration.

## Slices and decisions

- [ ] Finalize parameter shape, shared default path and missing-file behavior
      (proposed: return zero for a missing file; report malformed/unreadable files).
- [ ] Define a versioned JSON calibration subset; do not persist unrelated runtime
      settings merely because they share a settings namespace.
- [ ] Add native load/save helpers with validation and atomic replacement;
      write failures must be reported and preserve the previous file. Decide
      whether failed writes leave runtime state unchanged (preferred).
- [ ] Expose the same behavior to Rust and MMS without growing oversized host.rs;
      extract XR settings host handling before expanding it.
- [ ] Load once in both ship variants and other opted-in XR examples; persist
      floor-slider commits and define reset-to-zero versus delete-saved-default.
- [ ] Verify restart/scene sharing, valid endpoints, invalid/nonfinite values,
      missing/corrupt files, escaped paths and failed writes.

Calibration may become stale after SteamVR recentering. Keep an explicit reset;
automatic headset/runtime profile detection is deferred.
