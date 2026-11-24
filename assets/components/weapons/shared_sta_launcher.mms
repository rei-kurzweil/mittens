import { sta_rocket_launcher } from "../sta-rocket-launcher.mms"
import { shared_grabbable } from "../network/shared_grabbable.mms"
import { vec3, vec4 } from "types"

let pose_type = { translation: vec3, rotation_quat_xyzw: vec4, scale: vec3 }

// The grip and impacts share the scene's peer lifecycle. Only the holder
// creates a rocket; its final contact is replicated by the launcher device.
export fn shared_sta_launcher(label, options) {
    let tool = sta_rocket_launcher({
        name = label socket = options["socket"] weapon_id = options.weapon_id
        can_fire = options["can_fire"] on_hit = options["on_hit"]
    })
    let grip = T.rotation_quat(0.5, -0.5, -0.5, 0.5) {
        name = label + "_grip"
        Selectable.off()
        Grabbable.to_hand(true).hand_offset([0.0, 0.0, -0.105]) {}
        // Give the narrow handle a hand-sized pickup target from either side.
        T.scale(0.16, 0.16, 0.14) {
            R.cube() { Opacity.opacity(0.0).multiple_layers() Raycastable.enabled() }
        }
        tool.root
        tool.model
    }
    let grab = null
    if options["socket"] {
        grab = shared_grabbable({ socket = options.socket name = options.weapon_id + ".grip" target = grip rank = options.rank mounts = options["mounts"] })
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
    let breech = { pointer = null target = null outgoing = null incoming = null }
    fn update_hinge_bindings() {
        let local = breech.pointer != null
        if breech.outgoing { breech.outgoing.set_binding_enabled(local) }
        if breech.incoming { breech.incoming.set_binding_enabled(!local) }
    }
    fn bind_pointer(pointer) {
        grab.bind_pointer(pointer)
        on(pointer, "PointerGrabbed", fn(event) {
            if event.target != tool.hinge.query("[name='back_half']") { return }
            breech.pointer = event.pointer
            breech.target = event.target
            update_hinge_bindings()
        })
        on(pointer, "PointerReleased", fn(event) {
            if event.pointer != breech.pointer || event.target != breech.target { return }
            breech.pointer = null
            breech.target = null
            update_hinge_bindings()
        })
    }
    fn connect(group, address) {
        grab.connect(group, address)
        tool.network.connect(group, address)
        let channel_name = options.weapon_id + ".hinge"
        breech.outgoing = options.socket.publish(channel_name).to(address).type(pose_type).bind_transform(tool.hinge)
        breech.incoming = options.socket.subscribe(channel_name).from(address).type(pose_type).bind_transform(tool.hinge)
        update_hinge_bindings()
        group.attach(breech.outgoing)
        group.attach(breech.incoming)
    }
    fn disconnect() {
        tool.use_end()
        grab.disconnect()
        tool.network.disconnect()
        breech.outgoing = null
        breech.incoming = null
    }
    return { root = grab.root grip = grip bind_pointer = bind_pointer connect = connect disconnect = disconnect }
}
