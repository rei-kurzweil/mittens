// vtuber-desktop scene
// Corresponds to examples/vtuber-desktop.rs
//
// Note: the bone-marker debug overlay (find_component loop) is not expressed here;
// it lives in the .rs loader if needed.

// --- Renderer settings ---
RendererSettings.msaa_off() {
    window_size(1280, 720)
}

// --- Sky color and ambient light ---
BGC.rgba(0.62, 0.80, 1.00, 1.0)
AL.rgb(0.18, 0.18, 0.22)

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.06)
            half_res(true)
        }
    }

    Bloom {
        intensity(0.95)
        emissive_scale(1.2)
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
T.position(0.0, 1.2, 3.0) {
    C3D {}
    Pointer {}
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
}

// --- Set dressing for bloom + editing ---
ED {
    T.position(0.0, -0.78, -0.4).scale(12.0, 0.18, 9.5) {
        R.cube() { C.rgba(0.18, 0.18, 0.22, 1.0) }
    }

    T.position(0.0, 2.15, -7.2).scale(8.8, 3.6, 0.24) {
        R.cube() { C.rgba(0.11, 0.10, 0.14, 1.0) }
    }

    T.position(-3.0, 0.35, -1.2).scale(0.55, 0.55, 0.55) {
        R.cube() { C.rgba(0.70, 0.66, 0.60, 1.0) }
    }
    T.position(-2.35, 0.35, -1.2).scale(0.55, 0.55, 0.55) {
        R.cube() { C.rgba(0.62, 0.68, 0.72, 1.0) }
    }
    T.position(-1.70, 0.35, -1.2).scale(0.55, 0.55, 0.55) {
        R.cube() { C.rgba(0.74, 0.70, 0.64, 1.0) }
    }
    T.position(-2.35, 0.95, -1.2).scale(0.48, 0.48, 0.48) {
        R.cube() { C.rgba(0.80, 0.76, 0.70, 1.0) }
    }

    T.position(2.10, 0.35, -0.85).scale(0.52, 0.52, 0.52) {
        R.cube() { C.rgba(0.64, 0.69, 0.78, 1.0) }
    }
    T.position(2.75, 0.35, -0.85).scale(0.52, 0.52, 0.52) {
        R.cube() { C.rgba(0.76, 0.72, 0.68, 1.0) }
    }
    T.position(3.40, 0.35, -0.85).scale(0.52, 0.52, 0.52) {
        R.cube() { C.rgba(0.66, 0.74, 0.70, 1.0) }
    }
    T.position(2.75, 0.92, -0.85).scale(0.44, 0.44, 0.44) {
        R.cube() { C.rgba(0.84, 0.80, 0.74, 1.0) }
    }
    T.position(3.40, 0.92, -0.85).scale(0.44, 0.44, 0.44) {
        R.cube() { C.rgba(0.74, 0.80, 0.76, 1.0) }
    }

    T.position(-0.95, 0.35, 1.15).scale(0.50, 0.50, 0.50) {
        R.cube() { C.rgba(0.72, 0.68, 0.62, 1.0) }
    }
    T.position(-0.30, 0.35, 1.15).scale(0.50, 0.50, 0.50) {
        R.cube() { C.rgba(0.62, 0.69, 0.75, 1.0) }
    }
    T.position(0.35, 0.35, 1.15).scale(0.50, 0.50, 0.50) {
        R.cube() { C.rgba(0.76, 0.73, 0.68, 1.0) }
    }
    T.position(-0.30, 0.90, 1.15).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.82, 0.79, 0.74, 1.0) }
    }

    T.position(-3.2, 0.55, -6.85).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(1.00, 0.22, 0.28, 1.0) EM.on() }
    }
    T.position(-1.9, 0.95, -6.85).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(1.00, 0.58, 0.12, 1.0) EM.on() }
    }
    T.position(-0.6, 1.35, -6.85).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(1.00, 0.88, 0.20, 1.0) EM.on() }
    }
    T.position(0.7, 1.75, -6.85).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.26, 1.00, 0.40, 1.0) EM.on() }
    }
    T.position(2.0, 2.15, -6.85).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.18, 0.68, 1.00, 1.0) EM.on() }
    }
    T.position(3.3, 2.55, -6.85).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.90, 0.26, 1.00, 1.0) EM.on() }
    }

    T.position(-3.2, 2.60, -6.55).scale(0.32, 0.32, 0.32) {
        R.cube() { C.rgba(1.00, 0.28, 0.36, 1.0) EM.on() }
    }
    T.position(-1.9, 2.20, -6.55).scale(0.32, 0.32, 0.32) {
        R.cube() { C.rgba(1.00, 0.66, 0.18, 1.0) EM.on() }
    }
    T.position(-0.6, 1.80, -6.55).scale(0.32, 0.32, 0.32) {
        R.cube() { C.rgba(1.00, 0.92, 0.28, 1.0) EM.on() }
    }
    T.position(0.7, 1.40, -6.55).scale(0.32, 0.32, 0.32) {
        R.cube() { C.rgba(0.34, 1.00, 0.46, 1.0) EM.on() }
    }
    T.position(2.0, 1.00, -6.55).scale(0.32, 0.32, 0.32) {
        R.cube() { C.rgba(0.28, 0.76, 1.00, 1.0) EM.on() }
    }
    T.position(3.3, 0.60, -6.55).scale(0.32, 0.32, 0.32) {
        R.cube() { C.rgba(0.96, 0.34, 1.00, 1.0) EM.on() }
    }
}

// --- VTuber avatar — desktop single-input topology ---
//
// InputComponent (fps_rotation, forward_z) drives body translation and head rotation
// via AvatarControlSystem. forward_plus_z() + initial_yaw(0.0) = desktop orientation.
//
// Topology (after AvatarControlSystem init):
//   ED
//     └── I (body_input)
//           └── T (driven_t)
//                 └── AVC
//                       ├── TransformPipeline (body yaw pipeline)
//                       │     └── … → T (model_root, y = -avatar_height)
//                       │                └── GLTF { EM }
//                       └── [sys] splice_head  (TC injected above J_Bip_C_Neck)
//                                   └── IKChain { AimConstraint }
ED {
    I.speed(1.5) {
        InputTransformMode.forward_z() {
            fps_rotation()
            roll_axis_y()
        }
        T {
            AVC {
                head_bone("J_Bip_C_Neck")
                forward_plus_z()
                initial_yaw(0.0)

                T.position(0.0, -1.6, 0.0) {
                    GLTF.new("assets/models/pc-rei.hoodie.glb") { EM.on() }
                }
            }
        }
    }
}
