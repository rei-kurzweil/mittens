// pride scene
// Corresponds to examples/pride.rs

RendererSettings {
    window_size(1280, 960)
}

BGC {
    C.rgba(0.05, 0.05, 0.6, 1.0)
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


Clock.bpm(240) {}

T.position(0.0, 3.5, 3.5) {
    PL {
        intensity(5.0)
        distance(20.0)
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

T.position(-0.15, 2.35, 0.2).scale(0.075, 0.075, 1.0) {
    TXT {
        "partial_annulus_2d()\nstar()\nheart()"
        C.rgba(0.08, 0.08, 0.10, 1.0)
        EM.on()
        TextureFiltering.linear()
    }
}

T.position(2.55, -2.1, -4.1).scale(0.05, 0.05, 1.0) {
    TXT {
        "all geometry authored\nin this MMS file"
        C.rgba(0.12, 0.12, 0.15, 1.0)
        EM.on()
        TextureFiltering.linear()
    }
}

let annulus_0_glow = Emissive.on() { intensity(0.2) }
let annulus_1_glow = Emissive.on() { intensity(0.2) }
let annulus_2_glow = Emissive.on() { intensity(0.2) }
let annulus_3_glow = Emissive.on() { intensity(0.2) }
let annulus_4_glow = Emissive.on() { intensity(0.2) }

T.position(-2.1, -2.1, -4.0) {
    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(0.55, 0.89, 0.0, 1.5707963, 48) { 
            C.rgba(0.89, 0.16, 0.11, 1.0) annulus_0_glow 
        }
    }
    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(0.92, 1.26, 0.0, 1.5707963, 48) { 
            C.rgba(0.98, 0.49, 0.10, 1.0) annulus_1_glow 
        }
    }
    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(1.29, 1.63, 0.0, 1.5707963, 48) { 
            C.rgba(0.99, 0.84, 0.13, 1.0) annulus_2_glow 
        }
    }
    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(1.66, 2.00, 0.0, 1.5707963, 48) { 
            C.rgba(0.16, 0.68, 0.27, 1.0) annulus_3_glow 
        }
    }
    T.scale(1.0, 1.0, 1.0) {
        R.partial_annulus_2d(2.03, 2.37, 0.0, 1.5707963, 48) { 
            C.rgba(0.10, 0.42, 0.91, 1.0) annulus_4_glow 
        }
    }
}

Animation.looping().length(5.0) {
    Keyframe.at(0.0) {
        annulus_0_glow.set_intensity(2.5)
        annulus_4_glow.set_intensity(1.0)
        annulus_1_glow.set_intensity(0.0)
        annulus_2_glow.set_intensity(0.0)
        annulus_3_glow.set_intensity(0.0)
    }
    Keyframe.at(1.0) {
        annulus_1_glow.set_intensity(2.5)
        annulus_0_glow.set_intensity(1.0)
        annulus_2_glow.set_intensity(0.0)
        annulus_3_glow.set_intensity(0.0)
        annulus_4_glow.set_intensity(0.0)
    }
    Keyframe.at(2.0) {
        annulus_2_glow.set_intensity(2.5)
        annulus_1_glow.set_intensity(1.0)
        annulus_0_glow.set_intensity(0.0)
        annulus_3_glow.set_intensity(0.0)
        annulus_4_glow.set_intensity(0.0)
    }
    Keyframe.at(3.0) {
        annulus_3_glow.set_intensity(2.5)
        annulus_2_glow.set_intensity(1.0)
        annulus_0_glow.set_intensity(0.0)
        annulus_1_glow.set_intensity(0.0)
        annulus_4_glow.set_intensity(0.0)
    }
    Keyframe.at(4.0) {
        annulus_4_glow.set_intensity(2.5)
        annulus_3_glow.set_intensity(1.0)
        annulus_0_glow.set_intensity(0.0)
        annulus_1_glow.set_intensity(0.0)
        annulus_2_glow.set_intensity(0.0)
    }
}

T.position(2.45, 0.55, -4.0).scale(1.6, 1.6, 1.0) {
    R.star(5, 0.48, 10, 10) {
        C.rgba(0.98, 0.91, 0.16, 1.0)
        EM.on()
    }
}

T.position(2.55, -1.85, -4.0).scale(1.7, 1.7, 1.0) {
    R.heart(96) {
        C.rgba(0.94, 0.12, 0.12, 1.0)
        EM.on()
    }
}
