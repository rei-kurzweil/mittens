// vr-input-gamepad scene
//
// VR-only input dashboard for InputVrGamepad.
// Shows A/B, X/Y, left/right sticks, and trigger/grip readouts.
//
// To run:
//   cargo run --release --example xr-input-gamepad

RendererSettings {
    window_size(640, 480)
}

BGC {
    C.rgba(0.90, 0.93, 0.98, 1.0)
}

AL.rgb(0.20, 0.20, 0.24)

RenderGraph {
    EmissivePass {
        BlurPass {
            radius_ndc(0.06)
            half_res(true)
        }
    }
    Bloom {
        intensity(0.90)
        radius_ndc(0.06)
        emissive_scale(1.2)
        half_res(true)
    }
}

T.position(0.15, -0.45, 1.0) {
    DL {
        intensity(1.0)
        color(1.0, 0.98, 0.95)
    }
}

T.position(-0.85, 0.55, 0.35) {
    DL {
        intensity(0.95)
        color(0.90, 0.94, 1.0)
    }
}

T.position(0.75, 0.35, -0.75) {
    DL {
        intensity(0.85)
        color(1.0, 1.0, 1.0)
    }
}

ED {
    T.position(0.0, -0.75, 0.0).scale(120.0, 0.12, 120.0) {
        Collision.static() {
            CollisionShape.cube([60.0, 0.06, 60.0])
        }
        R.cube() { C.rgba(0.75, 0.75, 0.75, 1.0) }
    }
}

let a_glow = Emissive.off()
let b_glow = Emissive.off()
let x_glow = Emissive.off()
let y_glow = Emissive.off()

let left_stick_text = Text {
    "Left Stick: (0.00, 0.00)"
    C.rgba(0.07, 0.08, 0.11, 1.0)
    TextureFiltering.linear()
}

let right_stick_text = Text {
    "Right Stick: (0.00, 0.00)"
    C.rgba(0.07, 0.08, 0.11, 1.0)
    TextureFiltering.linear()
}

let trigger_text = Text {
    "Triggers L/R: 0.00 / 0.00"
    C.rgba(0.07, 0.08, 0.11, 1.0)
    TextureFiltering.linear()
}

let grip_text = Text {
    "Grips L/R: 0.00 / 0.00"
    C.rgba(0.07, 0.08, 0.11, 1.0)
    TextureFiltering.linear()
}

let event_text = Text {
    "Press XR buttons or move sticks"
    C.rgba(0.07, 0.08, 0.11, 1.0)
    TextureFiltering.linear()
    Emissive.on()
}

let left_trigger_value = Text {
    "LT: 0.00"
    C.rgba(0.07, 0.08, 0.11, 1.0)
    TextureFiltering.linear()
}

let right_trigger_value = Text {
    "RT: 0.00"
    C.rgba(0.07, 0.08, 0.11, 1.0)
    TextureFiltering.linear()
}

let left_grip_value = Text {
    "LG: 0.00"
    C.rgba(0.07, 0.08, 0.11, 1.0)
    TextureFiltering.linear()
}

let right_grip_value = Text {
    "RG: 0.00"
    C.rgba(0.07, 0.08, 0.11, 1.0)
    TextureFiltering.linear()
}

let left_stick_dot = T.position(0.0, 0.0, 0.03) {
    T.scale(0.05, 0.05, 1.0) {
        R.circle2d() {
            C.rgba(0.18, 0.73, 1.0, 1.0)
            EM.on()
        }
    }
}

let right_stick_dot = T.position(0.0, 0.0, 0.03) {
    T.scale(0.05, 0.05, 1.0) {
        R.circle2d() {
            C.rgba(1.0, 0.46, 0.26, 1.0)
            EM.on()
        }
    }
}

let xr_gamepad = InputVrGamepad {
    locomotion()
    speed(1.5)
}

