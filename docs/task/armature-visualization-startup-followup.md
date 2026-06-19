# Armature Visualization Startup Follow-up

## Current Status

The glTF armature marker work is split into two runtime responsibilities:

- `GLTFSystem`
  - owns glTF import/resource caching
  - spawns the authored glTF subtree
  - records per-instance runtime data on `GLTFComponent`
  - tracks live `GLTFComponent` ids through `RegisterGLTF`

- `ArmatureVisualizationSystem`
  - owns spawning/removing armature marker subtrees
  - consumes `GLTFSystem`'s tracked component list instead of scanning the whole world
  - keeps `HashMap<ComponentId, Vec<ComponentId>>` as the runtime registry of spawned marker roots

## Important Default

Armature visualization is currently **off by default**.

That default exists in:

- `GLTFComponent::new()` via `armature_visible: false`
- `EditorContextState::default()` via `armature_visible: false`
- `EditorSettingsPanelState::default()` via `armature_visible: false`
- the settings-panel row payload in `assets/components/panels.mms` via `visible = false`

So the current startup regression on `vtuber-mirror-example` should not be explained by markers being enabled automatically.

## Recent Implementation Adjustments

Two issues were fixed in `ArmatureVisualizationSystem::tick_with_queue()`:

1. It no longer scans `world.all_components()` every frame for `GLTFComponent`.
   - `GLTFComponent::init()` now emits `IntentValue::RegisterGLTF`.
   - `RxMutationExecutor` routes that into `GLTFSystem::register_component()`.
   - `ArmatureVisualizationSystem` iterates `gltf_system.tracked_components()`.

2. It no longer clones joint lists every frame in the steady state.
   - the joint-transform `Vec<ComponentId>` is only cloned on the actual marker spawn path
   - steady-state visible and hidden cases do not allocate

## What We Tested

Targeted automated checks:

- `cargo test armature_visibility_spawns_once_and_removes_idempotently -- --nocapture`
- `cargo test armature_settings_click_toggles_state_renders_checkmark_and_fans_out_to_all_editors -- --nocapture`

Manual/default-state verification:

- confirmed the toggle defaults to off in code paths listed above

Example runtime check:

- `cargo run --example vtuber-mirror-example --release`
- observed startup proceeds through asset-module scanning and editor-panel setup
- observed no evidence that armature visualization is auto-enabled

## Current Runtime Concern

The reported failure mode for:

- `cargo run --example vtuber-mirror-example --release`

is:

- no window opening
- long startup with normal inspector/editor logs
- eventual plain `Killed`

That looks more like:

- OS/process termination
- likely memory pressure or some other startup/runtime resource problem

and less like:

- a Rust panic
- an armature-visualization default-on regression

## Next Investigation Options

1. Add temporary startup tracing around panel setup, GLTF spawn, and first frame/render preparation.
2. Run the example under `/usr/bin/time -v` to capture memory growth before the kill.
3. Compare against the pre-armature-visualization commit to see whether the regression is causal or coincidental.
4. Audit whether the editor panel asset/module loading path is doing repeated work during startup for this example.
