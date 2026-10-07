use crate::engine::ecs::component::{
    AmplitudeComponent, AnimationComponent, AnimationState, AnimationStepDirection,
    AudioBandPassFilterComponent, AudioInputComponent, AvatarControlComponent,
    BoneRestPoseComponent, EmissiveComponent, GravityComponent, InputComponent,
    InputXRGamepadComponent, RayCastComponent, ShadingComponent, ShadingModel, SliderComponent,
    TextComponent, TransformComponent, TransitionComponent, VelocityComponent,
    VolumeNormalizationComponent,
};
use crate::engine::ecs::{ComponentId, IntentValue, PoseApplyMode, World};
use crate::engine::transform::TransformSpace;
use crate::scripting::object::Value;

fn arg_f32(args: &[Value], index: usize) -> Result<f32, String> {
    match args.get(index) {
        Some(Value::Number(value)) if value.is_finite() => Ok(*value as f32),
        Some(value) => Err(format!(
            "argument {index} must be a finite number, got {value:?}"
        )),
        None => Err(format!("missing argument {index}")),
    }
}

/// Compatibility vocabulary used only by the legacy engine-owned evaluator.
/// Configured MMS runtimes validate methods through RuntimeSpec and dispatch
/// them by OperationId without consulting this predicate.
pub(crate) fn legacy_supports_component_method(component_type: &str, method: &str) -> bool {
    matches!(
        method,
        "attach" | "attach_clone" | "detach" | "remove_child" | "remove_subtree" | "set_color"
    ) || (matches!(component_type, "T" | "Transform" | "transform")
        && matches!(
            method,
            "update_transform"
                | "rest_relative_rotation"
                | "look_at"
                | "translation"
                | "trs"
                | "local_bounds"
        ))
        || (component_type == "TransformWorld" && method == "trs")
        || (matches!(component_type, "PoseCapturePose" | "pose_capture_pose")
            && matches!(method, "apply" | "overlay" | "apply_blended"))
        || (matches!(component_type, "EM" | "Emissive" | "emissive")
            && matches!(method, "set_intensity" | "on" | "off"))
        || (matches!(component_type, "Shading" | "shading")
            && matches!(
                method,
                "get_shade_strength"
                    | "set_shade_strength"
                    | "get_shade_threshold"
                    | "set_shade_threshold"
                    | "get_lit_threshold"
                    | "set_lit_threshold"
                    | "get_rim_strength"
                    | "set_rim_strength"
                    | "get_rim_power"
                    | "set_rim_power"
            ))
        || (matches!(component_type, "Slider" | "slider")
            && matches!(
                method,
                "value" | "set_value" | "sync_value" | "track_mount" | "thumb_mount"
            ))
        || (matches!(component_type, "Amplitude" | "amplitude")
            && matches!(method, "value" | "set_highpass" | "set_highpass_resonance"))
        || (matches!(component_type, "Gravity" | "gravity") && method == "set_enabled")
        || (matches!(component_type, "Velocity" | "velocity")
            && matches!(
                method,
                "reset" | "translate" | "translate_world" | "linear" | "grounded"
            ))
        || (matches!(
            component_type,
            "VolumeNormalization" | "volume_normalization"
        ) && matches!(
            method,
            "value"
                | "gain_db"
                | "set_gain_limits"
                | "set_target_rms"
                | "set_activity_gate"
                | "set_quiet_hold"
                | "set_high_hold"
                | "set_gain_rise"
                | "set_gain_fall"
                | "set_peak_headroom"
        ))
        || (matches!(component_type, "Raycast" | "RayCast" | "raycast")
            && method == "request_raycast")
        || (matches!(
            component_type,
            "AudioBandPassFilter" | "audio_band_pass_filter"
        ) && method == "set_center_hz")
        || (matches!(component_type, "AudioInput" | "audio_input")
            && method == "select_device_number")
        || (matches!(component_type, "I" | "Input" | "input")
            && matches!(method, "enable" | "disable"))
        || (matches!(
            component_type,
            "InputXRGamepad"
                | "InputXrGamepad"
                | "InputVRGamepad"
                | "InputVrGamepad"
                | "input_vr_gamepad"
        ) && matches!(method, "enable" | "disable"))
        || (matches!(component_type, "AvatarControl" | "AVC" | "avatar_control")
            && matches!(
                method,
                "mouth_open_from_amplitude"
                    | "set_mouth_open_rms_center_range"
                    | "set_mouth_open_amount"
                    | "capsule_ready"
            ))
        || (matches!(component_type, "HttpClient" | "http_client")
            && matches!(method, "get" | "post" | "put" | "delete"))
        || (matches!(component_type, "HttpServer" | "http_server")
            && matches!(method, "reply_text"))
}

