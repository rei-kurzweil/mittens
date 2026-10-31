// Generic custom opaque/albedo surface, without a legacy material fallback.
import { ui_unlit_material } from "../../assets/components/materials/ui_unlit.mms"
BGC.rgba(0.03, 0.05, 0.08, 1.0)
T.position(0, 1.4, -2).scale(0.3, 0.3, 0.3) {
    R.cube() { ui_unlit_material() C.rgba(0.64, 0.97, 0.87, 1.0) }
}
