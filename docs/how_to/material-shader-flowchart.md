# Material and shader flowchart

Run from the repository root:

```sh
cargo run --release -- load examples/material-shader-flowchart.mms
```

The scene follows the OpenXR flowchart's clickable cards, orthogonal arrows and
movable detail panel. It explains the current desktop/Vulkano material path and
the proposed touch-deformation extension. It does not enable squishing or load
an avatar, and it does not require an XR runtime.

Click a card for its role, ownership contract, example and source locations.
Drag any section or the detail panel by its title. The title arrow minimizes
the body; restoring rebuilds it. The detail panel retains the last selected
card, including selections made while minimized. Use WASD / R / F to move and
the right mouse button to look.

Outlined cards show existing facilities. Gold-filled cards explicitly mark
proposed work. Cyan identifies shader interfaces, violet material parameters
and green geometry/output. The gold return in the third section means a
proposed input/operation in compute, not a feedback simulation.

The three sections cover:

1. **MMS authoring:** `MaterialBase` supplies the renderer contract;
   `MaterialDefinition` supplies a shader/schema; `Material` supplies values;
   renderables consume the shared appearance with independent geometry state.
2. **Current custom path:** the typed record and GLSL reflection determine
   numeric value validation and packing within a supported descriptor layout.
   The engine supplies the static surface vertex stage; the authored `.frag`
   supplies the fragment stage. A type record does not select a vertex shader
   or define arbitrary descriptor bindings.
3. **Deformation:** existing morph/skin compute and the built-in cached vertex
   path, followed by the missing custom-material cache pairing and proposed
   `Squishable` receiver/contact operation. No squish-specific material base is
   proposed. Static squishable meshes would also need cached-path admission.

This documents the source as of 2026-10-31. Custom definitions currently have
`deformed_program: None`; built-in Toon/Anime definitions have static and cached
programs. `Squishable`, painted squish import, contact publication and the touch
compute step remain proposals. Android's new renderer is a separate static
custom opaque/albedo slice; desktop cache support does not imply mobile support.

The notes' example snippets are displayed text. Some illustrate a hypothetical
schema and must be matched to a real fragment uniform block before use.

- [Runnable scene](../../examples/material-shader-flowchart.mms)
- [Card notes](../../assets/components/diagrams/material_shader_notes.mms)
- [OpenXR reference](../../examples/0-9-x-openxr-flowchart.mms)
- [Typed material authoring](../draft/material-builtin-authoring-api.md)
- [Mesh deformation contract](../spec/mesh-deformation-pipeline.md)