pub(crate) fn invoke_component_method(
    world: &mut World,
    id: ComponentId,
    component_type: &str,
    method: &str,
    args: &[Value],
    mut emit_intent: impl FnMut(IntentValue),
) -> Result<Value, String> {
    match (component_type, method) {
        ("AvatarControl" | "AVC" | "avatar_control", "capsule_ready") => {
            if !args.is_empty() {
                return Err("capsule_ready(): expected no arguments".into());
            }
            let avatar = world
                .get_component_by_id_as::<AvatarControlComponent>(id)
                .ok_or("capsule_ready(): not an AvatarControl component")?;
            Ok(Value::Bool(
                avatar.collision_enabled && avatar.capsule_ready,
            ))
        }
        ("Gravity" | "gravity", "set_enabled") => {
            let [Value::Bool(enabled)] = args else {
                return Err("set_enabled(): expected one boolean argument".into());
            };
            world
                .get_component_by_id_as_mut::<GravityComponent>(id)
                .ok_or("set_enabled(): not a Gravity component")?
                .enabled = *enabled;
            Ok(Value::Null)
        }
        ("Velocity" | "velocity", "reset") => {
            if !args.is_empty() {
                return Err("reset(): expected no arguments".into());
            }
            world
                .get_component_by_id_as::<VelocityComponent>(id)
                .ok_or("reset(): not a Velocity component")?;
            emit_intent(IntentValue::VelocityReset { component_id: id });
            Ok(Value::Null)
        }
        ("Velocity" | "velocity", "grounded") => {
            if !args.is_empty() {
                return Err("grounded(): expected no arguments".into());
            }
            let velocity = world
                .get_component_by_id_as::<VelocityComponent>(id)
                .ok_or("grounded(): not a Velocity component")?;
            Ok(Value::Bool(velocity.enabled && velocity.grounded))
        }
        ("Velocity" | "velocity", "linear") => {
            if !args.is_empty() {
                return Err("linear(): expected no arguments".into());
            }
            let velocity = world
                .get_component_by_id_as::<VelocityComponent>(id)
                .ok_or("linear(): not a Velocity component")?;
            Ok(Value::Array(
                velocity
                    .linear_local_mps
                    .into_iter()
                    .map(|value| Value::Number(value as f64))
                    .collect(),
            ))
        }
        ("Velocity" | "velocity", "translate" | "translate_world") => {
            let [delta] = args else {
                return Err(format!("{method}(): expected one vec3 delta"));
            };
            let delta_mps = value_as_f32_array::<3>(delta)?;
            if !delta_mps.iter().all(|v| v.is_finite()) {
                return Err(format!("{method}(): delta must be finite"));
            }
            world
                .get_component_by_id_as::<VelocityComponent>(id)
                .ok_or_else(|| format!("{method}(): not a Velocity component"))?;
            emit_intent(IntentValue::VelocityTranslate {
                component_id: id,
                delta_mps,
                world_space: method == "translate_world",
            });
            Ok(Value::Null)
        }
        ("Amplitude" | "amplitude", "value") => {
            if !args.is_empty() {
                return Err(format!("value(): expected no arguments, got {args:?}"));
            }
            world
                .get_component_by_id_as::<AmplitudeComponent>(id)
                .ok_or_else(|| "value(): not an AmplitudeComponent".to_string())?;
            let diagnostics = crate::engine::ecs::component::read_level_diagnostics(world, id)
                .expect("confirmed AmplitudeComponent has diagnostics");
            Ok(Value::Number(diagnostics.rms as f64))
        }
        ("Amplitude" | "amplitude", "set_highpass" | "set_highpass_resonance") => {
            let current = world
                .get_component_by_id_as::<AmplitudeComponent>(id)
                .ok_or_else(|| format!("{method}(): not an AmplitudeComponent"))?
                .clone();
            let value = arg_f32(args, 0)?;
            let updated = if method == "set_highpass" {
                current.with_highpass(value)?
            } else {
                current.with_highpass_resonance(value)?
            };
            *world
                .get_component_by_id_as_mut::<AmplitudeComponent>(id)
                .unwrap() = updated;
            Ok(Value::Null)
        }
        ("VolumeNormalization" | "volume_normalization", "value" | "gain_db") => {
            if !args.is_empty() {
                return Err(format!("{method}(): expected no arguments, got {args:?}"));
            }
            world
                .get_component_by_id_as::<VolumeNormalizationComponent>(id)
                .ok_or_else(|| format!("{method}(): not a VolumeNormalizationComponent"))?;
            let diagnostics = crate::engine::ecs::component::read_level_diagnostics(world, id)
                .expect("confirmed VolumeNormalizationComponent has diagnostics");
            let value = if method == "value" {
                diagnostics.rms
            } else {
                diagnostics
                    .gain_db
                    .expect("normalization diagnostics always include gain")
            };
            Ok(Value::Number(value as f64))
        }
        (
            "VolumeNormalization" | "volume_normalization",
            "set_gain_limits" | "set_target_rms" | "set_activity_gate" | "set_quiet_hold"
            | "set_high_hold" | "set_gain_rise" | "set_gain_fall" | "set_peak_headroom",
        ) => {
            let current = world
                .get_component_by_id_as::<VolumeNormalizationComponent>(id)
                .ok_or_else(|| format!("{method}(): not a VolumeNormalizationComponent"))?
                .clone();
            let updated = match method {
                "set_gain_limits" => {
                    current.with_gain_limits(arg_f32(args, 0)?, arg_f32(args, 1)?)?
                }
                "set_target_rms" => {
                    current.with_target_rms(arg_f32(args, 0)?, arg_f32(args, 1)?)?
                }
                "set_activity_gate" => {
                    current.with_nonnegative("activity_gate", arg_f32(args, 0)?)?
                }
                "set_quiet_hold" => current.with_nonnegative("quiet_hold", arg_f32(args, 0)?)?,
                "set_high_hold" => current.with_nonnegative("high_hold", arg_f32(args, 0)?)?,
                "set_gain_rise" => current.with_nonnegative("gain_rise", arg_f32(args, 0)?)?,
                "set_gain_fall" => current.with_nonnegative("gain_fall", arg_f32(args, 0)?)?,
                "set_peak_headroom" => current.with_peak_headroom(arg_f32(args, 0)?)?,
                _ => unreachable!("match arm restricts VolumeNormalization setters"),
            };
            *world
                .get_component_by_id_as_mut::<VolumeNormalizationComponent>(id)
                .expect("component was inspected above") = updated;
            Ok(Value::Null)
        }
        ("AvatarControl" | "AVC" | "avatar_control", "mouth_open_from_amplitude") => {
            let source = crate::scripting::component_registry::arg_component_ref(world, args, 0)?;
            if let Some(source_id) = crate::engine::ecs::component::resolve_component_ref(
                world,
                &source,
                Some(id),
                crate::engine::ecs::component::QueryRootMode::WorldRoot,
            ) && !crate::engine::ecs::component::is_level_provider(world, source_id)
            {
                return Err(
                    "AvatarControl.mouth_open_from_amplitude requires an Amplitude or VolumeNormalization component"
                        .into(),
                );
            }
            let avc = world
                .get_component_by_id_as_mut::<crate::engine::ecs::component::AvatarControlComponent>(id)
                .ok_or_else(|| "mouth_open_from_amplitude(): not an AvatarControlComponent".to_string())?;
            *avc = avc.clone().with_mouth_open_from_amplitude(source);
            Ok(Value::Null)
        }
        ("AvatarControl" | "AVC" | "avatar_control", "set_mouth_open_rms_center_range") => {
            let updated = world
                .get_component_by_id_as::<crate::engine::ecs::component::AvatarControlComponent>(id)
                .ok_or_else(|| {
                    "set_mouth_open_rms_center_range(): not an AvatarControlComponent".to_string()
                })?
                .clone()
                .with_mouth_open_rms_center_range(arg_f32(args, 0)?, arg_f32(args, 1)?)?;
            *world
                .get_component_by_id_as_mut::<crate::engine::ecs::component::AvatarControlComponent>(id)
                .expect("component was inspected above") = updated;
            Ok(Value::Null)
        }
        ("AvatarControl" | "AVC" | "avatar_control", "set_mouth_open_amount") => {
            let updated = world
                .get_component_by_id_as::<crate::engine::ecs::component::AvatarControlComponent>(id)
                .ok_or_else(|| {
                    "set_mouth_open_amount(): not an AvatarControlComponent".to_string()
                })?
                .clone()
                .with_mouth_open_amount(arg_f32(args, 0)?)?;
            *world
                .get_component_by_id_as_mut::<crate::engine::ecs::component::AvatarControlComponent>(id)
                .expect("component was inspected above") = updated;
            Ok(Value::Null)
        }
        ("T" | "Transform" | "transform", "local_bounds") => {
            if !args.is_empty() {
                return Err("local_bounds(): expected no arguments".into());
            }
            world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "local_bounds(): not a TransformComponent".to_string())?;
            use crate::engine::ecs::component::{
                BoundsComponent, GLTFComponent, RenderableComponent,
            };
            use crate::engine::ecs::system::bounds_system::BoundsSystem;
            // Never return a partial box while an import or mesh upload is pending.
            let mut pending = vec![id];
            while let Some(node) = pending.pop() {
                if world
                    .get_component_by_id_as::<GLTFComponent>(node)
                    .is_some_and(|gltf| !gltf.spawned)
                {
                    return Ok(Value::Null);
                }
                if world
                    .get_component_by_id_as::<RenderableComponent>(node)
                    .is_some()
                    && !world.children_of(node).iter().any(|&child| {
                        world
                            .get_component_by_id_as::<BoundsComponent>(child)
                            .is_some()
                    })
                {
                    return Ok(Value::Null);
                }
                pending.extend(world.children_of(node).iter().copied());
            }
            let Some(bounds) =
                BoundsSystem::measure_cached_renderable_subtree_bounds(world, id, |_| false)
            else {
                return Ok(Value::Null);
            };
            let vector = |values: [f32; 3]| {
                Value::Array(
                    values
                        .into_iter()
                        .map(|v| Value::Number(v as f64))
                        .collect(),
                )
            };
            Ok(Value::Map(
                [
                    ("min".to_string(), vector(bounds.min)),
                    ("max".to_string(), vector(bounds.max)),
                ]
                .into_iter()
                .collect(),
            ))
        }

        ("A" | "Animation" | "animation", method @ ("play" | "loop_anim" | "pause")) => {
            if !args.is_empty() {
                return Err(format!("{method}(): expected no arguments, got {args:?}"));
            }
            world
                .get_component_by_id_as::<AnimationComponent>(id)
                .ok_or_else(|| format!("{method}(): not an AnimationComponent"))?;
            let state = match method {
                "play" => AnimationState::Playing,
                "loop_anim" => AnimationState::Looping,
                "pause" => AnimationState::Paused,
                _ => unreachable!(),
            };
            emit_intent(IntentValue::SetAnimationState {
                component_id: id,
                state,
            });
            Ok(Value::Null)
        }
        ("A" | "Animation" | "animation", method @ ("next" | "previous")) => {
            if !args.is_empty() {
                return Err(format!("{method}(): expected no arguments, got {args:?}"));
            }
            world
                .get_component_by_id_as::<AnimationComponent>(id)
                .ok_or_else(|| format!("{method}(): not an AnimationComponent"))?;
            emit_intent(IntentValue::StepAnimation {
                component_id: id,
                direction: if method == "next" {
                    AnimationStepDirection::Next
                } else {
                    AnimationStepDirection::Previous
                },
            });
            Ok(Value::Null)
        }
        ("I" | "Input" | "input", "enable" | "disable") => {
            if !args.is_empty() {
                return Err(format!("{method}(): expected no arguments, got {args:?}"));
            }
            let input = world
                .get_component_by_id_as_mut::<InputComponent>(id)
                .ok_or_else(|| format!("{method}(): not an InputComponent"))?;
            input.enabled = method == "enable";
            Ok(Value::Null)
        }
        (
            "I" | "Input" | "input",
            method @ ("set_translation_enabled" | "set_rotation_enabled"),
        ) => {
            if args.len() != 1 {
                return Err(format!(
                    "{method}(): expected one boolean argument, got {args:?}"
                ));
            }
            let enabled = match &args[0] {
                Value::Bool(enabled) => *enabled,
                value => return Err(format!("{method}(): expected boolean, got {value:?}")),
            };
            let input = world
                .get_component_by_id_as_mut::<InputComponent>(id)
                .ok_or_else(|| format!("{method}(): not an InputComponent"))?;
            if method == "set_translation_enabled" {
                input.translation_enabled = enabled;
            } else {
                input.rotation_enabled = enabled;
            }
            Ok(Value::Null)
        }
        (
            "InputXRGamepad" | "InputXrGamepad" | "InputVRGamepad" | "InputVrGamepad"
            | "input_vr_gamepad",
            "enable" | "disable",
        ) => {
            if !args.is_empty() {
                return Err(format!("{method}(): expected no arguments, got {args:?}"));
            }
            let input = world
                .get_component_by_id_as_mut::<InputXRGamepadComponent>(id)
                .ok_or_else(|| format!("{method}(): not an InputXRGamepadComponent"))?;
            // Keep the component and its canonical axis/button events live;
            // only transfer authority away from the built-in locomotion map.
            input.locomotion = method == "enable";
            Ok(Value::Null)
        }
        (
            "Shading" | "shading",
            method @ ("get_shade_strength"
            | "set_shade_strength"
            | "get_shade_threshold"
            | "set_shade_threshold"
            | "get_lit_threshold"
            | "set_lit_threshold"
            | "get_rim_strength"
            | "set_rim_strength"
            | "get_rim_power"
            | "set_rim_power"),
        ) => {
            let shading = world
                .get_component_by_id_as_mut::<ShadingComponent>(id)
                .ok_or_else(|| format!("{method}(): not a ShadingComponent"))?;
            if shading.model != ShadingModel::Anime {
                return Err(format!("{method}(): requires the Anime shading model"));
            }
            if method.starts_with("get_") {
                if !args.is_empty() {
                    return Err(format!("{method}(): expected no arguments"));
                }
                let value = match method {
                    "get_shade_strength" => shading.shade_strength,
                    "get_shade_threshold" => shading.shade_threshold,
                    "get_lit_threshold" => shading.lit_threshold,
                    "get_rim_strength" => shading.rim_strength,
                    "get_rim_power" => shading.rim_power,
                    _ => unreachable!(),
                };
                return Ok(Value::Number(value as f64));
            }
            let [Value::Number(value)] = args else {
                return Err(format!("{method}(): expected one number"));
            };
            *shading = match method {
                "set_shade_strength" => shading.with_shade_strength(*value as f32),
                "set_shade_threshold" => shading.with_shade_threshold(*value as f32),
                "set_lit_threshold" => shading.with_lit_threshold(*value as f32),
                "set_rim_strength" => shading.with_rim_strength(*value as f32),
                "set_rim_power" => shading.with_rim_power(*value as f32),
                _ => unreachable!(),
            };
            emit_intent(IntentValue::RegisterAnimeShading { component_id: id });
            Ok(Value::Null)
        }
        ("Text" | "TXT" | "text", "set_text") => {
            world
                .get_component_by_id_as::<TextComponent>(id)
                .ok_or_else(|| "set_text(): not a TextComponent".to_string())?;
            let [Value::String(text)] = args else {
                return Err(format!(
                    "set_text(): expected one string argument, got {args:?}"
                ));
            };
            emit_intent(IntentValue::SetText {
                component_id: id,
                text: text.clone(),
            });
            Ok(Value::Null)
        }
        ("Slider" | "slider", "value") => {
            if !args.is_empty() {
                return Err(format!("value(): expected no arguments, got {args:?}"));
            }
            let value = world
                .get_component_by_id_as::<SliderComponent>(id)
                .ok_or_else(|| "value(): not a SliderComponent".to_string())?
                .value();
            Ok(Value::Number(value as f64))
        }
        ("Slider" | "slider", method @ ("set_value" | "sync_value")) => {
            let [Value::Number(value)] = args else {
                return Err(format!("{method}(): expected one number, got {args:?}"));
            };
            if !value.is_finite() {
                return Err(format!("{method}(): value must be finite"));
            }
            world
                .get_component_by_id_as::<SliderComponent>(id)
                .ok_or_else(|| format!("{method}(): not a SliderComponent"))?;
            emit_intent(IntentValue::SliderSet {
                component_id: id,
                value: *value as f32,
                emit_changed: method == "set_value",
            });
            Ok(Value::Null)
        }
        ("Slider" | "slider", method @ ("track_mount" | "thumb_mount")) => {
            if !args.is_empty() {
                return Err(format!("{method}(): expected no arguments, got {args:?}"));
            }
            let slider = world
                .get_component_by_id_as::<SliderComponent>(id)
                .ok_or_else(|| format!("{method}(): not a SliderComponent"))?;
            let mount = if method == "track_mount" {
                slider.track_mount
            } else {
                slider.thumb_mount
            }
            .ok_or_else(|| format!("{method}(): slider is not initialized"))?;
            Ok(Value::ComponentObject {
                id: mount,
                component_type: "Transform".into(),
            })
        }
        ("TransformWorld", "trs") => {
            world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "world.trs(): not a TransformComponent".to_string())?;
            match args {
                [] => crate::engine::ecs::system::TransformSystem::world_trs(world, id)
                    .map(Value::TransformTrs)
                    .map_err(|error| format!("world.trs(): {error}")),
                [Value::TransformTrs(value)] => {
                    emit_intent(IntentValue::SetTransformTrs {
                        component_id: id,
                        space: TransformSpace::World,
                        value: *value,
                    });
                    Ok(Value::Null)
                }
                other => Err(format!(
                    "world.trs(): expected no arguments for a getter or one TransformTrs value for a setter, got {other:?}"
                )),
            }
        }
        (_, "attach") => {
            let child = match args {
                [Value::ComponentObject { id, .. }] => *id,
                other => {
                    return Err(format!(
                        "attach(): expected one component child argument, got {other:?}"
                    ));
                }
            };
            if world.get_component_record(id).is_none() {
                return Err("attach(): parent component does not exist".to_string());
            }
            if world.get_component_record(child).is_none() {
                return Err("attach(): child component does not exist".to_string());
            }
            emit_intent(IntentValue::Attach { parent: id, child });
            Ok(Value::Null)
        }
        (_, "attach_clone") => {
            let prefab_root = match args {
                [Value::ComponentObject { id, .. }] => *id,
                other => {
                    return Err(format!(
                        "attach_clone(): expected one component prefab argument, got {other:?}"
                    ));
                }
            };
            require_live_component(world, id, "attach_clone(): parent")?;
            require_live_component(world, prefab_root, "attach_clone(): prefab")?;
            emit_intent(IntentValue::AttachClone {
                parent: id,
                prefab_root,
            });
            Ok(Value::Null)
        }
        (_, "detach") => {
            if !args.is_empty() {
                return Err(format!("detach(): expected no arguments, got {args:?}"));
            }
            require_live_component(world, id, "detach()")?;
            emit_intent(IntentValue::Detach { component_id: id });
            Ok(Value::Null)
        }
        (_, "remove_child") => {
            let index = match args {
                [Value::Number(index)]
                    if index.is_finite()
                        && *index >= 0.0
                        && index.fract() == 0.0
                        && *index <= usize::MAX as f64 =>
                {
                    *index as usize
                }
                other => {
                    return Err(format!(
                        "remove_child(): expected one non-negative integer index, got {other:?}"
                    ));
                }
            };
            require_live_component(world, id, "remove_child(): parent")?;
            emit_intent(IntentValue::RemoveChild { parent: id, index });
            Ok(Value::Null)
        }
        (_, "remove_subtree") => {
            if !args.is_empty() {
                return Err(format!(
                    "remove_subtree(): expected no arguments, got {args:?}"
                ));
            }
            require_live_component(world, id, "remove_subtree()")?;
            emit_intent(IntentValue::RemoveSubtree { component_id: id });
            Ok(Value::Null)
        }
        (_, "set_color") => {
            let rgba = match args {
                [rgba] => value_as_f32_array::<4>(rgba)?,
                other => {
                    return Err(format!(
                        "set_color(): expected one rgba array argument, got {other:?}"
                    ));
                }
            };
            require_live_component(world, id, "set_color()")?;
            emit_intent(IntentValue::SetColor {
                component_id: id,
                rgba,
            });
            Ok(Value::Null)
        }
        (
            "PoseCapturePose" | "pose_capture_pose",
            method @ ("apply" | "overlay" | "apply_blended"),
        ) => {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::PoseCapturePoseComponent>(
                    id,
                )
                .ok_or_else(|| format!("{method}(): not a PoseCapturePoseComponent"))?;
            let target = match args.first() {
                Some(Value::ComponentObject { id, .. }) => *id,
                other => {
                    return Err(format!(
                        "{method}(): expected a component target, got {other:?}"
                    ));
                }
            };
            let expected = if method == "apply_blended" { 2 } else { 1 };
            if args.len() != expected {
                return Err(format!(
                    "{method}(): expected {expected} argument(s), got {}",
                    args.len()
                ));
            }
            let mode = match method {
                "apply" => PoseApplyMode::Replace,
                "overlay" => PoseApplyMode::Overlay,
                "apply_blended" => {
                    let amount = match args.get(1) {
                        Some(Value::Number(value)) => *value as f32,
                        other => {
                            return Err(format!(
                                "apply_blended(): expected numeric amount, got {other:?}"
                            ));
                        }
                    };
                    PoseApplyMode::RestBlend {
                        amount: amount.clamp(0.0, 1.0),
                    }
                }
                _ => unreachable!(),
            };
            emit_intent(IntentValue::PoseApply {
                target,
                pose: id,
                mode,
            });
            Ok(Value::Null)
        }
        ("T" | "Transform" | "transform", "translation") => {
            if !args.is_empty() {
                return Err(format!(
                    "translation(): expected no arguments, got {args:?}"
                ));
            }
            let translation = world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "translation(): not a TransformComponent".to_string())?
                .transform
                .translation;
            Ok(Value::Array(
                translation
                    .into_iter()
                    .map(|value| Value::Number(value as f64))
                    .collect(),
            ))
        }
        ("T" | "Transform" | "transform", "trs") => {
            let current = world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "trs(): not a TransformComponent".to_string())?
                .trs();

            match args {
                [] => Ok(Value::TransformTrs(current)),
                [Value::TransformTrs(replacement)] => {
                    emit_intent(IntentValue::UpdateTransform {
                        component_id: id,
                        translation: replacement.translation,
                        rotation_quat_xyzw: replacement.rotation_quat_xyzw,
                        scale: replacement.scale,
                    });
                    Ok(Value::Null)
                }
                other => Err(format!(
                    "trs(): expected no arguments for a getter or one TransformTrs value for a setter, got {other:?}"
                )),
            }
        }
        ("T" | "Transform" | "transform", "update_transform") => {
            if args.len() == 1 || args.len() == 2 {
                let translation = value_as_f32_array::<3>(&args[0])?;
                if !translation.iter().all(|v| v.is_finite()) {
                    return Err("update_transform(): position must be finite".into());
                }
                let t = world
                    .get_component_by_id_as::<TransformComponent>(id)
                    .ok_or("update_transform(): not a Transform")?;
                let rotation_quat_xyzw = if let Some(rotation) = args.get(1) {
                    let euler = value_as_f32_array::<3>(rotation)?;
                    if !euler.iter().all(|v| v.is_finite()) {
                        return Err("update_transform(): rotation must be finite".into());
                    }
                    TransformComponent::new()
                        .with_rotation_euler(euler[0], euler[1], euler[2])
                        .transform
                        .rotation
                } else {
                    t.transform.rotation
                };
                emit_intent(IntentValue::UpdateTransform {
                    component_id: id,
                    translation,
                    rotation_quat_xyzw,
                    scale: t.transform.scale,
                });
                return Ok(Value::Null);
            }
            let [translation, rotation_euler, scale] = match args {
                [translation, rotation, scale] => [
                    value_as_f32_array::<3>(translation)?,
                    value_as_f32_array::<3>(rotation)?,
                    value_as_f32_array::<3>(scale)?,
                ],
                other => {
                    return Err(format!(
                        "update_transform: expected one to three vec3 array arguments, got {:?}",
                        other
                    ));
                }
            };

            world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "update_transform(): not a TransformComponent".to_string())?;

            emit_intent(IntentValue::UpdateTransform {
                component_id: id,
                translation,
                rotation_quat_xyzw: TransformComponent::new()
                    .with_rotation_euler(rotation_euler[0], rotation_euler[1], rotation_euler[2])
                    .transform
                    .rotation,
                scale,
            });
            Ok(Value::Null)
        }
        ("T" | "Transform" | "transform", "rest_relative_rotation") => {
            let [pitch, yaw, roll] = match args {
                [rotation] => value_as_f32_array::<3>(rotation)?,
                other => {
                    return Err(format!(
                        "rest_relative_rotation: expected one vec3 Euler-radians argument, got {other:?}"
                    ));
                }
            };
            let current = world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "rest_relative_rotation(): not a TransformComponent".to_string())?
                .transform;
            let rest = world.children_of(id).iter().find_map(|child| {
                world
                    .get_component_by_id_as::<BoneRestPoseComponent>(*child)
                    .copied()
            });
            let (translation, rotation, scale) = rest.map_or(
                (current.translation, current.rotation, current.scale),
                |rest| (rest.translation, rest.rotation, rest.scale),
            );
            let offset = TransformComponent::new()
                .with_rotation_euler(pitch, yaw, roll)
                .transform
                .rotation;
            emit_intent(IntentValue::UpdateTransform {
                component_id: id,
                translation,
                rotation_quat_xyzw: crate::utils::math::quat_mul(offset, rotation),
                scale,
            });
            Ok(Value::Null)
        }
        ("T" | "Transform" | "transform", "look_at") => {
            let [target_world] = match args {
                [target_world] => [value_as_f32_array::<3>(target_world)?],
                other => {
                    return Err(format!(
                        "look_at: expected one vec3 array argument, got {:?}",
                        other
                    ));
                }
            };

            world
                .get_component_by_id_as::<TransformComponent>(id)
                .ok_or_else(|| "look_at(): not a TransformComponent".to_string())?;

            emit_intent(IntentValue::LookAt {
                component_id: id,
                target_world,
            });
            Ok(Value::Null)
        }
        ("EM" | "Emissive" | "emissive", "set_intensity" | "on" | "off") => {
            let intensity = match method {
                "on" => 1.0,
                "off" => 0.0,
                "set_intensity" => match args.first() {
                    Some(Value::Number(n)) => (*n as f32).max(0.0),
                    Some(other) => {
                        return Err(format!(
                            "set_intensity: expected number argument, got {:?}",
                            other
                        ));
                    }
                    None => return Err("set_intensity: missing number argument".into()),
                },
                _ => unreachable!(),
            };

            world
                .get_component_by_id_as::<EmissiveComponent>(id)
                .ok_or_else(|| format!("{method}(): not an EmissiveComponent"))?;

            let has_transition_child = world.children_of(id).iter().any(|&child| {
                world
                    .get_component_by_id_as::<TransitionComponent>(child)
                    .is_some()
            });
            let is_attached = world.parent_of(id).is_some();
            if !(is_attached && has_transition_child) {
                let emissive = world
                    .get_component_by_id_as_mut::<EmissiveComponent>(id)
                    .ok_or_else(|| format!("{method}(): not an EmissiveComponent"))?;
                emissive.intensity = intensity;
            }

            emit_intent(IntentValue::SetEmissiveIntensity {
                component_id: id,
                intensity,
            });
            Ok(Value::Null)
        }
        ("Raycast" | "RayCast" | "raycast", "request_raycast") => {
            if !args.is_empty() {
                return Err(format!(
                    "request_raycast(): expected no arguments, got {args:?}"
                ));
            }
            world
                .get_component_by_id_as::<RayCastComponent>(id)
                .ok_or_else(|| "request_raycast(): not a RayCastComponent".to_string())?;
            emit_intent(IntentValue::RequestRaycast { component_id: id });
            Ok(Value::Null)
        }
        ("AudioBandPassFilter" | "audio_band_pass_filter", "set_center_hz") => {
            let center_hz = match args {
                [Value::Number(value)] if value.is_finite() && *value >= 0.0 => *value as f32,
                other => {
                    return Err(format!(
                        "set_center_hz(): expected one finite non-negative number, got {other:?}"
                    ));
                }
            };
            world
                .get_component_by_id_as::<AudioBandPassFilterComponent>(id)
                .ok_or_else(|| {
                    "set_center_hz(): not an AudioBandPassFilterComponent".to_string()
                })?;
            emit_intent(IntentValue::AudioBandPassSetCenterHz {
                component_id: id,
                center_hz,
            });
            Ok(Value::Null)
        }
        ("AudioInput" | "audio_input", "select_device_number") => {
            let device_number = match args {
                [Value::Number(value)]
                    if value.is_finite()
                        && *value >= 0.0
                        && value.fract() == 0.0
                        && *value <= usize::MAX as f64 =>
                {
                    *value as usize
                }
                other => {
                    return Err(format!(
                        "select_device_number(): expected one non-negative integer, got {other:?}"
                    ));
                }
            };
            let input = world
                .get_component_by_id_as_mut::<AudioInputComponent>(id)
                .ok_or_else(|| "select_device_number(): not an AudioInputComponent".to_string())?;
            input.select_device_number(device_number);
            Ok(Value::Null)
        }
        ("HttpClient" | "http_client", "get" | "delete") => {
            let [url] = match args {
                [url] => [value_as_string(url, method)?],
                other => {
                    return Err(format!(
                        "{method}: expected one string url argument, got {:?}",
                        other
                    ));
                }
            };
            emit_intent(IntentValue::HttpClientRequest {
                component_id: id,
                method: method.to_ascii_uppercase(),
                url,
                headers: vec![],
                body_text: None,
            });
            Ok(Value::Null)
        }
        ("HttpClient" | "http_client", "post" | "put") => {
            let (url, body_text) = match args {
                [url, body_text] => (
                    value_as_string(url, method)?,
                    value_as_string(body_text, method)?,
                ),
                other => {
                    return Err(format!(
                        "{method}: expected url and body_text string arguments, got {:?}",
                        other
                    ));
                }
            };
            emit_intent(IntentValue::HttpClientRequest {
                component_id: id,
                method: method.to_ascii_uppercase(),
                url,
                headers: vec![],
                body_text: Some(body_text),
            });
            Ok(Value::Null)
        }
        ("HttpServer" | "http_server", "reply_text") => {
            let (request_id, status, body_text) = match args {
                [request, status, body_text] => (
                    request_id_from_value(request)?,
                    value_as_u16(status, method)?,
                    value_as_string(body_text, method)?,
                ),
                other => {
                    return Err(format!(
                        "reply_text: expected request, status, body_text arguments, got {:?}",
                        other
                    ));
                }
            };
            emit_intent(IntentValue::HttpServerReply {
                component_id: id,
                request_id,
                status,
                headers: vec![],
                body_text,
            });
            Ok(Value::Null)
        }
        _ => Err(format!(
            "unsupported live component method '{}.{}'",
            component_type, method
        )),
    }
}

