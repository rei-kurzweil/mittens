// bloom scene
// Corresponds to examples/bloom.rs

RendererSettings {
    window_size(1280, 960)
}

let emissive_debug_texture = Texture.render_image("render_graph.emissive_pass.output")
let bloom_debug_texture = Texture.render_image("render_graph.bloom.blur")

BGC.rgba(0.25, 0.05, 0.15, 1.0)
AL.rgb(0.2, 0.05, 0.15)

RenderGraph {
    EmissivePass {
        Texture {}
    }

    Bloom {
        intensity(1.25)
        radius_ndc(0.075)
        emissive_scale(1.35)
        half_res(true)
        bloom_debug_texture
    }
}

I.speed(2.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }
    T.position(0.0, 1.8, 8.5) {
        C3D {}
        Pointer {}

        T.position(-2.85, -1.1, -4.5).scale(2.35, 1.8, 1.0) {
            OV {
                R.square() { C.rgba(0.96, 0.96, 0.98, 1.0) }
            }
        }

        T.position(-2.85, -1.1, -4.49).scale(2.05, 1.5, 1.0) {
            OV {
                R.square() {
                    C.rgba(1.0, 1.0, 1.0, 1.0)
                    emissive_debug_texture
                    TextureFiltering.linear()
                }
            }
        }

        T.position(-2.85, -1.95, -4.48).scale(0.06, 0.06, 1.0) {
            OV {
                TXT {
                    "emissive"
                    C.rgba(0.02, 0.02, 0.02, 1.0)
                    TextBackground {
                        padding(0.36)
                        padding_right(0.56)
                        C.rgba(0.95, 0.95, 0.98, 0.98)
                    }
                }
            }
        }

        T.position(2.35, -1.1, -4.5).scale(2.35, 1.8, 1.0) {
            OV {
                R.square() { C.rgba(0.96, 0.96, 0.98, 1.0) }
            }
        }

        T.position(2.35, -1.1, -4.49).scale(2.05, 1.5, 1.0) {
            OV {
                R.square() {
                    C.rgba(1.0, 1.0, 1.0, 1.0)
                    bloom_debug_texture
                    TextureFiltering.linear()
                }
            }
        }

        T.position(2.35, -1.95, -4.48).scale(0.06, 0.06, 1.0) {
            OV {
                TXT {
                    "bloom"
                    C.rgba(0.02, 0.02, 0.02, 1.0)
                    TextBackground {
                        padding(0.36)
                        padding_right(0.56)
                        C.rgba(0.95, 0.95, 0.98, 0.98)
                    }
                }
            }
        }
    }
}

ED {
    T.position(0.0, 3.5, 2.5) {
        PL {
            intensity(4.5)
            distance(18.0)
            color(1.0, 0.92, 0.84)
        }
    }

    T.position(-4.0, 1.5, 1.0) {
        PL {
            intensity(3.2)
            distance(14.0)
            color(0.30, 0.55, 1.0)
        }
    }

    T.position(4.0, 1.5, 1.0) {
        PL {
            intensity(3.2)
            distance(14.0)
            color(1.0, 0.35, 0.35)
        }
    }

    T.position(0.0, -0.75, -2.0).scale(16.0, 0.15, 10.0) {
        R.cube() { C.rgba(0.12, 0.12, 0.14, 1.0) }
    }

    T.position(-4.5,  0.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.85, 0.22, 0.22, 1.0) } }
    T.position(-2.25, 0.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.88, 0.52, 0.18, 1.0) } }
    T.position( 0.0,  0.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.90, 0.82, 0.24, 1.0) } }
    T.position( 2.25, 0.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.26, 0.78, 0.32, 1.0) } }
    T.position( 4.5,  0.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.22, 0.62, 0.92, 1.0) } }

    T.position(-4.5,  1.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.72, 0.22, 0.82, 1.0) } }
    T.position(-2.25, 1.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.86, 0.36, 0.56, 1.0) } }
    T.position( 0.0,  1.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.32, 0.78, 0.80, 1.0) } }
    T.position( 2.25, 1.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.36, 0.44, 0.94, 1.0) } }
    T.position( 4.5,  1.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.92, 0.48, 0.28, 1.0) } }

    T.position(-4.5,  3.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.55, 0.70, 0.22, 1.0) } }
    T.position(-2.25, 3.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.24, 0.90, 0.62, 1.0) } }
    T.position( 0.0,  3.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.30, 0.62, 0.95, 1.0) } }
    T.position( 2.25, 3.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.72, 0.42, 0.92, 1.0) } }
    T.position( 4.5,  3.0, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.92, 0.30, 0.64, 1.0) } }

    T.position(-4.5,  4.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.76, 0.76, 0.84, 1.0) } }
    T.position(-2.25, 4.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.62, 0.62, 0.70, 1.0) } }
    T.position( 0.0,  4.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.54, 0.54, 0.60, 1.0) } }
    T.position( 2.25, 4.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.70, 0.70, 0.78, 1.0) } }
    T.position( 4.5,  4.5, -5.5).scale(0.70, 0.70, 0.70) { R.cube() { C.rgba(0.84, 0.84, 0.92, 1.0) } }

    T.position(-3.3, 1.0, -1.8).rotation(0.2, 0.45, 0.0).scale(0.55, 0.55, 0.55) {
        R.cube() { C.rgba(1.0, 0.18, 0.18, 1.0) EM.on() }
    }

    T.position(-0.9, 2.7, -2.6).rotation(0.0, 0.25, 0.35).scale(0.45, 0.45, 0.45) {
        R.cube() { C.rgba(1.0, 0.65, 0.10, 1.0) EM.on() }
    }

    T.position(1.0, 0.8, -2.2).rotation(0.3, -0.15, 0.2).scale(0.50, 0.50, 0.50) {
        R.cube() { C.rgba(0.25, 1.0, 0.35, 1.0) EM.on() }
    }

    T.position(3.2, 2.0, -2.9).rotation(-0.2, -0.35, 0.1).scale(0.58, 0.58, 0.58) {
        R.cube() { C.rgba(0.18, 0.62, 1.0, 1.0) EM.on() }
    }

    T.position(0.0, 4.0, -3.2).rotation(0.45, 0.35, 0.2).scale(0.62, 0.62, 0.62) {
        R.cube() { C.rgba(0.95, 0.20, 1.0, 1.0) EM.on() }
    }

    T.position(0.85, 2.7, 5.6).scale(0.055, 0.055, 1.0) {
        TXT {
            "bloom example\nmove with wasd/rf/qe\nand right-mouse drag"
            C.rgba(0.0, 0.0, 0.0, 1.0)
            TextBackground {
                padding(0.5)
                padding_right(1.4)
                C.rgba(0.94, 0.94, 0.98, 0.95)
            }
            TextureFiltering.linear()
        }
    }
}