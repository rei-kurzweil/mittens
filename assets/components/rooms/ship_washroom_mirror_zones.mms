// Non-solid sensors before the room and at the far end of the hallway.
fn is_player(event, player_probe) {
    if player_probe != null { return event.other_zone == player_probe }
    let target = event.movement_target
    return target==query("#ship_player_root") || target==query("#ship_xr_free_rig")
        || target==query("#ship_desktop_camera_rig")
}
fn set_mirror_enabled(selector, value) {
    let mirror = query(selector)
    if mirror { mirror.set_enabled(value) }
}
export fn washroom_mirror_zones(options) {
    let inside = Zone.cube([0.9,1.4,0.25]).enable_events() { name = "ship_washroom_mirror_enable_zone" }
    let outside = Zone.cube([0.9,1.4,0.25]).enable_events() { name = "ship_washroom_mirror_disable_zone" }
    let entrance = options.entrance
    let exit = options.exit
    on(inside, "ZoneEntered", fn(event) {
        if is_player(event, options["player_probe"]) { set_mirror_enabled(options.mirror, true) }
    })
    on(outside, "ZoneEntered", fn(event) {
        if is_player(event, options["player_probe"]) { set_mirror_enabled(options.mirror, false) }
    })
    return T {
        name = "ship_washroom_mirror_zones"
        T.position(entrance[0], entrance[1], entrance[2]) { inside }
        T.position(exit[0], exit[1], exit[2]).rotation(0.0, options.exit_yaw, 0.0) { outside }
    }
}

export fn ship_washroom_mirror_zones() {
    return washroom_mirror_zones({
        mirror = "#ship_womens_washroom_mirror"
        entrance = [-18.0, 6.45, 12.8]
        exit = [-18.0, 6.45, 14.1]
        exit_yaw = 0.0
    })
}
