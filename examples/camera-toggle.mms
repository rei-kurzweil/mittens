// camera-toggle.mms
//
// MMS-authored camera toggle demo using the shared button component.
//
// Notes:
// - Turning Camera3D off blanks the desktop scene view by design when there is
//   no other active Camera3D/Camera2D.
// - CameraXR toggling updates the XR rig selection path while VR stays live.

RendererSettings {
    window_size(1280, 960)
}

BGC.rgba(0.10, 0.11, 0.14, 1.0)
AL.rgb(0.32, 0.32, 0.35)

T.position(2.4, 3.8, 3.0) {
    DL {
        intensity(0.95)
        color(1.0, 0.97, 0.92)
    }
}

T.position(-2.0, 2.2, 1.5) {
    DL {
        intensity(0.35)
        color(0.85, 0.90, 1.0)
    }
}

// Desktop camera + pointer.
let desktop_cam = C3D.enabled(true) {
    Pointer {}
}

I.speed(2.5) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 1.25, 5.0) {
        name = "desktop_rig"
        desktop_cam
    }
}

// XR rig selection target.
let xr_cam = CXR.on()

InputXR.on() {
    T.position(0.0, 1.55, 0.0) {
        xr_cam {
            Pointer {}
        }
    }
}

XR.on()

// Simple scene reference so toggling the desktop camera has an obvious effect.
T.position(0.0, -0.15, 0.0).scale(12.0, 0.2, 12.0) {
    R.cube() { C.rgba(0.74, 0.74, 0.78, 1.0) }
}

T.position(-1.7, 0.55, -0.8).scale(0.7, 1.1, 0.7) {
    R.cube() {
        C.rgba(0.92, 0.48, 0.24, 1.0)
        EM.on()
    }
}

T.position(0.0, 1.0, -1.6).scale(0.65, 1.7, 0.65) {
    R.cube() {
        C.rgba(0.22, 0.74, 0.96, 1.0)
        EM.on()
    }
}

T.position(1.8, 1.45, -2.6).scale(0.8, 0.8, 0.8) {
    R.cube() {
        C.rgba(0.96, 0.86, 0.22, 1.0)
        EM.on()
    }
}

import { button } from "../assets/components/button.mms"

let cam3d_toggle_btn = button("toggle Camera3D")
let xr_toggle_btn = button("toggle CameraXR")

let cam3d_status_wrap = T {
    Style {
        display("inline-block")
        padding_xy(0.6, 0.6)
        text_align("center")
    }
    T.position(0.0, 0.0, 0.05) {
        Text {
            name = "cam3d_status"
            "Camera3D: on"
        }
    }
}

let xr_status_wrap = T {
    Style {
        display("inline-block")
        padding_xy(0.6, 0.6)
        text_align("center")
    }
    T.position(0.0, 0.0, 0.05) {
        Text {
            name = "xr_status"
            "CameraXR: on"
        }
    }
}

let note_wrap = T {
    Style {
        display("inline-block")
        padding_xy(0.6, 0.6)
        text_align("center")
        font_size(0.07wu)
    }
    T.position(0.0, 0.0, 0.05) {
        Text {
            name = "camera_toggle_note"
            "desktop off => window scene draw stops\nuse REPL or XR path to recover if needed"
        }
    }
}

let control_panel = T.position(-4.6, 3.1, 0.0).scale(0.28, 0.28, 0.28) {
    LayoutRoot {
        name = "camera_toggle_panel"
        available_width(36.0)
        available_height(20.0)
        cam3d_toggle_btn
        cam3d_status_wrap
        xr_toggle_btn
        xr_status_wrap
        note_wrap
    }
}
control_panel

let cam3d_status = control_panel.query("#cam3d_status")
let xr_status = control_panel.query("#xr_status")

on(cam3d_toggle_btn, "Click", fn(e) {
    let enabled = desktop_cam.enabled()
    desktop_cam.enabled(!enabled)
    if enabled {
        cam3d_status.set_text("Camera3D: off")
        print("Camera3D.enabled(false)")
    } else {
        cam3d_status.set_text("Camera3D: on")
        desktop_cam.make_active_camera()
        print("Camera3D.enabled(true)")
    }
})

on(xr_toggle_btn, "Click", fn(e) {
    let enabled = xr_cam.enabled()
    xr_cam.enabled(!enabled)
    if enabled {
        xr_status.set_text("CameraXR: off")
        print("CameraXR.enabled(false)")
    } else {
        xr_status.set_text("CameraXR: on")
        xr_cam.make_active_camera()
        print("CameraXR.enabled(true)")
    }
})
