// Small Android-compatible script for the browser's source preview.
import { ui_unlit_material } from "../../assets/components/materials/ui_unlit.mms"
BGC.rgba(0.03, 0.05, 0.08, 1.0)
T.position(-0.8, 1.7, -2.0).scale(0.08, 0.08, 0.08) {
    ui_unlit_material()
    Text { "Hello from mitten-droid" C.rgba(0.64, 0.97, 0.87, 1.0) }
}
