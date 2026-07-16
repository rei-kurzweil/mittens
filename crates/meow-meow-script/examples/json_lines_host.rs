use meow_meow_script::{
    ComponentSpec, HostApiSpec, HostCapabilities, JsonLinesHost, Runtime, ValueSignature,
    ValueType,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = configured_runtime()?;
    let host = JsonLinesHost::new(Vec::new(), configured_capabilities());
    let mut session = runtime.session(host)?;

    session.eval(
        r#"
        button.new("save") {
            label = "Save"
        }

        audit.write("button emitted")
        "#,
    )?;

    let bytes = session.host_mut().into_inner_ref();
    print!("{}", String::from_utf8_lossy(bytes));

    Ok(())
}

fn configured_runtime() -> Result<Runtime, Box<dyn std::error::Error>> {
    let mut builder = Runtime::builder();
    builder.register_component(
        ComponentSpec::new("Button")
            .alias("button")
            .constructor(
                "new",
                ValueSignature::new(vec![ValueType::String], ValueType::Component),
            )
            .property("label", ValueType::String)
            .method("click", ValueSignature::new(vec![], ValueType::Null)),
    )?;
    builder.register_host_api(
        HostApiSpec::method(
            "audit",
            "write",
            ValueSignature::new(vec![ValueType::String], ValueType::Null),
        )
        .requires("audit.write"),
    )?;
    Ok(builder.build())
}

fn configured_capabilities() -> HostCapabilities {
    HostCapabilities::default()
        .supports_component("Button")
        .supports_api("audit.write")
}
