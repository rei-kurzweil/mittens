import { button } from "../assets/components/button.mms"

RendererSettings {
    window_size(1280, 860)
}

BGC {
    C.rgba(0.40, 0.45, 0.82, 1.0)
}

AL {
    C.rgba(0.24, 0.26, 0.30, 1.0)
}

T.position(-2.4, 3.0, 2.6) {
    DL {
        intensity(0.95)
        C.rgba(1.0, 0.95, 0.90, 1.0)
    }
}

T.position(2.8, 1.8, 2.0) {
    DL {
        intensity(0.45)
        C.rgba(0.78, 0.90, 1.0, 1.0)
    }
}

I.speed(3.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 0.9, 4.0) {
        C3D {
            Pointer {}
        }
    }
}

BG {
    T.position(0.0, -10.0, 0.0).scale(10000.0, 1.0, 10000.0) {
        R.cube() {
            C.rgba(0.22, 0.24, 0.28, 1.0)
        }
    }
}

let cool_rows = [
    {
        key = "dataset"
        label = "dataset"
        value = "cool"
    },
    {
        key = "status"
        label = "status"
        value = "nominal"
    },
    {
        key = "power"
        label = "power"
        value = "71%"
    },
    {
        key = "glow"
        label = "glow"
        value = "cyan"
    }
]

let warm_rows = [
    {
        key = "dataset"
        label = "dataset"
        value = "warm"
    },
    {
        key = "status"
        label = "status"
        value = "boosted"
    },
    {
        key = "power"
        label = "power"
        value = "93%"
    },
    {
        key = "glow"
        label = "glow"
        value = "amber"
    }
]

let current_rows = cool_rows

let cool_button = button("cool")
let warm_button = button("warm")

let panel_root = T.position(0.0, 1.3, -1.4).scale(0.08, 0.08, 0.08) {
    name = "mms_tables_panel_root"

    LayoutRoot {
        name = "mms_tables_layout_root"
        available_width(48.0)
        available_height(24.0)

        T {
            name = "table_row"
            Style {
                display("flex")
                flex_direction("row")
                align_items("center")
            }

            T {
                name = "left_button_slot"
                Style {
                    display("flex")
                    justify_content("center")
                    align_items("center")
                    width(8.0)
                    padding_xy(0.35, 0.0)
                }
                cool_button
            }

            T {
                name = "table_shell"
                Style {
                    width(28.0)
                    margin_xy(0.5, 0.0)
                    padding(1.0)
                    font_size(1.15)
                    background_color = [0.93, 0.96, 0.99, 0.94]
                }

                T {
                    Style {
                        padding_xy(0.0, 0.4)
                        background_color = [0.95, 0.95, 0.95, 1.0]
                    }
                    Text {
                        "mms tables"
                        C.rgba(0.09, 0.09, 0.09, 1.0)
                    }
                }

                T {
                    Style {
                        margin(0.35)
                        padding_xy(0.5, 0.35)
                    }
                    Text {
                        name = "table_status_text"
                        "click a side button"
                        C.rgba(0.09, 0.09, 0.09, 1.0)
                    }
                }

                T {
                    Style {
                        display("flex")
                        flex_direction("row")
                        padding_xy(0.75, 0.5)
                        background_color = [0.6, 0.6, 0.6, 1.0]
                    }

                    T {
                        Style {
                            display("inline-block")
                            width(11.0)
                        }
                        Text {
                            "field"
                            C.rgba(0.96, 0.97, 0.98, 1.0)
                        }
                    }

                    T {
                        Style {
                            display("inline-block")
                            width(13.0)
                        }
                        Text {
                            "value"
                            C.rgba(0.96, 0.97, 0.98, 1.0)
                        }
                    }
                }

                for row in current_rows {
                    T {
                        name = row.key + "_row"
                        Style {
                            display("flex")
                            flex_direction("row")
                            margin(0.3)
                            padding_xy(0.75, 0.55)
                            background_color = [0.84, 0.89, 0.95, 1.0]
                        }

                        T {
                            Style {
                                display("inline-block")
                                width(11.0)
                            }
                            Text {
                                row.label
                                C.rgba(0.09, 0.09, 0.09, 1.0)
                            }
                        }

                        T {
                            Style {
                                display("inline-block")
                                width(13.0)
                            }
                            Text {
                                name = row.key + "_value"
                                row.value
                                C.rgba(0.09, 0.09, 0.09, 1.0)
                            }
                        }
                    }
                }
            }

            T {
                name = "right_button_slot"
                Style {
                    display("flex")
                    justify_content("center")
                    align_items("center")
                    width(8.0)
                    padding_xy(0.35, 0.0)
                }
                warm_button
            }
        }
    }
}

panel_root


let status_text = panel_root.query("#table_status_text")
let dataset_value = panel_root.query("#dataset_value")
let status_value = panel_root.query("#status_value")
let power_value = panel_root.query("#power_value")
let glow_value = panel_root.query("#glow_value")

fn apply_rows(rows, title) {
    for row in rows {
        if row.key == "dataset" {
            dataset_value.set_text(row.value)
        } else if row.key == "status" {
            status_value.set_text(row.value)
        } else if row.key == "power" {
            power_value.set_text(row.value)
        } else if row.key == "glow" {
            glow_value.set_text(row.value)
        }
    }

    status_text.set_text(title)
}

on(cool_button, "Click", fn(event) {
    apply_rows(cool_rows, "loaded cool_rows")
})

on(warm_button, "Click", fn(event) {
    apply_rows(warm_rows, "loaded warm_rows")
})
