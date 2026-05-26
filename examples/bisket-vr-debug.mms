// bisket-vr-debug scene
//
// Headless verification scene for the head-driven AVC redesign.
//
// Two bisket avatars share the world:
//   1. REFERENCE — no AVC, no driver. Bones sit at GLTF rest pose. The
//      "what would natural FK do" baseline.
//   2. AVC-DRIVEN — full AVC with spine FABRIK wired. Its driven_t is a
//      plain TransformComponent that the harness (bisket-vr-debug.rs)
//      mutates each pose to simulate HMD movement, with NO OpenXR or
//      InputXR in the loop.
//
// The harness:
//   - Spawns the scene, ticks GLTF to instantiate armatures.
//   - Scripts driven_t through a fixed sequence of poses.
//   - Per pose, samples world Y/Z of hips → spine → chest → upper_chest →
//     neck → head bones on both avatars, and prints a diff table.
//
// No camera renders are required, but a C3D is kept so AVC's camera_bone
// reparent logic doesn't emit warnings.
//
// To run:
//   cargo run --release --example bisket-vr-debug

RendererSettings.msaa_off() {
    window_size(320, 240)
}

BGC.rgba(0.40, 0.42, 0.50, 1.0)
AL.rgb(0.25, 0.25, 0.30)

ED {
    // --- REFERENCE avatar (left, x=-1.2) ---
    // Plain GLTF subtree.  No AVC, no input.  Bones stay at FK rest pose
    // for the lifetime of the harness, so they form the comparison baseline.
    T.position(-1.2, 0.0, 0.0) {
        T {
            GLTF.new("assets/models/bisket.8.0.glb")
        }
    }

    // --- AVC-DRIVEN avatar (right, x=+1.2) ---
    // The OUTER T at +1.2 is just the world-space placement.  The INNER T
    // (initially at y=1.55) is `driven_t` — AVC reads it via parent_of(avc).
    // The harness emits UpdateTransform on this T each pose to simulate the
    // HMD moving, without needing OpenXR.
    T.position(1.2, 0.0, 0.0) {
        T.position(0.0, 1.55, 0.0) {
            AVC {
                head_bone("J_Bip_C_Head")
                camera_bone("J_Bip_C_Head")
                hips_bone("J_Bip_C_Hips")
                initial_yaw(3.14159)

                T {
                    GLTF.new("assets/models/bisket.8.0.glb")
                }

                // Eye-offset T-wrapper, mirroring bisket-vr-demo.  The
                // C3D won't actually render anything we care about here
                // (no window loop), but its presence is what feeds the
                // head_target_offset into AVC.
                T.position(0.0, 0.08, 0.04) {
                    C3D {}
                }
            }
        }
    }
}
