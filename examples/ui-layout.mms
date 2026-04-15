BGC {
    C.rgba(0.9, 0.9, 0.9, 1.0)
}

BG {
    T.position(0.0, 0.0, -5.0)
    .scale(10.0, 10.0, 1.0)
    .rotation(2,3,0) {
        R.cube() {
            C.rgba(1.0, 1.0, 1.0, 1.0)
        }
    }
    T.position(0.0, 0.0,  4.9).scale(9.5, 9.5, 1.0) {
        R.cube() {
            C.rgba(0.8, 0.8, 0.8, 1.0)

        }
    }
}


I {
    speed(1.0)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(0.0, 1.2, 3.5) {
        C3D {
            Pointer {}
        }
    }
}

T.position(0, 0, -5.0).scale(1.8, 1.8, 1.0) {
    // Sketch of the intended topology:
    // - this scaled T defines the viewport pose + size
    // - StencilClip owns the content branch
    // - The clip shape renderable owns StencilClip
    // - the plane keeps viewport scale and defines the stencil boundary
    // - the pipeline drops scale before producing the scroll/content branch
    R.plane() {
        C.rgba(0.9, 0.9, 0.9, 1.0)
    
        StencilClip {
            TransformPipeline {
                TransformForkTRS {
                    TransformMapTranslation {}
                    TransformMapRotation {}
                    TransformMapScale {
                        TransformDrop {}
                    }
                    TransformMergeTRS {}
                }
                TransformPipelineOutput {
                    T {
                        Scrolling.new(1.0, 100.0) {
                            for y in range(100) {
                                T.position(0, y, 0.01).scale(0.12, 0.12, 0.12) {
                                    Text {
                                        "item "+y
                                        C.rgba(0.6, 0.6, 0.6, 1.0)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// lighting
AL {
    C.rgba(0.14, 0.14, 0.14, 1.0)
}
T.position(-1, -1, 0) {
    DL {
        intensity(0.9)
        C.rgba(1.0, 0.75, 0.75, 1.0)
    }
}
T.position(1,1,0) {
    DL {
        intensity(0.9)
        C.rgba(0.75, 0.75, 1.0, 1.0)
    }
}

// render graph
RenderGraph {
    EmissivePass {

    }
    Bloom {
        intensity(0.95)
        radius_ndc(0.06)
        emissive_scale(1.2)
        half_res(true)
    }
}

// perimeter cubes
ED {
T.position(0.0, -5.0, -5.0) {
    let i = 0;
    for x in range(-5, 6) {
        for y in range(-5, 6) {
            i = i + 1;
            T.position(x, y, 0.0).scale(0.9, 0.9, 0.9) {
                if i % 2 == 0 {
                    T.position(0.0, 0.0, 2.0).scale(0.25, 0.25, 0.25) {
                        R.cube() {
                            C.rgba(i / 10.0, i / 10.0, i / 10.0, 0.9)
                            Emissive.on()
                        }
                    }
                }
                if x % 2 == 0 && y % 2 == 0 {
                    R.cube() {
                        C.rgba(1.0, x / 5.0, y / 5.0, 1.0)
                        Emissive.on()
                    }

                } else {
                    R.cube() {
                        C.rgba(x / 5.0, 1.0, y / 5.0, 1.0)
                    }
                }
            }

        }
    }
}
}