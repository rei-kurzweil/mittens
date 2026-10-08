# Mittens 0.9.0 release notes

This release pairs `mittens-engine 0.9.0` with `meow-meow-script 0.9.0`.
`mittens-query` remains at `0.6.0`. The engine's crates.io dependency now
requires the 0.9 series of Meow Meow Script.

The window title uses the engine's Cargo package version, so it displays
`mittens 0.9.0` and follows future version bumps automatically.

The engine package excludes Blender source assets, including photogrammetry
reference photos, to stay below crates.io's upload limit. Runtime textures and
the existing example model downloader remain included.

## Upgrading

Use `mittens-engine = "0.9.0"` for the engine, or
`meow-meow-script = "0.9.0"` for a standalone script host. Cargo treats the
0.9 series as a new compatibility boundary from 0.8; update direct
dependencies together if your application uses both crates.

See the [MMS crate documentation](../../crates/meow-meow-script/README.md),
the [script-host configuration guide](../../crates/meow-meow-script/docs/how_to/configuring_a_script_host.md),
and the [humanoid rigging guide](../how_to/rigging_and_controlling_humanoid_characters.md)
for the current APIs. Notes and task documents for 0.8 retain their historical
version references; unchecked roadmap items remain planned work.
