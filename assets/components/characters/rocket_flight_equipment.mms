// One loose STA rocket must be attached directly to each thigh drop zone.
export fn require_leg_rockets_for_flight(avatar_root, gamepad) {
    let state = { enabled = false departures = null }
    gamepad.set_flight_enabled(false)

    fn has_rocket(selector) {
        for item in avatar_root.query_all(selector) {
            if item.claim_sta_round && item.query("#sta_rocket_ammo_zone") { return true }
        }
        return false
    }

    fn refresh(event) {
        let enabled = has_rocket("#equipment_left_thigh > transform") && has_rocket("#equipment_right_thigh > transform")
        if enabled != state.enabled {
            state.enabled = enabled
            gamepad.set_flight_enabled(enabled)
            state.departures.bypass(!enabled)
        }
    }

    // A pickup or deletion can move a rocket out of the avatar's signal scope.
    // Listen outside that scope only while both rockets are equipped.
    state.departures = on_global("ParentChanged", refresh)
    state.departures.bypass(true)
    on(avatar_root, "ParentChanged", fn(event) { if !state.enabled { refresh(event) } })
    refresh(null)
}
