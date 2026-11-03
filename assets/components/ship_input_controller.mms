// One persistent controller for this example. Streamed assets never register
// global input/timer callbacks; look up live instances only when needed.
export fn ship_input_controller(patrol_enabled) {
    let root=T {name="ship_input_controller"}
    fn forward(kind,event) {
        for truck in query_all("#pickup_truck") {
            if truck.drive_status().mounted {truck.ship_input(kind,event)}
        }
        for index in range(3) {
            let grip=query("#ship_rack_pistol_"+index+"_grip")
            if grip {grip.ship_input(kind,event)}
        }
    }
    on_global("KeyDown",fn(event) {forward("KeyDown",event)})
    on_global("KeyUp",fn(event) {forward("KeyUp",event)})
    on_global("XrAxisChanged",fn(event) {forward("XrAxisChanged",event)})
    on_global("XrButtonDown",fn(event) {forward("XrButtonDown",event)})
    on_global("XrButtonUp",fn(event) {forward("XrButtonUp",event)})
    on_global("FrameTick",fn(event) {
        for truck in query_all("#pickup_truck") {truck.refresh_heading()}
        if patrol_enabled != false {
            let patrol=query("#ship_lumae_motorcycle_patrol")
            if patrol {patrol.advance(event.dt_sec)}
        }
    })
    return root
}
