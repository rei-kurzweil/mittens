use meow_meow_script::{
    CallbackInvocation, EventStreamHost, HostEvent, Runtime, RuntimeSpec, StandardHost,
    TransportValue, TypeTag, Value, ValueSignature, ValueType,
};

#[test]
fn field_validation_does_not_duplicate_json_lines_host_effects() {
    let mut session = Runtime::standard().session(meow_meow_script::JsonLinesHost::new(Vec::new()));
    session
        .eval("let node = T {}; node.count = 1; node.read = fn() { return node.count }")
        .unwrap();
    assert_eq!(
        session.eval("node.read()").unwrap().value,
        Some(Value::Number(1.0))
    );
    let lines = std::str::from_utf8(session.host().into_inner_ref()).unwrap();
    assert_eq!(
        lines.lines().count(),
        1,
        "only component registration is an effect: {lines}"
    );
}

#[test]
fn component_fields_retain_values_aliases_and_closures_between_evaluations() {
    let mut session = Runtime::standard().session(StandardHost::new());
    session
        .eval(
            r#"
        import { f32 } from "types"
        fn counter() {
            let state = { hits = 0 }
            let root = T {}
            root.label = "target"
            root.enabled = true
            root.weight = 1.5
            root.tags = ["metal", "lime"]
            root.schema = { intensity: f32 }
            root.scalar_type = f32
            root.settings = { scale = 2.0 }
            root.empty = null
            root.on_hit = fn(amount) {
                state.hits = state.hits + amount
                return state.hits
            }
            root.count = fn() { return state.hits }
            return root
        }
        let first = counter()
        let second = counter()
        let holder = T {}
        holder.target = first
        let alias = holder.target
        let settings = alias.settings
        settings.scale = 4.0
    "#,
        )
        .unwrap();
    for (source, expected) in [
        ("alias == first", Value::Bool(true)),
        ("alias.label", Value::String("target".into())),
        ("first.enabled", Value::Bool(true)),
        ("first.weight", Value::Number(1.5)),
        ("first.tags[1]", Value::String("lime".into())),
        ("first.schema.intensity", Value::TypeTag(TypeTag::F32)),
        ("first.scalar_type", Value::TypeTag(TypeTag::F32)),
        ("first.settings.scale", Value::Number(4.0)),
        ("first.empty", Value::Null),
        ("first.missing", Value::Null),
        ("alias.on_hit(2)", Value::Number(2.0)),
        ("first.on_hit(3)", Value::Number(5.0)),
        ("first.count()", Value::Number(5.0)),
        ("second.count()", Value::Number(0.0)),
    ] {
        assert_eq!(
            session.eval(source).unwrap().value,
            Some(expected),
            "{source}"
        );
    }
    session
        .eval("alias.on_hit = fn(amount) { return amount * 10 }; first.weight = 7")
        .unwrap();
    assert_eq!(
        session.eval("first.on_hit(3)").unwrap().value,
        Some(Value::Number(30.0))
    );
    assert_eq!(
        session.eval("alias.weight").unwrap().value,
        Some(Value::Number(7.0))
    );
    for invalid in ["first.weight()", "first.empty()", "first.missing()"] {
        assert!(session.eval(invalid).is_err(), "accepted {invalid}");
    }
}

#[test]
fn function_fields_survive_callback_delivery_without_an_implicit_receiver() {
    let mut builder = RuntimeSpec::builder::<()>();
    builder.with_standard_builtins();
    builder.component("T", |_| {});
    builder.signal("Hit", Vec::<(String, ValueType)>::new(), ());
    let configured = builder.build().unwrap();
    let mut session = configured.runtime().session(EventStreamHost::new());
    session
        .eval(
            r#"
        let target = T {}
        let state = { hits = 0 }
        target.on_hit = fn(amount) { state.hits = state.hits + amount }
        target.count = fn() { return state.hits }
        on_global("Hit", fn(event) { event.target.on_hit(event.amount) })
    "#,
        )
        .unwrap();
    let callback = session
        .host()
        .events
        .iter()
        .find_map(|event| match event {
            HostEvent::SignalById { callback, .. } => Some(*callback),
            _ => None,
        })
        .unwrap();
    let Value::ComponentObject { id, .. } = session.eval("target").unwrap().value.unwrap() else {
        panic!("component")
    };
    session
        .invoke_callback_invocation(CallbackInvocation {
            callback,
            args: vec![TransportValue::Table(vec![
                (
                    "target".into(),
                    TransportValue::TypedComponent {
                        handle: id,
                        component_type: "component-alias".into(),
                    },
                ),
                ("amount".into(), TransportValue::Number(3.0)),
            ])],
        })
        .unwrap();
    assert_eq!(
        session.eval("target.count()").unwrap().value,
        Some(Value::Number(3.0))
    );
}

#[test]
fn foreign_component_field_access_keeps_a_typed_error() {
    let mut builder = RuntimeSpec::builder::<()>();
    builder.component("T", |_| {});
    builder.signal("Access", Vec::<(String, ValueType)>::new(), ());
    let configured = builder.build().unwrap();
    let mut session = configured.runtime().session(EventStreamHost::new());
    session
        .eval("on_global(\"Access\", fn(target) { return target.label })")
        .unwrap();
    let callback = session
        .host()
        .events
        .iter()
        .find_map(|event| match event {
            HostEvent::SignalById { callback, .. } => Some(*callback),
            _ => None,
        })
        .unwrap();
    let mut foreign = configured.runtime().session(EventStreamHost::new());
    foreign
        .eval("let target = T {}; target.label = \"foreign\"")
        .unwrap();
    let target = foreign.eval("target").unwrap().value.unwrap();
    let meow_meow_script::EvalError::Host(error) =
        session.invoke_callback(callback, vec![target]).unwrap_err()
    else {
        panic!("typed foreign handle error")
    };
    assert_eq!(error.kind, meow_meow_script::HostErrorKind::ForeignHandle);
}

#[test]
fn script_fields_cannot_replace_native_methods_properties_or_navigation() {
    let mut builder = RuntimeSpec::builder::<()>();
    builder.component("T", |component| {
        component.property("name", ValueType::String);
        component.method("reset", ValueSignature::new(vec![], ValueType::Null), ());
    });
    let configured = builder.build().unwrap();
    let mut session = configured.runtime().session(EventStreamHost::new());
    session.eval("let node = T {}").unwrap();
    for field in ["reset", "RESET", "name", "query", "query_all"] {
        let error = session
            .eval(&format!("node.{field} = fn() {{}}"))
            .unwrap_err();
        assert!(
            error.to_string().contains("native component member"),
            "{error}"
        );
    }
    session.eval("node.reset()").unwrap();
    assert!(
        session
            .host()
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::MethodById { .. }))
    );
}
