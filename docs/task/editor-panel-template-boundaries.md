# Editor panel template boundaries

Status: proposed. `assets/components/internal/panels.mms` combines several
editor panel families. A future split should group panels by responsibility,
preserve shared accordion geometry and materials, and avoid additional runtime
subscriptions or per-frame resource creation. Validate all built-in editor
panels, title dragging/grabbing, minimize/restore, and live ship accordions.

Root cause: the editor panel adapter calls the runner's template materialization
helper, which explicitly discards the supplied live world and emitter. Ordinary
MMS imports therefore encounter ComponentExpr values without `import ast`.
Migrate this adapter to live instantiation; do not add template-only options to
the accordion or weaken ordinary MMS component semantics. The temporary
expose_controls workaround has been removed.

## Isolated reproduction

Run `cargo run --release -- load examples/triage/editor-settings-runtime.mms`.
Sakura's XR example has had both its Editor wrapper and explicit settings
EditorUI removed. Test the trigger matrix with
`cargo test --lib editor_settings_trigger_tests -- --nocapture`.

Editor defaults to `spawn_panels=true`, so `ED {}` / `Editor {}` alone creates
a fallback EditorUI and enters the faulty adapter. EditorUI alone does not
create an editor: it only requests panel setup for existing panel-enabled
editors. `Editor.panels(false) {}` does not enter panel setup. With both
components, EditorUI selects which panels are built and can retrigger setup.
