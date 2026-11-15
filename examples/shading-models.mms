// Side-by-side Bisket shading-model comparison.
//
// Run with:
//   cargo run --release -- load examples/shading-models.mms

// The left model explicitly uses Toon; the right uses Anime. The panel edits
// the right model's shared shade strength live. Drag the slider, Reset, and
// minimize/restore to verify updates without reloading either model.

import { pose as relaxed_pose_factory } from "../assets/components/poses/bisket/000-relaxed.pose.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"
import { bisket_secondary_motion } from "../assets/components/secondary_motion/bisket.mms"
import { tripod_light } from "../assets/components/tripod_light.mms"
import { info_panel, info_panel_body } from "../assets/components/ui/info_panel.mms"
import { anime_shading_controls } from "../assets/components/ui/anime_shading_controls.mms"
import { truss } from "../assets/components/truss.mms"

RendererSettings { window_size(1280, 720) }
BGC.rgba(0.035, 0.040, 0.055, 1.0)
AL.rgb(0.06, 0.06, 0.06)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.025) half_res(true) } }
    Bloom { intensity(0.30) radius_ndc(0.025) emissive_scale(1.0) half_res(true) }
}

Material(MaterialDefinition.anime()).inputs({
    shade_color = [0.4, 0.4, 0.65]
    shade_strength = 0.8
    shade_threshold = 0.4
    lit_threshold = 0.55
    rim_color = [1.0, 1.0, 1.0]
    rim_strength = 0.38
}) {
    // Neutral floor and backdrop make differences in the two model materials clear.
    T.position(0.0, -0.06, 0.0).scale(12.0, 0.12, 10.0) {
        name = "shading_models_floor"
        R.cube() { C.rgba(0.11, 0.12, 0.15, 1.0) }
    }
    T.position(0.0, 2.4, -2.2).scale(12.0, 4.8, 0.12) {
        name = "shading_models_backdrop"
        R.cube() { C.rgba(0.15, 0.16, 0.20, 1.0) }
    }
}

// A subdued overhead truss frames the comparison without drawing attention
// away from the two shading models.
T.position(0.0, 4.05, -0.7).scale(0.78, 0.78, 0.78) {
    name = "shading_models_overhead_truss"
    Unlit {
        truss(32)
    }
}

let left_model_x = -1.65
let right_model_x = 1.65

// Explicit Toon comparison baseline.
T.position(left_model_x, 0.0, 0.0) {
    name = "bisket_default_shading"
    GLTF.new("assets/models/bisket.glb") {
        Material(MaterialDefinition.toon())
        relaxed_pose_factory()
        bisket_secondary_motion(false)
    }
}

let anime_target = bisket_anime_shading()
let anime_reset_values = {
    shade_strength = anime_target.get_input("shade_strength")
    shade_threshold = anime_target.get_input("shade_threshold")
    lit_threshold = anime_target.get_input("lit_threshold")
    rim_strength = anime_target.get_input("rim_strength")
    rim_power = anime_target.get_input("rim_power")
}

// Albedo-derived anime shading model with a two-state light ramp and rim light.
T.position(right_model_x, 0.0, 0.0) {
    name = "bisket_anime_shading"
    GLTF.new("assets/models/bisket.glb") {
        relaxed_pose_factory()
        bisket_secondary_motion(false)
        anime_target
    }
}

// Matching grabbable fixtures flank the models and aim at their upper bodies.
tripod_light(
    "default_shading_spotlight",
    [-4.5, 0.0, 2.2],
    [left_model_x, 1.25, 0.0],
    SL.color(1.0, 1.0, 1.0).intensity(8.0).distance(12.0).angle(0.58).penumbra(0.25),
)
tripod_light(
    "anime_shading_spotlight",
    [4.5, 0.0, 2.2],
    [right_model_x, 1.25, 0.0],
    SL.color(1.0, 1.0, 1.0).intensity(8.0).distance(12.0).angle(0.58).penumbra(0.25),
)

// Movable desktop overview camera. C3D looks along local -Z.
I.speed(2.0) {
    name = "shading_models_camera_input"
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 1.55, 7.5) {
        name = "shading_models_camera"
        C3D { Pointer {} }
    }
}

// Ordinary world-space UI using the existing panel prefab.
let anime_panel = info_panel({
    root_name = "anime_shading_panel"
    title = "Anime shading"
    width_gu = 40.0
    unit_scale = 0.075
    content = anime_shading_controls(anime_target, anime_reset_values)
})
T.position(0.0, 3.05, 0.8) { anime_panel }

let anime_body_mount = anime_panel.query("#accordion_body_mount")
on(anime_panel, "DataEvent", fn(event) {
    if event == "AccordionRestoreRequested" {
        anime_body_mount.attach(info_panel_body({
            content = anime_shading_controls(anime_target, anime_reset_values)
        }))
    }
})
