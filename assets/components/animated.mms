export fn rainbow_animated() {
    
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

    return T.scale(2,2,2) {

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
    }
}