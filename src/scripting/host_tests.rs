use super::*;

#[test]
fn material_inputs_use_type_records_for_definitions_and_values_for_instances() {
    let configured = crate::scripting::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let host =
        MittensHost::new(&mut world, &mut queue, &mut intents).with_bindings(configured.bindings());
    let mut session = configured.runtime().session(host);
    session
        .eval(
            r#"
            import { f32 as scalar, vec2, vec3, vec4, str } from "types"
            let schema = {
                gain: scalar
                phase: scalar
            }
            let definition = MaterialDefinition(MaterialBase())
                .fragment("assets/shaders/youtuber-led-wave.frag")
                .inputs(schema)
            let initial = 0.75
            let first = Material(definition).inputs({
                gain = initial
                phase = initial * 2
            })
            let second = Material(definition).inputs({gain = 0.25,})
            first.set_input("gain", 0.9)
        "#,
        )
        .unwrap();
    for (expression, expected) in [
        ("first.get_input(\"gain\")", 0.9_f32 as f64),
        ("first.get_input(\"phase\")", 1.5),
        ("second.get_input(\"gain\")", 0.25),
        ("second.get_input(\"phase\")", 0.0),
    ] {
        assert_eq!(
            session.eval(expression).unwrap().value,
            Some(mms::Value::Number(expected))
        );
    }
    session
        .eval(
            r#"
            let vectors = MaterialDefinition(MaterialBase()).inputs({ uv: vec2
                tint: vec3
                rgba: vec4 })
            let vector_values = Material(vectors).inputs({ uv = [0.25,0.5]
                tint = [0.1,0.2,0.3] })
            let anime = MaterialDefinition.anime()
            let anime_again = MaterialDefinition.anime()
            let builtin = Material(anime)
            let builtin_defaults = builtin.get_input("rim_color")
            builtin.set_input("rim_color", [0.25,0.5,0.75])
        "#,
        )
        .unwrap();
    assert_eq!(
        session
            .eval("vector_values.get_input(\"uv\")")
            .unwrap()
            .value,
        Some(mms::Value::Array(vec![
            mms::Value::Number(0.25),
            mms::Value::Number(0.5)
        ]))
    );
    assert_eq!(
        session
            .eval("vector_values.get_input(\"rgba\")")
            .unwrap()
            .value,
        Some(mms::Value::Array(vec![mms::Value::Number(0.0); 4]))
    );
    assert_eq!(
        session.eval("anime == anime_again").unwrap().value,
        Some(mms::Value::Bool(true))
    );
    for invalid in [
        "vector_values.set_input(\"tint\", [1,2])",
        "vector_values.set_input(\"tint\", [1,2,3,4])",
        "vector_values.set_input(\"uv\", [1,scalar])",
        "vector_values.set_input(\"uv\", [1,1e300])",
        "vector_values.set_input(\"rgba\", 1)",
        "anime.inputs({anything: scalar})",
        "anime.fragment(\"anything.frag\")",
    ] {
        assert!(session.eval(invalid).is_err(), "{invalid}");
    }
    for (source, message) in [
        (
            "MaterialDefinition(MaterialBase()).inputs({gain=1})",
            "record_type",
        ),
        (
            "MaterialDefinition(MaterialBase()).inputs({gain=scalar})",
            "record_type",
        ),
        (
            "MaterialDefinition(MaterialBase()).inputs({gain: str})",
            "unsupported material input type",
        ),
        (
            "MaterialDefinition(MaterialBase()).inputs({gain: missing})",
            "primitive type tag",
        ),
        ("Material(definition).inputs(schema)", "table"),
        ("Material(definition).inputs({gain=scalar})", "finite f32"),
        ("Material(definition).inputs({gain=0/0})", "finite f32"),
        (
            "Material(definition).inputs({gain=10000000000000000000000000000000000000000})",
            "finite f32",
        ),
        (
            "Material(definition).inputs({missing=1})",
            "unknown material field",
        ),
        ("JSON.stringify(schema)", "not JSON"),
    ] {
        let error = session.eval(source).unwrap_err().to_string();
        assert!(error.contains(message), "{source}: {error}");
    }
    let value = session.eval("schema").unwrap().value.unwrap();
    let retained = external_value_to_legacy(value.clone()).unwrap();
    let formatted =
        crate::scripting::repl::format_repl_value(&retained, session.host().world).unwrap();
    assert!(formatted.contains("gain: f32"), "{formatted}");
    assert_eq!(legacy_value_to_external(retained).unwrap(), value);
    assert_eq!(session.host().legacy_component_fallbacks(), 0);
    assert_eq!(session.host().legacy_method_fallbacks(), 0);
}

