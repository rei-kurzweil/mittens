// Small XR frame-rate readout: the smoothed number with a static "fps" label.
// RendererStats rebuilds only the number's glyphs; the label is built once.
// Mount it inside an existing hud() display so it adds no spring of its own.
// Position is in metres relative to that display.
export fn xr_fps_readout(position) {
    return T.position(position[0], position[1], position[2]).scale(0.025, 0.025, 0.025) {
        name = "xr_fps_readout"
        Raycastable.disabled()
        Selectable.off()
        RendererStats {
            camera_target("Xr")
            update_interval_sec(0.5)
            smoothing(0.85)
            color([1.0, 1.0, 1.0, 1.0])
            emissive(true)
        }
        T.position(2.4, -0.15, 0.0) {
            Text { "fps" font_size(0.5) C.rgba(1.0, 1.0, 1.0, 1.0) Unlit {} }
        }
    }
}
