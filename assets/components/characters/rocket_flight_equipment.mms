// One loose STA rocket must be attached directly to each thigh drop zone.
export fn require_leg_rockets_for_flight(avatar_root, gamepad) {
    let state = { enabled = false moving = false mode = null left = null right = null departures = null }
    gamepad.set_flight_enabled(false)

    fn equipped_rocket(selector) {
        for item in avatar_root.query_all(selector) {
            if item.claim_sta_round && item.query("#sta_rocket_ammo_zone") { return item }
        }
        return null
    }

    fn update_round(previous, next, mode) {
        if previous && previous.is_alive() && (previous != next || !mode) { previous.deactivate() }
        if next && next.is_alive() && mode && (previous != next || state.mode != mode) {
            next.activate(mode)
        }
    }

    fn update_exhaust(left, right) {
        let mode = null
        if left && right && gamepad.is_alive() && gamepad.flying() {
            mode = "hover"
            if state.moving { mode = "thrust" }
        }
        update_round(state.left, left, mode)
        update_round(state.right, right, mode)
        state.left = left
        state.right = right
        state.mode = mode
    }

    fn refresh(event) {
        if !avatar_root.is_alive() || !gamepad.is_alive() {
            update_exhaust(null, null)
            state.departures.remove()
            return
        }
        let left = equipped_rocket("#equipment_left_thigh > transform")
        let right = equipped_rocket("#equipment_right_thigh > transform")
        let enabled = left != null && right != null
        if enabled != state.enabled {
            state.enabled = enabled
            gamepad.set_flight_enabled(enabled)
            state.departures.bypass(!enabled)
        }
        if enabled { update_exhaust(left, right) }
        else { update_exhaust(null, null) }
    }

    // A pickup or deletion can move a rocket out of the avatar's signal scope.
    state.departures = on_global("ParentChanged", refresh)
    state.departures.bypass(true)
    on(avatar_root, "ParentChanged", fn(event) { if !state.enabled { refresh(event) } })
    on(gamepad, "XrGamepadLocomotion", fn(event) {
        state.moving = event.active
        update_exhaust(state.left, state.right)
    })
    on(gamepad, "XrGamepadFlightChanged", fn(event) {
        update_exhaust(state.left, state.right)
    })
    refresh(null)
}
