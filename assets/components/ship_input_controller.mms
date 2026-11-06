// One persistent controller. Idle frames never search the whole scene for trucks.
// Mount events supply the current driver; inputs still find live weapon grips.
export fn ship_input_controller(patrol_enabled) {
    let root=T {name="ship_input_controller"}
    let state={driver=null}
    root.set_driver=fn(truck) {state.driver=truck}
    root.clear_driver=fn(truck) {if state.driver==truck {state.driver=null}}
    fn driver() {
        if state.driver {
            // Only occupied trucks pay for validation. Compare live handles;
            // never dereference the retained handle after a streamed removal.
            for current in query_all("#pickup_truck") {
                if current==state.driver {return current}
            }
            state.driver=null
        }
        return null
    }
    fn forward(kind,event) {
        let current=driver()
        if current {current.ship_input(kind,event)}
        // Weapon grips do not consume analog axes. Avoid three global lookups
        // on every locomotion-stick update while walking through the ship.
        if kind!="XrAxisChanged" {
            for index in range(3) {
                let grip=query("#ship_rack_pistol_"+index+"_grip")
                if grip {grip.ship_input(kind,event)}
            }
        }
    }
    on_global("KeyDown",fn(event) {forward("KeyDown",event)})
    on_global("KeyUp",fn(event) {forward("KeyUp",event)})
    on_global("XrAxisChanged",fn(event) {forward("XrAxisChanged",event)})
    on_global("XrButtonDown",fn(event) {forward("XrButtonDown",event)})
    on_global("XrButtonUp",fn(event) {forward("XrButtonUp",event)})
    on_global("FrameTick",fn(event) {
        let current=driver()
        if current {current.refresh_heading()}
        if patrol_enabled==true {
            let patrol=query("#ship_lumae_motorcycle_patrol")
            if patrol {patrol.advance(event.dt_sec)}
        }
    })
    return root
}
