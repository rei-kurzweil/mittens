import { traffic_light } from "./traffic_light.mms"
import { accordion_down_arrow_icon } from "./icons.mms"
import { accordion, accordion_body } from "./internal/ui/accordion.mms"
import { ship_solid } from "./ship_parts.mms"

fn content(label, factory) {
    return T {
        name = label + "_content"
        Material(MaterialDefinition.anime())
        factory()
    }
}
fn status_body() {
    return accordion_body(T { Style { height(0.1) } })
}

// A retained storefront control, a disposable 3D scene, and a stable floor.
// Closing removes the store's lights, fixtures, collisions and callbacks.
export fn ship_store_accordion(label, title, factory, store_width, store_length, control_position, initial_expanded, control_yaw, before_unload) {
    if control_position == null { control_position = [1.0,2.85,-0.15] }
    let expanded = initial_expanded != false
    if control_yaw == null { control_yaw = Math.pi }
    let mount = T { name = label + "_mount" if expanded { content(label,factory) } }
    let lamp_state = "red"
    if expanded { lamp_state = "green" }
    let signal = traffic_light(lamp_state,true)
    let initial_body = null
    if expanded { initial_body = status_body() }
    let panel = accordion({
        root_name = label + "_accordion"
        initial_expanded = expanded
        toggle_decoration = signal
        toggle_icon = T.position(0,-0.0255,0).scale(0.34,0.34,1) {
            accordion_down_arrow_icon([1,1,1,1],0)
        }
        width_gu = 36.0 unit_scale = 0.055
        unlit = true background_color = [0.12,0.19,0.22,1]
        toggle_icon_glow_intensity = 0.0
        children = []
        title_bar = T {
            Grabbable.off()
            Style { display("flex") flex_direction("row") width(100%) height(3.5)
                align_items("center") background_color([0.12,0.19,0.22,1]) }
            T { Style { flex_grow(1) color([0.85,0.95,0.92,1]) }
                Text { title } }
        }
        body = initial_body
    })
    let panel_root = panel.query("#" + label + "_accordion")
    let body_mount = panel.query("#accordion_body_mount")
    on(panel_root,"DataEvent",fn(event) {
        if event == "AccordionMinimized" && !panel_root.is_expanded() {
            signal.set_state("red")
            if before_unload { before_unload() }
            let current = mount.query("#" + label + "_content")
            if current { current.remove_subtree() }
        } else if event == "AccordionRestoreRequested" && panel_root.is_expanded() {
            signal.set_state("yellow")
            if !mount.query("#" + label + "_content") {
                mount.attach(content(label,factory))
            }
            if !body_mount.query("#accordion_body") { body_mount.attach(status_body()) }
            signal.set_state("green")
        }
    })
    let root = T {
        name = label
        if store_width != null {
            ship_solid([0,-0.10,store_length/2.0],[store_width,0.20,store_length],[0.19,0.23,0.25])
        }
        mount
        T.position(control_position[0],control_position[1],control_position[2]).rotation(0,control_yaw,0) { panel }
    }
    root.set_expanded = panel_root.set_expanded
    root.is_expanded = panel_root.is_expanded
    return root
}
