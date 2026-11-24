// Triggering starts the launch sequence. The visual-time event at 100 ms is
// where the rear flash will finish and the independent rocket will be launched.
// on_launch receives the muzzle's world pose and the user captured at trigger.
// Optional can_fire(user) gates trigger and launch. on_hit(hit) receives the contact.
import { launch_rocket, apply_rocket_impact } from "./projectiles/rocket.mms"
import { sta_rocket_body, claim_sta_rocket_ammo } from "./projectiles/sta_rocket_ammo.mms"
import { shared_impact } from "./network/shared_impact.mms"

let chambered_rocket_material = MaterialDefinition.toon().opaque_bucket("far")

export fn sta_rocket_launcher(options) {
    let label = "sta_rocket_launcher"
    if options["name"] != null { label = options.name }
    let can_fire = options["can_fire"]
    let on_hit = options["on_hit"]
    let on_explosion = options["on_explosion"]
    let on_ammo = options["on_ammo"]
    let can_reload = options["can_reload"]
    // A socket-enabled caller connects network to its peer channel group.
    let network = null
    fn apply_impact(point, amount, rotation) {
        apply_rocket_impact(point, amount, rotation)
        if on_explosion { on_explosion(point) }
    }
    if options["socket"] {
        let weapon_id = label
        if options["weapon_id"] { weapon_id = options.weapon_id }
        network = shared_impact({ socket = options.socket name = weapon_id apply = apply_impact oriented = true })
    }
    fn impact(point, amount, rotation) {
        if network { network.hit(point, amount, rotation) }
        else { apply_impact(point, amount, rotation) }
    }
    let initial_round = sta_rocket_body(label + "_initial_round")
    let state = { trigger_held = false pending = false user = null round = initial_round }
    let hinge = TOP.hinge([-1.0, 0.0, 0.0]).limits(0.0, 75.0 * Math.pi / 180.0) {
        name = label + "_hinge"
    }
    // Lower edge of the seam in the front_half node's authored coordinates.
    let pivot = [0.0, 4.14594, -13.4373]
    let breech = T.position(pivot[0], pivot[1], pivot[2]) { hinge }
    // The export faces -Z. Center its remaining X offset on the grip and
    // level its 4.54-degree barrel pitch to match the device's muzzle axis.
    let mesh = GLTF.new("assets/models/STA_launcher.glb") { name = label }
    let model = T.position(0.191245, 0, 0).rotation(0.079284, 0, 0) { mesh }
    on(mesh, "GLTFInitialized", fn(event) {
        let front = event.gltf.query("[name='front_half']")
        let rear = event.gltf.query("[name='back_half']")
        if !front || !rear { return }
        let rest = rear.trs()
        front.attach(breech)
        hinge.attach(rear)
        rear.trs({
            translation = [rest.translation[0] - pivot[0], rest.translation[1] - pivot[1], rest.translation[2] - pivot[2]]
            rotation_quat_xyzw = rest.rotation_quat_xyzw
            scale = rest.scale
        })
        let rear_grab = Grabbable {}
        rear.attach(rear_grab)
    })
    let chamber = T.position(0.00031, 0.12039, 0.0) {
        name = label + "_chamber"
        Material(chambered_rocket_material)
        initial_round
    }
    let reload_zone = Zone.cube([0.055, 0.055, 0.09]).enable_events() { name = label + "_reload_zone" }
    on(reload_zone, "ZoneEntered", fn(event) {
        if state.round || hinge.axis_angle().angle < Math.pi / 4.0 { return }
        if can_reload && !can_reload() { return }
        let round = claim_sta_rocket_ammo(event)
        if !round { return }
        state.round = round
        chamber.attach(round)
        round.trs({ translation = [0, 0, 0] rotation_quat_xyzw = [0, 0, 0, 1] scale = [1, 1, 1] })
        if on_ammo { on_ammo(1) }
    })
    fn closed() {
        return hinge.axis_angle().angle <= Math.pi / 90.0
    }
    let muzzle = T.position(0.00031, 0.12039, -0.18366) { name = label + "_muzzle" }
    let events = T { name = label + "_events" }
    // Animation time is in beats at the default two beats per second.
    let launch_sequence = Animation.paused().length(0.2) {
        name = label + "_launch_sequence"
        Keyframe.at(0.0) {}
        Keyframe.at(0.2) { emit_data(events, "Launch") }
    }
    let root = T {
        name = label + "_device"
        Raycastable.disabled()
        muzzle
        chamber
        T.position(0.0, 0.12039, 0.1776) { reload_zone }
        events
        launch_sequence
    }
    on(events, "DataEvent", fn(event) {
        if event == "Launch" && state.pending {
            state.pending = false
            if !state.round || !closed() || (can_fire && !can_fire(state.user)) { return }
            let round = state.round
            state.round = null
            launch_rocket(round, { pose = muzzle.world_trs() source_tool = root source_user = state.user impact = impact on_hit = on_hit })
            if on_ammo { on_ammo(0) }
            if options["on_launch"] != null {
                options.on_launch({ pose = muzzle.world_trs() source_tool = root source_user = state.user })
            }
        }
    })
    fn use_start(context) {
        if can_fire && !can_fire(context.user) { return false }
        if state.trigger_held { return false }
        state.trigger_held = true
        if state.pending || !state.round || !closed() { return false }
        state.pending = true
        state.user = context.user
        launch_sequence.play()
        return true
    }
    fn use_end() { state.trigger_held = false }
    fn apply_ammo(amount, returned) {
        if returned {
            state.pending = false
            use_end()
            launch_sequence.pause()
            hinge.set_angle(0.0)
        }
        if amount > 0 && !state.round {
            let round = sta_rocket_body(label + "_initial_round")
            chamber.attach(round)
            state.round = round
        }
        if amount == 0 && state.round {
            state.round.remove_subtree()
            state.round = null
        }
    }
    return { root = root model = model muzzle = muzzle hinge = hinge reload_zone = reload_zone
        has_ammo = fn() { return state.round != null } apply_ammo = apply_ammo
        animation = launch_sequence use_start = use_start use_end = use_end network = network }
}
