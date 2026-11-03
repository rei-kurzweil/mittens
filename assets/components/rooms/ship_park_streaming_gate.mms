// Two separate, non-solid volumes give the doorway a direction and a deadband.
// The parked shops and both vehicle groups remain cold until the player exits.
fn is_player(target) {
    return target == query("#ship_player_root") || target == query("#ship_xr_free_rig")
        || target == query("#ship_desktop_camera_rig")
}
fn set_park_expanded(value) {
    for label in ["ship_convenience_shop","ship_liquor_shop","ship_motorhomes","ship_driving_vehicles"] {
        let section = query("#"+label)
        if section { section.set_expanded(value) }
    }
}
export fn ship_park_streaming_gate() {
    let inside = Zone.cube([0.30,1.4,1.0]).enable_events() { name="ship_park_unload_zone" }
    let outside = Zone.cube([0.30,1.4,1.0]).enable_events() { name="ship_park_load_zone" }
    on(inside,"ZoneEntered",fn(event) {
        if is_player(event.movement_target) { set_park_expanded(false) }
    })
    on(outside,"ZoneEntered",fn(event) {
        if is_player(event.movement_target) { set_park_expanded(true) }
    })
    return T {
        name="ship_park_streaming_gate"
        T.position(9.8,6.45,15.0) { inside }
        T.position(12.2,6.45,15.0) { outside }
    }
}
