// bisket-bones-and-ik scene
//
// Loads assets/models/bisket.5.0.glb wrapped in I { AVC { ... } } so the body
// drives via WASD + right-mouse look (desktop input mode), the head splices via
// AimConstraint IK, and the arms get TwoBoneIK once controllers / hands resolve.
//
// Next step (separate iteration): wiggle the kemonomimi ears via
// DynamicBoneComponent / QuatSpring chain — the bone names for the ear strand
// need to come from the .glb skeleton.
//
// World panel: a `world_panel` is mounted to the right of bisket. Its "Save"
// button enables layout box-model viz on the panel's LayoutRoot
// (`InspectLayout` semantics); its "Load" button disables it. Per-tree toggle
// so the rest of the scene stays clean.

import { world_panel } from "../assets/components/world_panel.mms"

// ── Renderer settings ─────────────────────────────────────────────────────────
RendererSettings {
    window_size(1280, 960)
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
//                       ├── TransformForkTRS (body yaw pipeline root)
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
                head_bone("J_Bip_C_Head")
                camera_bone("J_Bip_C_Head")
                initial_yaw(0.0)
                body_yaw_threshold(3.14 / 7.5)

                T.position(0.0, -1.6, 0.0) {
                    GLTF.new("assets/models/bisket.8.0.glb") { EM.on() }
                }

                // Camera reparented under J_Bip_C_Head by AVC.
                //   - position: eye offset relative to head bone pivot (forward +Z, up +Y)
                //   - rotation(0, π, 0): cameras render down -Z but the avatar's anatomical
                //     forward is +Z (VRM); flip the camera 180° so its view direction matches
                //     the avatar's forward. (CameraXR doesn't need this — OpenXR overrides pose.)
                T.position(0.0, 0.08, 0.07).rotation(0.0, 3.14159, 0.0) {
                    C3D {}
                    Pointer {}
                }
            }
        }
    }
}

// ── World panel (right of bisket) with viz toggle buttons ───────────────────
let panel_items = [
    "Save → enable_inspect on panel_layout_root",
    "Load → disable_inspect",
    "viz quads are local to this LayoutRoot"
]
let panel = world_panel("Inspect", panel_items)

T.position(2.2, 1.4, -0.6) {
    Selectable.off() {
        panel
    }
}

let panel_layout = panel.query("#panel_layout_root")
let inspect_on_btn = panel.query("#save_button")
let inspect_off_btn = panel.query("#load_button")
let panel_status = panel.query("#panel_status_value")

on(inspect_on_btn, "Click", fn(e) {
    panel_layout.enable_inspect()
    panel_status.set_text("inspect: on")
})

on(inspect_off_btn, "Click", fn(e) {
    panel_layout.disable_inspect()
    panel_status.set_text("inspect: off")
})