#[test]
fn custom_material_mms_schemas_and_instances_validate_before_attachment() {
    let configured = crate::scripting::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let host =
        MittensHost::new(&mut world, &mut queue, &mut intents).with_bindings(configured.bindings());
    let mut session = configured.runtime().session(host);
    session.eval(r#"
            import { f32 } from "types"
            let base = MaterialBase()
            let one = MaterialDefinition(base).fragment("assets/shaders/youtuber-led-wave.frag").input_f32("gain")
            let two = MaterialDefinition(base).fragment("assets/shaders/youtuber-led-wave.frag").input_f32("speed").input_f32("phase")
            let first = Material(one).descriptor_set({gain=0.75})
            let second = Material(one).descriptor_set({gain=0.25})
            let third = Material(two).descriptor_set({speed=1.5})
            first.set_input("gain", 0.9)
        "#).unwrap();
    assert_eq!(
        session.eval("first.get_input(\"gain\")").unwrap().value,
        Some(mms::Value::Number(0.9_f32 as f64))
    );
    assert_eq!(
        session.eval("second.get_input(\"gain\")").unwrap().value,
        Some(mms::Value::Number(0.25))
    );
    assert_eq!(
        session.eval("third.get_input(\"phase\")").unwrap().value,
        Some(mms::Value::Number(0.0))
    );
    for invalid in [
        "first.set_input(\"absent\", 1.0)",
        "first.set_input(\"gain\", 0/0)",
        "first.set_input(\"gain\", 1e300)",
        "first.set_input(\"gain\", f32)",
        "Material(one).descriptor_set({gain=f32})",
        "Material(one).descriptor_set({absent=1.0})",
        "MaterialDefinition(base).input_f32(\"duplicate\").input_f32(\"duplicate\")",
    ] {
        assert!(session.eval(invalid).is_err(), "{invalid}");
    }
    assert_eq!(session.host().legacy_component_fallbacks(), 0);
    assert_eq!(session.host().legacy_method_fallbacks(), 0);
}

#[test]
fn type_tags_work_in_the_configured_mittens_runtime_and_repl() {
    let configured = crate::scripting::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let host =
        MittensHost::new(&mut world, &mut queue, &mut intents).with_bindings(configured.bindings());
    let mut session = configured.runtime().session(host);
    let value = session
        .eval(
            r#"
            import { f32, vec2, vec3, vec4 } from "types"
            let schema = { progress = f32, direction = vec2, tint = vec3, color = vec4 }
            let read = fn() { return schema.progress }
            read()
        "#,
        )
        .unwrap()
        .value
        .unwrap();
    assert_eq!(value, mms::Value::TypeTag(mms::TypeTag::F32));
    let retained = external_value_to_legacy(value.clone()).unwrap();
    assert_eq!(
        crate::scripting::repl::format_repl_value(&retained, session.host().world).unwrap(),
        "<type f32>"
    );
    assert_eq!(legacy_value_to_external(retained).unwrap(), value);
    let schema = external_value_to_legacy(session.eval("schema").unwrap().value.unwrap()).unwrap();
    let formatted =
        crate::scripting::repl::format_repl_value(&schema, session.host().world).unwrap();
    assert!(formatted.contains("progress = f32"), "{formatted}");
    assert!(formatted.contains("direction = vec2"), "{formatted}");
    assert_eq!(session.host().legacy_component_fallbacks(), 0);
}

#[test]
fn type_tags_are_not_silently_coerced_through_json() {
    let configured = crate::scripting::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let host =
        MittensHost::new(&mut world, &mut queue, &mut intents).with_bindings(configured.bindings());
    let mut session = configured.runtime().session(host);
    session.eval("import { f32 } from \"types\"").unwrap();
    for source in ["JSON.stringify(f32)", "JSON.stringify({ field = f32 })"] {
        let error = session.eval(source).unwrap_err().to_string();
        assert!(error.contains("type tags are not JSON values"), "{error}");
    }
    assert_eq!(
        session
            .eval(
                r#"
            let parsed = JSON.parse("{\"field\":\"f32\"}")
            parsed.field != f32
        "#
            )
            .unwrap()
            .value,
        Some(mms::Value::Bool(true))
    );
}

#[test]
fn emit_data_queues_named_events_and_validates_optional_component_payloads() {
    let configured = crate::scripting::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let host =
        MittensHost::new(&mut world, &mut queue, &mut intents).with_bindings(configured.bindings());
    let mut session = configured.runtime().session(host);
    session
        .eval(
            r#"
            let scope = Transform { name = "event_scope" }
            let payload = Transform { name = "event_payload" }
            emit_data(scope, "without_payload")
            emit_data(scope, "null_payload", null)
            emit_data(scope, "component_payload", payload)
        "#,
        )
        .unwrap();
    let label = |world: &World, name: &str| {
        world
            .all_components()
            .find(|&id| world.component_label(id) == Some(name))
            .unwrap()
    };
    let scope = label(session.host().world, "event_scope");
    let payload = label(session.host().world, "event_payload");
    for invalid in [
        "emit_data(scope)",
        "emit_data(null, \"invalid\")",
        "emit_data(scope, 1)",
        "emit_data(scope, \"invalid\", 42)",
        "emit_data(scope, \"invalid\", null, null)",
    ] {
        assert!(session.eval(invalid).is_err(), "accepted {invalid}");
    }
    session
        .host_mut()
        .world
        .remove_component_subtree(payload)
        .unwrap();
    assert!(
        session
            .eval("emit_data(scope, \"stale\", payload)")
            .is_err()
    );
    drop(session);

    let mut rx = RxWorld::default();
    queue.drain_into_rx(&mut rx);
    let events: Vec<_> = rx
        .drain_ready_events()
        .into_iter()
        .filter_map(|signal| match signal.event {
            Some(crate::engine::ecs::EventSignal::DataEvent { name, payload }) => {
                assert_eq!(signal.scope, scope);
                Some((name, payload))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        events,
        [
            ("without_payload".into(), None),
            ("null_payload".into(), None),
            ("component_payload".into(), Some(payload)),
        ]
    );
}

#[test]
fn component_handles_round_trip_losslessly() {
    let mut world = World::default();
    let id = world.add_component(crate::engine::ecs::component::DataComponent::default());
    assert_eq!(
        MittensHost::component_id(MittensHost::component_handle(id)),
        id
    );
}

#[test]
fn runtime_spec_smoke_uses_no_legacy_component_conversion() {
    let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut command_queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let invocations = Arc::new(Mutex::new(Vec::new()));
    let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
        .with_bindings(configured.bindings())
        .with_rx(&mut rx)
        .with_callback_invocations(invocations);
    let mut session = configured.runtime().session(host);

    session
        .eval(include_str!("../../examples/runtime-spec-smoke.mms"))
        .unwrap();
    session.eval("let cube = R.cube() {} T { cube }").unwrap();

    assert_eq!(session.host().legacy_component_fallbacks(), 0);
}

#[test]
fn configured_components_outside_the_direct_slice_use_the_explicit_fallback() {
    let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut command_queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
        .with_bindings(configured.bindings());
    let mut session = configured.runtime().session(host);

    session.eval("Grid.spacing(1.0) {}").unwrap();

    assert_eq!(session.host().legacy_component_fallbacks(), 1);
}

#[test]
fn configured_component_methods_dispatch_by_operation_id() {
    use mms::Host;

    let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
    let translation = configured
        .spec()
        .component("Transform")
        .unwrap()
        .method("translation")
        .unwrap()
        .operation_id()
        .unwrap();
    let smoke_api = configured
        .spec()
        .api(Some("mittens"), "smoke")
        .unwrap()
        .operation_id();

    let mut world = World::default();
    let id = world.add_component(
        crate::engine::ecs::component::TransformComponent::new().with_position(1.0, 2.0, 3.0),
    );
    let mut command_queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let mut host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
        .with_bindings(configured.bindings());

    let response = host
        .dispatch(mms::HostRequest::InvokeComponentMethod {
            operation_id: translation,
            component: MittensHost::component_handle(id),
            args: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        response,
        mms::HostResponse::Value(mms::Value::Array(vec![
            mms::Value::Number(1.0),
            mms::Value::Number(2.0),
            mms::Value::Number(3.0),
        ]))
    );

    let error = host
        .dispatch(mms::HostRequest::InvokeComponentMethod {
            operation_id: smoke_api,
            component: MittensHost::component_handle(id),
            args: Vec::new(),
        })
        .unwrap_err();
    assert_eq!(error.kind, mms::HostErrorKind::InvalidRequest);
    assert!(
        error
            .message
            .contains("cannot be invoked as a component method")
    );
}

#[test]
fn configured_pointer_grip_reference_uses_direct_authoring_with_other_options() {
    use crate::engine::ecs::component::{ComponentRef, PointerComponent};
    let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut commands = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    {
        let host = MittensHost::new(&mut world, &mut commands, &mut intents)
            .with_bindings(configured.bindings());
        let mut session = configured.runtime().session(host);
        session.eval(r#"Pointer.grab_attachment("[name='grip']").min_grab_distance(0.1).debug_enable(true) { name = "hand_pointer" }"#).unwrap();
        assert_eq!(session.host().legacy_component_fallbacks(), 0);
    }
    let pointer = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<PointerComponent>(id))
        .unwrap();
    assert_eq!(
        pointer.grab_attachment,
        Some(ComponentRef::Query("[name='grip']".into()))
    );
    assert_eq!(pointer.min_grab_distance, Some(0.1));
    assert!(pointer.debug_enabled);
}

#[test]
fn configured_audio_clip_play_schedules_directly_without_music_notes() {
    let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let clip = world.add_component(crate::engine::ecs::component::AudioClipComponent::new(
        "assets/audio/tool/laser_machine_pistol_C.0.wav",
    ));
    world.get_component_record_mut(clip).unwrap().name = "shot_sound".into();
    let mut commands = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    {
        let host = MittensHost::new(&mut world, &mut commands, &mut intents)
            .with_bindings(configured.bindings());
        let mut session = configured.runtime().session(host);
        session.eval("query(\"#shot_sound\").play()").unwrap();
        assert!(session.eval("query(\"#shot_sound\").play(1.0)").is_err());
        assert_eq!(session.host().legacy_method_fallbacks(), 0);
    }
    assert!(
        matches!(intents.as_slice(), [IntentValue::AudioSchedulePlay {
            component_id, beat_offset: 0.0, beat_context: None,
            note: None, gain: None, rate: None, duration: None,
        }] if *component_id == clip)
    );
}

#[test]
fn configured_method_call_never_uses_the_by_name_compatibility_request() {
    let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let id = world.add_component(
        crate::engine::ecs::component::TransformComponent::new().with_position(4.0, 5.0, 6.0),
    );
    world.get_component_record_mut(id).unwrap().name = "target".into();
    let mut command_queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
        .with_bindings(configured.bindings());
    let mut session = configured.runtime().session(host);

    session.eval("query(\"#target\").translation()").unwrap();

    assert_eq!(session.host().legacy_method_fallbacks(), 0);
}

#[test]
fn configured_signal_registration_resolves_to_an_opaque_mittens_route() {
    let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
    let click_id = configured.spec().signal("Click").unwrap().operation_id();
    assert_eq!(
        configured.bindings().get(click_id),
        Some(&super::super::runtime_config::MittensBinding::Signal { name: "Click" })
    );

    let mut world = World::default();
    let root = world.add_component(crate::engine::ecs::component::TransformComponent::new());
    world.get_component_record_mut(root).unwrap().name = "signal-root".into();
    let mut command_queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let mut routes = Vec::new();
    let mut rx = RxWorld::default();
    let invocations = Arc::new(Mutex::new(Vec::new()));
    let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
        .with_bindings(configured.bindings())
        .with_signal_routes(&mut routes)
        .with_rx(&mut rx)
        .with_callback_invocations(Arc::clone(&invocations));
    let mut session = configured.runtime().session(host);

    session
        .eval("on(query(\"#signal-root\"), \"Click\", fn(event) {})")
        .unwrap();
    drop(session);

    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].kind, SignalKind::Click);
    assert!(routes[0].scope.is_some());

    rx.dispatch_event_handlers(
        &mut world,
        &crate::engine::ecs::Signal::event(
            root,
            crate::engine::ecs::EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: root,
                hit_point: [0.0; 3],
                screen_pos_px: None,
            },
        ),
    );
    let invocations = invocations.lock().unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0].callback, routes[0].callback);
    assert_eq!(invocations[0].args, vec![mms::TransportValue::Null]);
}

