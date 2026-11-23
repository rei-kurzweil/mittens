// dock.mms — a prototype launcher that opens and closes MMS panels.
//
// dock(options) required options:
//   root_name: the dock's movable root. Its title bar collapses the button
//     list and grabs the dock; dock_dropzone slots accept it.
//   mount: a transform that receives opened panels; the caller places it.
//   groups: lists of items, drawn top to bottom with a gap between groups.
//     Each item is { id, label, root_name, position, rotation?, build }.
//     root_name: the opened panel's movable root. Grip grabbing may reparent
//       it, so closing removes it by name as well as through its slot.
//     position/rotation: the panel's placement in mount space.
//     build: zero-argument function returning a fresh panel subtree.
// Clicking an item opens its panel; clicking it again closes it.

import { info_panel } from "./info_panel.mms"

let UNIT_SCALE = 0.06
let WIDTH_GU = 12.0
let BUTTON_WIDTH = 10.0
let GROUP_GAP_GU = 1.4
let TITLE_HEIGHT_GU = 3.5

fn item_text(item, open) {
    if open { return "> " + item.label }
    return "  " + item.label
}

// Buttons are rebuilt whenever the dock is restored from collapsed; the slot
// keeps whether the item's panel is open across those rebuilds.
fn dock_button(slot, mount) {
    let item = slot.item
    let label = Text { name = "dock_" + item.id + "_label" item_text(item, slot.frame != null) }
    let button = T {
        name = "dock_" + item.id
        Grabbable.off()
        Raycastable.click_only() { interaction_priority(180.0) }
        Style {
            display("flex") width(BUTTON_WIDTH) height(2.6)
            align_items("center") padding_xy(0.6, 0.0) font_size(0.72)
            background_color([0.88, 0.81, 0.66, 1.0]) background_z(0.01)
        }
        T.position(0.0, 0.0, 0.03) { Raycastable.disabled() label }
    }
    on(button, "Click", fn(event) {
        if slot.frame {
            let panel = query("#" + item.root_name)
            if panel { panel.remove_subtree() }
            slot.frame.remove_subtree()
            slot.frame = null
            label.set_text(item_text(item, false))
            return
        }
        let rotation = [0.0, 0.0, 0.0]
        if item["rotation"] != null { rotation = item.rotation }
        let frame = T.position(item.position[0], item.position[1], item.position[2])
            .rotation(rotation[0], rotation[1], rotation[2]) {
            name = "dock_" + item.id + "_frame"
            item.build()
        }
        mount.attach(frame)
        slot.frame = frame
        label.set_text(item_text(item, true))
    })
    return button
}

fn dock_content(slot_groups, mount) {
    return T {
        Style { display("flex") flex_direction("column") width(100%) row_gap(0.45) }
        for index in range(len(slot_groups)) {
            if index > 0 { T { Style { width(100%) height(GROUP_GAP_GU) } } }
            for slot in slot_groups[index] { dock_button(slot, mount) }
        }
    }
}

export fn dock(options) {
    let slot_groups = []
    for group in options.groups {
        let slots = []
        for item in group { slots.push({ item = item frame = null }) }
        slot_groups.push(slots)
    }
    let panel = info_panel({
        root_name = options.root_name
        width_gu = WIDTH_GU
        unit_scale = UNIT_SCALE
        title = "Panels"
        toggle_icon_glow_intensity = 0.0
        content_factory = fn() { return dock_content(slot_groups, options.mount) }
    })
    let root = panel.query("#" + options.root_name)
    root.attach(Selectable.off())
    // Drop zones test this probe against themselves, so the dock docks when
    // its title bar, not its corner, is released over a slot.
    root.attach(T.position(WIDTH_GU * UNIT_SCALE / 2.0, -TITLE_HEIGHT_GU * UNIT_SCALE / 2.0, 0.0) {
        Zone.sphere(0.06) { name = options.root_name + "_dock_probe" }
    })
    root.dock_panel = true
    return panel
}

// A slot that holds a dock. The caller attaches the returned transform where
// the dock should ride, such as a player's movement root. Its origin is
// where the dock's title bar sits; +Z faces the person using the dock.
// Required: name. Optional: radius (default 0.25).
export fn dock_dropzone(options) {
    let radius = 0.25
    if options["radius"] != null { radius = options.radius }
    let zone = Zone.sphere(radius).receive_drops(0) { name = options.name }
    // The dock's root is its top-left corner, offset from the title bar.
    let rest = T.position(-WIDTH_GU * UNIT_SCALE / 2.0, TITLE_HEIGHT_GU * UNIT_SCALE / 2.0, 0.0) {}
    let frame = T {
        name = options.name + "_frame"
        zone
        rest
        // A small marker shows where the slot is.
        T.scale(0.03, 0.03, 0.03) {
            R.sphere() { C.rgba(0.88, 0.81, 0.66, 1.0) Raycastable.disabled() }
        }
    }
    on(zone, "ZoneDropped", fn(drop) {
        if drop.target.dock_panel != true { return }
        let pose = drop.target.world_trs()
        let slot = rest.world_trs()
        zone.attach(drop.target)
        drop.target.world_trs({ translation = slot.translation rotation_quat_xyzw = slot.rotation_quat_xyzw scale = pose.scale })
    })
    return frame
}