fn require_live_component(world: &World, id: ComponentId, context: &str) -> Result<(), String> {
    if world.get_component_record(id).is_some() {
        Ok(())
    } else {
        Err(format!("{context}: component does not exist"))
    }
}

fn value_as_f32_array<const N: usize>(value: &Value) -> Result<[f32; N], String> {
    let Value::Array(values) = value else {
        return Err(format!("expected array, got {:?}", value));
    };
    if values.len() != N {
        return Err(format!("expected array of len {}, got {}", N, values.len()));
    }
    let mut out = [0.0_f32; N];
    for (i, value) in values.iter().enumerate() {
        match value {
            Value::Number(n) => out[i] = *n as f32,
            other => return Err(format!("expected numeric array element, got {:?}", other)),
        }
    }
    Ok(out)
}

fn value_as_string(value: &Value, method: &str) -> Result<String, String> {
    match value {
        Value::String(s) => Ok(s.clone()),
        other => Err(format!("{method}: expected string, got {:?}", other)),
    }
}

fn value_as_u16(value: &Value, method: &str) -> Result<u16, String> {
    match value {
        Value::Number(n) if *n >= 0.0 && *n <= u16::MAX as f64 => Ok(*n as u16),
        other => Err(format!("{method}: expected status number, got {:?}", other)),
    }
}

