// assets/components/world_panel_status.mms
//
// Small rerenderable status subtree for the world panel.

let TEXT_SCALE = 0.08

export fn world_panel_status(label) {
    let root = T {
        name = "panel_status_root"
        T.position(0.0, 0.0, 0.015) {
            Text {
                name = "panel_status_value"
                label
            }
        }
    }

    return root
}