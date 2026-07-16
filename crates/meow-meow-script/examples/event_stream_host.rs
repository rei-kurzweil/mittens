use meow_meow_script::{
    ComponentSpec, EventStreamHost, HostApiSpec, HostCapabilities, Runtime, ValueSignature,
    ValueType,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = configured_runtime()?;
    let host = EventStreamHost::new(configured_capabilities());
    let mut session = runtime.session(host)?;

    session.eval(
        r#"
        panel.new(320) {
            title = "Inventory"
        }

        telemetry.record({
            screen = "inventory",
            count = 12,
        })
        "#,
    )?;

    for event in &session.host().events {
        println!("{event:?}");
    }

    Ok(())
}

fn configured_runtime() -> Result<Runtime, Box<dyn std::error::Error>> {
    let mut builder = Runtime::builder();
    builder.register_component(
        ComponentSpec::new("Panel")
            .alias("panel")
            .constructor(
                "new",
                ValueSignature::new(vec![ValueType::Number], ValueType::Component),
            )
            .property("title", ValueType::String)
            .method("show", ValueSignature::new(vec![], ValueType::Null))
            .normalize_with(|tree| {
                tree.component_type = "Panel".to_string();
                Ok(())
            }),
    )?;
    builder.register_host_api(
        HostApiSpec::method(
            "telemetry",
            "record",
            ValueSignature::new(vec![ValueType::Any], ValueType::Null),
        )
        .requires("telemetry.record"),
    )?;
    Ok(builder.build())
}

fn configured_capabilities() -> HostCapabilities {
    HostCapabilities::default()
        .supports_component("Panel")
        .supports_api("telemetry.record")
}
