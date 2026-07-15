import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"
import { voxel_terrain } from "../assets/components/floors/voxel_terrain.mms"

// bisket-vr-demo scene
//
// Shared VR room-scale demo using the bisket avatar (bisket.8.0.glb).
// Mirrors examples/vr-input.{rs,mms} topology but with the head-driven
// AVC convention (head_bone="J_Bip_C_Head") and the bisket model.
//
// Drivers:
//   - InputXR.on(): HMD pose → driven_t (translation + rotation)
//   - XRHand(Left/Right, Grip): hand pose → splices into J_Bip_{L,R}_Hand
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
//                       ├── XRHand(Left,  Grip) ─→ TwoBoneIK on left arm chain
//                       └── XRHand(Right, Grip) ─→ TwoBoneIK on right arm chain
//
// To run:
//   cargo run --release --example bisket-vr-demo

// --- Renderer settings ---
RendererSettings {
    window_size(640, 480)
}

BGC {
    C.rgba(0.9, 0.5, 0.3, 1.0)
}

AL.rgb(0.10, 0.11, 0.16)

Clock.bpm(60) {}

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.06)
            half_res(true)
        }
    }
    Bloom {
        intensity(0.90)
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

// --- Terrain + back wall so the room has visual reference ---
// ED {
    T.position(0.0, -6.0, 0.0) {
        voxel_terrain()
    }

    // back wall
    T.position(0.0, 2.15, -7.2).scale(8.8, 3.6, 0.24) {
        R.cube() { C.rgba(0.95, 0.55, 0.22, 1.0) }
    }

    let repro_cube_a_transform = T.position(-0.9, -0.44, -1.0) {
        name = "repro_cube_a_transform"
        Transition {
            duration_beats(1.0)
            ease_in_out_sine()
            replace_same_target()
        }
        T.scale(0.50, 0.50, 0.50) {
            name = "repro_cube_a"
            R.cube() {
                C.rgba(1.0, 0.88, 0.15, 1.0)
                EM.on()
                Raycastable.enabled()
            }
        }
    }

    let repro_cube_b_transform = T.position(0.0, -0.44, -0.7) {
        name = "repro_cube_b_transform"
        Transition {
            duration_beats(1.0)
            ease_in_out_sine()
            replace_same_target()
        }
        T.scale(0.50, 0.50, 0.50) {
            name = "repro_cube_b"
            R.cube() {
                C.rgba(1.0, 0.35, 0.78, 1.0)
                EM.on()
                Raycastable.enabled()
            }
        }
    }

    let repro_cube_c_transform = T.position(0.9, -0.44, -1.1) {
        name = "repro_cube_c_transform"
        Transition {
            duration_beats(1.0)
            ease_in_out_sine()
            replace_same_target()
        }
        T.scale(0.50, 0.50, 0.50) {
            name = "repro_cube_c"
            R.cube() {
                C.rgba(0.10, 0.95, 1.0, 1.0)
                EM.on()
                Raycastable.enabled()
            }
        }
    }

    let repro_rotating_parent = T.position(2.0, 0.15, 1.0) {
        name = "repro_rotating_parent"
        Transition {
            duration_beats(1.0)
            ease_in_out_sine()
            replace_same_target()
        }
        repro_cube_a_transform
        repro_cube_b_transform
        repro_cube_c_transform
    }
    repro_rotating_parent
    
    Animation.looping().length(4.0) {
        Keyframe.at(0.0) {
            repro_rotating_parent.update_transform([1.0, 0.15, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_a_transform.update_transform([-0.9, -0.44, -1.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_b_transform.update_transform([0.0, -0.44, -0.7], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_c_transform.update_transform([0.9, -0.44, -1.1], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        }
        Keyframe.at(1.0) {
            repro_rotating_parent.update_transform([1.0, 0.15, 0.0], [0.0, 0.65, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_a_transform.update_transform([-1.25, -0.10, -0.55], [0.0, 1.570795, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_b_transform.update_transform([0.0, 0.45, -1.35], [1.570795, 0.0, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_c_transform.update_transform([1.25, -0.20, -0.55], [0.0, 0.0, 1.570795], [1.0, 1.0, 1.0])
        }
        Keyframe.at(2.0) {
            repro_rotating_parent.update_transform([1.0, 0.15, 0.0], [0.0, 1.35, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_a_transform.update_transform([-0.35, 0.55, -1.45], [0.0, 3.14159, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_b_transform.update_transform([0.0, -0.55, -0.25], [3.14159, 0.0, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_c_transform.update_transform([0.35, 0.55, -1.45], [0.0, 0.0, 3.14159], [1.0, 1.0, 1.0])
        }
        Keyframe.at(3.0) {
            // repro_rotating_parent.update_transform([1.0, 0.15, 0.0], [0.0, 2.10, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_a_transform.update_transform([-1.35, -0.25, -1.35], [0.0, 4.712385, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_b_transform.update_transform([0.0, 0.30, 0.10], [4.712385, 0.0, 0.0], [1.0, 1.0, 1.0])
            // repro_cube_c_transform.update_transform([1.35, -0.25, -1.35], [0.0, 0.0, 4.712385], [1.0, 1.0, 1.0])
        }
    }
// }

// --- Background sun + stars ---
BG.occlusion_and_lighting() {
    star_kawaii_background([1.0, 0.88, 0.42, 1.0])

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
    T {
      InputXR.on() {
        InputXRGamepad {
            locomotion()
            speed(1.5)
        }
        T {
            AVC {
                head_bone("J_Bip_C_Head")
                camera_bone("J_Bip_C_Head")
                left_hand_bone("J_Bip_L_Hand")
                right_hand_bone("J_Bip_R_Hand")

                initial_yaw(3.14159)

                // Body-local pole hints. The solver rotates them by the
                // current model_root world rotation each tick, so author them
                // as anatomical body-space directions. Bias slightly downward
                // as well as outward so elbows drop when the hands pull back
                // toward the chest instead of lifting upward.
                left_arm_pole_direction([  1, -0.35, 1])
                right_arm_pole_direction([-1, -0.35, 1])

                hand_rotation_smoothing(220.0)
                // Trial: yaw inward 90 degrees, then apply the opposite
                // mirrored pitch branch in the post-yaw local frame.
                // Keep the near-correct left/right branches, then add a shared
                // post-correction twist back toward the thumb by ~40 degrees.
                hand_grip_rotation_left([-0.6408564, 0.29883623, 0.29883623, 0.6408564])
                hand_grip_rotation_right([-0.6408564, -0.29883623, -0.29883623, 0.6408564])

                T {
                    GLTF.new("assets/models/bisket.11.0.glb") { 
                        EM.on() 
                        PoseCapture { label("Bisket") }
                    }
                }

                // Camera wrapped in T(eye_offset). The T's translation is the
                // eye position relative to the head bone pivot (head-local
                // frame; +Y up, +Z forward). AVC discovers it during init and
                // reparents the T under J_Bip_C_Head.
            // This authored offset is used to place the head pivot relative
                // to the fixed HMD pose AND to offset the whole avatar baseline,
                // so changing it moves body/neck together instead of crushing the
                // upper torso with a head-only correction.
                T.position(0.0, 0.08, 0.12) {
                    name = "xr_camera_wrapper"
                    // Collision.kinematic() {
                    //     CollisionShape.sphere(0.18)
                    //     KineticResponse.slide() {}
                    // }
                    CXR { Pointer {} }
                }
                
                // Tracked Grip controllers — re-parented to lower-arm bones
                // by AVC, drive J_Bip_{L,R}_Hand via TwoBoneIK.
                XRHand.new(true, Left,  Grip) { T { Pointer {} } }
                XRHand.new(true, Right, Grip) { T { Pointer {} } }
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
} 

// // --- Controller debug cubes (Aim pose, rotation-smoothed) ---
    // //
    // // Sit alongside the avatar — useful to see raw controller tracking before
    // // any IK/splice transforms touch them.
    // InputXR.on() {
    //     T {
    //         // T.position(0.0, 1.85, 0.6) {
    //         //     RendererStats {
    //         //         camera_target(Xr)
    //         //     }
    //         // }

    //         XRHand.new(true, Left, Aim) {
    //             T.scale(0.06, 0.06, 0.12) {
    //                 TransformForkTRS {
    //                     TransformMapTranslation {}
    //                     TransformMapRotation {
    //                         QuatTemporalFilter.smoothing_factor(220.0)
    //                     }
    //                     TransformMapScale {}
    //                     T {
    //                         R.cube() { C.rgba(0.10, 0.90, 1.00, 1.0) }
    //                     }
    //                 }
    //             }
    //         }

    //         XRHand.new(true, Right, Aim) {
    //             T.scale(0.06, 0.06, 0.12) {
    //                 TransformForkTRS {
    //                     TransformMapTranslation {}
    //                     TransformMapRotation {
    //                         QuatTemporalFilter.smoothing_factor(220.0)
    //                     }
    //                     TransformMapScale {}
    //                     T {
    //                         R.cube() { C.rgba(1.00, 0.35, 0.35, 1.0) }
    //                     }
    //                 }
    //             }
    //         }

    //         // Grip pose markers (yellow = left, green = right) — compare with Aim above.
    //         XRHand.new(true, Left, Grip) {
    //             T.scale(0.05, 0.05, 0.10) {
    //                 T {
    //                     R.cube() { C.rgba(1.0, 1.0, 0.0, 1.0) EM.on() }
    //                 }
    //             }
    //         }
    //         XRHand.new(true, Right, Grip) {
    //             T.scale(0.05, 0.05, 0.10) {
    //                 T {
    //                     R.cube() { C.rgba(0.2, 1.0, 0.2, 1.0) EM.on() }
    //                 }
    //             }
    //         }
    //     }
    // }

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
I.speed(2.0) {
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
        
    }
    T.position(3.0, 1.2, 3.5) {
        name = "desktop_camera_rig"
        Collision.kinematic() {
            CollisionShape.sphere(0.22)
            KineticResponse.slide() {}
        }
        C3D {
            Pointer {}
        }
    }

}

// --- OpenXR runtime ---
XR.on()


// pink yellow and orange lighting
T.position(0, 2, 0) {
    DL {
        intensity(0.8)
        color(1.0, 0.9, 0.9)
    }
}


T.position(-0.5, 0.5, 0) {
    DL {
        intensity(0.6)
        color(1.0, 0.8, 0.9)
    }
}

T.position(0.5, -0.5, 0) {
    DL {
        intensity(0.6)
        color(1.0, 0.8, 0.9)
    }
}
