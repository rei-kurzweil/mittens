// array-access.mms
//
// Sketch-only MMS example for future array indexing + array mutation support.
// This file is intended to show the target authoring shape before the parser
// and evaluator support `values[i]` reads/writes.
//
// Desired not-yet-implemented features exercised here:
//   1. array indexing: values[0]
//   2. array mutation: values[0] = values[0] + 1
//   3. loading display text from the same indexed array slot
//
// Layout: 6 rows inside one LayoutRoot. Each row has:
//   up button  |  value text  |  down button
//
// Button icons reuse the floating triangle style from component-method-call.mms.

RendererSettings {
    window_size(1100, 760)
}

BGC {
    C.rgba(0.93, 0.93, 0.95, 1.0)
}

AL.rgb(0.55, 0.55, 0.60)

T.position(3.0, 5.0, 4.0) {
    DL {}
}

I.speed(3.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, -0.3, 3.2) {
        C3D {}
        Pointer {}
    }
}

BG {
    T.position(0, -10, 0).scale(10000, 1, 10000) {
        R.cube() { C.rgba(0.65, 0.65, 0.67, 1.0) }
    }
}

let values = [0, 0, 0, 0, 0, 0]
let layout_root = null

T.position(-1.5, 0.5, 0.0).scale(0.065, 0.065, 0.065) {
    layout_root = LayoutRoot {
        available_width(42.0)
        available_height(70.0)

        Text {
            Style.padding_bottom(1.5)
                    .font_size(1.2) {}
            "Array access sketch"
        }

        Text {
            Style.padding_bottom(1.5)
                    .font_size(0.75) {}
            "Six counters driven from one array"
        }

        T.position(0, 0, 0.05) {
            name = "row_0"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.95, 0.72, 0.72, 1.0]) {}

            T.position(0, 0, 0.08) {
                name = "up_0"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.82, 0.28, 0.28, 1.0]) {}
                T.position(0, 0, 0.6).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(2.4, 2.0)
                        .background_color([0.60, 0.18, 0.18, 1.0]) {}
                Text {
                    name = "text_0"
                    values[0]
                }
            }

            T.position(0, 0, 0.08) {
                name = "down_0"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.70, 0.20, 0.20, 1.0]) {}
                T.position(0, 0, 0.6).rotation(0, 0, 3.14159).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }
        }

        T.position(0, 0, 0.05) {
            name = "row_1"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.95, 0.80, 0.67, 1.0]) {}

            T.position(0, 0, 0.08) {
                name = "up_1"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.85, 0.48, 0.10, 1.0]) {}
                T.position(0, 0, 0.6).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(2.4, 2.0)
                        .background_color([0.64, 0.36, 0.06, 1.0]) {}
                Text {
                    name = "text_1"
                    values[1]
                }
            }

            T.position(0, 0, 0.08) {
                name = "down_1"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.74, 0.40, 0.08, 1.0]) {}
                T.position(0, 0, 0.6).rotation(0, 0, 3.14159).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }
        }

        T.position(0, 0, 0.05) {
            name = "row_2"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.95, 0.90, 0.64, 1.0]) {}

            T.position(0, 0, 0.08) {
                name = "up_2"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.82, 0.66, 0.12, 1.0]) {}
                T.position(0, 0, 0.6).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(2.4, 2.0)
                        .background_color([0.62, 0.49, 0.08, 1.0]) {}
                Text {
                    name = "text_2"
                    values[2]
                }
            }

            T.position(0, 0, 0.08) {
                name = "down_2"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.72, 0.58, 0.10, 1.0]) {}
                T.position(0, 0, 0.6).rotation(0, 0, 3.14159).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }
        }

        T.position(0, 0, 0.05) {
            name = "row_3"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.78, 0.94, 0.68, 1.0]) {}

            T.position(0, 0, 0.08) {
                name = "up_3"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.22, 0.64, 0.26, 1.0]) {}
                T.position(0, 0, 0.6).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(2.4, 2.0)
                        .background_color([0.16, 0.45, 0.18, 1.0]) {}
                Text {
                    name = "text_3"
                    values[3]
                }
            }

            T.position(0, 0, 0.08) {
                name = "down_3"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.18, 0.54, 0.22, 1.0]) {}
                T.position(0, 0, 0.6).rotation(0, 0, 3.14159).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }
        }

        T.position(0, 0, 0.05) {
            name = "row_4"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.67, 0.86, 0.95, 1.0]) {}

            T.position(0, 0, 0.08) {
                name = "up_4"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.18, 0.44, 0.82, 1.0]) {}
                T.position(0, 0, 0.6).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(2.4, 2.0)
                        .background_color([0.12, 0.30, 0.60, 1.0]) {}
                Text {
                    name = "text_4"
                    values[4]
                }
            }

            T.position(0, 0, 0.08) {
                name = "down_4"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.14, 0.36, 0.70, 1.0]) {}
                T.position(0, 0, 0.6).rotation(0, 0, 3.14159).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }
        }

        T.position(0, 0, 0.05) {
            name = "row_5"
            Style.display("flex")
                    .flex_direction("row")
                    .gap(1.0)
                    .padding(1.0)
                    .background_color([0.84, 0.74, 0.95, 1.0]) {}

            T.position(0, 0, 0.08) {
                name = "up_5"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.48, 0.26, 0.82, 1.0]) {}
                T.position(0, 0, 0.6).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }

            T.position(0, 0, 0.05) {
                Style.padding_xy(2.4, 2.0)
                        .background_color([0.34, 0.16, 0.60, 1.0]) {}
                Text {
                    name = "text_5"
                    values[5]
                }
            }

            T.position(0, 0, 0.08) {
                name = "down_5"
                Style.padding_xy(1.6, 1.6)
                        .background_color([0.42, 0.20, 0.72, 1.0]) {}
                T.position(0, 0, 0.6).rotation(0, 0, 3.14159).scale(0.75, 0.75, 0.75) {
                    R.triangle() {
                        C.rgba(1.0, 1.0, 1.0, 1.0)
                        Raycastable.enabled()
                    }
                }
            }
        }
    }

    layout_root
}

