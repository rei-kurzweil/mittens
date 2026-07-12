import { rainbow_animated }       from "../assets/components/animated.mms"
import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"

RendererSettings {
    window_size(1280, 960)
}

BGC {
    C.rgba(0.03, 0.02, 0.11, 1.0)
}
AL.rgb(0.20, 0.18, 0.34)

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.06)
            half_res(true)
        }
    }
    Bloom {
        intensity(1.0)
        radius_ndc(0.06)
        emissive_scale(1.2)
        half_res(true)
    }
}

let moon = T.position(4.2, 10.9, -8.8).scale(1.8, 1.8, 1.0).rotation(0.0, 0.0, 0.4) {
    Transition {
        duration_beats(0.7)
        ease_in_out_sine()
        replace_same_target()
    }
    R.circle2d(0.5, 64) {
        C.rgba(0.7, 0.9, 1.0, 1.0)
        EM.on() {
            intensity(1.8)
        }
    }
}

BG.occlusion_and_lighting() {
    ED {
        moon
    }
    
    star_kawaii_background([1, 0.84, 0.15, 1])
    
}

Clock.bpm(140) {}

T.position(0.0, 3.5, 3.5) {
    PL {
        intensity(5.0)
        distance(200.0)
        color(1.0, 0.96, 1.0)
    }
}

T.position(-2.5, 1.4, -6.5) {
    PL {
        intensity(3.0)
        distance(120.0)
        color(0.45, 0.82, 1.0)
    }
}

I.speed(1.5) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 0.15, 8.4) {
        C3D {
            Pointer {}
        }
    }
}

T.position(0.0, -3.0, 0.0).scale(10000, 0.1, 10000) {
    R.cube() {
        C.rgba(0.04, 0.02, 0.09, 1.0)
    }
}

let rainbow = rainbow_animated()

ED {
    rainbow
    rainbow.update_transform([0.0, -1.2, -5.4], [0.0, 0.0, 0.0], [2.6, 2.6, 2.6])
}

Animation.looping().length(8.0) {
    Keyframe.at(0.0) {
        moon.update_transform([4.0, 2.9, -8.8], [0.0, 0.0, 0.0], [1.8, 1.8, 1.0])
    }
    Keyframe.at(4.0) {
        moon.update_transform([4.0, 3.1, -8.8], [0.0, 0.0, 0.0], [1.8, 1.8, 1.0])
    }
    Keyframe.at(7.0) {
        moon.update_transform([4.0, 2.9, -8.8], [0.0, 0.0, 0.0], [1.8, 1.8, 1.0])
    }
}
