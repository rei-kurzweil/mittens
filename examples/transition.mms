// transition scene
// Corresponds to examples/transition.rs

RendererSettings {
    window_size(1280, 960)
}

BGC.rgba(0.07, 0.08, 0.11, 1.0)
AL.rgb(0.22, 0.22, 0.26)
Clock.bpm(90.0)

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.05)
            half_res(true)
        }
    }

    Bloom {
        intensity(0.90)
        emissive_scale(1.15)
    }
}

T.position(0.15, -0.55, 1.0) {
    DL {
        intensity(1.1)
        color(1.0, 0.98, 0.94)
    }
}

I.speed(2.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 1.35, 6.5) {
        C3D {
            Pointer {}
        }

        T.position(0.0, 2.25, -2.8).scale(0.065, 0.065, 1.0) {
            TXT {
                "transition demo\nthree cubes move on even beats\nand rotate on odd beats\ntransitions live on Transform"
                C.rgba(0.02, 0.02, 0.03, 1.0)
                TextureFiltering.linear()
            }
        }
    }
}

ED {
    T.position(0.0, -0.9, -5.2).scale(12.0, 0.16, 8.0) {
        R.cube() { C.rgba(0.14, 0.15, 0.18, 1.0) }
    }

    T.position(0.0, 2.3, -8.6).scale(11.5, 5.0, 0.22) {
        R.cube() { C.rgba(0.10, 0.10, 0.14, 1.0) }
    }

    let transition_cube_a = T.position(-2.8, 0.0, -5.2).scale(0.75, 0.75, 0.75) {
        name = "transition_cube_a"
        Transition {
            duration_beats(0.85)
            ease_in_out_sine()
            replace_same_target()
        }
        R.cube() {
            C.rgba(1.0, 0.34, 0.30, 1.0)
            Emissive.on()
        }
    }
    transition_cube_a

    let transition_cube_b = T.position(0.0, 0.0, -5.2).scale(0.75, 0.75, 0.75) {
        name = "transition_cube_b"
        Transition {
            duration_beats(0.85)
            ease_in_out_sine()
            replace_same_target()
        }
        R.cube() {
            C.rgba(0.30, 1.0, 0.45, 1.0)
            Emissive.on()
        }
    }
    transition_cube_b

    let transition_cube_c = T.position(2.8, 0.0, -5.2).scale(0.75, 0.75, 0.75) {
        name = "transition_cube_c"
        Transition {
            duration_beats(0.85)
            ease_in_out_sine()
            replace_same_target()
        }
        R.cube() {
            C.rgba(0.28, 0.62, 1.0, 1.0)
            Emissive.on()
        }
    }
    transition_cube_c

    Animation.looping() {
        Keyframe.at(0) {
            transition_cube_a.update_transform([-2.8, 0.0, -5.2], [0.0, 0.0, 0.0], [0.75, 0.75, 0.75])
            transition_cube_b.update_transform([0.0, 0.0, -5.2], [0.0, 0.0, 0.0], [0.75, 0.75, 0.75])
            transition_cube_c.update_transform([2.8, 0.0, -5.2], [0.0, 0.0, 0.0], [0.75, 0.75, 0.75])
        }

        Keyframe.at(1) {
            transition_cube_a.update_transform([-2.8, 0.0, -5.2], [0.0, 0.6, 0.0], [0.75, 0.75, 0.75])
            transition_cube_b.update_transform([0.0, 0.0, -5.2], [0.45, 0.0, 0.35], [0.75, 0.75, 0.75])
            transition_cube_c.update_transform([2.8, 0.0, -5.2], [0.0, -0.6, 0.0], [0.75, 0.75, 0.75])
        }

        Keyframe.at(2) {
            transition_cube_a.update_transform([-1.6, 1.1, -5.2], [0.0, 0.6, 0.0], [0.75, 0.75, 0.75])
            transition_cube_b.update_transform([0.0, -0.4, -4.2], [0.45, 0.0, 0.35], [0.75, 0.75, 0.75])
            transition_cube_c.update_transform([1.6, 1.1, -5.2], [0.0, -0.6, 0.0], [0.75, 0.75, 0.75])
        }

        Keyframe.at(3) {
            transition_cube_a.update_transform([-1.6, 1.1, -5.2], [0.35, 1.4, 0.25], [0.75, 0.75, 0.75])
            transition_cube_b.update_transform([0.0, -0.4, -4.2], [-0.4, 0.6, -0.5], [0.75, 0.75, 0.75])
            transition_cube_c.update_transform([1.6, 1.1, -5.2], [0.25, -1.4, 0.4], [0.75, 0.75, 0.75])
        }

        Keyframe.at(4) {
            transition_cube_a.update_transform([-3.5, -0.2, -4.5], [0.35, 1.4, 0.25], [0.75, 0.75, 0.75])
            transition_cube_b.update_transform([0.0, 1.25, -5.9], [-0.4, 0.6, -0.5], [0.75, 0.75, 0.75])
            transition_cube_c.update_transform([3.5, -0.2, -4.5], [0.25, -1.4, 0.4], [0.75, 0.75, 0.75])
        }

        Keyframe.at(5) {
            transition_cube_a.update_transform([-3.5, -0.2, -4.5], [-0.2, 2.2, 0.0], [0.75, 0.75, 0.75])
            transition_cube_b.update_transform([0.0, 1.25, -5.9], [0.8, 1.3, 0.2], [0.75, 0.75, 0.75])
            transition_cube_c.update_transform([3.5, -0.2, -4.5], [0.2, -2.2, -0.2], [0.75, 0.75, 0.75])
        }

        Keyframe.at(6) {
            transition_cube_a.update_transform([-2.2, 0.9, -6.1], [-0.2, 2.2, 0.0], [0.75, 0.75, 0.75])
            transition_cube_b.update_transform([0.0, 0.2, -5.0], [0.8, 1.3, 0.2], [0.75, 0.75, 0.75])
            transition_cube_c.update_transform([2.2, 0.9, -6.1], [0.2, -2.2, -0.2], [0.75, 0.75, 0.75])
        }

        Keyframe.at(7) {
            transition_cube_a.update_transform([-2.2, 0.9, -6.1], [0.0, 6.2831855, 0.0], [0.75, 0.75, 0.75])
            transition_cube_b.update_transform([0.0, 0.2, -5.0], [0.0, 3.1415927, 3.1415927], [0.75, 0.75, 0.75])
            transition_cube_c.update_transform([2.2, 0.9, -6.1], [0.0, -6.2831855, 0.0], [0.75, 0.75, 0.75])
        }
    }
}
