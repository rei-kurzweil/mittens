import { bisket_anime_shading } from "../materials/bisket_anime_shading.mms"
import { bisket_humanoid_bone_map } from "../humanoid_bone_maps/bisket.mms"
import { bisket_colliders } from "../colliders/bisket.mms"
import { bisket_shirt_physics } from "../secondary_motion/bisket-shirt-physics.mms"
import { mitten_bow_secondary_motion } from "../secondary_motion/mitten-bow.mms"
import { tail_wag } from "../animations/tail_wag.mms"
import { install_avatar_equipment } from "./avatar_equipment.mms"

// Shared mitten avatar model, for VR and desktop scenes. Returns the spec
// shape used by rooftop_player's avatar factories:
//   { model, eye_height, sole_offset }
// `model_name` may be false to leave the GLTF unnamed. Options (all optional):
//   xr               add OpenXR hand tracking
//   tail_wag         "canine", "feline", or false for a still tail
//   beats_per_second scene Clock BPM / 60 (default 2.0, a 120 BPM clock)
//   material         replaces the default bisket_anime_shading()
//   children         array of extra components attached to the GLTF root,
//                    such as poses, EM.on(), PoseCapture, or Raycastable
// The caller owns the body (AVC), cameras, and other scene-specific nodes.
export fn mitten_model(model_name, options) {
    let xr = options["xr"]
    let tail_style = options["tail_wag"]
    let beats_per_second = options["beats_per_second"]
    if !beats_per_second { beats_per_second = 2.0 }
    let material = options["material"]
    let children = options["children"]
    let model = GLTF.new("assets/models/mitten.glb") {
        if model_name { name = model_name }
        if xr { HandTracking.new().source("openxr") }
        if material { material }
        if !material { bisket_anime_shading() }
        bisket_humanoid_bone_map()
        MorphTargetMap.new()
            .slot("left_eye_blink", "Fcl_EYE_Close_L")
            .slot("right_eye_blink", "Fcl_EYE_Close_R")
            .slot("viseme_aa", "Fcl_MTH_A")
        bisket_colliders()
        bisket_shirt_physics(false)
        mitten_bow_secondary_motion()
        if children {
            for child in children { child }
        }
    }
    install_avatar_equipment(model, [
        ["J_Bip_L_UpperLeg", [0.13, 0.17, 0.0], 0.12],
        ["J_Bip_R_UpperLeg", [-0.13, 0.17, 0.0], 0.12],
        ["J_Bip_C_UpperChest", [0.0, 0.05, -0.21], 0.16]
    ])
    if tail_style {
        on(model, "GLTFInitialized", fn(event) {
            let tail = event.gltf.query("[name='tail']")
            if tail {
                model.attach(tail_wag(tail, tail_style, beats_per_second))
            } else {
                print("mitten_model: tail joint not found; tail wag was not attached")
            }
        })
    }
    return { model = model eye_height = 1.58 sole_offset = [0.0, 0.0530325, 0.0874534] }
}
