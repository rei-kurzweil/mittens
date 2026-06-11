import { button } from "../assets/components/button.mms"


struct AppState {
    blinking_light_blocked: bool
}

let app_state = AppState {
    blinking_light_blocked: false
}

let observer_router = ObserverRouter { 
    blacklist = ["blinking_light"]
}
let router_root = T {
    name = "router_root"

}


// Buttons to toggle blacklist
let block_button = T.position(-1.0, 1.0, 0.0) {
    button("Block")
}
let allow_button = T.position(1.0, 1.0, 0.0) {
    button("Allow")
}

on(block_button, "Click", fn() {
    app_state.blinking_light_blocked = true
})

on(allow_button, "Click", fn() {
    app_state.blinking_light_blocked = false
})

block_button
allow_button


I {
    T.position(0,0,-2) {
        Camera3D {}
    }
}