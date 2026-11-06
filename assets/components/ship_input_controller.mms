// One persistent controller; vehicle bindings exist only after first use and
// are bypassed natively while no driver needs them. Weapon input stays separate.
export fn ship_input_controller(patrol_enabled) {
    let root=T {name="ship_input_controller"}
    let state={driver=null frame_binding=null input_bindings=null}
    fn driver() {
        if state.driver {
            // Validate only an occupied vehicle; never dereference a stale handle.
            for current in query_all("#pickup_truck") {
                if current==state.driver {return current}
            }
            state.driver=null
        }
        return null
    }
    fn bypass_inputs(value) {
        if state.input_bindings {
            for binding in state.input_bindings {binding.bypass(value)}
        }
    }
    fn retire_idle() {
        if !state.driver {
            bypass_inputs(true)
            if state.frame_binding && patrol_enabled!=true {state.frame_binding.bypass(true)}
        }
    }
    fn forward_vehicle(kind,event) {
        let current=driver()
        if current {current.ship_input(kind,event)}
        retire_idle()
    }
    fn forward_tools(kind,event) {
        // Only Space is used by these weapons on desktop.
        if (kind=="KeyDown" || kind=="KeyUp") && event.code!="Space" {return}
        for index in range(3) {
            let grip=query("#ship_rack_pistol_"+index+"_grip")
            if grip {grip.ship_input(kind,event)}
        }
    }
    on(world,"KeyDown",fn(event) {forward_tools("KeyDown",event)})
    on(world,"KeyUp",fn(event) {forward_tools("KeyUp",event)})
    on(world,"XrButtonDown",fn(event) {forward_tools("XrButtonDown",event)})
    on(world,"XrButtonUp",fn(event) {forward_tools("XrButtonUp",event)})
    fn enable_frames() {
        if state.frame_binding {state.frame_binding.bypass(false)}
        else {
            let frames=on(world,"FrameTick",fn(event) {
                let current=driver()
                if current {current.refresh_heading()}
                if patrol_enabled==true {
                    let patrol=query("#ship_lumae_motorcycle_patrol")
                    if patrol {patrol.advance(event.dt_sec)}
                }
                retire_idle()
            })
            state.frame_binding=frames
        }
    }
    root.set_driver=fn(truck) {
        state.driver=truck
        if state.input_bindings {bypass_inputs(false)}
        else {
            let down=on(world,"KeyDown",fn(event) {forward_vehicle("KeyDown",event)})
            let up=on(world,"KeyUp",fn(event) {forward_vehicle("KeyUp",event)})
            let axis=on(world,"XrAxisChanged",fn(event) {forward_vehicle("XrAxisChanged",event)})
            state.input_bindings=[down,up,axis]
        }
        enable_frames()
    }
    root.clear_driver=fn(truck) {
        if state.driver==truck {state.driver=null retire_idle()}
    }
    if patrol_enabled==true {enable_frames()}
    return root
}
