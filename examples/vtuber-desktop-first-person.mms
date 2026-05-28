// vtuber-desktop-first-person scene
//
// Demonstrates a 1st-person desktop controller where the camera is driven
// by the avatar's head, rather than the avatar being warped under a fixed
// world-space camera.

// --- Renderer settings ---
RendererSettings {
    window_size(1280, 960)
}

BGC.rgba(0.62, 0.80, 1.00, 1.0)
AL.rgb(0.18, 0.18, 0.22)

RenderGraph {
    EmissivePass {
        BlurPass { radius_ndc(0.06) half_res(true) }
    }
    Bloom { intensity(0.95) emissive_scale(1.2) }
}

T.position(0.15, -0.45, 1.0) {
    DL { intensity(1.1) color(1.0, 0.98, 0.95) }
}

// Floor
ED {
    T.position(0.0, -0.78, -0.4).scale(12.0, 0.18, 9.5) {
        R.cube() { C.rgba(0.18, 0.18, 0.22, 1.0) }
    }
}

// --- First-person desktop avatar ---
//
// In this setup:
// 1. The Avatar (model_root) stays at y=0 (grounded).
// 2. The Input component drives the head rotation.
// 3. The Camera (C3D) is re-parented to the head bone.
//
// TODO: When driven by Input (not InputXR), AVC should ideally avoid
// forcing the body to sit at driven_t.world.y + offset.
ED {
    I.speed(1.5) {
        InputTransformMode.forward_z() {
            fps_rotation()
            roll_axis_y()
        }
        T {
            AVC {
                head_bone("J_Bip_C_Head")
                camera_bone("J_Bip_C_Head") // Re-parent C3D here
                forward_plus_z()
                initial_yaw(0.0)

                T.position(0.0, -1.6, 0.0) {
                    GLTF.new("assets/models/pc-rei.hoodie.glb") { EM.on() }
                }

                // Camera wrapped in T(eye_offset).
                // In desktop mode, we want this T to move the camera relative
                // to the head bone, NOT move the head relative to the camera.
                T.position(0.0, 0.08, 0.12) {
                    C3D { Pointer {} }
                }
            }
        }
    }
}
