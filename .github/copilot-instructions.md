
# Copilot instructions

## Compatibility policy

- Do **not** add backward compatibility for JSON schema changes.
	- No legacy field-name aliases in `decode()`.
	- No accepting multiple shapes/types for the same field to support older saved data.
	- No keeping old component type names in `component_codec` as aliases.
- Do **not** keep old method/type names “for compatibility” after a rename.

## Running examples

- When running examples, use `cargo run --release --example <name>` by default.
- **Performance:** ALWAYS use the `--release` flag to ensure the engine runs at maximum performance (especially for IK and animation systems).
- Only use non-release example runs when the task specifically calls for debug-mode behavior.
- **Remote GUI (SSH):** If windowing fails when running over SSH, ensure display environment variables are set correctly for the host (e.g., `WAYLAND_DISPLAY=wayland-0` or `DISPLAY=:0`).

