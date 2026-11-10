# Scripting registry modules

Focused component authoring modules live here:

- `*_registry.rs` implements constructors and builder argument handling.
- `*_runtime_config.rs` declares the corresponding MMS API vocabulary and signatures.
- `renderer_settings_legacy.rs` handles the existing renderer-settings compatibility path.

Renderable shapes share `renderable_registry.rs` and
`renderable_runtime_config.rs`; individual shapes do not need separate files.

The central entry points remain in `scripting/component_registry.rs`,
`component_method_registry.rs`, `configured_registry.rs`, and `runtime_config.rs`.
Their path declarations preserve the existing Rust module hierarchy and imports.
