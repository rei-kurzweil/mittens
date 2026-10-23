// XR acceptance companion: cargo run --release -- load examples/shading-models-xr.mms
// Compare both eyes, turn/translate your head to inspect the Anime rim, then
// drag shade strength with a controller pointer. Right model + sphere share
// one Anime source; left model + sphere remain explicitly Toon.
import { pose } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"
import { anime_shading_controls } from "../assets/components/ui/anime_shading_controls.mms"

BGC.rgba(0.035, 0.040, 0.055, 1.0)
AL.rgb(0.06, 0.06, 0.06)
T.position(-0.5, 0.7, 1.0) { DL.color(1.0, 1.0, 1.0).intensity(1.0) }

T {
    InputXR.on() {
        InputXRGamepad { locomotion() speed(1.5) }
        T { CXR { Pointer {} } }
        XRHand.new(true, "Left", "GripAim").laser() { T { Pointer {} } }
        XRHand.new(true, "Right", "GripAim").laser() { T { Pointer {} } }
    }
}
XR.on()

let anime_target = bisket_anime_shading()
let reset_values = {
    shade_strength = anime_target.get_shade_strength()
    shade_threshold = anime_target.get_shade_threshold()
    lit_threshold = anime_target.get_lit_threshold()
    rim_strength = anime_target.get_rim_strength()
    rim_power = anime_target.get_rim_power()
}
T.position(1.1, 0.0, -3.5) {
    anime_target
    GLTF.new("assets/models/bisket.glb") { pose() }
    T.position(0.65, 0.45, 0.0).scale(0.3, 0.3, 0.3) {
        R.sphere() { C.rgba(0.85, 0.65, 0.55, 1.0) }
    }
}
T.position(-1.1, 0.0, -3.5) {
    Shading.toon()
    GLTF.new("assets/models/bisket.glb") { pose() }
    T.position(-0.65, 0.45, 0.0).scale(0.3, 0.3, 0.3) {
        R.sphere() { C.rgba(0.85, 0.65, 0.55, 1.0) }
    }
}
let panel = info_panel({
    root_name = "anime_shading_panel"
    title = "Anime shading / XR"
    width_gu = 40.0
    unit_scale = 0.075
    content = anime_shading_controls(anime_target, reset_values)
})
T.position(-1.5, 2.65, -2.5) { panel }
let body_mount = panel.query("#accordion_body_mount")
on(panel, "DataEvent", fn(event) {
    if event == "AccordionRestoreRequested" {
        body_mount.attach(info_panel_body({ content = anime_shading_controls(anime_target, reset_values) }))
    }
})
