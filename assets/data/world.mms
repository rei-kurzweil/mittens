RendererSettings.window_size(640.0, 480.0)

BackgroundColor {
    Color.rgba(1.0, 0.6499999761581421, 0.75, 1.0)
    overlay {
        Transform.position(0.3499999940395355, 1.0499999523162842, -0.3499999940395355).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 0.05999999865889549, 0.05999999865889549) {
            Renderable.cube() {
                Color.rgba(0.949999988079071, 0.20000000298023224, 0.4000000059604645, 1.0)
                Emissive.on()
                Raycastable.enabled()
                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
            }
        }
    }
}

AmbientLight.rgb(0.18000000715255737, 0.18000000715255737, 0.2199999988079071)

RenderGraph.on() {
    EmissivePass {}
    Bloom.enabled(true).intensity(0.949999988079071).radius_ndc(0.05999999865889549).emissive_scale(1.2000000476837158).half_res(true)
}

Transform.position(0.15000000596046448, -0.44999998807907104, 1.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    DirectionalLight.intensity(1.100000023841858).color(1.0, 0.9800000190734863, 0.949999988079071)
}

Editor.translation_space("world").rotation_space("local") {
    Raycastable.enabled() {
        name = "editor_auto_raycastable"
        Transform.position(0.0, -0.7799999713897705, -0.4000000059604645).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(12.0, 0.18000000715255737, 9.5) {
            Renderable.cube() {
                Color.rgba(0.18000000715255737, 0.18000000715255737, 0.2199999988079071, 1.0)
                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
            }
        }
    }
    Raycastable.enabled() {
        name = "editor_auto_raycastable"
        Transform.position(0.0, 2.1500000953674316, -7.199999809265137).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(8.800000190734863, 3.5999999046325684, 0.23999999463558197) {
            Renderable.cube() {
                Color.rgba(0.10999999940395355, 0.10000000149011612, 0.14000000059604645, 1.0)
                Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
            }
        }
    }
    Raycastable.enabled() {
        name = "editor_auto_raycastable"
        Transform.position(2.0, 0.0, 1.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
            Transform.position(-0.8999999761581421, -0.4399999976158142, -1.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.5, 0.5, 0.5) {
                Renderable.cube() {
                    Color.rgba(1.0, 0.8799999952316284, 0.15000000596046448, 1.0)
                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                }
            }
            Transform.position(0.0, -0.4399999976158142, -0.699999988079071).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.5, 0.5, 0.5) {
                Renderable.cube() {
                    Color.rgba(1.0, 0.3499999940395355, 0.7799999713897705, 1.0)
                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                }
            }
            Transform.position(0.8999999761581421, -0.4399999976158142, -1.100000023841858).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.5, 0.5, 0.5) {
                Renderable.cube() {
                    Color.rgba(0.10000000149011612, 0.949999988079071, 1.0, 1.0)
                    Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                }
            }
        }
    }
}

Background {
    Transform.position(2.0, 1.5, -8.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(3.5, 3.5, 3.5) {
        Renderable.circle2d() {
            Color.rgba(1.0, 0.8500000238418579, 0.15000000596046448, 1.0)
            Emissive.on()
            Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
        }
        Transform.position(-0.3499999940395355, 0.3499999940395355, -0.009999999776482582).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.44999998807907104, 0.44999998807907104, 0.44999998807907104) {
            Renderable.circle2d() {
                Color.rgba(1.0, 1.0, 1.0, 1.0)
                Emissive.on()
                Bounds.aabb([-0.5, -0.5, -0.009999999776482582], [0.5, 0.5, 0.009999999776482582])
            }
        }
    }
}

Editor.translation_space("world").rotation_space("local") {
    InputXR.on() {
        Transform.position(-0.9250506162643433, 1.6984316110610962, -0.17940136790275574).rotation_quat(0.4078806936740875, -0.18548893928527832, -0.13585136830806732, 0.8836128115653992).scale(1.0, 1.0, 1.0) {
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
}

InputXR.on() {
    Transform.position(-0.9250506162643433, 1.6984316110610962, -0.17940136790275574).rotation_quat(0.4078806936740875, -0.18548893928527832, -0.13585136830806732, 0.8836128115653992).scale(1.0, 1.0, 1.0) {
        ControllerXR.new(true, "Left", "Aim") {
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 0.05999999865889549, 0.11999999731779099) {
                transform_fork_trs {
                    transform_map_translation {}
                    transform_map_rotation {
                        QuatTemporalFilter.smoothing_factor(220.0)
                    }
                    transform_map_scale {}
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Renderable.cube() {
                            Color.rgba(0.10000000149011612, 0.8999999761581421, 1.0, 1.0)
                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                        }
                    }
                }
            }
        }
        ControllerXR.new(true, "Right", "Aim") {
            Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(0.05999999865889549, 0.05999999865889549, 0.11999999731779099) {
                transform_fork_trs {
                    transform_map_translation {}
                    transform_map_rotation {
                        QuatTemporalFilter.smoothing_factor(220.0)
                    }
                    transform_map_scale {}
                    Transform.position(0.0, 0.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
                        Renderable.cube() {
                            Color.rgba(1.0, 0.3499999940395355, 0.3499999940395355, 1.0)
                            Bounds.aabb([-0.5, -0.5, -0.5], [0.5, 0.5, 0.5])
                        }
                    }
                }
            }
        }
    }
}

Input.speed(1.0) {
    InputTransformMode.forward_z().roll_axis_y().fps_rotation()
    Transform.position(3.0, 1.3220915794372559, 3.5).rotation_quat(0.0, 0.24740396440029144, 0.0, 0.9689124226570129).scale(1.0, 1.0, 1.0) {
        Camera3D.target("window").fov(60.0).near(0.10000000149011612).far(150.0)
        Pointer {
            Raycast.event_driven().max_distance(200.0)
        }
    }
}

OpenXR.on()

Transform.position(0.0, 2.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    DirectionalLight.intensity(0.800000011920929).color(1.0, 0.44999998807907104, 0.8500000238418579)
}

Transform.position(-1.0, -1.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    DirectionalLight.intensity(0.800000011920929).color(1.0, 0.8999999761581421, 0.15000000596046448)
}

Transform.position(1.0, -1.0, 0.0).rotation_quat(0.0, 0.0, 0.0, 1.0).scale(1.0, 1.0, 1.0) {
    DirectionalLight.intensity(0.800000011920929).color(1.0, 0.6000000238418579, 0.15000000596046448)
}

