// vr-input scene
// Corresponds to examples/vr-input.rs

// --- Renderer settings ---
let renderer = RendererSettings.msaa_off() {
    window_size(320, 240)
}

// --- Sky color and ambient light ---
let sky     = BGC.rgba(0.62, 0.80, 1.00, 1.0)
let ambient = AL.rgb(0.18, 0.18, 0.22)

renderer
sky
ambient

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
        C3D {}
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
        TextBackground {
            padding(0.5)
            padding_right(1.5)
            C.rgba(0.9, 0.9, 0.9, 1.0)
        }
        EM.on()
        TextureFiltering.linear()

    }
}

// --- Background sun ---
BG {
    T {
        position(2.0, 1.5, -8.0)
        scale(3.5, 3.5, 3.5)
        R.circle2d() {
            C.rgba(1.0, 0.85, 0.15, 1.0)
            EM.on()
        }
        T {
            position(-0.35, 0.35, -0.01)
            scale(0.45, 0.45, 0.45)
            R.circle2d() {
                C.rgba(1.0, 1.0, 1.0, 1.0)
                EM.on()
            }
        }
    }
}

// --- VTuber avatar — single-input topology ---
//
// InputXR drives body translation and head rotation via AvatarControlSystem.
// ControllerXR and CameraXR children are discovered by topology.
// camera_bone triggers two things at AVC init:
//   1. model_root.y is auto-calibrated to -J_Bip_C_Head_local_y (no hardcoded constant).
//   2. CXR is re-parented under J_Bip_C_Head for first-person XR alignment.
//
// Topology (after AvatarControlSystem init):
//   ED
//     └── InputXR
//           └── T (driven_t)
//                 └── AVC
//                       ├── TransformPipeline (body pipeline)
//                       │     TransformForkTRS
//                       │       TransformMapRotation
//                       │         QuatYawFollow { threshold, rate, initial_yaw: π }
//                       │       TransformMergeTRS
//                       │     TransformPipelineOutput
//                       │       └── T  ← model_root (y auto-calibrated from J_Bip_C_Head)
//                       │             └── GLTF { EM }
//                       │                   └── ... → J_Bip_C_Head
//                       │                                 └── CXR  ← re-parented here
//                       ├── CTLXR(Left, Grip)   ← discovered; re-parented to lower_arm
//                       │     └── T
//                       └── CTLXR(Right, Grip)
//                             └── T
ED {
    InputXR.on() {
        T {
            AVC {
                head_bone("J_Bip_C_Neck")
                //avatar_height(1.85)
                camera_bone("J_Bip_C_Head")
                left_hand_bone("J_Bip_L_Hand")
                right_hand_bone("J_Bip_R_Hand")
                initial_yaw(3.14159)
                hand_rotation_smoothing(220.0)

                T {
                    GLTF.new("assets/models/pc-rei.hoodie.glb") { EM.on() }
                }

                CXR {}
                CTLXR.new(true, Left, Grip) { T {} }
                CTLXR.new(true, Right, Grip) { T {} }
            }
        }
    }
}

// --- XR rig (Aim controller debug cubes; camera has moved to AVC above) ---
InputXR.on() {
    T {
        T.position(0.0, 1.85, 0.6) {
            RendererStats {
                camera_target(Xr)
            }
        }

        // Controller debug cubes (Aim pose, rotation-smoothed)
        CTLXR.new(true, Left, Aim) {
            T.scale(0.06, 0.06, 0.12) {
                TransformPipeline {
                    TransformForkTRS {
                        TransformMapTranslation {}
                        TransformMapRotation {
                            QuatTemporalFilter.smoothing_factor(220.0)
                        }
                        TransformMapScale {}
                        TransformMergeTRS {}
                    }
                    TransformPipelineOutput {
                        T {
                            R.cube() {
                                C.rgba(0.10, 0.90, 1.00, 1.0)
                            }
                        }
                    }
                }
            }
        }

        CTLXR.new(true, Right, Aim) {
            T.scale(0.06, 0.06, 0.12) {
                TransformPipeline {
                    TransformForkTRS {
                        TransformMapTranslation {}
                        TransformMapRotation {
                            QuatTemporalFilter.smoothing_factor(220.0)
                        }
                        TransformMapScale {}
                        TransformMergeTRS {}
                    }
                    TransformPipelineOutput {
                        T {
                            R.cube() {
                                C.rgba(1.00, 0.35, 0.35, 1.0)
                            }
                        }
                    }
                }
            }
        }
    }
}

// --- OpenXR runtime ---
XR.on()
