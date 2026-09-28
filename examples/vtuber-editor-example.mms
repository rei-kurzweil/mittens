// vtuber-mirror-example scene
//
// XR-first temple scene for mirror/render-view validation.
// Derived from bisket-vr-demo but without the Rust-side bone debug harness.
//
// To run:
//   cargo run --release --example vtuber-mirror-example

import { bisket_shirt_physics } from "../assets/components/secondary_motion/bisket-shirt-physics.mms"
import { bisket_colliders } from "../assets/components/colliders/bisket.mms"
import { bisket_anime_shading } from "../assets/components/materials/bisket_anime_shading.mms"

// Default capture is optional: unavailable input leaves AVC's mouth driver neutral.
let microphone = AudioInput {}
let voice_level = Amplitude.rolling_window(0.080).from(microphone) {}

RendererSettings {
    window_size(640, 480)
}

BGC {
    C.rgba(0.90, 0.93, 0.98, 1.0)
}

AL.rgb(0.20, 0.20, 0.24)

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

T.position(0.15, -0.45, 1.0) {
    DL {
        intensity(1.0)
        color(1.0, 0.98, 0.95)
    }
}

T.position(-0.85, 0.55, 0.35) {
    DL {
        intensity(0.55)
        color(0.90, 0.94, 1.0)
    }
}

T.position(0.65, 0.35, -0.75) {
    DL {
        intensity(0.45)
        color(1.0, 0.92, 0.88)
    }
}

// --- Temple shell and floor collision ---
ED {
    T.position(0.0, -1.75, 0.0).scale(120.0, 0.12, 120.0) {
        Collision.static() {
            CollisionShape.cube([60.0, 0.06, 60.0])
        }
        R.cube() { C.rgba(0.73, 0.71, 0.70, 1.0) }
    }

    T.position(0.0, -1.55, -0.2).scale(8.0, 0.18, 10.5) {
        R.cube() { C.rgba(0.86, 0.84, 0.82, 1.0) }
    }

    T.position(0.0, -1.28, 1.55).scale(4.4, 0.14, 1.3) {
        R.cube() { C.rgba(0.90, 0.88, 0.86, 1.0) }
    }

    T.position(0.0, 1.8, -4.7).scale(7.4, 3.2, 0.26) {
        R.cube() { C.rgba(0.94, 0.93, 0.92, 1.0) }
    }

    T.position(0.0, 3.35, -1.9).scale(8.2, 0.34, 5.8) {
        R.cube() { C.rgba(0.90, 0.89, 0.87, 1.0) }
    }

    // rear columns
    T.position(-2.7, 0.50, -3.2) {
        T.scale(0.42, 1.95, 0.42) {
            R.cube() { C.rgba(0.98, 0.98, 0.97, 1.0) }
        }
        T.position(0.0, 2.15, 0.0).scale(0.52, 0.12, 0.52) {
            R.annulus_2d() { C.rgba(0.93, 0.92, 0.90, 1.0) }
        }
        T.position(0.0, -2.15, 0.0).scale(0.56, 0.14, 0.56) {
            R.annulus_2d() { C.rgba(0.90, 0.89, 0.87, 1.0) }
        }
    }
    T.position(2.7, 0.50, -3.2) {
        T.scale(0.42, 1.95, 0.42) {
            R.cube() { C.rgba(0.98, 0.98, 0.97, 1.0) }
        }
        T.position(0.0, 2.15, 0.0).scale(0.52, 0.12, 0.52) {
            R.annulus_2d() { C.rgba(0.93, 0.92, 0.90, 1.0) }
        }
        T.position(0.0, -2.15, 0.0).scale(0.56, 0.14, 0.56) {
            R.annulus_2d() { C.rgba(0.90, 0.89, 0.87, 1.0) }
        }
    }

    // front columns
    T.position(-2.7, 0.50, 0.55) {
        T.scale(0.42, 1.95, 0.42) {
            R.cube() { C.rgba(0.98, 0.98, 0.97, 1.0) }
        }
        T.position(0.0, 2.15, 0.0).scale(0.52, 0.12, 0.52) {
            R.annulus_2d() { C.rgba(0.93, 0.92, 0.90, 1.0) }
        }
        T.position(0.0, -2.15, 0.0).scale(0.56, 0.14, 0.56) {
            R.annulus_2d() { C.rgba(0.90, 0.89, 0.87, 1.0) }
        }
    }
    T.position(2.7, 0.50, 0.55) {
        T.scale(0.42, 1.95, 0.42) {
            R.cube() { C.rgba(0.98, 0.98, 0.97, 1.0) }
        }
        T.position(0.0, 2.15, 0.0).scale(0.52, 0.12, 0.52) {
            R.annulus_2d() { C.rgba(0.93, 0.92, 0.90, 1.0) }
        }
        T.position(0.0, -2.15, 0.0).scale(0.56, 0.14, 0.56) {
            R.annulus_2d() { C.rgba(0.90, 0.89, 0.87, 1.0) }
        }
    }

    T.position(0.0, 2.45, 0.55).scale(6.35, 0.24, 0.62) {
        R.cube() { C.rgba(0.92, 0.91, 0.89, 1.0) }
    }
    T.position(0.0, 0.55, -3.95).scale(2.2, 2.5, 0.08) {
        R.cube() {
            C.rgba(0.82, 0.88, 0.94, 1.0)
            Mirror.quality(1024) {}
        }
    }

    T.position(0.0, 0.55, -4.08).scale(2.45, 2.75, 0.10) {
        R.cube() { C.rgba(0.66, 0.56, 0.34, 1.0) }
    }

    T.position(-1.1, -0.95, -1.7).scale(0.45, 0.9, 0.45) {
        R.cube() { C.rgba(0.75, 0.28, 0.26, 1.0) EM.on() Raycastable.enabled() }
    }
    T.position(0.0, -0.75, -1.25).scale(0.40, 1.3, 0.40) {
        R.cube() { C.rgba(0.15, 0.70, 0.98, 1.0) EM.on() Raycastable.enabled() }
    }
    T.position(1.1, -0.95, -1.7).scale(0.45, 0.9, 0.45) {
        R.cube() { C.rgba(1.0, 0.84, 0.18, 1.0) EM.on() Raycastable.enabled() }
    }
}

