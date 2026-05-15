// font-size.mms — compare block and inline-block layout with container-driven
// and authored text font sizes.

BGC {
    C.rgba(0.14, 0.14, 0.16, 1.0)
}

I {
    speed(1.0)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }
    T.position(0.0, 1.0, 3.0) {
        C3D {}
        Pointer {}
    }
}

let panel_bg = [0.96, 0.94, 0.82, 1.0]
let section_bg = [0.88, 0.93, 0.98, 1.0]
let chip_bg = [0.76, 0.88, 0.78, 1.0]
let sample_bg = [0.98, 0.84, 0.86, 1.0]
let text_color = C.rgba(0.08, 0.08, 0.10, 1.0)

fn inline_chip(label, size, bg) {
    let chip = T.position(0.0, 0.0, 0.2) {
        Style {
            display("inline-block")
            margin_xy(0.4, 0.4)
            padding_xy(0.45, 0.65)
            background_color = bg
            font_size(size)
        }
        T.position(0.0, 0.0, 0.15) {
            Text {
                label
                text_color
                EM.on()
            }
        }
    }
    return chip
}

fn block_sample(title, size, subtitle) {
    let card = T.position(0.0, 0.0, 0.2) {
        Style {
            margin(0.5)
            padding(0.65)
            background_color = sample_bg
        }

        T.position(0.0, 0.0, 0.2) {
            Style {
                margin_bottom(0.45)
                font_size(size)
            }
            T.position(0.0, 0.0, 0.15) {
                Text {
                    title
                    text_color
                    EM.on()
                }
            }
        }

        T.position(0.0, 0.0, 0.2) {
            Style {
                font_size(0.75)
            }
            T.position(0.0, 0.0, 0.15) {
                Text {
                    subtitle
                    text_color
                    EM.on()
                }
            }
        }
    }
    return card
}

T.position(-3.4, 2.3, 0.0).scale(0.10, 0.10, 0.10) {
    LayoutRoot {
        name = "font_size_demo_root"
        available_width(18.0)
        available_height(48.0)

        T {
            name = "panel"
            Style {
                width(84.0)
                padding(0.8)
                background_color = panel_bg
            }

            T.position(0.0, 0.0, 0.2) {
                name = "title_block"
                Style {
                    margin_bottom(0.7)
                    padding_xy(0.45, 0.55)
                    background_color = section_bg
                    font_size(1.4)
                }
                T.position(0.0, 0.0, 0.15) {
                    Text {
                        "Style.font_size on block containers"
                        text_color
                        EM.on()
                    }
                }
            }

            T.position(0.0, 0.0, 0.2) {
                name = "inline_section"
                Style {
                    margin_bottom(0.8)
                    padding(0.55)
                    background_color = section_bg
                }

                inline_chip("inline 0.60", 0.60, chip_bg)
                inline_chip("inline 1.00", 1.00, chip_bg)
                inline_chip("inline 1.60", 1.60, chip_bg)

                T.position(0.0, 0.0, 0.2) {
                    Style {
                        display("inline-block")
                        margin_xy(0.4, 0.4)
                        padding_xy(0.45, 0.65)
                        background_color = [0.86, 0.78, 0.92, 1.0]
                    }
                    T.position(0.0, 0.0, 0.15) {
                        Text {
                            font_size(1.9)
                            "Text.font_size(1.9)"
                            text_color
                            EM.on()
                        }
                    }
                }
            }

            T.position(0.0, 0.0, 0.2) {
                name = "block_section"
                Style {
                    padding(0.55)
                    background_color = section_bg
                }

                block_sample("block font_size 0.75", 0.75, "container-driven font scale")
                block_sample("block font_size 1.20", 1.20, "same layout, larger glyphs")
                block_sample("block font_size 1.80", 1.80, "more vertical pressure in the same card")
            }
        }
    }
}

// lighting

AL {
    C.rgba(0.12, 0.12, 0.3, 1.0)
}

T.position(0.0, 0.0, 4) {
    DL {
        intensity(0.8)
        color(0.98, 0.98, 0.95)
    }
}

