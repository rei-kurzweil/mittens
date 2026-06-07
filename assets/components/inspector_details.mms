// inspector_details.mms — inspector detail form (=^･ω･^=)

let LABEL_WIDTH_GU = 6.5
let ROW_GAP_GU = 0.35
let VALUE_PADDING_X_GU = 0.3
let FIELD_BG = [0.94, 0.97, 0.94, 0.92]
let LABEL_COLOR = [0.12, 0.18, 0.12, 1.0]
let VALUE_COLOR = [0.04, 0.06, 0.04, 1.0]

fn detail_row(label, value) {
    return T {
        name = "inspector_detail_row"
        Style {
            display("block")
            width(100%)
            margin_bottom(ROW_GAP_GU)
        }

        T {
            name = "inspector_detail_label"
            Style {
                display("inline-block")
                width(LABEL_WIDTH_GU)
                padding_xy(0.1, 0.35)
                text_align("right")
                vertical_align("middle")
                color = LABEL_COLOR
            }
            T.position(0.0, 0.0, 0.0) {
                Text { label }
            }
        }

        T {
            name = "inspector_detail_value"
            Style {
                display("inline-block")
                width(68%)
                padding_xy(VALUE_PADDING_X_GU, 0.35)
                vertical_align("middle")
                background_color(FIELD_BG)
                background_z(-0.01)
                color = VALUE_COLOR
                word_wrap("normal")
            }
            T.position(0.0, 0.0, 0.0) {
                Text { value }
            }
        }
    }
}

export fn inspector_details(component_name, component_id, component_guid) {
    return T {
        name = "inspector_details_root"
        Style {
            display("block")
            width(100%)
            padding(0.5)
            font_size(1)
        }

        detail_row("Name", component_name)
        detail_row("ID", component_id)
        detail_row("GUID", component_guid)
    }
}
