// Focused repro for docs/bugs/vtuber-desktop-scrolling-interference.md
//
// Setup: one scrolling panel sitting in front of a wall of raycastable cubes.
// Expectation: clicking and dragging within the yellow scroll viewport scrolls
// the panel content. Actual: drags are "stolen" by the cubes behind the panel,
// scroll fails to engage.
//
// Strip everything not relevant — no avatar, no bloom, no GLTF, just:
//   - desktop camera + pointer
//   - one scroll viewport (layout-mock style, owned __bg drag surface)
//   - a grid of raycastable cubes directly behind the panel

BGC.rgba(0.08, 0.10, 0.14, 1.0)
AL.rgb(0.30, 0.30, 0.34)

// FPS camera rig: wasd/rf/qe + right-mouse drag to look (matches scrolling.mms).
I {
    speed(1.5)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(0.0, 1.1, 3.8) {
        C3D {}
        Pointer {}
        T.position(0.0, 2.05, -1.2).scale(0.06, 0.06, 1.0) {
            TXT {
                "wasd/rf/qe + right-mouse to look\nclick+drag yellow area"
                Raycastable.enabled()
                C.rgba(0.95, 0.95, 1.0, 1.0)
                EM.on()
            }
        }
    }
}

// --- Wall of raycastable cubes directly behind where the panel sits ---
ED {
    T.position(-1.8, 1.6, -0.6).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.85, 0.40, 0.45, 1.0) }
    }
    T.position(-0.6, 1.6, -0.6).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.90, 0.65, 0.30, 1.0) }
    }
    T.position(0.6, 1.6, -0.6).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.55, 0.85, 0.45, 1.0) }
    }
    T.position(1.8, 1.6, -0.6).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.40, 0.65, 0.95, 1.0) }
    }

    T.position(-1.8, 0.6, -0.6).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.75, 0.50, 0.85, 1.0) }
    }
    T.position(-0.6, 0.6, -0.6).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.80, 0.80, 0.40, 1.0) }
    }
    T.position(0.6, 0.6, -0.6).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.50, 0.85, 0.80, 1.0) }
    }
    T.position(1.8, 0.6, -0.6).scale(0.42, 0.42, 0.42) {
        R.cube() { C.rgba(0.85, 0.55, 0.65, 1.0) }
    }
}

// --- Scroll panel placed in front of the cubes ---
// Mirrors layout_mock_demo from scrolling.mms: viewport with sibling __bg as
// drag source, stencil clip, owned scrolling track.
T.position(0.0, 1.1, 1.4).scale(0.10, 0.10, 0.10) {
    name="panel_pierce_demo"

    T.position(0.0, 7.0, 0.02) {
        TXT {
            "scroll panel (Overlay)"
            C.rgba(0.95, 0.95, 0.98, 1.0)
            EM.on()
        }
    }

    T.position(-2.6, 3.4, 0.02) {
        name="panel_viewport"

        StencilClip {}

        // sibling __bg renderable acts as drag source for the viewport
        T.position(2.5, -10.0, 0.0).scale(5.2, 20.0, 5.0) {
            name="__bg"
            R.square() {
                C.rgba(0.96, 0.92, 0.18, 0.85)
                Raycastable.enabled()
            }
        }

        Scrolling.new(8.0, 28.0) {
            name="panel_scroll"
        }
    }
}

T.position(-2.0, 3.0, 2.0) {
    PL {
        intensity(2.2)
        distance(40.0)
        C.rgba(1.0, 1.0, 1.0, 1.0)
    }
}
