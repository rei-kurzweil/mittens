// A seat holds its rider still: walking is off while mounted and restored to
// its previous state on dismount, including a forced dismount (MountEnded
// still fires). The vehicle's script owns this; the attachment system only
// makes and removes the connection.
export fn hold_rider_walking(mountable) {
    let state = { previous = null }
    on(mountable, "MountStarted", fn(event) {
        let rider = event.rider
        if !rider.is_alive() { return }
        state.previous = rider.locomotion_enabled()
        rider.set_locomotion_enabled(false)
    })
    on(mountable, "MountEnded", fn(event) {
        let rider = event.rider
        if state.previous != null && rider.is_alive() { rider.set_locomotion_enabled(state.previous) }
        state.previous = null
    })
}
