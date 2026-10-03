import { vroid_arm_ik } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik(-0.35, -1)

// PC-Rei XR mirror with model-specific hair, ear, bust, and tail motion.
import { pc_rei_secondary_motion } from "../assets/components/secondary_motion/pc-rei.mms"
import { pc_rei_colliders } from "../assets/components/colliders/pc-rei.mms"

RendererSettings {
    window_size(640, 480)
    msaa_samples(4)
}

BGC.rgba(0.05, 0.07, 0.12, 1.0)
AL.rgb(0.18, 0.18, 0.22)

RenderGraph {
    EmissivePass { BlurPass { radius_ndc(0.06) half_res(true) } }
    Bloom { intensity(0.8) emissive_scale(1.1) }
}

T.position(1.0, 2.5, 1.5) {
    DL { intensity(1.2) color(1.0, 0.98, 0.95) }
}

ED {
    // Room markers make the avatar's lag and settling easy to judge.
    T.position(0.0, -0.85, -1.5).scale(6.0, 0.12, 6.0) {
        R.cube() { C.rgba(0.20, 0.22, 0.27, 1.0) }
    }
    T.position(-1.8, 0.0, -2.8).scale(0.25, 0.9, 0.25) {
        R.cube() { C.rgba(0.2, 0.8, 1.0, 1.0) EM.on() Raycastable.enabled() }
    }
    T.position(1.8, 0.0, -2.8).scale(0.25, 0.9, 0.25) {
        R.cube() { C.rgba(1.0, 0.35, 0.7, 1.0) EM.on() Raycastable.enabled() }
    }

    // Full-body mirror in front of the XR start pose.
    T.position(0.0, 1.25, -4.5).scale(2.4, 2.4, 0.08) {
        R.cube() { Mirror.quality(2048) {} }
    }

    T {
        name = "avatar_locomotion_root"
        InputXR.on() {
            InputXRGamepad { locomotion() speed(1.5) }
            T {
                AVC {
                initial_yaw(3.14159)
                left_two_bone_ik(arm_ik.left)
                right_two_bone_ik(arm_ik.right)
                hand_rotation_smoothing(220.0)
                T {
                    GLTF.new("assets/models/pc-rei.hoodie.glb") {
                        EM.on()
                        PoseCapture { label("PC-Rei") asset_name("pc-rei") }
                        pc_rei_colliders()
                        pc_rei_secondary_motion()
                    }
                }

                T.position(0.0, 0.18, 0.12) {
                    name = "pc_rei_mirror_xr_camera"
                    CXR { Pointer {} }
                }

                XRHand.new(true, "Left", "GripAim").laser() {
                    T {
                        RestAttachment.new("[name='J_Bip_L_Hand']", "[name='J_Bip_L_Middle3']") {
                            Pointer {}
                        }
                    }
                }
                XRHand.new(true, "Right", "GripAim").laser() {
                    T {
                        RestAttachment.new("[name='J_Bip_R_Hand']", "[name='J_Bip_R_Middle3']") {
                            Pointer {}
                        }
                    }
                }
                }
            }
        }
    }
}

XR.on()
