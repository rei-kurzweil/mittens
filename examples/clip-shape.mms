BGC {
    C.rgba(1.0, 0.80, 0.1, 1.0)
}

AL {
    C.rgba(0.35, 0.35, 0.35, 1.0)
}

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.06)
            half_res(true)
        }
    }
    Bloom {
        intensity(0.95)
        radius_ndc(0.06)
        emissive_scale(1.2)
        half_res(true)
    }
}


T.position(-1.3, 1.5, 1.3) {
    PL {
        intensity(7.5)
        distance(28.0)
        color(1.0, 1.0, 1.0)
    }
}

I.speed(1.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 0.0, 4.2) {
        C3D {
            Pointer {}
        }
    }
}

T.position(0.0, 0.0, -4.8).scale(2.55, 2.55, 1.0).rotation(0.0, 0.0, 3.14159) {
    T.position(0, 0, -0.05).scale(1.1, 1.1, 1) {
        R.star(5, 0.48, 6, 6) {
            C.rgba(0.9, 0.7, 0.2, 1.0)
        }    
    }
    R.star(5, 0.48, 10, 10) {
        C.rgba(1.0, 1.0, 1.0, 1.0)

        StencilClip {
            
            T.position(0.0, 0, -1.25).scale(0.4, 0.4, 0.4).rotation(-0.5, -0.7, 0.0) {
                Overlay {
                    R.cube() {
                        C.rgba(0.66, 0.66, 0.69, 1.0)
                    }
                }
            }
            
        }
    }
}
