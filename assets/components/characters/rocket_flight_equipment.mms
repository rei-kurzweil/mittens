// One loose STA rocket must be attached directly to each thigh drop zone.
export fn require_leg_rockets_for_flight(avatar_root, gamepad) {
    let state = { enabled = false thrust = false lit_left = null lit_right = null departures = null }
    gamepad.set_flight_enabled(false)

    fn equipped_rocket(selector) {
        for item in avatar_root.query_all(selector) {
            if item.claim_sta_round && item.query("#sta_rocket_ammo_zone") { return item }
        }
        return null
    }

    fn switch_rocket(previous, next) {
        if previous == next { return }
        if previous && previous.is_alive() { previous.deactivate() }
        if next { next.activate() }
    }

    fn refresh(event) {
        if !avatar_root.is_alive() || !gamepad.is_alive() {
            switch_rocket(state.lit_left, null)
            switch_rocket(state.lit_right, null)
            state.lit_left = null
            state.lit_right = null
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
            if !enabled { state.thrust = false }
        }
        if !enabled || !state.thrust {
            left = null
            right = null
        }
        switch_rocket(state.lit_left, left)
        switch_rocket(state.lit_right, right)
        state.lit_left = left
        state.lit_right = right
    }

    // A pickup or deletion can move a rocket out of the avatar's signal scope.
    // Listen outside that scope only while both rockets are equipped.
    state.departures = on_global("ParentChanged", refresh)
    state.departures.bypass(true)
    on(avatar_root, "ParentChanged", fn(event) { if !state.enabled { refresh(event) } })
    on(gamepad, "XrGamepadLocomotion", fn(event) {
        state.thrust = event.active && gamepad.flying()
        refresh(event)
    })
    refresh(null)
}
