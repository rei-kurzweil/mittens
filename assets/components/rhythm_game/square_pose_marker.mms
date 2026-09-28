// Local +Z faces the player. Query a returned marker with
// marker.query("#square_pose_marker_fill_animation").play().

export fn square_pose_marker(config) {
    let color = [1.0, 0.55, 0.18, 1.0]
    let glow_intensity = 1.5
    if config != null {
        if config["color"] != null { color = config["color"] }
        if config["intensity"] != null { glow_intensity = config["intensity"] }
    }

    let fill = T.position(0.0, 0.0, 0.003).scale(0.001, 0.001, 1.0) {
        name = "square_pose_marker_fill"
        Transition {
            duration_beats(1.0)
            replace_same_target()
        }
        R.square() {
            C.rgba(color[0], color[1], color[2], color[3])
            EM.on() { intensity(glow_intensity) }
        }
    }

    let fill_animation = Animation.paused().length(1.0) {
        name = "square_pose_marker_fill_animation"
        Keyframe.at(0.0) {
            // Transition interpolates from the authored tiny scale over one
            // beat. The plane meets the 0.1-thick outline's inner edge.
            fill.update_transform([0.0, 0.0, 0.003], [0.0, 0.0, 0.0], [0.8, 0.8, 1.0])
        }
    }

    return T {
        name = "square_pose_marker"
        R.wireframe_square(0.1) {
            C.rgba(color[0], color[1], color[2], color[3])
            EM.on() { intensity(glow_intensity) }
        }
        fill
        fill_animation
    }
}