T {
    InputVR.on() {
        xr_gamepad
        T {
            name = "xr_pose"
            AVC {
                head_bone("J_Bip_C_Head")
                camera_bone("J_Bip_C_Head")
                left_hand_bone("J_Bip_L_Hand")
                right_hand_bone("J_Bip_R_Hand")

                initial_yaw(3.14159)
                ik_debug()

                left_arm_pole_direction([  1, -0.35, -1])
                right_arm_pole_direction([-1, -0.35, -1])

                hand_rotation_smoothing(220.0)
                hand_grip_rotation_left([-0.6408564, 0.29883623, 0.29883623, 0.6408564])
                hand_grip_rotation_right([-0.6408564, -0.29883623, -0.29883623, 0.6408564])

                T {
                    GLTF.new("assets/models/bisket.11.0.glb") {
                        EM.on()
                        PoseCapture { label("Bisket") }
                    }
                }

                T.position(0.0, 0.08, 0.12) {
                    name = "xr_camera_wrapper"
                    Collision.kinematic() {
                        CollisionShape.sphere(0.18)
                        KineticResponse.slide() {}
                    }
                    CXR { Pointer {} }
                }

                VrHand.new(true, Left,  Grip) { T { Pointer {} } }
                VrHand.new(true, Right, Grip) { T { Pointer {} } }
            }

            T.position(0.0, 0.2, -2.0) {
                T.scale(1.85, 1.15, 0.08) {
                    R.cube() {
                        C.rgba(0.97, 0.98, 1.0, 0.96)
                    }
                }

                T.position(0.0, 0.43, 0.05).scale(0.055, 0.055, 1.0) {
                    event_text
                }

                T.position(-0.58, 0.05, 0.05) {
                    T.scale(0.18, 0.18, 0.08) {
                        R.cube() {
                            C.rgba(0.22, 0.52, 0.92, 1.0)
                            a_glow
                        }
                    }
                    T.position(0.0, -0.20, 0.05).scale(0.05, 0.05, 1.0) {
                        Text { "A" C.rgba(0.07, 0.08, 0.11, 1.0) TextureFiltering.linear() }
                    }
                }

                T.position(-0.28, 0.05, 0.05) {
                    T.scale(0.18, 0.18, 0.08) {
                        R.cube() {
                            C.rgba(0.22, 0.76, 0.46, 1.0)
                            b_glow
                        }
                    }
                    T.position(0.0, -0.20, 0.05).scale(0.05, 0.05, 1.0) {
                        Text { "B" C.rgba(0.07, 0.08, 0.11, 1.0) TextureFiltering.linear() }
                    }
                }

                T.position(0.28, 0.05, 0.05) {
                    T.scale(0.18, 0.18, 0.08) {
                        R.cube() {
                            C.rgba(0.95, 0.73, 0.16, 0.38)
                            y_glow
                        }
                    }
                    T.position(0.0, -0.20, 0.05).scale(0.05, 0.05, 1.0) {
                        Text { "Y" C.rgba(0.07, 0.08, 0.11, 1.0) TextureFiltering.linear() }
                    }
                }

                T.position(0.58, 0.05, 0.05) {
                    T.scale(0.18, 0.18, 0.08) {
                        R.cube() {
                            C.rgba(0.88, 0.28, 0.82, 0.38)
                            x_glow
                        }
                    }
                    T.position(0.0, -0.20, 0.05).scale(0.05, 0.05, 1.0) {
                        Text { "X" C.rgba(0.07, 0.08, 0.11, 1.0) TextureFiltering.linear() }
                    }
                }

                T.position(-0.44, -0.38, 0.05) {
                    T.scale(0.19, 0.19, 1.0) {
                        R.circle2d() {
                            C.rgba(0.77, 0.82, 0.90, 1.0)
                        }
                    }
                    T.scale(0.15, 0.15, 1.0) {
                        R.circle2d() {
                            C.rgba(0.16, 0.18, 0.24, 1.0)
                        }
                    }
                    left_stick_dot
                    T.position(0.0, -0.26, 0.05).scale(0.05, 0.05, 1.0) {
                        left_stick_text
                    }
                }

                T.position(0.44, -0.38, 0.05) {
                    T.scale(0.19, 0.19, 1.0) {
                        R.circle2d() {
                            C.rgba(0.77, 0.82, 0.90, 1.0)
                        }
                    }
                    T.scale(0.15, 0.15, 1.0) {
                        R.circle2d() {
                            C.rgba(0.16, 0.18, 0.24, 1.0)
                        }
                    }
                    right_stick_dot
                    T.position(0.0, -0.26, 0.05).scale(0.05, 0.05, 1.0) {
                        right_stick_text
                    }
                }

                T.position(-0.52, -0.71, 0.05).scale(0.045, 0.045, 1.0) { left_trigger_value }
                T.position(-0.16, -0.71, 0.05).scale(0.045, 0.045, 1.0) { right_trigger_value }
                T.position( 0.16, -0.71, 0.05).scale(0.045, 0.045, 1.0) { left_grip_value }
                T.position( 0.52, -0.71, 0.05).scale(0.045, 0.045, 1.0) { right_grip_value }

                T.position(-0.34, -0.88, 0.05).scale(0.045, 0.045, 1.0) { trigger_text }
                T.position( 0.34, -0.88, 0.05).scale(0.045, 0.045, 1.0) { grip_text }
            }
        }
    }
}

