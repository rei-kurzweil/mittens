Editor.translation_space("world").rotation_space("local") {
    ObserverRouter.blacklist(["editor_panel_refresh", "paint_system"]) {
        name = "editor_signal_observer_router"
    }
    Transform.position(2.0, -0.576999306678772, 0.6062082052230835).rotation_quat(-0.8383449912071228, 0.0, 0.0, 0.5451400876045227).scale(1.0, 1.0, 1.0) {
        name = "grid_1"
        Grid.spacing(1.0).size_x(16.0).size_z(16.0).enabled(true).selectable(true) {
            name = "grid_1_component"
        }
    }
    Transform.position(2.0, 0.35079091787338257, -0.002019137144088745).rotation_quat(0.7654410004615784, 0.0, 0.0, 0.6435061097145081).scale(1.0, 1.0, 1.0) {
        name = "grid_2"
        Grid.spacing(1.0).size_x(16.0).size_z(16.0).enabled(true).selectable(true) {
            name = "grid_2_component"
        }
    }
    Transform.position(2.0, 0.35079091787338257, -0.002019137144088745).rotation_quat(0.7654410004615784, 0.0, 0.0, 0.6435061097145081).scale(1.0, 1.0, 1.0) {
        name = "grid_3"
        Grid.spacing(1.0).size_x(16.0).size_z(16.0).enabled(true).selectable(true) {
            name = "grid_3_component"
        }
    }
    Transform.position(2.0, 0.35079091787338257, -0.002019137144088745).rotation_quat(0.7654410004615784, 0.0, 0.0, 0.6435061097145081).scale(1.0, 1.0, 1.0) {
        name = "grid_4"
        Grid.spacing(1.0).size_x(16.0).size_z(16.0).enabled(true).selectable(true) {
            name = "grid_4_component"
        }
    }
    Transform.position(1.100000023841858, -0.13640326261520386, -0.05431175231933594).rotation_quat(0.015358460135757923, 0.9758306741714478, 0.07296088337898254, 0.20541495084762573).scale(1.0, 1.0, 1.0) {
        name = "grid_5"
        Grid.spacing(1.0).size_x(16.0).size_z(16.0).enabled(true).selectable(true) {
            name = "grid_5_component"
        }
    }
}

Editor.translation_space("world").rotation_space("local") {
    InputXR.on() {
        Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            AvatarControl.head_bone("J_Bip_C_Head").body_yaw_threshold(0.7853981852531433).body_yaw_rate(3.0).left_hand_bone("J_Bip_L_Hand").right_hand_bone("J_Bip_R_Hand").hand_rotation_smoothing(220.0).camera_bone("J_Bip_C_Head") {
                ControllerXR.new(true, "Left", "Grip") {
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Pointer {
                            Raycast.event_driven().max_distance(200.0)
                        }
                    }
                }
                ControllerXR.new(true, "Right", "Grip") {
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Pointer {
                            Raycast.event_driven().max_distance(200.0)
                        }
                    }
                }
            }
            overlay {
                Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 0.05999999865889549, 0.11999999731779099) {
                    Renderable.cube() {
                        Color.rgba(0.0, 1.0, 1.0, 0.5)
                        Emissive.on()
                        Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                    }
                }
            }
            Transform.position(0.000000010490732726964325, -0.07999999821186066, 0.11999999731779099).rotation_quat(0.0, 1.0, 0.0, -0.00000004371138828673793).scale(1.0, 1.0, 1.0)
        }
    }
    ObserverRouter.blacklist(["editor_panel_refresh", "paint_system"]) {
        name = "editor_signal_observer_router"
    }
}