// --- bisket avatar — preserve the useful XR topology from bisket-vr-demo ---
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

                mouth_open_from_amplitude(voice_level)
                mouth_open_rms_floor(0.005)
                mouth_open_rms_ceiling(0.09)
                mouth_open_smoothing(16.0)
                voice_level

                initial_yaw(3.14159)
                ik_debug()

                // Match bisket-vr-demo: body-local elbow hints that bias the
                // bend downward and slightly outward from the torso.
                left_arm_pole_direction([  1, -0.35, 1])
                right_arm_pole_direction([-1, -0.35, 1])

                hand_rotation_smoothing(220.0)
                T {
                    GLTF.new("assets/models/bisket.glb") {
                        bisket_anime_shading()
                        MorphTargetMap.new()
                            .slot("left_eye_blink", "Fcl_EYE_Close_L")
                            .slot("right_eye_blink", "Fcl_EYE_Close_R")
                            .slot("viseme_aa", "Fcl_MTH_A")
                        EM.on()
                        PoseCapture { label("Bisket") asset_name("bisket") }
                        bisket_colliders()
                        bisket_shirt_physics(false)
                    }
                }

                T.position(0.0, 0.08, 0.12) {
                    name = "xr_camera_wrapper"
                    CXR { Pointer {} }
                }
                XREyeTracking.on()

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

// Aim/controller reference cubes kept so mirror hand motion is easy to judge.
InputXR.on() {
    T {
        T.position(0.0, 1.95, 0.8) {
            RendererStats {
                camera_target("Xr")
            }
        }

        // XRHand.new(true, "Left", "Aim") {
        //     T.scale(0.06, 0.06, 0.12) {
        //         TransformForkTRS {
        //             TransformMapTranslation {}
        //             TransformMapRotation {
        //                 QuatTemporalFilter.smoothing_factor(220.0)
        //             }
        //             TransformMapScale {}
        //             T {
        //                 R.cube() { C.rgba(0.10, 0.90, 1.00, 1.0) }
        //             }
        //         }
        //     }
        // }

        // XRHand.new(true, "Right", "Aim") {
        //     T.scale(0.06, 0.06, 0.12) {
        //         TransformForkTRS {
        //             TransformMapTranslation {}
        //             TransformMapRotation {
        //                 QuatTemporalFilter.smoothing_factor(220.0)
        //             }
        //             TransformMapScale {}
        //             T {
        //                 R.cube() { C.rgba(1.00, 0.35, 0.35, 1.0) }
        //             }
        //         }
        //     }
        // }

        XRHand.new(true, "Left", "Grip") {
            T.scale(0.05, 0.05, 0.10) {
                T {
                    R.cube() { C.rgba(1.0, 1.0, 0.0, 1.0) EM.on() }
                }
            }
        }
        XRHand.new(true, "Right", "Grip") {
            T.scale(0.05, 0.05, 0.10) {
                T {
                    R.cube() { C.rgba(0.2, 1.0, 0.2, 1.0) EM.on() }
                }
            }
        }
    }
}

// --- Desktop overview camera (Window target) ---
//
// Companion camera for desktop validation when XR is also active.
// This keeps the example usable without a headset and avoids the white-screen
// case where only the clear color is visible in the window.
I.speed(1.0) {
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(3.1, 1.45, 3.9).rotation(0.0, 0.58, 0.0) {
        name = "desktop_camera_rig"
        C3D {
            Pointer {}
        }
    }
}

XR.on()