on(xr_gamepad, "XrButtonDown", fn(event) {
    let control = event[1]
    if control == "ButtonA" {
        a_glow.set_intensity(2.8)
        event_text.set_text("A down")
    } else if control == "ButtonB" {
        b_glow.set_intensity(2.8)
        event_text.set_text("B down")
    } else if control == "ButtonX" {
        x_glow.set_intensity(2.4)
        event_text.set_text("X down")
    } else if control == "ButtonY" {
        y_glow.set_intensity(2.4)
        event_text.set_text("Y down")
    } else if control == "LeftTrigger" {
        event_text.set_text("Left trigger pressed")
    } else if control == "RightTrigger" {
        event_text.set_text("Right trigger pressed")
    } else if control == "LeftGrip" {
        event_text.set_text("Left grip pressed")
    } else if control == "RightGrip" {
        event_text.set_text("Right grip pressed")
    }
})

on(xr_gamepad, "XrButtonUp", fn(event) {
    let control = event[1]
    if control == "ButtonA" {
        a_glow.off()
        event_text.set_text("A up")
    } else if control == "ButtonB" {
        b_glow.off()
        event_text.set_text("B up")
    } else if control == "ButtonX" {
        x_glow.off()
        event_text.set_text("X up")
    } else if control == "ButtonY" {
        y_glow.off()
        event_text.set_text("Y up")
    }
})

on(xr_gamepad, "XrAxisChanged", fn(event) {
    let control = event[1]
    let value = event[2]
    if control == "LeftStick" {
        left_stick_dot.set_position(value[0] * 0.14, value[1] * 0.14, 0.03)
        left_stick_text.set_text("Left Stick: (" + value[0] + ", " + value[1] + ")")
    } else if control == "RightStick" {
        right_stick_dot.set_position(value[0] * 0.14, value[1] * 0.14, 0.03)
        right_stick_text.set_text("Right Stick: (" + value[0] + ", " + value[1] + ")")
    } else if control == "LeftTrigger" {
        left_trigger_value.set_text("LT: " + value[0])
        trigger_text.set_text("Left trigger moved: " + value[0])
    } else if control == "RightTrigger" {
        right_trigger_value.set_text("RT: " + value[0])
        trigger_text.set_text("Right trigger moved: " + value[0])
    } else if control == "LeftGrip" {
        left_grip_value.set_text("LG: " + value[0])
        grip_text.set_text("Left grip moved: " + value[0])
    } else if control == "RightGrip" {
        right_grip_value.set_text("RG: " + value[0])
        grip_text.set_text("Right grip moved: " + value[0])
    }
})

VR.on()
