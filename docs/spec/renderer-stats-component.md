# RendererStatsComponent

## Goal

`RendererStatsComponent` displays a small on-screen diagnostic (currently FPS + frame time) by auto-creating a `TextComponent` subtree under the component.

This is intended for quick perf sanity checks (e.g. comparing window vs XR cadence) and should be considered best-effort diagnostics.

## Topology / authoring

Attach `RendererStatsComponent` as a child of a `TransformComponent` and set its `target` explicitly:

- `target = "window"` → shows window frame timing.
- `target = "xr"` → shows XR render timing (as measured by the OpenXR render path).

Typical authoring:

- `TransformComponent` (camera rig)
  - `CameraXRComponent` (or `Camera3DComponent` / `Camera2DComponent`)
    - `RendererStatsComponent`

You do not need to manually add a `TextComponent`; it is spawned automatically.

## Auto-managed subtree

At runtime, the stats system ensures this subtree exists under the `RendererStatsComponent` node:

- `TextComponent`
  - `ColorComponent` (immediate child; used for glyph color inheritance)
  - `EmissiveComponent` (immediate child; propagated to glyphs)
  - (glyph transform/renderable subtrees spawned by `TextSystem`)

## Timing sources

- **Window**: taken from the `dt_sec` passed into the ECS tick (stored into `VisualWorld`).
- **XR**: measured from the wall-clock time between `OpenXRSystem::render_xr` calls (stored into `VisualWorld`).

Notes:
- XR timing is not the runtime-reported predicted display period; it’s a practical “what cadence are we submitting at?” measurement.

## Component fields (JSON)

`RendererStatsComponent` encodes/decodes these fields:

- `enabled: bool` (default `true`)
- `target: "window" | "xr"` (default `"window"`)
- `update_interval_sec: f32` (default `0.25`)
- `smoothing: f32` EMA factor in $[0, 1]$ (default `0.9`)
- `color: [f32; 4]` (default `[1, 1, 1, 1]`)
- `emissive: bool` (default `true`)

## Performance considerations

Updating text rebuilds glyph subtrees. Use `update_interval_sec` to keep this cheap (e.g. 0.25–1.0s is usually enough).
