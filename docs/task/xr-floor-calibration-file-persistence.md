# Shared XR floor calibration persistence

Status: implemented; user review and two-demo VR acceptance pending.

## Delivered contract

`XrSettings.floor.correction_meters()` reads memory and
`set_correction_meters(meters)` previews in memory. Their optional `{file = path}`
argument explicitly reads a saved default or saves and applies a validated value.
`XrSettings.floor.default_file()` supplies one shared user-config location.
A file getter does not modify runtime state; scene startup explicitly applies it.
Missing files return zero; malformed/unreadable/version/validation errors are reported.

Version 1 JSON contains only `version` and `floor_correction_meters`. Finite values
in [-0.5, +0.5] metres are accepted. Positive correction lowers tracked head/hands.
Local paths and percent-encoded local file URLs work; remote URLs are rejected.
Saves sync a unique temporary file beside the destination and atomically replace
it. Failed writes preserve the previous file and leave runtime state unchanged
by that call. Prior live slider preview is retained if saving its commit fails.

The XR panel accepts `calibration_file`; slider movement previews without I/O,
release saves, and reset saves zero. Accordion restoration retains that option.
Memory-only panels still work without it. All nine demos importing the panel opt
in: XR settings, five Mittens Corp variants, two Sakura drone-conversion variants,
and lower-body walking. They load once at scene startup from the same path.

See [the spec](../spec/xr-settings.md#shared-floor-defaults) for APIs, paths and
error behavior. XR host handling was extracted to `src/scripting/xr_settings_api.rs`;
native file handling lives in `src/engine/ecs/system/xr_calibration_file.rs`.
No tracked asset stores personal calibration.

## Verification

Automated coverage checks roundtrips and endpoints, missing/corrupt/versioned
records, escaped paths, remote URL rejection, failed-write preservation and temp
cleanup, explicit read versus apply, strict MMS argument validation, panel commit
versus preview, scene sharing, fresh-Universe restart, accordion restoration and
persisted reset. Existing XR panel/backend tests cover synchronization/lifetime.

Automated validation on 2026-11-08:

- `cargo test --lib xr_calibration`: 5 passed, including loading the authored
  XR settings and lower-body walking demos against one temporary saved default.
- `cargo test --lib xr_settings`: 10 passed.
- `cargo test --lib graphics_settings_`: 8 passed.
- `cargo check --lib --examples`: passed with existing warnings.
- Changed Rust files pass `rustfmt --check`; `git diff --check` passes.

## Pending VR acceptance

1. Run `cargo run --release -- load examples/xr-settings.mms`. Adjust floor
   correction, release the slider, and note the signed readout. Verify tracking,
   head/hand/eye alignment and floor contact.
2. Close and run a different opted-in demo, for example
   `cargo run --release -- load examples/mittens-corp.mms`. Confirm the initial
   readout and physical floor correction match without another adjustment.
3. Adjust in the second demo, release, then reopen the first and confirm it loads
   the new value. Minimize/restore the panel and check its controls still save.
4. Reset to zero, reopen either demo and confirm zero persists. Test both signs.

Calibration may become stale after SteamVR recentering. Automatic headset/runtime
profile detection, other XR settings, and deleting saved calibration are deferred.
