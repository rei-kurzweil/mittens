// Animation time is relative to the first keyframe. The clip stays idle
// visually and runs only after the last round is fired. An empty weapon
// respawns a minute later; while someone holds it, the shared grabbable
// holds the restock until it is let go.
let RESTOCK_SECONDS = 60.0

export fn weapon_restock(name, beats_per_second, restore) {
    let state = { pending = false }
    let delay = RESTOCK_SECONDS * beats_per_second
    let events = T {}
    let timer = Animation.paused().length(delay) {
        name = name + "_restock_timer"
        Keyframe.at(0.0) {}
        Keyframe.at(delay) { emit_data(events, "RestockDue") }
    }
    on(events, "DataEvent", fn(event) {
        if event == "RestockDue" && state.pending {
            state.pending = false
            restore()
        }
    })
    fn start() {
        if state.pending { return }
        state.pending = true
        timer.play()
    }
    fn cancel() {
        if !state.pending { return }
        state.pending = false
        timer.pause()
    }
    let root = T { timer events }
    return { root = root start = start cancel = cancel }
}
