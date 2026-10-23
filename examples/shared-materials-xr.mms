// Materials v3 checkpoint: stable material instances, using Anime.
// cargo run --release -- load examples/shared-materials-xr.mms
// Shared and independent instances use the same built-in Anime definition.
// Optional allocation/upload diagnostics: CAT_DEBUG_MATERIAL_INSTANCES=1
import { info_panel } from "../assets/components/ui/info_panel.mms"
import { anime_shading_controls } from "../assets/components/ui/anime_shading_controls.mms"

BGC.rgba(0.035, 0.040, 0.055, 1.0)
AL.rgb(0.06, 0.06, 0.06)
T.position(-0.5, 0.7, 1.0) { DL.color(1.0, 0.95, 0.90).intensity(1.0) }
T {
    InputXR.on() {
        InputXRGamepad { locomotion() speed(1.5) }
        T { CXR { Pointer {} } }
        XRHand.new(true, "Left", "GripAim").laser() {
            T { name = "left_controller_pose" Pointer {} }
        }
        XRHand.new(true, "Right", "GripAim").laser() {
            T { name = "right_controller_pose" Pointer {} }
        }
    }
}
XR.on()

let shared = Shading.anime().shade_strength(0.75)
    .shade_color([0.25, 0.10, 0.45]).rim_strength(0.35) { name = "shared_anime_source" }
let independent = Shading.anime().shade_strength(0.75)
    .shade_color([0.25, 0.10, 0.45]).rim_strength(0.35) { name = "independent_anime_source" }
let reset_values = {
    shade_strength = shared.get_shade_strength()
    shade_threshold = shared.get_shade_threshold()
    lit_threshold = shared.get_lit_threshold()
    rim_strength = shared.get_rim_strength()
    rim_power = shared.get_rim_power()
}

let left = R.cube() { name = "shared_cube" C.rgba(0.90, 0.65, 0.40, 1.0) shared }
let right = R.sphere() { name = "shared_sphere" C.rgba(0.90, 0.65, 0.40, 1.0) shared }
let right_transform = T.position(0.9, 1.35, -2.7).scale(0.36, 0.36, 0.36) { right }
T.position(0.0, 1.35, -2.7).scale(0.65, 0.65, 0.65) { left }
right_transform
T.position(1.8, 1.35, -2.7).scale(0.36, 0.36, 0.36) {
    R.sphere() { name = "independent_sphere" C.rgba(0.90, 0.65, 0.40, 1.0) independent }
}
T.position(0.0, 0.85, -2.7).scale(0.12, 0.12, 0.12) { Text { "Shared cube" } }
T.position(0.75, 0.85, -2.7).scale(0.12, 0.12, 0.12) { Text { "Shared sphere" } }
T.position(1.65, 0.85, -2.7).scale(0.12, 0.12, 0.12) { Text { "Independent" } }
T.position(0.0, -0.04, -2.0).scale(10.0, 0.05, 10.0) {
    R.cube() { C.rgba(0.15, 0.17, 0.21, 1.0) Shading.toon() }
}

fn action_button(label, button_name) {
    return T {
        name = button_name
        Raycastable.enabled() { interaction_priority(120.0) }
        Style {
            display("flex") width(100%) height(2.6) margin_top(0.3)
            align_items("center") padding_xy(0.6, 0.2)
            background_color([0.24, 0.20, 0.34, 1.0]) background_z(-0.02)
        }
        T.position(0.0, 0.0, 0.03) { Text { label } }
    }
}

let motion = { raised = false }
let pulse = { enabled = false seconds = 0.0 }
on_global("FrameTick", fn(event) {
    if pulse.enabled {
        pulse.seconds = pulse.seconds + event.dt_sec
        shared.set_shade_strength(0.5 + 0.45 * Math.sin(pulse.seconds * 1.8))
    }
})
fn controls() {
    let pulse_label = "Start shared pulse"
    if pulse.enabled { pulse_label = "Stop shared pulse / resume sliders" }
    let content = T {
        Style { display("block") width(100%) font_size(0.8) color([0.96, 0.96, 0.98, 1.0]) }
        anime_shading_controls(shared, reset_values)
        action_button(pulse_label, "shared_pulse")
        action_button("Detach sphere / cube stays shared", "shared_detach_one")
        action_button("Detach both / keep source alive", "shared_detach_all")
        action_button("Restore both attachments", "shared_restore")
        action_button("Move sphere independently", "shared_move")
    }
    on(content.query("#shared_pulse"), "Click", fn(event) {
        pulse.enabled = !pulse.enabled
        let label = "Start shared pulse"
        if pulse.enabled { label = "Stop shared pulse / resume sliders" }
        content.query("#shared_pulse").query("Text").set_text(label)
        if !pulse.enabled {
            let strength = shared.get_shade_strength()
            content.query("#anime_shade_strength_slider").sync_value(strength)
            content.query("#anime_shade_strength_readout").set_text("" + Math.round(strength * 100.0) / 100.0)
        }
    })
    on(content.query("#shared_detach_one"), "Click", fn(event) { right.detach(shared) })
    on(content.query("#shared_detach_all"), "Click", fn(event) { shared.detach_all() })
    on(content.query("#shared_restore"), "Click", fn(event) {
        shared.detach_all()
        left.attach(shared)
        right.attach(shared)
    })
    on(content.query("#shared_move"), "Click", fn(event) {
        motion.raised = !motion.raised
        let height = 1.35
        if motion.raised { height = 1.85 }
        right_transform.update_transform([0.9, height, -2.7])
    })
    return content
}
T.position(-2.4, 2.6, -2.0) {
    info_panel({
        root_name = "shared_materials_panel"
        title = "Shared materials / Anime"
        width_gu = 36.0
        unit_scale = 0.05
        content_factory = controls
    })
}
