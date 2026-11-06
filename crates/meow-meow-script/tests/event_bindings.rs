use meow_meow_script::*;

fn runtime() -> ConfiguredRuntime<u8> {
    let mut builder = RuntimeSpec::builder::<u8>();
    builder
        .with_standard_builtins()
        .host_component("T", 0, |_| {})
        .signal("FrameTick", Vec::new(), 1)
        .signal("Click", Vec::new(), 2);
    builder.build().unwrap()
}
fn invoke(session: &mut Session<EventStreamHost>, callback: CallbackHandle) -> Value {
    session
        .invoke_callback_invocation(CallbackInvocation {
            callback,
            args: vec![TransportValue::Number(1.)],
        })
        .unwrap()
}
#[test]
fn world_and_component_scopes_return_binding_resources() {
    let configured = runtime();
    let mut session = configured.runtime().session(EventStreamHost::new());
    let world = session
        .eval("on(world, \"FrameTick\", fn(event) {return event})")
        .unwrap()
        .value
        .unwrap();
    let Value::EventBinding(handle) = world else {
        panic!("expected binding")
    };
    assert!(
        matches!(session.host().events.last(),Some(HostEvent::SignalById {scope:None,callback,..}) if *callback==handle)
    );
    assert_eq!(invoke(&mut session, handle), Value::Number(1.));
    session
        .eval("let node=T {}; let binding=on(node, \"Click\", fn(event) {})")
        .unwrap();
    assert!(matches!(
        session.host().events.last(),
        Some(HostEvent::SignalById { scope: Some(_), .. })
    ));
}
#[test]
fn bypass_cancels_queued_delivery_resumes_and_remove_is_permanent() {
    let configured = runtime();
    let mut session = configured.runtime().session(EventStreamHost::new());
    let Value::EventBinding(handle) = session
        .eval("let binding=on(world, \"FrameTick\", fn(event) {return 7}); binding")
        .unwrap()
        .value
        .unwrap()
    else {
        panic!()
    };
    session.eval("binding.bypass(true)").unwrap();
    assert_eq!(invoke(&mut session, handle), Value::Null);
    let events = session.host().events.len();
    session.eval("binding.bypass(true)").unwrap();
    assert_eq!(
        events,
        session.host().events.len(),
        "repeated bypass is a no-op"
    );
    session.eval("binding.bypass(false)").unwrap();
    assert_eq!(invoke(&mut session, handle), Value::Number(7.));
    session.eval("binding.remove()").unwrap();
    assert_eq!(
        invoke(&mut session, handle),
        Value::Null,
        "queued removed work is discarded"
    );
    let events = session.host().events.len();
    session.eval("binding.remove()").unwrap();
    assert_eq!(events, session.host().events.len());
    assert!(
        session
            .eval("binding.bypass(false)")
            .unwrap_err()
            .to_string()
            .contains("removed")
    );
    assert!(
        session.invoke_callback(handle, vec![]).is_err(),
        "direct stale callback remains invalid"
    );
}
#[test]
fn invalid_calls_and_foreign_queued_bindings_are_rejected() {
    let configured = runtime();
    let mut first = configured.runtime().session(EventStreamHost::new());
    let Value::EventBinding(handle) = first
        .eval("let binding=on(world, \"FrameTick\", fn(event) {}); binding")
        .unwrap()
        .value
        .unwrap()
    else {
        panic!()
    };
    for source in [
        "binding.bypass()",
        "binding.bypass(1)",
        "binding.remove(true)",
        "binding.enable()",
        "on(null, \"FrameTick\", fn(event) {})",
    ] {
        assert!(first.eval(source).is_err(), "{source}");
    }
    let mut second = configured.runtime().session(EventStreamHost::new());
    assert!(
        second
            .invoke_callback_invocation(CallbackInvocation {
                callback: handle,
                args: vec![]
            })
            .unwrap_err()
            .to_string()
            .contains("another session")
    );
}
