// assets/components/world-panel.mms
//
// Reusable World panel chrome for MMS-first iteration.
//
// `world_panel(title, show_placeholders)` returns a panel root with:
// - named title bar nodes
// - named save/load buttons
// - named status text
// - a router that routes direct children of the panel root into `rows_mount`
//
// For now, `show_placeholders = true` lets a standalone MMS example render
// visible routed content before Rust-side row rebuilding is wired into this.

let WORLD_PANEL_WIDTH_GU = 9.5
let WORLD_PANEL_CONTENT_HEIGHT_GU = 54.0
let TITLE_BAR_HEIGHT_GU = 3.0
let TITLE_CONTENT_GAP_GU = 0.5
let WORLD_PANEL_TOTAL_HEIGHT_GU = TITLE_BAR_HEIGHT_GU + TITLE_CONTENT_GAP_GU + WORLD_PANEL_CONTENT_HEIGHT_GU
let TITLEBAR_BUTTON_WIDTH_GU = 6.875
let TITLEBAR_BUTTON_HEIGHT_GU = 2.4
let TITLEBAR_BUTTON_MARGIN_TOP_BOTTOM_GU = 0.3
let TITLEBAR_BUTTON_MARGIN_LEFT_GU = 0.625
let TITLE_LABEL_WIDTH_GU = 14.5
let TITLE_LABEL_PADDING_X_GU = 0.25

fn panel_button(node_name, label) {
    let root = T {
        name = node_name
        Raycastable.enabled()
        Style {
            display("inline-block")
            width(TITLEBAR_BUTTON_WIDTH_GU)
            height(TITLEBAR_BUTTON_HEIGHT_GU)
            margin_xy(TITLEBAR_BUTTON_MARGIN_TOP_BOTTOM_GU, TITLEBAR_BUTTON_MARGIN_LEFT_GU)
            text_align("center")
            background_color = [0.08, 0.50, 0.16, 1.0]
            color = [0.75, 1.00, 0.45, 1.0]
        }
        T.position(0.0, 0.0, 0.05) {
            Text { label }
        }
    }
    return root
}

fn placeholder_row(label, bg) {
    let row = T.position(0.0, 0.0, 0.2) {
        Style {
            margin_xy(0.25, 0.20)
            padding_xy(0.55, 0.45)
            background_color = bg
        }
        T.position(0.0, 0.0, 0.15) {
            Text {
                label
                C.rgba(0.06, 0.09, 0.08, 1.0)
            }
        }
    }
    return row
}

export fn world_panel(title, show_placeholders) {
    let save_button = panel_button("save_button", "Save")
    let load_button = panel_button("load_button", "Load")

    let root = T {
        name = "world_panel_root"

        Router {
            target = "rows_mount"
            ignore = ["panel_layout_root", "title_bar", "content_slot", "save_status_wrap"]
        }

        T.position(0.0, TITLE_BAR_HEIGHT_GU + 0.8, 0.25) {
            name = "save_status_wrap"
            T.position(0.0, 0.0, 0.05) {
                Text {
                    name = "panel_status_value"
                    "idle"
                    C.rgba(0.92, 1.00, 0.92, 1.0)
                }
            }
        }

        LayoutRoot {
            name = "panel_layout_root"
            available_width(WORLD_PANEL_WIDTH_GU)
            available_height(WORLD_PANEL_TOTAL_HEIGHT_GU)

            T {
                name = "title_bar"
                Style {
                    height(TITLE_BAR_HEIGHT_GU)
                    margin_bottom(TITLE_CONTENT_GAP_GU)
                    background_color = [0.16, 0.66, 0.22, 0.96]
                }

                T {
                    name = "title_label_wrap"
                    Style {
                        display("inline-block")
                        width(TITLE_LABEL_WIDTH_GU)
                        height(TITLE_BAR_HEIGHT_GU)
                        padding_xy(0.0, TITLE_LABEL_PADDING_X_GU)
                        color = [0.90, 1.00, 0.92, 1.0]
                    }
                    T.position(0.0, 0.0, 0.05) {
                        Text {
                            name = "title_label"
                            title
                        }
                    }
                }

                save_button
                load_button
            }

            T {
                name = "content_slot"
                Style {
                    height(WORLD_PANEL_CONTENT_HEIGHT_GU)
                    overflow("scroll")
                    background_color = [0.94, 0.90, 0.18, 0.82]
                }

                T {
                    name = "rows_mount"
                    Style {
                    }
                }
            }

        }

        if show_placeholders {
            placeholder_row("root child routed into rows_mount", [0.92, 0.97, 0.92, 1.0])
            placeholder_row("save/load handlers can query named descendants", [0.90, 0.95, 0.98, 1.0])
            placeholder_row("rows_mount is the intended Rust injection target", [0.97, 0.92, 0.92, 1.0])
        }
    }

    return root
}