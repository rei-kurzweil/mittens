import { button } from "../assets/components/button.mms"

RendererSettings {
    window_size(1400, 920)
}

BGC.rgba(0.92, 0.90, 0.84, 1.0)
AL.rgb(0.30, 0.30, 0.32)

T.position(2.5, 4.2, 3.5) {
    DL {
        intensity(0.95)
        C.rgba(1.0, 0.96, 0.90, 1.0)
    }
}

I.speed(3.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 1.0, 4.2) {
        C3D {
            Pointer {}
        }
    }
}

BG {
    T.position(0.0, -10.0, 0.0).scale(10000.0, 1.0, 10000.0) {
        R.cube() {
            C.rgba(0.28, 0.30, 0.34, 1.0)
        }
    }
}

let app_state = {
    text = "hello table fields"
    draft_text = "hello table fields"
    status = "idle"
    send_count = 0
}

let send_btn = button("send", {
    background_color = [0.16, 0.52, 0.30, 1.0]
    color = [0.97, 0.98, 0.96, 1.0]
})

let panel = T.position(0.0, 1.45, -1.45).scale(0.08, 0.08, 0.08) {
    name = "table_field_reassign_panel"

    LayoutRoot {
        name = "table_field_reassign_panel_layout"
        available_width(52.0)
        available_height(42.0)

        T {
            name = "panel_shell"
            Style {
                display("flex")
                flex_direction("column")
                row_gap(1.1)
                width(100%)
            }

            T {
                name = "table_header_island"
                Style {
                    display("flex")
                    flex_direction("column")
                    row_gap(0.55)
                    width(100%)
                    padding(0.9)
                    background_color = [0.97, 0.95, 0.90, 0.97]
                }

                T {
                    Style {
                        padding_xy(0.5, 0.35)
                    }
                    Text {
                        "table field reassignment"
                        C.rgba(0.14, 0.14, 0.16, 1.0)
                    }
                }

                T {
                    Style {
                        padding_xy(0.5, 0.2)
                    }
                    Text {
                        "type in the input, then click send to commit the draft variable into app_state"
                        C.rgba(0.28, 0.28, 0.32, 1.0)
                    }
                }
            }

            T {
                name = "table_island"
                Style {
                    display("flex")
                    flex_direction("column")
                    row_gap(0.35)
                    width(100%)
                    padding(0.8)
                    background_color = [0.91, 0.88, 0.80, 1.0]
                }

                T {
                    Style {
                        display("flex")
                        flex_direction("row")
                        align_items("center")
                        padding_xy(0.9, 0.7)
                        background_color = [0.39, 0.38, 0.34, 1.0]
                    }

                    T {
                        Style {
                            display("flex")
                            align_items("center")
                            width(14.0)
                        }
                        Text {
                            "Key"
                            C.rgba(0.97, 0.97, 0.95, 1.0)
                        }
                    }

                    T {
                        Style {
                            display("flex")
                            align_items("center")
                            width(24.0)
                        }
                        Text {
                            "Value"
                            C.rgba(0.97, 0.97, 0.95, 1.0)
                        }
                    }
                }

                T {
                    Style {
                        display("flex")
                        flex_direction("row")
                        align_items("center")
                        padding_xy(0.9, 0.7)
                        background_color = [0.98, 0.98, 0.96, 1.0]
                    }
                    T {
                        Style { display("flex") align_items("center") width(14.0) }
                        Text { "text" C.rgba(0.12, 0.12, 0.14, 1.0) }
                    }
                    T {
                        Style { display("flex") align_items("center") width(24.0) }
                        Text { name = "text_value" "" C.rgba(0.12, 0.12, 0.14, 1.0) }
                    }
                }

                T {
                    Style {
                        display("flex")
                        flex_direction("row")
                        align_items("center")
                        padding_xy(0.9, 0.7)
                        background_color = [0.95, 0.94, 0.90, 1.0]
                    }
                    T {
                        Style { display("flex") align_items("center") width(14.0) }
                        Text { "draft_text" C.rgba(0.12, 0.12, 0.14, 1.0) }
                    }
                    T {
                        Style { display("flex") align_items("center") width(24.0) }
                        Text { name = "draft_value" "" C.rgba(0.12, 0.12, 0.14, 1.0) }
                    }
                }

                T {
                    Style {
                        display("flex")
                        flex_direction("row")
                        align_items("center")
                        padding_xy(0.9, 0.7)
                        background_color = [0.98, 0.98, 0.96, 1.0]
                    }
                    T {
                        Style { display("flex") align_items("center") width(14.0) }
                        Text { "status" C.rgba(0.12, 0.12, 0.14, 1.0) }
                    }
                    T {
                        Style { display("flex") align_items("center") width(24.0) }
                        Text { name = "status_value" "" C.rgba(0.12, 0.12, 0.14, 1.0) }
                    }
                }

                T {
                    Style {
                        display("flex")
                        flex_direction("row")
                        align_items("center")
                        padding_xy(0.9, 0.7)
                        background_color = [0.95, 0.94, 0.90, 1.0]
                    }
                    T {
                        Style { display("flex") align_items("center") width(14.0) }
                        Text { "send_count" C.rgba(0.12, 0.12, 0.14, 1.0) }
                    }
                    T {
                        Style { display("flex") align_items("center") width(24.0) }
                        Text { name = "send_count_value" "" C.rgba(0.12, 0.12, 0.14, 1.0) }
                    }
                }
            }

            T {
                name = "controls_island"
                Style {
                    display("flex")
                    flex_direction("column")
                    row_gap(0.55)
                    width(100%)
                    padding(0.9)
                    background_color = [0.93, 0.91, 0.85, 1.0]
                }

                T {
                    Style {
                        padding_xy(0.2, 0.1)
                    }
                    Text {
                        "Draft input"
                        C.rgba(0.20, 0.20, 0.24, 1.0)
                    }
                }

                T {
                    name = "controls_row"
                    Style {
                        display("flex")
                        flex_direction("row")
                        align_items("center")
                        column_gap(0.9)
                    }

                    T {
                        name = "draft_input_shell"
                        Style {
                            display("inline-block")
                            width(33.0)
                            padding_xy(0.75, 0.65)
                            background_color = [1.0, 1.0, 1.0, 1.0]
                            color = [0.10, 0.11, 0.14, 1.0]
                            font_size(1.3)
                        }
                        TextInput {
                            name = "draft_input"
                            "hello table fields"
                        }
                    }

                    send_btn
                }
            }
        }
    }
}

panel

let draft_input = panel.query("#draft_input")
let text_value = panel.query("#text_value")
let draft_value = panel.query("#draft_value")
let status_value = panel.query("#status_value")
let send_count_value = panel.query("#send_count_value")

fn render_app_state(state) {
    print("render_app_state text=", state.text, " draft_text=", state.draft_text, " send_count=", state.send_count, " status=", state.status)
    text_value.set_text(state.text)
    draft_value.set_text(state.draft_text)
    status_value.set_text(state.status)
    send_count_value.set_text("" + state.send_count)
}

render_app_state(app_state)

on(draft_input, "TextInputChanged", fn(event) {
    print("draft_input TextInputChanged text=", event.text)
    app_state.draft_text = event.text
    app_state.status = "draft updated"
    render_app_state(app_state)
})

on(send_btn, "Click", fn(event) {
    print("send_btn Click draft_text=", app_state.draft_text, " app_state.text(before)=", app_state.text)
    app_state.text = app_state.draft_text
    app_state.send_count = app_state.send_count + 1
    app_state.status = "sent"
    print("send_btn Click updated app_state.text=", app_state.text, " send_count=", app_state.send_count)
    render_app_state(app_state)
})
