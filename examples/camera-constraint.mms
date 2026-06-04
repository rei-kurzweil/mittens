// camera-constraint scene
// Demonstrates constraining a camera by placing a TransformForkTRS with
// TransformDrop (rotation) between the Input and Camera.
// The camera follows the input's position but always faces forward
// (no mouse-look rotation).

RendererSettings { window_size(1280, 960) }

BGC.rgba(0.62, 0.80, 1.00, 1.0)
AL.rgb(0.18, 0.18, 0.22)

T.position(0.15, -0.45, 1.0) {
    DL { intensity(1.1) color(1.0, 0.98, 0.95) }
}

// Floor + visual reference objects
ED {
    T.position(0.0, -0.78, -0.4).scale(12.0, 0.18, 9.5) {
        R.cube() { C.rgba(0.18, 0.18, 0.22, 1.0) }
    }

    // Colored cubes for spatial reference
    T.position(-3.0, 0.0, -4.0).scale(0.7, 0.7, 0.7) { R.cube() { C.rgba(0.85, 0.22, 0.22, 1.0) } }
    T.position( 0.0, 0.0, -4.0).scale(0.7, 0.7, 0.7) { R.cube() { C.rgba(0.26, 0.78, 0.32, 1.0) } }
    T.position( 3.0, 0.0, -4.0).scale(0.7, 0.7, 0.7) { R.cube() { C.rgba(0.22, 0.62, 0.92, 1.0) } }

    T.position(-3.0, 1.5, -4.0).scale(0.7, 0.7, 0.7) { R.cube() { C.rgba(0.72, 0.22, 0.82, 1.0) } }
    T.position( 3.0, 1.5, -4.0).scale(0.7, 0.7, 0.7) { R.cube() { C.rgba(0.92, 0.48, 0.28, 1.0) } }
}

// Camera with constrained rotation
//
// Input drives position+rotation on the outer T.
// TransformForkTRS drops rotation while passing translation and scale through.
// The inner T captures the processed transform, and C3D reads from it.
//
// Result: WASD moves the camera, but mouse-look rotation is stripped — the
// camera always faces forward (down -Z).
I.speed(2.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 1.8, 5.0) {
        TransformForkTRS {
            TransformMapTranslation {}
            TransformMapRotation {
                TransformDrop {}
            }
            TransformMapScale {}
            T {
                C3D { Pointer {} }
            }
        }
    }
}

// Info overlay
T.position(-4.5, 2.5, 3.0).scale(0.055, 0.055, 1.0) {
    TXT {
        "Camera Constraint Demo\nMovement: WASD/RF/QE\nRotation:  DROPPED\n\nThe camera moves but\nalways faces forward\n(no mouse-look).\nThe rotation channel\nis stripped by the\npipeline."
        C.rgba(0.0, 0.0, 0.0, 1.0)
        TextureFiltering.linear()
    }
}
