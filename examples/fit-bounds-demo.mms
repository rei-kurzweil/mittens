// fit-bounds-demo.mms — isolated paint-panel-style FitBounds repro scene.

BGC {
    C.rgba(0.25, 0.25, 0.25, 1.0)
}

I {
    speed(1.0)
    InputTransformMode.forward_z() {
        roll_axis_y()
        fps_rotation()
    }

    T.position(0.0, 1.1, 3.6) {
        C3D {
            Pointer {}
        }
    }
}

import { pencil_icon } from "../assets/components/icons.mms"
import { line_icon } from "../assets/components/icons.mms"
import { spray_can_icon } from "../assets/components/icons.mms"
import { fill_icon } from "../assets/components/icons.mms"
import { erase_icon } from "../assets/components/icons.mms"

let TEXT_SCALE = 0.08
let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let PAINT_PANEL_WIDTH_GU = 41.0
let PAINT_PANEL_STATUS_BAR_HEIGHT_GU = 4.0
let PAINT_PANEL_CONTENT_STATUS_GAP_GU = 0.5
let PAINT_PANEL_CONTENT_HEIGHT_GU = 8.5
let PAINT_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + PAINT_PANEL_CONTENT_HEIGHT_GU + PAINT_PANEL_CONTENT_STATUS_GAP_GU + PAINT_PANEL_STATUS_BAR_HEIGHT_GU

let title_color = [0.90, 1.00, 0.88, 1.0]
let panel_background_color = [0.08, 0.24, 0.11, 0.92]
let item_background_color = [0.92, 0.97, 0.92, 1.0]

fn fit_bounds_paint_panel_item(label, icon) {
    return T {
        name = "paint_panel_item"
        Option {}
        Raycastable.enabled()
        Style {
            display("inline-block")
            width(7.0)
            height(7.5)
            margin(0.4)
            background_color(item_background_color)
            background_z(-0.01)
            text_align("center")
            vertical_align("middle")
        }
        T {
            Style {
                display("block")
                width(100%)
            }
            T {
                Style {
                    display("block")
                    height(4.0)
                    margin_top(0.4)
                    margin_bottom(0.3)
                    text_align("center")
                    vertical_align("middle")
                }
                FitBounds.to_container() {
                    T {
                        icon
                    }
                }
            }
            T {
                Style {
                    display("block")
                    width(100%)
                    padding_xy(0.2, 0.0)
                    color = [0,0,0,1]
                    word_wrap("normal")
                }
                Text {
                    name = "selection_item_label"
                    label
                }
            }
        }
    }
}

let panel = T {
    name = "fit_bounds_demo_panel"
    Style {
        display("block")
        width(PAINT_PANEL_WIDTH_GU)
        height(PAINT_PANEL_TOTAL_HEIGHT_GU)
        margin_xy(0.5, 0.5)
    }

    T {
        name = "title_bar"
        Raycastable.enabled()
        Style {
            display("block")
            height(TITLE_BAR_HEIGHT_GU)
            margin_bottom(TITLE_CONTENT_GAP_GU)
            padding_xy(0.5, 0.5)
            color = title_color
            background_color(panel_background_color)
            text_align("left")
            vertical_align("middle")
            background_z(-0.01)
        }
        T.position(0.0, 0.0, 0.0) {
            Text { "FitBounds Demo" }
        }
    }

    T {
        name = "content_slot"
        Raycastable.enabled()
        Style {
            display("block")
            height(PAINT_PANEL_CONTENT_HEIGHT_GU)
            margin_bottom(PAINT_PANEL_CONTENT_STATUS_GAP_GU)
            background_color([0.96, 0.92, 0.18, 0.80])
            background_z(-0.001)
            padding(0.5)
        }

        T {
            name = "paint_tool_options_wrap"
            Selection {
                name = "paint_tool_selection"
            }
            Style {
                display("block")
                width(100%)
            }

            fit_bounds_paint_panel_item("Free Draw", pencil_icon())
            fit_bounds_paint_panel_item("Line", line_icon())
            fit_bounds_paint_panel_item("Spray Can", spray_can_icon())
            fit_bounds_paint_panel_item("Fill", fill_icon())
            fit_bounds_paint_panel_item("Erase", erase_icon())
        }
    }

    T {
        name = "paint_status_wrap"
        Raycastable.enabled()
        Style {
            display("block")
            height(PAINT_PANEL_STATUS_BAR_HEIGHT_GU)
            width(100%)
            padding_xy(0.25, 0.45)
            text_align("left")
            vertical_align("middle")
            word_wrap("normal")
            background_color([0.08, 0.24, 0.11, 0.92])
            background_z(-0.01)
        }
        T.position(0.0, 0.0, 0.0) {
            Text {
                "debug scene: FitBounds icon sizing still enabled"
            }
        }
    }
}

Selectable.off() {
    T.position(0.0, 1.6, -1.2) {
        Overlay {
            LayoutRoot {
                name = "fit_bounds_demo_layout_root"
                available_width(PAINT_PANEL_WIDTH_GU + 2.0)
                unit_scale(TEXT_SCALE)

                T {
                    name = "fit_bounds_demo_panel_shell"
                    Style {
                        display("inline-block")
                        width(PAINT_PANEL_WIDTH_GU)
                        height(PAINT_PANEL_TOTAL_HEIGHT_GU)
                        margin_xy(0.5, 0.5)
                    }

                    panel
                }
            }
        }
    }
}

AL {
    C.rgba(0.22, 0.22, 0.22, 1.0)
}

T.position(-1.6, 2.8, 2.2) {
    DL {
        intensity(0.95)
        color(1.0, 1.0, 1.0)
    }
}

T.position(1.8, 1.4, 2.8) {
    DL {
        intensity(0.35)
        color(1.0, 1.0, 1.0)
    }
}
