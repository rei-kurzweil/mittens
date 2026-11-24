import { ui_unlit_material } from "./materials/ui_unlit.mms"

// A 10 x 1 bar facing local +Z. Pass a title component or null.
// The returned transform's set_health(health: f32) takes a fraction in [0, 1].
export fn health_bar(title) {
    let material = ui_unlit_material()
    let pink = [1.0, 0.32, 0.60, 1.0]
    let red = [1.0, 0.025, 0.055, 1.0]
    let width = 10.0
    let height = 1.0
    let border = 0.2
    let inner_width = width - 2.0 * border
    let inner_height = height - 2.0 * border
    let left = -inner_width / 2.0
    let state = { critical = false }

    fn white_prism(x, y, z, w, h, depth) {
        return T.position(x, y, z).scale(w, h, depth) {
            R.cube() { material C.rgba(1.0, 1.0, 1.0, 1.0) }
        }
    }
    let fill_mesh = R.cube() { material C.rgba(pink[0], pink[1], pink[2], pink[3]) }
    let fill = T.position(0.0, 0.0, 0.065).scale(inner_width, 0.44, 0.09) {
        name = "health_bar_fill"
        fill_mesh
    }
    let pulse = Animation.paused().length(1.0) {
        name = "health_bar_pulse"
        Keyframe.at(0.0) { if state.critical { fill_mesh.set_color(red) } }
        Keyframe.at(0.5) { if state.critical { fill_mesh.set_color([0.0, 0.0, 0.0, 1.0]) } }
    }
    let root = T {
        name = "health_bar"
        Raycastable.disabled()
        Selectable.off()
        T.position(0.0, 0.0, -0.12).scale(inner_width, inner_height, 0.08) {
            R.cube() { material C.rgba(0.08, 0.05, 0.08, 1.0) }
        }
        for side in [-1.0, 1.0] {
            white_prism(0.0, side * (height - border) / 2.0, 0.0, width, border, 0.24)
            white_prism(side * (width - border) / 2.0, 0.0, 0.0, border, inner_height, 0.24)
        }
        for index in range(5) {
            white_prism(left + (index + 1.0) * inner_width / 6.0, 0.0, -0.02, 0.08, inner_height, 0.10)
        }
        fill
        if title { T.position(-width / 2.0, 1.32, 0.12) { title } }
        pulse
    }
    root.set_health = fn(health) {
        let fraction = Math.clamp(health, 0.0, 1.0)
        let filled_width = inner_width * fraction
        fill.update_transform([left + filled_width / 2.0, 0.0, 0.065],
            [0.0, 0.0, 0.0], [filled_width, 0.44, 0.09])
        let critical = fraction < 0.15
        if critical != state.critical {
            state.critical = critical
            if critical {
                fill_mesh.set_color(red)
                pulse.loop_anim()
            } else {
                pulse.pause()
                fill_mesh.set_color(pink)
            }
        }
    }
    return root
}
