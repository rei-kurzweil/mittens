// examples/component-method-call.mms
//
// Demonstrates MMS method dispatch: anim.pause() and anim.play().
//
// A cube steps through 4 rotations driven by a looping Animation.
// • Click the BLUE cube  → anim.pause()
// • Click the GREEN cube → anim.play()
//
// The Animation component is bound to a local variable (let anim = A { ... }),
// and the signal handlers close over it, calling methods on the live handle.
//
// Run: cargo run --release --example component-method-call

RendererSettings {
    window_size(1280, 720)
}

BGC.rgba(0.10, 0.10, 0.14, 1.0)
AL.rgb(0.35, 0.35, 0.38)

T.position(3.0, 5.0, 4.0) {
    DL {}
}

// Camera + pointer
I.speed(3.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 0.5, 5.0) {
        C3D {}
        Pointer {}

        T.position(0.0, 1.6, -2.0).scale(0.055, 0.055, 1.0) {
            TXT {
                "click blue  → pause\nclick green → play\nwasd/rf/qe to move\nright-mouse drag to look"
                C.rgba(0.0, 0.0, 0.0, 1.0)
                TextureFiltering.linear()
            }
        }
    }
}

// The spinning cube — must be defined before the Animation so that
// Action.update_transform("cube") can resolve the name at spawn time.
let cube_t = T.position(0.0, 0.0, 0.0).scale(0.5, 0.5, 0.5) {
    name = "cube"
    Transition {
        duration_beats(0.85)
        ease_in_out_sine()
        replace_same_target()
    }
    R.cube() {
        C.rgba(0.90, 0.75, 0.30, 1.0)
        Raycastable.enabled()
    }
}

cube_t

// Clock drives the beat (60 BPM → one beat per second).
Clock.bpm(60) {}

// Looping animation: four keyframes, 90° Y-rotation each beat.
// One full rotation every 4 beats = 4 seconds at 60 BPM.
let anim = A {
    Keyframe.at(0) {
        Action.update_transform("cube", [0.0, 0.0, 0.0], [0.0,   0.0, 0.0], [0.5, 0.5, 0.5])
    }
    Keyframe.at(1) {
        Action.update_transform("cube", [0.0, 0.0, 0.0], [0.0,  90.0, 0.0], [0.5, 0.5, 0.5])
    }
    Keyframe.at(2) {
        Action.update_transform("cube", [0.0, 0.0, 0.0], [0.0, 180.0, 0.0], [0.5, 0.5, 0.5])
    }
    Keyframe.at(3) {
        Action.update_transform("cube", [0.0, 0.0, 0.0], [0.0, 270.0, 0.0], [0.5, 0.5, 0.5])
    }
}

anim

// BLUE cube — click to pause the animation.
let pause_btn = T.position(-1.2, -1.2, 0.0).scale(0.35, 0.35, 0.35) {
    R.cube() {
        C.rgba(0.25, 0.55, 1.0, 1.0)
        Raycastable.enabled()
    }
    T.position(0,0,0.6) {
        T.position(-0.25, 0, 0).scale(0.3, 0.8, 0.05) {
            R.cube() {
                C.rgba(1.0, 1.0, 1.0, 1.0)
            }
        }
        T.position( 0.25, 0, 0).scale(0.3, 0.8, 0.05) {
            R.cube() {
                C.rgba(1.0, 1.0, 1.0, 1.0)
            }
        }
    }
}

// GREEN cube — click to resume the animation.
let play_btn = T.position(1.2, -1.2, 0.0).scale(0.35, 0.35, 0.35) {
    R.cube() {
        C.rgba(0.30, 0.85, 0.45, 1.0)
        Raycastable.enabled()
    }
    T.position(0,0,0.6).rotation(0,0,-3.1415 / 2).scale(0.8, 0.8, 0.8) {
        R.triangle() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
        }
    }
}

pause_btn
play_btn

// playback status
let playback_status = Text {
    name="playback_status"
    "Playing"
}

T.position(-0.6, -1.2, 0).scale(0.2, 0.2, 0.2) {
    playback_status
}

on(pause_btn, "Click", fn(event) {
    anim.pause()
    playback_status.set_text("Paused")
    print("anim.pause()")
})

on(play_btn, "Click", fn(event) {
    anim.play()
    playback_status.set_text("Playing")
    print("anim.play()")
})


