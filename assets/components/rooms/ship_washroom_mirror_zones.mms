// Non-solid sensors separated by a deadband. Entering the inner one warms the
// mirror before the player reaches the room; the outer one freezes it on exit.
fn is_player(target) {
    return target==query("#ship_player_root") || target==query("#ship_xr_free_rig")
        || target==query("#ship_desktop_camera_rig")
}
fn set_mirror_enabled(value) {
    let mirror=query("#ship_womens_washroom_mirror")
    if mirror {mirror.set_enabled(value)}
}
export fn ship_washroom_mirror_zones() {
    let inside=Zone.cube([0.9,1.4,0.25]).enable_events() {name="ship_washroom_mirror_enable_zone"}
    let outside=Zone.cube([0.9,1.4,0.25]).enable_events() {name="ship_washroom_mirror_disable_zone"}
    on(inside,"ZoneEntered",fn(event) {
        if is_player(event.movement_target) {set_mirror_enabled(true)}
    })
    on(outside,"ZoneEntered",fn(event) {
        if is_player(event.movement_target) {set_mirror_enabled(false)}
    })
    return T {
        name="ship_washroom_mirror_zones"
        T.position(-18.0,6.45,12.8) {inside}
        T.position(-18.0,6.45,14.1) {outside}
    }
}
