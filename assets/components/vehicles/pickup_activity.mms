// Proximity activation using existing zone events and native motion state.
// Park only after support contact; no per-frame polling of sleeping trucks.
fn is_player(target) {
    return target==query("#ship_player_root") || target==query("#ship_xr_free_rig")
        || target==query("#ship_desktop_camera_rig")
}
export fn pickup_activity(truck,motion,gravity,gravity_enabled) {
    let state={mode="settling" avatar_near=false free_near=false desktop_near=false mounted=false commanded=false manual=null timer=null}
    let root=T {name="pickup_activity"}
    fn stop_timer() {
        if state.timer {
            let previous=state.timer
            state.timer=null
            previous.remove_subtree()
        }
    }
    fn park_if_supported() {
        if state.mode!="settling" {return}
        if gravity_enabled==false || motion.grounded() {
            state.mode="asleep"
            gravity.set_enabled(false)
            motion.reset()
            stop_timer()
        }
    }
    fn settle() {
        if state.mode=="asleep" || state.mode=="settling" {return}
        state.mode="settling"
        // Keep falling and contact correction alive until the truck has support.
        gravity.set_enabled(gravity_enabled!=false)
        if !state.timer {
            let timer=Animation.looping().length(0.5) {
                name="pickup_parking_check"
                Keyframe.at(0.0) {park_if_supported()}
            }
            state.timer=timer
            root.attach(timer)
        }
    }
    fn wake() {
        if state.mode=="active" {return}
        state.mode="active"
        stop_timer()
        gravity.set_enabled(gravity_enabled!=false)
    }
    fn nearby() {return state.avatar_near || state.free_near || state.desktop_near}
    fn set_near(target,value) {
        if target==query("#ship_player_root") {state.avatar_near=value}
        if target==query("#ship_xr_free_rig") {state.free_near=value}
        if target==query("#ship_desktop_camera_rig") {state.desktop_near=value}
    }
    fn refresh() {
        if state.mounted || state.commanded || state.manual==true {wake()}
        else if state.manual!=false && nearby() {wake()}
        else {settle()}
    }
    // Two sensors provide a deadband: wake inside, stay awake until outside.
    let outer=Zone.cube([7,4,8]).enable_events() {name="pickup_activity_outer"}
    let inner=Zone.cube([5,3,6]).enable_events() {name="pickup_activity_inner"}
    on(outer,"ZoneEntered",fn(event) {
        if is_player(event.movement_target) {set_near(event.movement_target,true)}
    })
    on(inner,"ZoneEntered",fn(event) {
        if is_player(event.movement_target) && state.manual!=false {wake()}
    })
    on(outer,"ZoneExited",fn(event) {
        if is_player(event.movement_target) {
            set_near(event.movement_target,false)
            refresh()
        }
    })
    let sensors=T.position(0,1,0) {outer inner}
    root.attach(sensors)
    truck.set_active=fn(value) {state.manual=value refresh()}
    truck.use_proximity_activation=fn() {state.manual=null refresh()}
    truck.activity_status=fn() {return {active=state.mode=="active" mode=state.mode nearby=nearby() mounted=state.mounted commanded=state.commanded}}
    truck.activity_mounted=fn(value) {state.mounted=value refresh()}
    truck.activity_commanded=fn(value) {state.commanded=value refresh()}
    // Initial settling also handles trucks spawned above their supporting deck.
    state.mode="initial"
    settle()
    return root
}
