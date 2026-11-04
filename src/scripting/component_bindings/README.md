# Component bindings

Mittens-side adapters that expose native components to MMS live here. Group
related components in one file when they share a responsibility.

An adapter can declare constructors, builder calls and runtime methods in the
MMS RuntimeSpec, validate arguments, and dispatch native construction or mutation.
Native simulation and rendering algorithms stay in the engine systems.

The parent `scripting` and `runtime_config` modules retain their existing Rust
module ownership through `#[path]` declarations. Moving these files into this
folder does not change their helper visibility, dispatch or runtime behavior.
