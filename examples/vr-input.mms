// vr-input scene
// Corresponds to examples/vr-input.rs
//
// Demonstrates:
//   - free-standing component expressions (auto-emitted via EmitLiftTransform)
//   - let-bindings capturing ComponentObjects (created in engine, unattached)
//   - bare variable in statement position auto-emits (Option B runtime rule)

// --- Renderer settings ---
let renderer = RendererSettings.msaa_off() {
    with_window_size(320, 240)
}

// --- Sky color and ambient light ---
let sky     = BGC.rgba(0.62, 0.80, 1.00, 1.0)
let ambient = AL.rgb(0.18, 0.18, 0.22)

// Bare identifiers in statement position — each emits its ComponentObject.
renderer
sky
ambient

// --- Directional light (sun direction rig) ---
T.with_position(0.15, -0.45, 1.0) {
    DL {
        with_intensity(1.1)
        with_color(1.0, 0.98, 0.95)
    }
}

// --- Desktop camera rig ---
I.with_speed(1.5) {
    InputTransformMode.forward_z() {
        with_fps_rotation()
        with_roll_axis_y()
    }
    T.with_position(0.0, 1.2, 3.5) {
        C3D {}
    }
}

// --- Desktop camera controls hint ---
// NOTE: world-space position computed from camera rig spawn pose
// (camera at 0,1.2,3.5 + local offset 0.65,0.25,-1.7 → world ≈ 0.65,1.45,1.8).
// Intentionally not parented to the camera rig.
T {
    with_position(0.65, 1.45, 1.8)
    with_scale(0.055, 0.055, 1.0)
    ED {
        TXT {
            "use wasd/rf/qe\nand right-mouse\nclick and drag\nto move/look"
            Raycastable.enabled()
            C.rgba(1.0, 1.0, 1.0, 1.0)
            TextShadow {
                with_offset_xy([0.06, -0.06])
                with_z_offset(0.0025)
            }
            EM.on()
            TextureFiltering.nearest_magnification()
        }
    }
}

BG {
    T {
        with_position(2.0, 1.5, -8.0)
        with_scale(3.5, 3.5, 3.5)
        R.circle2d() {
            C.rgba(1.0, 0.85, 0.15, 1.0)
            EM.on()
        }
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


// --- VTuber model ---
// Outer InputXR drives avatar body translation to follow HMD.
// Head bone rotation splice (see vr-input.rs) wires a second InputXR
// into the neck with SampleAncestorTranslation to restore bone position.
InputXR {
    T {
        GLTF.new("assets/models/pc-rei.hoodie.glb") {
            EM.on()
        }
    }
}

// --- XR rig ---
InputXR {
    T {
        T.with_position(0.0, 1.85, 0.6) {
            RendererStats {
                with_camera_target(Xr)
            }
        }

        CXR {}


        // Controller cubes (ControllerHand and ControllerPoseKind variants as bare identifiers)
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
}

// --- OpenXR runtime ---
XR.on()