fn request_id_from_value(value: &Value) -> Result<u64, String> {
    let Value::Map(map) = value else {
        return Err(format!(
            "reply_text: expected request object, got {:?}",
            value
        ));
    };
    let Some(Value::Number(request_id)) = map.get("request_id") else {
        return Err("reply_text: request missing numeric request_id".to_string());
    };
    if *request_id < 0.0 {
        return Err("reply_text: request_id must be non-negative".to_string());
    }
    Ok(*request_id as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::{
        AmplitudeSample, AmplitudeStatus, AudioBandPassFilterComponent, ColorComponent,
        ComponentRef, RayCastComponent, TransformComponent, VolumeNormalizationComponent,
    };

    #[test]
    fn audio_analysis_scalar_reads_are_finite_and_expose_only_normalizer_gain() {
        let mut world = World::default();
        let raw_id = world.add_component(AmplitudeComponent::rolling_window(0.08).unwrap());
        let normalization_id = world.add_component(VolumeNormalizationComponent::from(
            ComponentRef::Query("#raw".into()),
        ));

        let live = AmplitudeSample {
            generation: 0,
            sequence: 1,
            timestamp_sec: 1.0,
            valid_frames: 480,
            rms: 0.04,
            peak: 0.10,
            status: AmplitudeStatus::Live,
        };
        world
            .get_component_by_id_as_mut::<AmplitudeComponent>(raw_id)
            .unwrap()
            .retained = live;
        let normalized = world
            .get_component_by_id_as_mut::<VolumeNormalizationComponent>(normalization_id)
            .unwrap();
        normalized.retained = AmplitudeSample { rms: 0.08, ..live };
        normalized.current_gain_db = 6.0;

        for (id, ty, method, expected) in [
            (raw_id, "Amplitude", "value", 0.04),
            (normalization_id, "VolumeNormalization", "value", 0.08),
            (normalization_id, "VolumeNormalization", "gain_db", 6.0),
        ] {
            let Value::Number(value) =
                invoke_component_method(&mut world, id, ty, method, &[], |_| {}).unwrap()
            else {
                panic!("{ty}.{method}() should return a number");
            };
            assert!(
                (value - expected).abs() < 1e-6,
                "{ty}.{method}() returned {value}"
            );
        }

        for status in [
            AmplitudeStatus::Pending,
            AmplitudeStatus::Neutral,
            AmplitudeStatus::Invalid,
        ] {
            let sample_for = |generation| match status {
                AmplitudeStatus::Pending => AmplitudeSample::pending(generation),
                AmplitudeStatus::Neutral | AmplitudeStatus::Invalid => {
                    AmplitudeSample::neutral(generation, status)
                }
                AmplitudeStatus::Live => unreachable!(),
            };
            let raw = world
                .get_component_by_id_as_mut::<AmplitudeComponent>(raw_id)
                .unwrap();
            raw.retained = sample_for(raw.generation);
            let normalization = world
                .get_component_by_id_as_mut::<VolumeNormalizationComponent>(normalization_id)
                .unwrap();
            normalization.retained = sample_for(normalization.generation);
            normalization.current_gain_db = f32::NAN;

            for (id, ty, method) in [
                (raw_id, "Amplitude", "value"),
                (normalization_id, "VolumeNormalization", "value"),
                (normalization_id, "VolumeNormalization", "gain_db"),
            ] {
                let Value::Number(value) =
                    invoke_component_method(&mut world, id, ty, method, &[], |_| {}).unwrap()
                else {
                    panic!("{ty}.{method}() should return a number");
                };
                assert!(
                    value.is_finite(),
                    "{status:?}: {ty}.{method}() returned {value}"
                );
                assert_eq!(value, 0.0);
            }
        }

        let error =
            invoke_component_method(&mut world, raw_id, "Amplitude", "gain_db", &[], |_| {})
                .expect_err("Amplitude must not invent a gain diagnostic");
        assert!(error.contains("unsupported live component method 'Amplitude.gain_db'"));
    }

    #[test]
    fn transform_local_bounds_waits_for_complete_geometry_and_uses_root_local_space() {
        use crate::engine::ecs::component::{BoundsComponent, GLTFComponent, RenderableComponent};
        use crate::engine::graphics::bounds::Aabb;
        let mut world = World::default();
        let root = world.add_component(
            TransformComponent::new()
                .with_position(100.0, 0.0, 0.0)
                .with_scale(5.0, 5.0, 5.0),
        );
        let child = world.add_component(
            TransformComponent::new()
                .with_position(2.0, 3.0, -4.0)
                .with_scale(2.0, 1.0, 3.0),
        );
        world.add_child(root, child).unwrap();
        let mesh = world.add_component(RenderableComponent::cube());
        world.add_child(child, mesh).unwrap();
        let read = |world: &mut World| {
            invoke_component_method(world, root, "Transform", "local_bounds", &[], |_| {
                panic!("read-only query")
            })
        };
        assert_eq!(read(&mut world).unwrap(), Value::Null);
        let bounds = world.add_component(BoundsComponent::new(Aabb {
            min: [-1.0, 0.0, -1.0],
            max: [1.0, 2.0, 1.0],
        }));
        world.add_child(mesh, bounds).unwrap();
        let Value::Map(measured) = read(&mut world).unwrap() else {
            panic!("bounds map expected")
        };
        assert_eq!(
            measured["min"],
            Value::Array(vec![
                Value::Number(0.0),
                Value::Number(3.0),
                Value::Number(-7.0)
            ])
        );
        assert_eq!(
            measured["max"],
            Value::Array(vec![
                Value::Number(4.0),
                Value::Number(5.0),
                Value::Number(-1.0)
            ])
        );
        let loading = world.add_component(GLTFComponent::new("pending.glb"));
        world.add_child(root, loading).unwrap();
        assert_eq!(
            read(&mut world).unwrap(),
            Value::Null,
            "never return partial import bounds"
        );
    }

    #[test]
    fn anime_shading_live_api_normalizes_reads_back_and_rejects_wrong_models() {
        let mut world = World::default();
        let anime = world.add_component(ShadingComponent::new());
        let toon = world.add_component(ShadingComponent::toon());
        for (value, expected) in [
            (2.0, 1.0),
            (-1.0, 0.0),
            (f64::NAN, ShadingComponent::DEFAULT_SHADE_STRENGTH as f64),
        ] {
            let mut intents = Vec::new();
            invoke_component_method(
                &mut world,
                anime,
                "Shading",
                "set_shade_strength",
                &[Value::Number(value)],
                |intent| intents.push(intent),
            )
            .unwrap();
            // Effective readback precedes deferred render-system processing.
            assert_eq!(
                invoke_component_method(
                    &mut world,
                    anime,
                    "Shading",
                    "get_shade_strength",
                    &[],
                    |_| {}
                )
                .unwrap(),
                Value::Number(expected)
            );
            assert!(
                matches!(intents.as_slice(), [IntentValue::RegisterAnimeShading { component_id }] if *component_id == anime)
            );
        }
        for (setter, getter, value, expected) in [
            ("set_shade_threshold", "get_shade_threshold", 0.8, 0.8),
            ("set_lit_threshold", "get_lit_threshold", 0.4, 0.4),
            ("set_rim_strength", "get_rim_strength", 2.0, 1.0),
            ("set_rim_power", "get_rim_power", 200.0, 128.0),
        ] {
            let mut intents = Vec::new();
            invoke_component_method(
                &mut world,
                anime,
                "Shading",
                setter,
                &[Value::Number(value)],
                |intent| intents.push(intent),
            )
            .unwrap();
            let Value::Number(effective) =
                invoke_component_method(&mut world, anime, "Shading", getter, &[], |_| {}).unwrap()
            else {
                panic!("{getter}() should return a number")
            };
            assert!((effective - expected).abs() < 1e-6);
            assert!(matches!(
                intents.as_slice(),
                [IntentValue::RegisterAnimeShading { component_id }] if *component_id == anime
            ));
        }
        assert_eq!(
            world
                .get_component_by_id_as::<ShadingComponent>(anime)
                .unwrap()
                .shade_threshold,
            0.4,
            "lowering lit threshold also lowers shade threshold"
        );
        for method in [
            "get_shade_strength",
            "set_shade_strength",
            "get_shade_threshold",
            "set_shade_threshold",
            "get_lit_threshold",
            "set_lit_threshold",
            "get_rim_strength",
            "set_rim_strength",
            "get_rim_power",
            "set_rim_power",
        ] {
            let error = invoke_component_method(
                &mut world,
                toon,
                "Shading",
                method,
                &[Value::Number(0.5)],
                |_| panic!("invalid target emitted an intent"),
            )
            .unwrap_err();
            assert!(error.contains("requires the Anime"));
        }
    }

    fn object(id: ComponentId, ty: &str) -> Value {
        Value::ComponentObject {
            id,
            component_type: ty.to_string(),
        }
    }

    #[test]
    fn input_live_master_gate_and_xr_locomotion_gate() {
        let mut world = World::default();
        let desktop = world.add_component(InputComponent::new());
        let xr_gamepad = world.add_component(InputXRGamepadComponent::new());

        invoke_component_method(&mut world, desktop, "Input", "disable", &[], |_| {}).unwrap();
        invoke_component_method(
            &mut world,
            xr_gamepad,
            "InputXRGamepad",
            "disable",
            &[],
            |_| {},
        )
        .unwrap();

        assert!(
            !world
                .get_component_by_id_as::<InputComponent>(desktop)
                .unwrap()
                .enabled
        );
        let xr = world
            .get_component_by_id_as::<InputXRGamepadComponent>(xr_gamepad)
            .unwrap();
        assert!(xr.enabled, "raw XR gamepad observation remains enabled");
        assert!(!xr.locomotion, "built-in XR locomotion is relinquished");

        invoke_component_method(&mut world, desktop, "Input", "enable", &[], |_| {}).unwrap();
        invoke_component_method(
            &mut world,
            xr_gamepad,
            "InputXRGamepad",
            "enable",
            &[],
            |_| {},
        )
        .unwrap();

        assert!(
            world
                .get_component_by_id_as::<InputComponent>(desktop)
                .unwrap()
                .enabled
        );
        assert!(
            world
                .get_component_by_id_as::<InputXRGamepadComponent>(xr_gamepad)
                .unwrap()
                .locomotion
        );
    }

    #[test]
    fn input_live_translation_and_rotation_gates_are_independent() {
        let mut world = World::default();
        let input = world.add_component(InputComponent::new());

        invoke_component_method(
            &mut world,
            input,
            "Input",
            "set_translation_enabled",
            &[Value::Bool(false)],
            |_| {},
        )
        .unwrap();
        let state = world
            .get_component_by_id_as::<InputComponent>(input)
            .unwrap();
        assert!(state.enabled);
        assert!(!state.translation_enabled);
        assert!(state.rotation_enabled);

        invoke_component_method(
            &mut world,
            input,
            "Input",
            "set_rotation_enabled",
            &[Value::Bool(false)],
            |_| {},
        )
        .unwrap();
        let state = world
            .get_component_by_id_as::<InputComponent>(input)
            .unwrap();
        assert!(state.enabled);
        assert!(!state.translation_enabled);
        assert!(!state.rotation_enabled);
    }

    #[test]
    fn transform_trs_is_an_opaque_copied_value_and_setter_emits_existing_update_intent() {
        let mut world = World::default();
        let source = world.add_component(
            TransformComponent::new()
                .with_position(1.0, 2.0, 3.0)
                .with_rotation_quat([0.0, 0.0, 0.70710677, 0.70710677])
                .with_scale(4.0, 5.0, 6.0),
        );
        let target = world.add_component(TransformComponent::new());
        let component_count = world.all_components().count();

        let pose = invoke_component_method(&mut world, source, "Transform", "trs", &[], |_| {})
            .expect("local TRS getter");
        assert_eq!(
            pose,
            Value::TransformTrs(crate::engine::transform::TransformTrs::new(
                [1.0, 2.0, 3.0],
                [0.0, 0.0, 0.70710677, 0.70710677],
                [4.0, 5.0, 6.0],
            ))
        );

        let mut emitted = Vec::new();
        invoke_component_method(&mut world, target, "Transform", "trs", &[pose], |intent| {
            emitted.push(intent)
        })
        .expect("local TRS setter");

        assert!(matches!(
            emitted.as_slice(),
            [IntentValue::UpdateTransform {
                component_id,
                translation: [1.0, 2.0, 3.0],
                rotation_quat_xyzw: [0.0, 0.0, 0.70710677, 0.70710677],
                scale: [4.0, 5.0, 6.0],
            }] if *component_id == target
        ));
        assert_eq!(world.all_components().count(), component_count);

        let error = invoke_component_method(
            &mut world,
            target,
            "Transform",
            "trs",
            &[Value::Array(Vec::new())],
            |_| {},
        )
        .expect_err("an array is not an opaque TransformTrs value");
        assert!(error.contains("one TransformTrs value"), "{error}");
    }

    #[test]
    fn transform_world_trs_reads_cached_pose_and_emits_a_world_space_setter() {
        let mut world = World::default();
        let source = world.add_component(TransformComponent::new().with_position(3.0, 4.0, 5.0));
        let target = world.add_component(TransformComponent::new());
        let source_transform = world
            .get_component_by_id_as_mut::<TransformComponent>(source)
            .unwrap();
        source_transform.transform.matrix_world = source_transform.transform.model;

        let pose =
            invoke_component_method(&mut world, source, "TransformWorld", "trs", &[], |_| {})
                .expect("world TRS getter");
        let mut emitted = Vec::new();
        invoke_component_method(
            &mut world,
            target,
            "TransformWorld",
            "trs",
            &[pose],
            |intent| emitted.push(intent),
        )
        .expect("world TRS setter");

        assert!(matches!(
            emitted.as_slice(),
            [IntentValue::SetTransformTrs {
                component_id,
                space: TransformSpace::World,
                value,
            }] if *component_id == target && value.translation == [3.0, 4.0, 5.0]
        ));
    }

    #[test]
    fn new_live_methods_validate_and_emit_scalar_recipients() {
        let mut world = World::default();
        let parent = world.add_component(TransformComponent::new());
        let child = world.add_component(ColorComponent::rgba(1.0, 1.0, 1.0, 1.0));
        let prefab = world.add_component(TransformComponent::new());
        let raycaster = world.add_component(RayCastComponent::event_driven());
        let band_pass = world.add_component(AudioBandPassFilterComponent::default());
        let mut emitted = Vec::new();

        invoke_component_method(
            &mut world,
            parent,
            "Transform",
            "set_color",
            &[Value::Array(vec![
                Value::Number(0.1),
                Value::Number(0.2),
                Value::Number(0.3),
                Value::Number(1.0),
            ])],
            |intent| emitted.push(intent),
        )
        .unwrap();
        invoke_component_method(&mut world, child, "Color", "detach", &[], |intent| {
            emitted.push(intent)
        })
        .unwrap();
        invoke_component_method(
            &mut world,
            parent,
            "Transform",
            "attach_clone",
            &[object(prefab, "Transform")],
            |intent| emitted.push(intent),
        )
        .unwrap();
        invoke_component_method(
            &mut world,
            parent,
            "Transform",
            "remove_child",
            &[Value::Number(2.0)],
            |intent| emitted.push(intent),
        )
        .unwrap();
        invoke_component_method(
            &mut world,
            prefab,
            "Transform",
            "remove_subtree",
            &[],
            |intent| emitted.push(intent),
        )
        .unwrap();
        invoke_component_method(
            &mut world,
            raycaster,
            "Raycast",
            "request_raycast",
            &[],
            |intent| emitted.push(intent),
        )
        .unwrap();
        invoke_component_method(
            &mut world,
            band_pass,
            "AudioBandPassFilter",
            "set_center_hz",
            &[Value::Number(880.0)],
            |intent| emitted.push(intent),
        )
        .unwrap();

        assert!(matches!(
            emitted[0],
            IntentValue::SetColor { component_id, .. } if component_id == parent
        ));
        assert!(matches!(
            emitted[1],
            IntentValue::Detach { component_id } if component_id == child
        ));
        assert!(matches!(
            emitted[2],
            IntentValue::AttachClone { parent: id, prefab_root }
                if id == parent && prefab_root == prefab
        ));
        assert!(matches!(
            emitted[3],
            IntentValue::RemoveChild { parent: id, index: 2 } if id == parent
        ));
        assert!(matches!(
            emitted[4],
            IntentValue::RemoveSubtree { component_id } if component_id == prefab
        ));
        assert!(matches!(
            emitted[5],
            IntentValue::RequestRaycast { component_id } if component_id == raycaster
        ));
        assert!(matches!(
            emitted[6],
            IntentValue::AudioBandPassSetCenterHz { component_id, center_hz }
                if component_id == band_pass && center_hz == 880.0
        ));

        assert!(
            invoke_component_method(
                &mut world,
                parent,
                "Transform",
                "remove_child",
                &[Value::Number(-1.0)],
                |_| {},
            )
            .is_err()
        );
        world.remove_component_leaf(child).unwrap();
        assert!(
            invoke_component_method(&mut world, child, "Color", "detach", &[], |_| {}).is_err()
        );
    }
}