#[test]
fn let_bound_component_attaches_and_its_signal_callback_uses_the_live_handle() {
    let configured = super::super::runtime_config::build_mittens_runtime().unwrap();
    let mut world = World::default();
    let mut command_queue = crate::engine::ecs::CommandQueue::new();
    let mut intents = Vec::new();
    let mut routes = Vec::new();
    let mut rx = RxWorld::default();
    let invocations = Arc::new(Mutex::new(Vec::new()));
    let host = MittensHost::new(&mut world, &mut command_queue, &mut intents)
        .with_bindings(configured.bindings())
        .with_signal_routes(&mut routes)
        .with_rx(&mut rx)
        .with_callback_invocations(Arc::clone(&invocations));
    let mut session = configured.runtime().session(host);

    session
        .eval(
            r#"
                let button = Transform.position(1.0, 2.0, 3.0) { name = "live-button" }
                on(button, "Click", fn(event) { return button.translation() })
                Transform { name = "root" button }
                "#,
        )
        .unwrap();

    assert_eq!(session.host().legacy_component_fallbacks(), 0);
    assert_eq!(session.host().legacy_method_fallbacks(), 0);
    let button = session.host().signal_routes().unwrap()[0].scope.unwrap();
    let event = crate::engine::ecs::Signal::event(
        button,
        crate::engine::ecs::EventSignal::Click {
            raycaster: ComponentId::default(),
            renderable: button,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    session.host_mut().dispatch_event_handlers(&event).unwrap();
    let invocation = invocations.lock().unwrap().remove(0);
    let result = session.invoke_callback_invocation(invocation).unwrap();

    assert_eq!(
        result,
        mms::Value::Array(vec![
            mms::Value::Number(1.0),
            mms::Value::Number(2.0),
            mms::Value::Number(3.0),
        ])
    );
    assert_eq!(session.host().legacy_method_fallbacks(), 0);
}
