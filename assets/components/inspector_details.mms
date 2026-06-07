// inspector_details.mms — inspector detail form (=^･ω･^=)

let ROW_GAP_GU = 0.5
let LABEL_BG = [0.84, 0.84, 0.84, 0.95]
let FIELD_BG = [0.92, 0.95, 0.92, 0.95]
let ROOT_BG = [0.58, 0.58, 0.58, 0.96]
let LABEL_COLOR = [0.12, 0.18, 0.12, 1.0]
let VALUE_COLOR = [0.04, 0.06, 0.04, 1.0]

fn detail_row(label, value) {
    return T {
        name = "inspector_detail_row"
        Style {
            display("block")
            width(100%)
            margin_bottom(ROW_GAP_GU)
            background_color([0.72, 0.72, 0.72, 0.92])
            background_z(-0.01)
            padding(0.35)
        }

        T {
            name = "inspector_detail_label"
            Style {
                display("block")
                width(100%)
                padding_xy(0.1, 0.25)
                margin_bottom(0.2)
                background_color(LABEL_BG)
                background_z(-0.01)
                color = LABEL_COLOR
            }
            T.position(0.0, 0.0, 0.0) {
                Text { label }
            }
        }

        T {
            name = "inspector_detail_value"
            Style {
                display("block")
                width(100%)
                padding_xy(0.3, 0.35)
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
            background_color(ROOT_BG)
            background_z(-0.01)
            font_size(1)
        }

        detail_row("Name", component_name)
        detail_row("ID", component_id)
        detail_row("GUID", component_guid)
    }
}
