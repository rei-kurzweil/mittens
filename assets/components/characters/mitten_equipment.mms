fn ground_distance(a, b) {
    let dx = a[0] - b[0]
    let dz = a[2] - b[2]
    return Math.sqrt(dx * dx + dz * dz)
}

export fn mitten_route(here, points) {
    let length = 0.0
    for point in points {
        length = length + ground_distance(here, point)
        here = point
    }
    return { count = len(points) point = fn(index) { return points[index] } length = length }
}

// Stocked stands and loose pistols share the same inventory after a grab.
// paths is an optional path finder with plan(here, goal) and
// nearest(here, goals); without one, routes are straight lines.
export fn mitten_armory(stations, paths) {
    let root = T { name = "mitten_loose_weapons" }
    let loose = { count = 0 items = {} }

    fn take(weapon) {
        let pose = weapon.world_trs()
        root.attach(weapon)
        weapon.world_trs(pose)
        for station in stations {
            for stocked in station.weapons { if stocked == weapon { return } }
        }
        for i in range(loose.count) { if loose.items["" + i] == weapon { return } }
        loose.items["" + loose.count] = weapon
        loose.count = loose.count + 1
    }

    fn loaded(weapon) { return weapon.rounds() > 0 && !weapon.player_holds() }

    fn path(here, goal) {
        if paths { return paths.plan(here, goal) }
        return mitten_route(here, [goal])
    }

    // Where to walk for a loaded weapon at this height: its stand's approach
    // point when the grip is near one, with the rest of the reach as cost.
    fn candidate(weapon, station, here) {
        if !loaded(weapon) { return null }
        let grip = weapon.grip.world_trs().translation
        if Math.abs(grip[1] - here[1]) > 1.6 { return null }
        let goal = [grip[0], here[1], grip[2]]
        if station && ground_distance(station, grip) < 1.6 { goal = station }
        return { weapon = weapon grip = grip point = goal cost = ground_distance(goal, grip) }
    }

    fn pickup(next, route) {
        return { weapon = next.weapon route = route grip = next.grip score = route.length + next.cost }
    }

    // One search for every candidate. Equal scores keep the first candidate.
    fn nearest(here) {
        let candidates = []
        for station in stations {
            for weapon in station.weapons {
                let next = candidate(weapon, station.approach, here)
                if next { candidates.push(next) }
            }
        }
        for i in range(loose.count) {
            let next = candidate(loose.items["" + i], null, here)
            if next { candidates.push(next) }
        }
        if len(candidates) == 0 { return null }
        if paths {
            let goals = []
            for next in candidates { goals.push({ point = next.point cost = next.cost }) }
            let route = paths.nearest(here, goals)
            if !route { return null }
            return pickup(candidates[route.index], route)
        }
        let best = null
        for next in candidates {
            let choice = pickup(next, mitten_route(here, [next.point]))
            if !best || choice.score < best.score { best = choice }
        }
        return best
    }

    fn available(pickup) {
        if !loaded(pickup.weapon) { return false }
        let grip = pickup.weapon.grip.world_trs().translation
        return ground_distance(grip, pickup.grip) < 0.55 && Math.abs(grip[1] - pickup.grip[1]) < 0.45
    }

    fn forward(kind, event) {
        for i in range(loose.count) { loose.items["" + i].grip.ship_input(kind, event) }
    }

    fn restore(weapon, pose) {
        root.attach(weapon)
        weapon.world_trs(pose)
        weapon.reset_grip()
    }

    return { root = root nearest = nearest available = available path = path take = take restore = restore forward = forward }
}

export fn mitten_weapon_slot(mount, armory, can_fire, on_shot, on_disarmed, upright_grip) {
    let state = { weapon = null binding = null target = null mode = "semi" rack_rest = null }
    fn disarm() {
        if !state.weapon { return }
        let weapon = state.weapon
        state.weapon = null
        if state.binding { state.binding.remove() state.binding = null }
        if state.target { state.target.remove_subtree() state.target = null }
        weapon.fire_end()
        weapon.set_scripted_fire(null, null)
    }
    fn equip(weapon, rack_gun) {
        state.weapon = weapon
        state.rack_rest = null
        if rack_gun { state.rack_rest = weapon.world_trs() }
        mount.attach(weapon)
        weapon.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
        weapon.reset_grip()
        if upright_grip { weapon.grip.set_rotation_quat([0.0, 0.0, 0.0, 1.0], "local") }
        weapon.set_mode(state.mode)
        weapon.set_scripted_fire(can_fire, on_shot)
        if armory {
            let pickup_target = T.scale(0.15, 0.17, 0.13) {
                R.cube() {
                    name = "mitten_weapon_pickup"
                    Opacity.opacity(0.0).multiple_layers()
                    Raycastable.drag_only() { interaction_priority(185.0) }
                }
            }
            weapon.grip.attach(pickup_target)
            state.target = pickup_target
            state.binding = on(world, "PointerGrabbed", fn(event) {
                if event.target != weapon.grip || state.weapon != weapon { return }
                armory.take(weapon)
                disarm()
                weapon.player_grab(event.pointer)
                on_disarmed()
            })
        }
    }
    fn discard(floor_y) {
        let weapon = state.weapon
        if !weapon { return }
        armory.take(weapon)
        let p = weapon.world_trs().translation
        armory.restore(weapon, {
            translation = [p[0], floor_y + 0.12, p[2]]
            rotation_quat_xyzw = [0.0, 0.0, 0.0, 1.0]
            scale = [1.0, 1.0, 1.0]
        })
        disarm()
        on_disarmed()
    }
    fn dispose() {
        // A borrowed rack pistol must survive when Mitten's rig is removed.
        if armory && state.weapon && state.rack_rest { armory.restore(state.weapon, state.rack_rest) }
        disarm()
    }
    return {
        equip = equip dispose = dispose discard = discard
        ready = fn() { return state.weapon != null }
        has_ammo = fn() { return state.weapon && state.weapon.rounds() > 0 }
        fire_start = fn() { if state.weapon { return state.weapon.fire_start() } return false }
        fire_end = fn() { if state.weapon { state.weapon.fire_end() } }
        muzzle_pose = fn() { return state.weapon.muzzle_pose() }
        set_mode = fn(mode) { state.mode = mode if state.weapon { state.weapon.set_mode(mode) } }
    }
}
