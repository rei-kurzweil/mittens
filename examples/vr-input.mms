import { vroid_arm_ik } from "../assets/components/arm_ik/vroid.mms"

let arm_ik = vroid_arm_ik(-0.35, -1)

// vr-input scene
// Corresponds to examples/vr-input.rs

import { pc_rei_secondary_motion } from "../assets/components/secondary_motion/pc-rei.mms"
import { pc_rei_colliders } from "../assets/components/colliders/pc-rei.mms"

// --- Renderer settings ---
let renderer = RendererSettings.msaa_off() {
    window_size(640, 480)
}

// --- Sky color and ambient light ---
let sky     = BGC.rgba(0.62, 0.80, 1.00, 1.0)
let ambient = AL.rgb(0.18, 0.18, 0.22)

renderer
sky
ambient

RenderGraph {
    EmissivePass {}

    Bloom {
        intensity(0.95)
        radius_ndc(0.06)
        emissive_scale(1.2)
        half_res(true)
    }
}

// --- Directional light ---
T.position(0.15, -0.45, 1.0) {
    DL {
        intensity(1.1)
        color(1.0, 0.98, 0.95)
    }
}

// --- Desktop camera rig ---
I.speed(1.5) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 1.2, 3.5) {
        C3D {
            Pointer {}
        }
    }
}

// --- Desktop camera controls hint ---
T {
    position(0.65, 1.45, 1.8)
    scale(0.055, 0.055, 1.0)
    TXT {
        "use wasd/rf/qe\nand right-mouse\nclick and drag\nto move/look"
        Raycastable.enabled()
        C.rgba(0.0, 0.0, 0.0, 1.0)
        EM.on()
        TextureFiltering.linear()

    }
}

// --- Background sun ---
BG {
    T {
        position(2.0, 1.5, -8.0)
        scale(3.5, 3.5, 3.5)
        R.circle_2d() {
            C.rgba(1.0, 0.85, 0.15, 1.0)
            EM.on()
        }
        T {
            position(-0.35, 0.35, -0.01)
            scale(0.45, 0.45, 0.45)
            R.circle_2d() {
                C.rgba(1.0, 1.0, 1.0, 1.0)
                EM.on()
            }
        }
    }
}

// --- VTuber avatar — single-input topology ---
//
// InputXR drives body translation and head rotation via AvatarControlSystem.
// XRHand and CameraXR children are discovered by topology.
// The resolved humanoid camera anchor triggers two things at AVC init:
//   1. model_root.y is auto-calibrated to -J_Bip_C_Head_local_y (no hardcoded constant).
//   2. CXR is re-parented under J_Bip_C_Head for first-person XR alignment.
//
// Topology (after AvatarControlSystem init):
//   ED
//     └── InputXR
//           └── T (driven_t)
//                 └── AVC
//                       ├── TransformForkTRS (body pipeline root)
//                       │     TransformMapRotation
//                       │       TransformMapRotation
//                       │         QuatYawFollow { threshold, rate, initial_yaw: π }
//                       │       TransformMergeTRS
//                       │       └── T  ← model_root (y auto-calibrated from J_Bip_C_Head)
//                       │             └── GLTF { EM }
//                       │                   └── ... → J_Bip_C_Head
//                       │                                 └── CXR  ← re-parented here
//                       ├── XRHand(Left, Grip)  ← discovered; re-parented to lower_arm
//                       │     └── T
//                       └── XRHand(Right, Grip)
//                             └── T
ED {
    T {
        name = "avatar_locomotion_root"
        InputXR.on() {
            InputXRGamepad {
                locomotion()
                speed(1.5)
            }
            T {
                AVC {
                //avatar_height(1.85)
                
                initial_yaw(3.14159)
                
                left_two_bone_ik(arm_ik.left)
                right_two_bone_ik(arm_ik.right)
                hand_rotation_smoothing(220.0)
                T {
                    GLTF.new("assets/models/pc-rei.hoodie.glb") {
                        EM.on()
                        pc_rei_colliders()
                        pc_rei_secondary_motion()
                    }
                }

                T.position(0.0, 0.18, 0.12) {
                        name = "xr_camera_wrapper"
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

// --- VR runtime ---
XR.on()
