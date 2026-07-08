// pride scene
// Corresponds to examples/pride.rs

RendererSettings {
    window_size(1280, 960)
}

BGC {
    C.rgba(0.07, 0.07, 0.4, 1.0)
}
AL.rgb(0.55, 0.55, 0.55)

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.06)
            half_res(true)
        }
    }
    Bloom {
        intensity(0.90)
        radius_ndc(0.06)
        emissive_scale(1.2)
        half_res(true)
    }
}


// ground box
T.position(0, -3, 0).scale(10000, 0.1, 10000) {
    R.cube() {
        C.rgba(0.2, 0.2, 1.0, 1.0)
    }
}

Clock.bpm(140) {}

T.position(0.0, 3.5, 3.5) {
    PL {
        intensity(5.0)
        distance(200.0)
        color(1.0, 1.0, 1.0)
    }
}

I.speed(1.5) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 0.0, 6.8) {
        C3D {
            Pointer {}
        }
    }
}


let trans = fn () {
    return Transition {
        duration_beats(1.0)
        ease_in_out_sine()
        replace_same_target()
    } 
}

let annulus_0_glow = Emissive.on() { 
    intensity(0.2) 
    trans();
}
let annulus_1_glow = Emissive.on() { 
    intensity(0.2) 
    trans();
}
let annulus_2_glow = Emissive.on() { 
    intensity(0.2) 
    trans();
}
let annulus_3_glow = Emissive.on() { 
    intensity(0.2) 
    trans();
}
let annulus_4_glow = Emissive.on() { 
    intensity(0.2) 
    trans();
}

T.position(-4.1, -2.1, -4.0).scale(2,2,2) {

    let start_angle = 0;

    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(0.55, 0.89, start_angle, 1.5707963, 48) { 
            C.rgba(0.89, 0.16, 0.11, 1.0) annulus_0_glow 
        }
    }
    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(0.92, 1.26, start_angle, 1.5707963, 48) { 
            C.rgba(0.98, 0.49, 0.10, 1.0) annulus_1_glow 
        }
    }
    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(1.29, 1.63, start_angle, 1.5707963, 48) { 
            C.rgba(0.99, 0.84, 0.13, 1.0) annulus_2_glow 
        }
    }
    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(1.66, 2.00, start_angle, 1.5707963, 48) { 
            C.rgba(0.16, 0.68, 0.27, 1.0) annulus_3_glow 
        }
    }
    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(2.03, 2.37, start_angle, 1.5707963, 48) { 
            C.rgba(0.10, 0.42, 0.91, 1.0) annulus_4_glow 
        }
    }
}

Animation.looping().length(2.5) {
    Keyframe.at(0.0) {
        annulus_0_glow.set_intensity(2.5)
        annulus_4_glow.set_intensity(1.0)
        annulus_1_glow.set_intensity(0.0)
        annulus_2_glow.set_intensity(0.0)
        annulus_3_glow.set_intensity(0.0)
    }
    Keyframe.at(0.5) {
        annulus_1_glow.set_intensity(2.5)
        annulus_0_glow.set_intensity(1.0)
        annulus_2_glow.set_intensity(0.0)
        annulus_3_glow.set_intensity(0.0)
        annulus_4_glow.set_intensity(0.0)
    }
    Keyframe.at(1.0) {
        annulus_2_glow.set_intensity(2.5)
        annulus_1_glow.set_intensity(1.0)
        annulus_0_glow.set_intensity(0.0)
        annulus_3_glow.set_intensity(0.0)
        annulus_4_glow.set_intensity(0.0)
    }
    Keyframe.at(1.5) {
        annulus_3_glow.set_intensity(2.5)
        annulus_2_glow.set_intensity(1.0)
        annulus_0_glow.set_intensity(0.0)
        annulus_1_glow.set_intensity(0.0)
        annulus_4_glow.set_intensity(0.0)
    }
    Keyframe.at(2.0) {
        annulus_4_glow.set_intensity(2.5)
        annulus_3_glow.set_intensity(1.0)
        annulus_0_glow.set_intensity(0.0)
        annulus_1_glow.set_intensity(0.0)
        annulus_2_glow.set_intensity(0.0)
    }
}


let star = T.position(2.45, 0.55, -4.0).scale(1.6, 1.6, 1.0).rotation(0, 3.14159 / 4, 0) {
    trans();
    R.star(5, 0.48, 10, 10) {
        C.rgba(0.98, 0.91, 0.16, 1.0)
        EM.on()
    }
}

let heart = T.position(2.55, -1.85, -4.0).scale(1.7, 1.7, 1.0).rotation(-3.14159 / 8, 0, 3.14159 / 5) {
    trans();
    R.heart(64) {
        C.rgba(1.0, 0.52, 0.52, 1.0)
        EM.on()
    }
}

star
heart

Animation.looping().length(4) {
    Keyframe.at(0) {
        star.update_transform([2.45, 0.0, -4.0], [0, 3.14159 / 4, 0], [1.6, 1.6, 1.6])
    }
    Keyframe.at(1) {
        star.update_transform([2.45, -0.1, -4.0], [0, 3.14159 / 4, 0], [1.6, 1.6, 1.6])
    }
}