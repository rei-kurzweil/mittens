// bisket-vr-demo scene
//
// OpenXR room-scale VR demo using the bisket avatar (bisket.8.0.glb).
// Mirrors examples/vr-input.{rs,mms} topology but with the head-driven
// AVC convention (head_bone="J_Bip_C_Head") and the bisket model.
//
// Drivers:
//   - InputXR.on(): HMD pose → driven_t (translation + rotation)
//   - CTLXR(Left/Right, Grip): hand pose → splices into J_Bip_{L,R}_Hand
//                              (TwoBoneIK on upper/lower arm when chain resolves)
//   - CXR (CameraXR): direct child of AVC, re-parented under J_Bip_C_Head.
//     XR runtime overrides pose; no manual flip needed.
//
// Topology (after AvatarControlSystem init):
//   ED
//     └── InputXR
//           └── T (driven_t)
//                 └── AVC
//                       ├── TransformForkTRS (body pipeline root)
//                       │     QuatYawFollow { threshold, rate, initial_yaw: π }
//                       │       T  ← model_root (y auto-calibrated from J_Bip_C_Head)
//                       │             └── GLTF { EM } → ... → J_Bip_C_Head
//                       │                                          └── CXR  ← re-parented here
//                       ├── splice_head → J_Bip_C_Head (AimConstraint, offset π)
//                       ├── CTLXR(Left,  Grip) ─→ TwoBoneIK on left arm chain
//                       └── CTLXR(Right, Grip) ─→ TwoBoneIK on right arm chain
//
// To run:
//   cargo run --release --example bisket-vr-demo

// --- Renderer settings ---
RendererSettings.msaa_off() {
    window_size(320, 240)
}

BGC.rgba(0.62, 0.80, 1.00, 1.0)
AL.rgb(0.18, 0.18, 0.22)

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

// --- Floor + back wall so the room has visual reference ---
ED {
    T.position(0.0, -0.78, -0.4).scale(12.0, 0.18, 9.5) {
        R.cube() { C.rgba(0.18, 0.18, 0.22, 1.0) }
    }
    T.position(0.0, 2.15, -7.2).scale(8.8, 3.6, 0.24) {
        R.cube() { C.rgba(0.11, 0.10, 0.14, 1.0) }
    }
}

// --- Background sun ---
BG {
    T.position(2.0, 1.5, -8.0).scale(3.5, 3.5, 3.5) {
        R.circle2d() {
            C.rgba(1.0, 0.85, 0.15, 1.0)
            EM.on()
        }
        T.position(-0.35, 0.35, -0.01).scale(0.45, 0.45, 0.45) {
            R.circle2d() {
                C.rgba(1.0, 1.0, 1.0, 1.0)
                EM.on()
            }
        }
    }
}

// --- bisket avatar — VR single-input topology ---
//
// initial_yaw(π): body starts facing -Z to match OpenXR HMD rest-forward.
// camera_bone == head_bone: head bone is the eye anchor (CXR re-parented here;
//   model_root.y auto-calibrated so head bone sits at HMD height).
ED {
    InputXR.on() {
        T {
            AVC {
                head_bone("J_Bip_C_Head")
                camera_bone("J_Bip_C_Head")
                left_hand_bone("J_Bip_L_Hand")
                right_hand_bone("J_Bip_R_Hand")
                initial_yaw(3.14159)
                hand_rotation_smoothing(220.0)
                head_ik_eye_height(0.04)

                T {
                    GLTF.new("assets/models/bisket.8.0.glb") { EM.on() }
                }

                // Camera wrapped in T(eye_offset). The T's translation is the
                // eye position relative to the head bone pivot (head-local
                // frame; +Y up, +Z forward). AVC discovers it during init and
                // reparents the T under J_Bip_C_Head. This position is used for
                // camera placement only; it does NOT affect how the spine bends.
                // The head IK targeting (spine bending) is controlled separately
                // via head_ik_eye_height(0.04) above, allowing the camera to be
                // positioned independently from the IK aim point.
                T.position(0.0, 0.2, 0.1) {
                    CXR { Pointer {} }
                }
                
                // Tracked Grip controllers — re-parented to lower-arm bones
                // by AVC, drive J_Bip_{L,R}_Hand via TwoBoneIK.
                CTLXR.new(true, Left,  Grip) { T { Pointer {} } }
                CTLXR.new(true, Right, Grip) { T { Pointer {} } }
            }
            
            // debug camera marker
            OV {
                T.scale(0.06, 0.06, 0.12) {
                    R.cube() { 
                        C.rgba(0.00, 1.0, 1.0, 0.5) 
                        EM.on()
                    }
                }
            }
        }
    }
}

// --- Controller debug cubes (Aim pose, rotation-smoothed) ---
//
// Sit alongside the avatar — useful to see raw controller tracking before
// any IK/splice transforms touch them.
InputXR.on() {
    T {
        T.position(0.0, 1.85, 0.6) {
            RendererStats {
                camera_target(Xr)
            }
        }

        CTLXR.new(true, Left, Aim) {
            T.scale(0.06, 0.06, 0.12) {
                TransformForkTRS {
                    TransformMapTranslation {}
                    TransformMapRotation {
                        QuatTemporalFilter.smoothing_factor(220.0)
                    }
                    TransformMapScale {}
                    T {
                        R.cube() { C.rgba(0.10, 0.90, 1.00, 1.0) }
                    }
                }
            }
        }

        CTLXR.new(true, Right, Aim) {
            T.scale(0.06, 0.06, 0.12) {
                TransformForkTRS {
                    TransformMapTranslation {}
                    TransformMapRotation {
                        QuatTemporalFilter.smoothing_factor(220.0)
                    }
                    TransformMapScale {}
                    T {
                        R.cube() { C.rgba(1.00, 0.35, 0.35, 1.0) }
                    }
                }
            }
        }
    }
}

// --- Desktop overview camera (Window target) ---
//
// Sits in front of the avatar (~3.5m away, eye height) looking at it, so the
// desktop window shows a 3rd-person view while the headset shows first-person.
// Bisket faces -Z at rest (initial_yaw = π flips the VRM +Z rest), so the
// camera sits at -Z and the .rotation(0, π, 0) turns its render direction
// toward +Z = the avatar.
//
// TODO: per-camera mesh culling. Right now this camera sees the head mesh,
// and so does the XR camera — the XR view shows the inside of the face when
// the user pitches because the head bone pivot is at skull-base height while
// the HMD pose is at eye height. Hide the head mesh from the XR camera once
// render-layer / visibility-mask support lands.
I.speed(1.0) {
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
        
    }
    T.position(0.0, 1.4, -3.5).rotation(0.0, 3.14159, 0.0) {
        C3D {}
        Pointer {}
    }

}

// --- OpenXR runtime ---
XR.on()
