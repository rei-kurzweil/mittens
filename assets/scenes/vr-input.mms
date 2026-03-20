// vr-input scene
// Corresponds to examples/vr-input.rs

// --- Renderer settings ---
RendererSettings.msaa_off() {
    with_window_size(320, 240)
}

// --- Sky color ---
BGC.rgba(0.62, 0.80, 1.00, 1.0)

// --- Ambient light ---
AL.rgb(0.18, 0.18, 0.22)

// --- Directional light (sun direction rig) ---
T.with_position(0.15, -0.45, 1.0) {
    DL {
        with_intensity(1.1)
        with_color(1.0, 0.98, 0.95)
    }
}

// --- Desktop camera rig ---
// InputComponent → InputTransformMode + TransformComponent (camera rig) → Camera3DComponent
I.with_speed(1.5) {
    ITM.forward_z() {
        with_fps_rotation()
        with_roll_axis_y()
    }
    T.with_position(0.0, 1.2, 3.5) {
        C3D {}
    }
}

// --- Desktop camera controls hint ---
// NOTE: In the Rust example this is placed in world space relative to the camera rig
// pose at spawn time (camera at 0.0, 1.2, 3.5 with identity rotation,
// local offset 0.65, 0.25, -1.7 → world pos ≈ 0.65, 1.45, 1.8).
// It is intentionally NOT parented under the camera rig.
T {
    with_position(0.65, 1.45, 1.8)
    with_scale(0.055, 0.055, 1.0)
    ED {
        TXT {
            "use wasd/rf/qe\nand right-mouse\nclick and drag\nto move/look"
            RCB.enabled()
            C.rgba(1.0, 1.0, 1.0, 1.0)
            TS {
                with_offset_xy([0.06, -0.06])
                with_z_offset(0.0025)
            }
            EM.on()
            TXTRF.nearest_magnification()
        }
    }
}



// Background (rendered without view translation — "skybox" layer)
// NOTE: Circle2D mesh needs a host-provided named constructor on Renderable.
BG {
    // Sun disk
    T {
        with_position(2.0, 1.5, -8.0)
        with_scale(3.5, 3.5, 3.5)
        R.circle2d() {
            C.rgba(1.0, 0.85, 0.15, 1.0)
            EM.on()
        }
        // Sun highlight
        T {
            with_position(-0.35, 0.35, -0.01)
            with_scale(0.45, 0.45, 0.45)
            R.circle2d() {
                C.rgba(1.0, 1.0, 1.0, 1.0)
                EM.on()
            }
        }
    }
}


// --- XR rig ---
T {
    // Renderer stats overlay (anchored near the XR camera origin)
    T.with_position(0.0, 1.85, 0.6) {
        RendererStats {
            with_camera_target(Xr)
        }
    }

    // XR camera
    CXR.on()

    // Left controller cube
    // NOTE: ControllerHand and ControllerPoseKind enum variants are bare identifiers.
    CTLXR.new(true, Left, Aim) {
        T.with_scale(0.06, 0.06, 0.12) {
            TransformPipeline {
                TransformForkTRS {
                    TransformMapTranslation {}
                    TransformMapRotation {
                        QuatTemporalFilter.with_smoothing_factor(220.0)
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

    // Right controller cube
    CTLXR.new(true, Right, Aim) {
        T.with_scale(0.06, 0.06, 0.12) {
            TransformPipeline {
                TransformForkTRS {
                    TransformMapTranslation {}
                    TransformMapRotation {
                        QuatTemporalFilter.with_smoothing_factor(220.0)
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

// --- VTuber model ---
T {
    GLTF.new("assets/models/pc-rei.hoodie.glb") {
        EM.on()
    }
}

// --- OpenXR runtime ---
XR.on()