let up_0 = layout_root.query("#up_0")
let down_0 = layout_root.query("#down_0")
let text_0 = layout_root.query("#text_0")

let up_1 = layout_root.query("#up_1")
let down_1 = layout_root.query("#down_1")
let text_1 = layout_root.query("#text_1")

let up_2 = layout_root.query("#up_2")
let down_2 = layout_root.query("#down_2")
let text_2 = layout_root.query("#text_2")

let up_3 = layout_root.query("#up_3")
let down_3 = layout_root.query("#down_3")
let text_3 = layout_root.query("#text_3")

let up_4 = layout_root.query("#up_4")
let down_4 = layout_root.query("#down_4")
let text_4 = layout_root.query("#text_4")

let up_5 = layout_root.query("#up_5")
let down_5 = layout_root.query("#down_5")
let text_5 = layout_root.query("#text_5")

// These handlers sketch the intended future syntax.
// They are expected to fail until array indexing and array mutation land.

on(up_0, "Click", fn(event) {
    values[0] = values[0] + 1
    text_0.set_text(values[0])
})

on(down_0, "Click", fn(event) {
    values[0] = values[0] - 1
    text_0.set_text(values[0])
})

on(up_1, "Click", fn(event) {
    values[1] = values[1] + 1
    text_1.set_text(values[1])
})

on(down_1, "Click", fn(event) {
    values[1] = values[1] - 1
    text_1.set_text(values[1])
})

on(up_2, "Click", fn(event) {
    values[2] = values[2] + 1
    text_2.set_text(values[2])
})

on(down_2, "Click", fn(event) {
    values[2] = values[2] - 1
    text_2.set_text(values[2])
})

on(up_3, "Click", fn(event) {
    values[3] = values[3] + 1
    text_3.set_text(values[3])
})

on(down_3, "Click", fn(event) {
    values[3] = values[3] - 1
    text_3.set_text(values[3])
})

on(up_4, "Click", fn(event) {
    values[4] = values[4] + 1
    text_4.set_text(values[4])
})

on(down_4, "Click", fn(event) {
    values[4] = values[4] - 1
    text_4.set_text(values[4])
})

on(up_5, "Click", fn(event) {
    values[5] = values[5] + 1
    text_5.set_text(values[5])
})

on(down_5, "Click", fn(event) {
    values[5] = values[5] - 1
    text_5.set_text(values[5])
})