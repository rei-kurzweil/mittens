# Editor component materialization

## Motivation

Editor UI, panels, and asset workflows should be able to instantiate live component trees from either Rust-side factories or MMS-defined factories.
A shared materialization abstraction makes these sources opaque to consumers and simplifies the FFI surface between Rust and MMS.

## What this should solve

- `AssetsPanel` can instantiate previews from both Rust-native and MMS-native assets.
- `PaintPanel` can place objects without caring whether the asset came from an MMS module or a Rust builder.
- `Selection`/`Inspector` can operate on the resulting component tree without needing separate handling.
- The FFI boundary between Rust and MMS becomes a smaller, centralized contract.

## Candidate abstraction

`Materializer` or `AssetFactory` would expose a simple, source-agnostic API:

- `materialize_asset(factory_ref, world, emit) -> ComponentId`
- `preview_asset(factory_ref, preview_context) -> PreviewInstance`

Where `factory_ref` can be:

- a Rust factory descriptor (`RustFactory(name, params)`)
- an MMS factory descriptor (`MmsFactory(module_path, export_name, args)`)

The editor can then build higher-level services on top of it:

- asset discovery / metadata caching
- preview lifecycle management
- selected asset instantiation
- panel-specific render contexts

## Benefits

- centralizes Rust/MMS bridges to one path
- keeps UI/panels decoupled from asset source details
- allows preview rendering and live placement to reuse the same instantiation logic
- reduces special-case code for `.mms` assets in editor systems

## Important constraints

- asset discovery must remain separate from materialization; discovery should not invoke factories.
- preview instances should be disposable and isolated from the main world.
- the materialization layer should only require the minimal FFI surface needed to spawn a component tree.

## Open questions

- should `Materializer` be a service in `SystemWorld` or a standalone editor utility?
- what shape should `factory_ref` take so it is easy to serialize/deserialize and reference across editor state?
- how do we manage temporary preview worlds versus live scene instantiation?
