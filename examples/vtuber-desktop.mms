// vtuber-desktop scene
// Corresponds to examples/vtuber-desktop.rs
//
// Note: the bone-marker debug overlay (find_component loop) is not expressed here;
// it lives in the .rs loader if needed.
// RayCastComponent (camera raycaster) and PointerComponent are not registered in
// the MMS component registry, so those are also omitted.

// --- Renderer settings ---
RendererSettings.msaa_off() {
    window_size(1280, 720)
}

// --- Sky color and ambient light ---
BGC.rgba(0.62, 0.80, 1.00, 1.0)
AL.rgb(0.18, 0.18, 0.22)

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
