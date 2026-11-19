import { sta_rocket_launcher } from "../sta-rocket-launcher.mms"
import { shared_grabbable } from "../network/shared_grabbable.mms"

// The grip and impacts share the scene's peer lifecycle. Only the holder
// creates a rocket; its final contact is replicated by the launcher device.
export fn shared_sta_launcher(label, options) {
    let tool = sta_rocket_launcher({ name = label socket = options["socket"] weapon_id = options.weapon_id })
    let grip = T.rotation_quat(0.5, -0.5, -0.5, 0.5) {
        name = label + "_grip"
        Selectable.off()
        Grabbable.to_hand(true).hand_offset([0.0, 0.0, -0.105]) {}
        tool.root
        T.position(0.191, -1.50, -0.15) { GLTF.new("assets/models/STA_launcher.glb") { name = label } }
    }
    let grab = null
    if options["socket"] {
        grab = shared_grabbable({ socket = options.socket name = options.weapon_id + ".grip" target = grip rank = options.rank })
    }
    let held = { pointer = null hand = "" }
    on(grip, "GrabStart", fn(event) {
        if event.target != grip { return }
        held.pointer = event.pointer
        held.hand = "Desktop"
        if event.pointer == query(options.left_pointer) { held.hand = "Left" }
        if event.pointer == query(options.right_pointer) { held.hand = "Right" }
    })
    on(grip, "GrabEnd", fn(event) {
        if event.target == grip && held.pointer == event.pointer { held.pointer = null tool.use_end() }
    })
    grip.ship_input = fn(kind, event) {
        if !held.pointer || (grab && grab.state.owner != options.rank) { return }
        if kind == "XrButtonDown" || kind == "XrButtonUp" {
            if event.hand != held.hand || (event.control != "LeftTrigger" && event.control != "RightTrigger") { return }
            if kind == "XrButtonDown" { tool.use_start({ user = query(options.user) }) } else { tool.use_end() }
        } else if held.hand == "Desktop" && event.code == "Space" {
            if kind == "KeyDown" { tool.use_start({ user = held.pointer }) } else { tool.use_end() }
        }
    }
    if !grab { return { root = grip grip = grip } }
    return { root = grab.root grip = grip bind_pointer = grab.bind_pointer
        connect = fn(group, address) { grab.connect(group, address) tool.network.connect(group, address) }
        disconnect = fn() { tool.use_end() grab.disconnect() tool.network.disconnect() } }
}
