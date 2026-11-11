// Triggering starts the launch sequence. The visual-time event at 100 ms is
// where the rear flash will finish and the independent rocket will be launched.
// on_launch receives the muzzle's world pose and the user captured at trigger.
import { rocket } from "./projectiles/rocket.mms"

export fn sta_rocket_launcher(options) {
    let label = "sta_rocket_launcher"
    if options["name"] != null { label = options.name }
    let state = { trigger_held = false pending = false user = null }
    let muzzle = T { name = label + "_muzzle" }
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
            rocket({ pose = muzzle.world_trs() source_tool = root source_user = state.user })
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
    return { root = root muzzle = muzzle animation = launch_sequence use_start = use_start use_end = use_end }
}
