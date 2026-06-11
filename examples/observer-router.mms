import { button } from "../assets/components/button.mms"

RendererSettings {
    window_size(1440, 960)
}

BGC.rgba(0.05, 0.06, 0.09, 1.0)
AL.rgb(0.20, 0.20, 0.24)

EmissivePass {}
Bloom {
    intensity(1.0)
    emissive_scale(1.35)
    radius_ndc(0.02)
}

I.speed(3.0) {
    InputTransformMode.forward_z() {
        fps_rotation()
        roll_axis_y()
    }

    T.position(0.0, 1.4, 4.6) {
        C3D {}
        Pointer {}
    }
}

let observer_router = ObserverRouter {}
let light_a_glow = Emissive.off()
let light_b_glow = Emissive.off()
let status_text = Text {
    "Pulse buttons emit DataEvent on router_root.\nBlock buttons blacklist named handlers."
    C.rgba(0.95, 0.97, 1.0, 1.0)
    TextureFiltering.linear()
    Emissive.on()
}

let router_root = T {
    name = "router_root"
    observer_router
}

let pulse_on_button = T.position(-1.85, 1.65, -1.5) { button("Pulse On") }
let pulse_off_button = T.position(-0.45, 1.65, -1.5) { button("Pulse Off") }

let block_a_button = T.position(1.15, 1.65, -1.5) { button("Block A") }
let allow_a_button = T.position(2.55, 1.65, -1.5) { button("Allow A") }
let block_b_button = T.position(1.15, 1.00, -1.5) { button("Block B") }
let allow_b_button = T.position(2.55, 1.00, -1.5) { button("Allow B") }

let light_a = T.position(-1.6, 0.35, -2.3).scale(0.55, 0.55, 0.55) {
    name = "light_a"
    R.cube() {
        C.rgba(1.0, 0.26, 0.20, 1.0)
        light_a_glow
    }
}

let light_b = T.position(1.6, 0.35, -2.3).scale(0.55, 0.55, 0.55) {
    name = "light_b"
    R.cube() {
        C.rgba(0.22, 0.74, 1.0, 1.0)
        light_b_glow
    }
}

T.position(0.0, -0.55, -2.4).scale(5.5, 0.12, 2.4) {
    R.cube() {
        C.rgba(0.11, 0.12, 0.16, 1.0)
    }
}

T.position(-1.6, 1.2, -2.3).rotation(0.0, 0.0, 0.7) {
    name = "light_a_arm"
    Animation.looping().length(2.0) {
        Keyframe.at(0.0) {
            Action.update_transform("#light_a_arm", [-1.6, 1.2, -2.3], [0.0, 0.0, 0.7], [1.0, 1.0, 1.0])
        }
        Keyframe.at(1.0) {
            Action.update_transform("#light_a_arm", [-1.6, 1.2, -2.3], [0.0, 0.0, -0.7], [1.0, 1.0, 1.0])
        }
        Keyframe.at(2.0) {
            Action.update_transform("#light_a_arm", [-1.6, 1.2, -2.3], [0.0, 0.0, 0.7], [1.0, 1.0, 1.0])
        }
    }
    T.position(0.0, -0.32, 0.0).scale(0.06, 0.6, 0.06) {
        R.cube() {
            C.rgba(0.35, 0.30, 0.30, 1.0)
        }
    }
}

T.position(1.6, 1.2, -2.3).rotation(0.0, 0.0, -0.7) {
    name = "light_b_arm"
    Animation.looping().length(2.0) {
        Keyframe.at(0.0) {
            Action.update_transform("#light_b_arm", [1.6, 1.2, -2.3], [0.0, 0.0, -0.7], [1.0, 1.0, 1.0])
        }
        Keyframe.at(1.0) {
            Action.update_transform("#light_b_arm", [1.6, 1.2, -2.3], [0.0, 0.0, 0.7], [1.0, 1.0, 1.0])
        }
        Keyframe.at(2.0) {
            Action.update_transform("#light_b_arm", [1.6, 1.2, -2.3], [0.0, 0.0, -0.7], [1.0, 1.0, 1.0])
        }
    }
    T.position(0.0, -0.32, 0.0).scale(0.06, 0.6, 0.06) {
        R.cube() {
            C.rgba(0.28, 0.34, 0.38, 1.0)
        }
    }
}

T.position(-2.0, 0.7, -2.8) {
    PL {
        intensity(2.0)
        distance(12.0)
        C.rgba(1.0, 0.86, 0.74, 1.0)
    }
}

T.position(2.0, 0.8, -2.1) {
    PL {
        intensity(2.0)
        distance(12.0)
        C.rgba(0.70, 0.84, 1.0, 1.0)
    }
}

T.position(-2.0, 2.35, -1.45).scale(0.05, 0.05, 1.0) {
    status_text
}

on(router_root, "DataEvent", "light_a", fn(event) {
    if event == "pulse_on" {
        light_a_glow.set_intensity(2.6)
    } else if event == "pulse_off" {
        light_a_glow.off()
    }
})

on(router_root, "DataEvent", "light_b", fn(event) {
    if event == "pulse_on" {
        light_b_glow.set_intensity(2.2)
    } else if event == "pulse_off" {
        light_b_glow.off()
    }
})

on(pulse_on_button, "Click", fn(event) {
    emit_data(router_root, "pulse_on")
    status_text.set_text("pulse_on emitted on router_root")
})

on(pulse_off_button, "Click", fn(event) {
    emit_data(router_root, "pulse_off")
    status_text.set_text("pulse_off emitted on router_root")
})

on(block_a_button, "Click", fn(event) {
    observer_router.block("light_a")
    status_text.set_text("blocked handler: light_a")
})

on(allow_a_button, "Click", fn(event) {
    observer_router.allow("light_a")
    status_text.set_text("allowed handler: light_a")
})

on(block_b_button, "Click", fn(event) {
    observer_router.block("light_b")
    status_text.set_text("blocked handler: light_b")
})

on(allow_b_button, "Click", fn(event) {
    observer_router.allow("light_b")
    status_text.set_text("allowed handler: light_b")
})

router_root
pulse_on_button
pulse_off_button
block_a_button
allow_a_button
block_b_button
allow_b_button
light_a
light_b
