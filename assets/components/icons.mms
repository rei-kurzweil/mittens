// assets/components/icons.mms — basic UI icons (=^･ω･^=)

// A compact movie-camera silhouette authored along the camera axis. The
// translucent warm body sits behind the half-width cone, whose aperture faces
// local -Z (the Camera3D forward direction).
export fn camera_icon() {
    let camera_color = [0.94, 0.43, 0.12, 0.72]
    return T {
        name = "camera_icon"
        T.position(0.0, 0.0, 0.30).scale(0.70, 0.70, 0.70) {
            R.cube() {
                C.rgba(camera_color[0], camera_color[1], camera_color[2], camera_color[3])
            }
        }
        T.position(0.0, 0.0, -0.42).scale(0.42, 0.42, 0.72) {
            R.cone() {
                C.rgba(camera_color[0], camera_color[1], camera_color[2], camera_color[3])
            }
        }
    }
}

export fn on_icon(plane_color, wireframe_color) {
    if plane_color == null { plane_color = [1.0, 1.0, 1.0, 1.0] }
    if wireframe_color == null { wireframe_color = [0.0, 0.0, 0.0, 1.0] }
    return T {
        name = "on_icon"
        R.plane() { C.rgba(plane_color[0], plane_color[1], plane_color[2], plane_color[3]) }
        // 0.10 GU frame thickness with an equal outer margin on a unit plane.
        T.position(0.0, 0.0, 0.001).scale(0.8, 0.8, 1.0) {
            R.wireframe_square(0.125) {
                C.rgba(wireframe_color[0], wireframe_color[1], wireframe_color[2], wireframe_color[3])
            }
        }
    }
}

export fn off_icon(plane_color, wireframe_color) {
    if plane_color == null { plane_color = [0.0, 0.0, 0.0, 1.0] }
    if wireframe_color == null { wireframe_color = [1.0, 1.0, 1.0, 1.0] }
    return T {
        name = "off_icon"
        R.plane() { C.rgba(plane_color[0], plane_color[1], plane_color[2], plane_color[3]) }
        // 0.10 GU frame thickness with an equal outer margin on a unit plane.
        T.position(0.0, 0.0, 0.001).scale(0.8, 0.8, 1.0) {
            R.wireframe_square(0.125) {
                C.rgba(wireframe_color[0], wireframe_color[1], wireframe_color[2], wireframe_color[3])
            }
        }
    }
}

export fn pencil_icon() {
    return T {
        name = "pencil_icon"
        // Simplified pencil: a tilted rectangle
        T.position(0.0, 0.0, 0.0) {
            T.rotation(0.0, 0.0, 0.785) { // 45 degrees
                T.scale(0.3, 1.2, 0.1) {
                    R.cube() {
                        C.rgba(0.8, 0.6, 0.4, 1.0)
                    }
                }
            }
        }
    }
}

export fn line_icon() {
    return T {
        name = "line_icon"
        T.position(0.0, 0.0, 0.0) {
            T.rotation(0.0, 0.0, 0.785) {
                T.scale(0.1, 1.8, 0.1) {
                    R.cube() {
                        C.rgba(0.9, 0.9, 0.9, 1.0)
                    }
                }
            }
        }
    }
}

export fn spray_can_icon() {
    return T {
        name = "spray_can_icon"
        // Body
        T.scale(0.8, 1.2, 0.1) {
            R.cube() { C.rgba(0.7, 0.2, 0.2, 1.0) }
        }
        // Cap
        T.position(0.0, 0.8, 0.01) {
            T.scale(0.4, 0.4, 0.1) {
                R.cube() { C.rgba(0.2, 0.2, 0.2, 1.0) }
            }
        }
    }
}

export fn fill_icon() {
    return T {
        name = "fill_icon"
        // Tilted bucket-ish shape
        T.rotation(0.0, 0.0, -0.4) {
            T.scale(1.2, 1.0, 0.1) {
                R.cube() { C.rgba(0.2, 0.6, 0.9, 1.0) }
            }
        }
    }
}

export fn erase_icon() {
    return T {
        name = "erase_icon"
        T.scale(1.2, 0.6, 0.1) {
            R.cube() { C.rgba(0.9, 0.4, 0.6, 1.0) }
        }
    }
}

export fn grid_tool_icon() {
    return T.scale(0.5, 0.5, 0.2) {
        name = "grid_tool_icon"

        let beam_color = [0.62, 0.62, 0.62, 1.0]
        let plus_color = [0.78, 0.78, 0.78, 1.0]
        // border beams
        T.position(0.0, 0.95, 0.0).scale(1.6, 0.18, 0.12) {
            R.cube() { C.rgba(beam_color[0], beam_color[1], beam_color[2], beam_color[3]) }
        }
        T.position(0.0, -0.95, 0.0).scale(1.6, 0.18, 0.12) {
            R.cube() { C.rgba(beam_color[0], beam_color[1], beam_color[2], beam_color[3]) }
        }
        T.position(-0.95, 0.0, 0.0).scale(0.18, 1.6, 0.12) {
            R.cube() { C.rgba(beam_color[0], beam_color[1], beam_color[2], beam_color[3]) }
        }
        T.position(0.95, 0.0, 0.0).scale(0.18, 1.6, 0.12) {
            R.cube() { C.rgba(beam_color[0], beam_color[1], beam_color[2], beam_color[3]) }
        }
        // center cross 
        T.scale(0.22, 1.65, 0.16) {
            R.cube() { C.rgba(plus_color[0], plus_color[1], plus_color[2], plus_color[3]) }
        }
        T.scale(1.65, 0.22, 0.16) {
            R.cube() { C.rgba(plus_color[0], plus_color[1], plus_color[2], plus_color[3]) }
        }
    }
}

export fn delete_x_icon() {
    return T {
        name = "delete_x_icon"
        let arm = T.scale(0.22, 1.15, 0.22) {
            R.cube() { C.rgba(0.92, 0.20, 0.20, 1.0) }
        }
        T.rotation(0.0, 0.0, 0.785) { arm }
        T.rotation(0.0, 0.0, -0.785) { arm }
    }
}

export fn grid_visibility_icon() {
    return T {
        name = "grid_visibility_icon"
        T.scale(1.2, 0.55, 0.15) {
            R.cube() { C.rgba(0.10, 0.16, 0.12, 1.0) }
        }
        T.scale(0.7, 0.25, 0.16) {
            R.cube() { C.rgba(0.92, 0.97, 0.92, 1.0) }
        }
        T.scale(0.18, 0.18, 0.2) {
            R.cube() { C.rgba(0.10, 0.16, 0.12, 1.0) }
        }
    }
}

export fn checkmark_icon() {
    return T.scale(0.42, 0.42, 0.42) {
        name = "checkmark_icon"
        T.position(0.18, 0.06, 0.0).rotation(0.0, 0.0, 0.72) {
            T.scale(0.16, 1.10, 0.12) {
                R.cube() { C.rgba(0.18, 0.68, 0.32, 1.0) }
            }
        }
        T.position(-0.22, -0.20, 0.0).rotation(0.0, 0.0, -0.62) {
            T.scale(0.16, 0.52, 0.12) {
                R.cube() { C.rgba(0.18, 0.68, 0.32, 1.0) }
            }
        }
    }
}
