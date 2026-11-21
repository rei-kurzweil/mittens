// Triggering starts the launch sequence. The visual-time event at 100 ms is
// where the rear flash will finish and the independent rocket will be launched.
// on_launch receives the muzzle's world pose and the user captured at trigger.
import { rocket, apply_rocket_impact } from "./projectiles/rocket.mms"
import { shared_impact } from "./network/shared_impact.mms"

export fn sta_rocket_launcher(options) {
    let label = "sta_rocket_launcher"
    if options["name"] != null { label = options.name }
    // A socket-enabled caller connects network to its peer channel group.
    let network = null
    if options["socket"] {
        let weapon_id = label
        if options["weapon_id"] { weapon_id = options.weapon_id }
        network = shared_impact({ socket = options.socket name = weapon_id apply = apply_rocket_impact })
    }
    fn impact(point, amount) {
        if network { network.hit(point, amount) }
        else { apply_rocket_impact(point, amount) }
    }
    let state = { trigger_held = false pending = false user = null }
    // The export faces -Z. Center its remaining X offset on the grip and
    // level its 4.54-degree barrel pitch to match the device's muzzle axis.
    let model = T.position(0.191245, 0, 0).rotation(0.079284, 0, 0) {
        GLTF.new("assets/models/STA_launcher.glb") { name = label }
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
        events
        launch_sequence
    }
    on(events, "DataEvent", fn(event) {
        if event == "Launch" && state.pending {
            state.pending = false
            rocket({ pose = muzzle.world_trs() source_tool = root source_user = state.user impact = impact })
            if options["on_launch"] != null {
                options.on_launch({ pose = muzzle.world_trs() source_tool = root source_user = state.user })
            }
        }
    })
    fn use_start(context) {
        if state.trigger_held { return false }
        state.trigger_held = true
        if state.pending { return false }
        state.pending = true
        state.user = context.user
        launch_sequence.play()
        return true
    }
    fn use_end() { state.trigger_held = false }
    return { root = root model = model muzzle = muzzle animation = launch_sequence use_start = use_start use_end = use_end network = network }
}
