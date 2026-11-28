// Small XR frame-rate readout ("XR: 72.0 fps" over the frame time in ms).
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
    }
}
