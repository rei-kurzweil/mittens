import { ui_unlit_material } from "../materials/ui_unlit.mms"

// Ordinary in-cab interaction: reaching in works without mounting the truck.
export fn pickup_power_panel(toggle) {
    let readout=Text {name="pickup_power_readout" "POWER OFF"}
    let panel=T.position(0.64,1.80,-0.70) {
        name="pickup_power_button"
        ui_unlit_material()
        Grabbable.off()
        Selectable.off()
        T.scale(0.20,0.09,0.03) {
            R.cube() {
                C.rgba(0.20,0.28,0.25,1)
                Raycastable.click_only() {interaction_priority(180)}
            }
        }
        T.position(-0.077,-0.009,0.021).scale(0.015,0.015,0.015) {readout}
    }
    on(panel,"Click",fn(event) {toggle()})
    panel.show_power=fn(value) {
        if value {readout.set_text("POWER ON")}
        else {readout.set_text("POWER OFF")}
    }
    return panel
}
