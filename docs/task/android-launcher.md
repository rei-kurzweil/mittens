# Android launcher (headset home screen)

Status: implemented on branch `android-skinning-mirrors`, not yet built or run
on the Focus 3. Written without a Rust toolchain on the authoring machine.

A launcher build of Mittens Host opens a home scene with a file browser over
MMS files pushed to the headset. Launching a file replaces the running scene;
holding the left menu button returns home from any scene. No PC connection is
needed (the LAN shell in [mitten-droid shell](../how_to/mitten-droid-shell.md)
still exists separately).

## Pieces

| Piece | Where |
|---|---|
| `App.launch(path)`, `App.home()`, `App.last_error()` | `src/scripting/app_api.rs`, request slot in `src/engine/app_launch.rs`. Hosts that don't enable it (desktop) report the API as unavailable. |
| `FileBrowser.selection(b).path` | Root-relative path of the selected file (`src/scripting/file_browser/model.rs`). |
| Launcher scene | `android/examples/launcher.mms`: a browser-only shell (no terminal pane) with Launch/Refresh, the selected path and status above FILES, and a dock with the Graphics panel. |
| Project root and Home | `android/src/launcher.rs`: project folder, bundled fallback, menu-hold Home. |
| Scene switching | `android/src/scene_runtime.rs` (`switch_scene`): new `Universe` and a new renderer per launch. |
| Sync | `android/packaging/sync-project.sh [--launch]`. |
| Dock material | `dock({ material })` passes a custom material to its panel; Android needs one on every surface. |

## Behavior

- **Project root**: `/sdcard/Android/data/org.mittens.engine.dev/files/project/`
  (the activity's external files folder), with the repository's layout. It is
  the process working directory and the engine asset root, so `assets/...`
  URIs and relative imports resolve as on desktop.
- **Bundled fallback**: the launcher and its imports ship in the APK and are
  written into the project root only where a file is missing. Pushed files
  win. Without a pushed project, the browser shows `android/examples/`.
- **Launch**: `App.launch` is taken at the next frame boundary. The host loads
  the file into a new `Universe` and rebuilds the renderer, which frees the
  previous scene's meshes, textures and pipelines.
- **Failure**: a scene that fails to load, or that the renderer rejects
  (legacy materials, bloom, mirrors, skinning until that lands, ...), returns
  home. The launcher shows the reason from `App.last_error()`. A launcher that
  fails to load is fatal, as before.
- **Home**: hold the left menu button for 0.6 s. The right menu button is the
  runtime's system button.
- **Fixed at startup**: the swapchain resolution and display refresh rate come
  from the launcher scene's `RendererSettings`. Launched scenes cannot change
  them.

## Smoke test (Focus 3)

```sh
export ANDROID_HOME="$HOME/Android/Sdk"
CARGO_PROFILE_DEV_OPT_LEVEL=3 android/packaging/build-apk.sh --features launcher
adb install --no-incremental -r target/debug/apk/mitten-droid.apk
android/packaging/sync-project.sh --launch
adb logcat -v threadtime 'mitten-droid:I' '*:S' | grep -E 'launcher|LAUNCH|FAILED|stdio'
```

- [ ] Launcher appears; the log shows `launcher project_root=...`.
- [ ] Browser lists `examples/`; Up stops at the project root; right stick scrolls.
- [ ] Selecting an MMS file shows its path above FILES; Launch (gold, top row) opens it.
      Try `android/examples/sakura-stage-lean.mms` first (known to render).
- [ ] Holding the left menu button returns home; the log shows `LAUNCH scene=android/examples/launcher.mms`.
- [ ] Launching a desktop-only example (for example one with bloom) returns
      home with the rejection reason shown.
- [ ] Repeated launch/home cycles don't grow memory (`adb shell dumpsys meminfo org.mittens.engine.dev`).
- [ ] Dock: Graphics opens and closes its panel; foveation can be changed.
- [ ] Without a pushed project (`adb shell rm -r .../files/project`), the
      launcher still starts and shows the push hint.

- [ ] Tracking space: logcat shows `reference_spaces=[...] using=STAGE floor_offset_m=0`.
  The list may now include `LOCAL_FLOOR` where the runtime supports
  `XR_EXT_local_floor`. Without STAGE the host uses `LOCAL_FLOOR`, else
  `LOCAL` lowered by `floor_offset_m=1.6`, so y = 0 stays near the floor.
  Floors sit at your real floor and the launcher window near eye height.
- [ ] Humanoid panel (dock, in the launcher and the first-person scene): the
  floor correction slider moves the view; after release it is saved to
  `project/.config/mittens/xr-calibration.json` and applies in the next scene.
  Desired height changes Mitten's size live in the first-person scene.
- [ ] First-person Mitten: launch `android/examples/mitten-first-person.mms`.
  Looking down shows the skinned body; controllers drive the arms (arm IK) and
  the left stick walks (walking IK). Eye/lash layers use alpha discard in
  `anime-cutout.frag` (opaque base), so they draw in the same tiled pass. A
  renderer rejection returns home with the reason in the status line.
- [ ] Native eyes (Focus 3 with the eye tracker module): logcat shows
  `native eyes: XR_HTC_eye_tracker advertised=true enabled=true ...
  supportsEyeTracking=true`, then `XR_HTC_eye_tracker created`, then
  `openness valid=true` once worn. Blinking each eye closes Mitten's matching
  eyelid. Without the module: `supportsEyeTracking=false`, and the scene runs
  normally.
  About once a second the log prints raw `L valid= open= R valid= open=`.
  Note whether `valid` stays true during a blink and a wink, and what open
  reads with relaxed eyes (ALVR saw about 0.91-0.93). Attach the module and
  calibrate (Settings > Inputs > Eye tracker > Calibrate) before launching.

## Next

- The avatar picker in the dock (Mitten, and Miku when its model is in the
  project), once Android skinning and avatar materials exist.
- A per-file compatibility hint, so desktop-only scenes are marked before launch.
- Optional: make the launcher the default APK scene.
