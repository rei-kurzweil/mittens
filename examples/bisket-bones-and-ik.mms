// bisket-bones-and-ik scene
//
// Loads assets/models/bisket.5.0.glb wrapped in I { AVC { ... } } so the body
// drives via WASD + right-mouse look (desktop input mode), the head splices via
// AimConstraint IK, and the arms get TwoBoneIK once controllers / hands resolve.
//
// Next step (separate iteration): wiggle the kemonomimi ears via
// DynamicBoneComponent / QuatSpring chain — the bone names for the ear strand
// need to come from the .glb skeleton.

// ── Renderer settings ─────────────────────────────────────────────────────────
RendererSettings.msaa_off() {
    window_size(1280, 720)
}

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

// ── Directional light ────────────────────────────────────────────────────────
T.position(0.15, -0.45, 1.0) {
    DL {
        intensity(1.1)
        color(1.0, 0.98, 0.95)
    }
}

// ── Desktop camera rig ───────────────────────────────────────────────────────
T.position(0.0, 1.2, 3.0).rotate(-0.15, 0.0, 0.0) {
    C3D {}
    Pointer {}
    T {
        position(0.65, 1.45, 1.8)
        scale(0.055, 0.055, 1.0)
        TXT {
            "wasd / rf / qe\nright-mouse drag\nto move + look"
            Raycastable.enabled()
            C.rgba(0.0, 0.0, 0.0, 1.0)
            EM.on()
            TextureFiltering.linear()
        }
    }
}

// ── Floor (so bisket isn't floating in the void) ─────────────────────────────
ED {
    T.position(0.0, -0.78, -0.4).scale(12.0, 0.18, 9.5) {
        R.cube() { C.rgba(0.18, 0.18, 0.22, 1.0) }
    }

    T.position(0.0, 2.15, -7.2).scale(8.8, 3.6, 0.24) {
        R.cube() { C.rgba(0.11, 0.10, 0.14, 1.0) }
    }
}

// ── bisket avatar — desktop single-input topology ────────────────────────────
//
// Topology after AvatarControlSystem init:
//   ED
//     └── I (body_input)
//           └── T (driven_t)
//                 └── AVC
//                       ├── TransformPipeline (body yaw pipeline)
//                       │     └── … → T (model_root, y = -1.6)
//                       │                └── GLTF { EM }
//                       └── [sys] splice_head  (TC injected above neck)
//                                   └── IKChain { AimConstraint }
//
// head_bone: VRM convention. Verify with the bisket skeleton — if the .glb
// uses different naming, swap to whatever sits one parent above the head mesh
// origin.
ED {
    I.speed(1.5) {
        InputTransformMode.forward_z() {
            fps_rotation()
            roll_axis_y()
        }
        T {
            AVC {
                head_bone("J_Bip_C_Neck")
                initial_yaw(0.0)
                body_yaw_threshold(3.14 / 7.5)

                T.position(0.0, -1.6, 0.0) {
                    GLTF.new("assets/models/bisket.8.0.glb") { EM.on() }
                }
            }
        }
    }
}
