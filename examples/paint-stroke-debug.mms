// Paint-stroke pointer/grid diagnostic scene.
//
// Run with:
//   MITTENS_DEBUG_PAINT_STROKE=1 cargo run --example paint-stroke-debug
//
// Marker colors while a paint gesture is active:
//   green   = gesture start point
//   magenta = point delivered to Paint after GestureSystem mapping
//   cyan    = projection of that point onto the workspace-selected grid
//   yellow  = snap Paint actually resolved from hit-owned grid geometry
//
// The markers are non-raycastable, non-selectable, non-serializable overlays.

RendererSettings { window_size(1100, 720) }
BGC.rgba(0.055, 0.070, 0.11, 1.0)
AL.rgb(0.28, 0.30, 0.36)

T.position(-3.5, 5.0, 4.5) {
    DL.color(1.0, 0.92, 0.82).intensity(1.15)
}

ED.active() {
    name = "paint_stroke_debug_editor"

    // Three independently raycastable, coplanar targets. A horizontal stroke
    // crosses two renderable boundaries without changing semantic surface.
    T.position(-2.05, 1.15, -3.0).scale(1.95, 2.3, 0.14) {
        name = "paint_target_left"
        R.cube() {
            C.rgba(0.20, 0.46, 0.82, 1.0)
            Raycastable.enabled()
        }
    }
    T.position(0.0, 1.15, -3.0).scale(1.95, 2.3, 0.14) {
        name = "paint_target_center"
        R.cube() {
            C.rgba(0.30, 0.72, 0.48, 1.0)
            Raycastable.enabled()
        }
    }
    T.position(2.05, 1.15, -3.0).scale(1.95, 2.3, 0.14) {
        name = "paint_target_right"
        R.cube() {
            C.rgba(0.88, 0.43, 0.30, 1.0)
            Raycastable.enabled()
        }
    }

    // Thin shelf and floor targets expose changes in surface normal/height.
    T.position(0.0, -0.10, -1.9).scale(6.2, 0.16, 2.0) {
        name = "paint_target_shelf"
        R.cube() {
            C.rgba(0.48, 0.42, 0.70, 1.0)
            Raycastable.enabled()
        }
    }
    T.position(0.0, -1.15, -2.0).scale(9.0, 0.12, 8.0) {
        name = "paint_target_floor"
        R.cube() {
            C.rgba(0.16, 0.18, 0.24, 1.0)
            Raycastable.enabled()
        }
    }

    // Grid local XZ is rotated into the wall's XY plane. Select this grid for
    // the clearest pointer-mapping comparison against the three wall targets.
    T.position(0.0, 1.15, -2.90).rotation(1.570796, 0.0, 0.0) {
        name = "debug_vertical_grid_spacing_0_5"
        Grid.spacing(0.5) {
            size_x(16)
            size_z(8)
            enabled(true)
            hidden(false)
            selectable(true)
        }
    }

    // A translated, yawed, non-unit grid for a second diagnostic pass.
    T.position(0.65, -1.05, -1.55).rotation(0.0, 0.35, 0.0) {
        name = "debug_floor_grid_spacing_0_75"
        Grid.spacing(0.75) {
            size_x(12)
            size_z(12)
            enabled(true)
            hidden(false)
            selectable(true)
        }
    }
}

// Shared editor panels required to select an asset, tool, color, and grid.
T.position(-3.7, 3.0, -0.5) {
    EditorUI {
        panels([
            { panel = "world" },
            { panel = "assets" },
            { panel = "paint" },
            { panel = "color" },
            { panel = "grid" },
            { panel = "settings" },
        ])
    }
}

// Desktop camera/pointer. Local -Z points toward the wall targets.
I.speed(2.2) {
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(0.0, 1.4, 6.5) {
        name = "paint_debug_desktop_camera"
        C3D { Pointer {} }
    }
}

// Minimal XR camera and trigger pointers. These intentionally avoid avatar
// retargeting so this scene stays focused on pointer/paint behavior.
InputXR.on() {
    T {
        name = "paint_debug_xr_rig"
        CXR { Pointer {} }

        XRHand(true, "Left", "GripAim").laser() {
            T { Pointer {} }
        }
        XRHand(true, "Right", "GripAim").laser() {
            T { Pointer {} }
        }
    }
}

XR.on()
