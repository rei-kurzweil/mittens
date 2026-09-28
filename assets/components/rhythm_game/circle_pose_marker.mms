// Local +Z faces the player. Query a returned marker with
// marker.query("#circle_pose_marker_fill_animation").play(). The animation
// starts paused and fills over one beat after play().

export fn circle_pose_marker(config) {
    let color = [0.20, 0.85, 1.0, 1.0]
    let glow_intensity = 1.5
    if config != null {
        if config["color"] != null { color = config["color"] }
        if config["intensity"] != null { glow_intensity = config["intensity"] }
    }

    let fill = T.position(0.0, 0.0, 0.003).scale(0.001, 0.001, 1.0) {
        name = "circle_pose_marker_fill"
        Transition {
            duration_beats(1.0)
            replace_same_target()
        }
        R.circle_2d() {
            C.rgba(color[0], color[1], color[2], color[3])
            EM.on() { intensity(glow_intensity) }
        }
    }

    let fill_animation = Animation.paused().length(1.0) {
        name = "circle_pose_marker_fill_animation"
        Keyframe.at(0.0) {
            // Transition interpolates from the authored tiny scale over one
            // beat. The disk meets the annulus's inner radius (0.45).
            fill.update_transform([0.0, 0.0, 0.003], [0.0, 0.0, 0.0], [0.9, 0.9, 1.0])
        }
    }

    return T {
        name = "circle_pose_marker"
        R.annulus_2d() {
            C.rgba(color[0], color[1], color[2], color[3])
            EM.on() { intensity(glow_intensity) }
        }
        fill
        fill_animation
    }
}
