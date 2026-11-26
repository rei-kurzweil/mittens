import { bisket_anime_shading } from "../../materials/bisket_anime_shading.mms"
import { vroid_humanoid_bone_map } from "../../humanoid_bone_maps/vroid.mms"
import { bisket_colliders } from "../../colliders/bisket.mms"
import { bisket_shirt_physics } from "../../secondary_motion/bisket-shirt-physics.mms"
import { tail_wag } from "../../animations/tail_wag.mms"
import { install_vroid_equipment } from "../avatar_equipment.mms"

// The GLB is built from the mitten base by make_demon.py (--variant succubus)
// and kept local; see assets/models/local/README.md.
let SUCCUBUS_URI = "assets/models/local/succubus.glb"

// Succubus avatar constructor, for VR and desktop scenes. Takes one options
// table and returns the GLTF component, carrying its calibration as fields:
//   eye_height   rest eye height in meters
//   sole_offset  local sole landmark for the AVC's crouch_sole_offsets
// The skeleton is mitten's (VRoid bones plus a two-joint `tail`), so mitten's
// colliders, shirt physics, tail wag and poses apply unchanged; only the bow
// is absent. Options (all optional):
//   name             the GLTF's name
//   xr               add OpenXR hand tracking
//   tail_wag         "canine", "feline", or false for a still tail
//   beats_per_second scene Clock BPM / 60 (default 2.0, a 120 BPM clock)
//   material         replaces the default bisket_anime_shading()
//   target_height    optional imported rest height in meters
//   gltf_children    array of extra components attached to the GLTF root,
//                    such as poses, EM.on(), PoseCapture, or Raycastable
// The caller owns the body (AVC), cameras, and other scene-specific nodes.
export fn succubus_avatar(options) {
    let model_name = options["name"]
    let xr = options["xr"]
    let tail_style = options["tail_wag"]
    let beats_per_second = options["beats_per_second"]
    if !beats_per_second { beats_per_second = 2.0 }
    let material = options["material"]
    let target_meters = options["target_height"]
    let children = options["gltf_children"]
    let model = GLTF.new(SUCCUBUS_URI) {
        if target_meters != null { target_height(target_meters) }
        if model_name { name = model_name }
        if xr { HandTracking.new().source("openxr") }
        if material { material }
        if !material { bisket_anime_shading() }
        vroid_humanoid_bone_map()
        MorphTargetMap.new()
            .slot("left_eye_blink", "Fcl_EYE_Close_L")
            .slot("right_eye_blink", "Fcl_EYE_Close_R")
            .slot("viseme_aa", "Fcl_MTH_A")
        bisket_colliders()
        bisket_shirt_physics(false)
        if children {
            for child in children { child }
        }
    }
    install_vroid_equipment(model)
    if tail_style {
        on(model, "GLTFInitialized", fn(event) {
            let tail = event.gltf.query("[name='tail']")
            if tail {
                model.attach(tail_wag(tail, tail_style, beats_per_second))
            } else {
                print("succubus_avatar: tail joint not found; tail wag was not attached")
            }
        })
    }
    model.eye_height = 1.659
    model.sole_offset = [0.0, 0.055684, 0.091826]
    return model
}
