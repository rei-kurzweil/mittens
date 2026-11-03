import { ship_box } from "./ship_parts.mms"

// Rectangular pressure window. Front is +Z; the caller builds the surrounding
// wall around the aperture. Only the frame is rendered; the pane is collision-only.
export fn ship_window(label, width, height) {
    return T {
        name = label
        CombineMesh {
            for side in [-1.0, 1.0] {
                ship_box([side*width/2.0, 0.0, 0.0], [0.08, height+0.08, 0.14], [0.26, 0.33, 0.34])
                ship_box([0.0, side*height/2.0, 0.0], [width, 0.08, 0.14], [0.26, 0.33, 0.34])
            }
        }
        // Clear aperture: no alpha panes until transparent sky composition is fixed.
        // The pressure boundary remains collidable, with no rendered backing layer.
        Zone.cube([width/2.0, height/2.0, 0.04]) { Collidable.static().friction(0.8) {} }
    }
}
