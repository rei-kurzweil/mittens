use meow_meow_script::{
    CallbackHandle, CallbackInvocation, Host, HostContext, HostError, HostRequest, HostResponse,
    Hostless, LoadedSource, Runtime, RuntimeSpec, SourceId, TransportValue, TypeTag, Value,
    ValueSignature, ValueType,
};

#[test]
fn type_imports_are_language_owned_and_preserve_identity_across_sessions() {
    // Hostless rejects every host request, including LoadSource.
    let runtime = Runtime::standard();
    let mut session = runtime.session(Hostless);
    session
        .eval("import { f32, vec2, vec3, vec4, str } from \"types\"")
        .unwrap();
    for (name, expected) in [
        ("f32", TypeTag::F32),
        ("vec2", TypeTag::Vec2),
        ("vec3", TypeTag::Vec3),
        ("vec4", TypeTag::Vec4),
        ("str", TypeTag::Str),
    ] {
        let value = session.eval(name).unwrap().value;
        assert_eq!(value, Some(Value::TypeTag(expected)));
        let mut another = runtime.session(Hostless);
        assert_eq!(
            another
                .eval(&format!(
                    "import {{ {name} as alias }} from \"types\"; alias"
                ))
                .unwrap()
                .value,
            value
        );
    }
    assert_eq!(
        session
            .eval("import { f32 as float_type } from \"types\"; f32 == float_type")
            .unwrap()
            .value,
        Some(Value::Bool(true))
    );
}

#[test]
fn tags_are_distinct_from_numbers_names_and_other_values() {
    let mut session = Runtime::standard().session(Hostless);
    session
        .eval("import { f32, vec2, vec3, vec4, str } from \"types\"")
        .unwrap();
    for expression in [
        "f32 == f32",
        "f32 != vec2",
        "vec2 != vec3",
        "vec3 != vec4",
        "str != f32",
        "f32 != 0.0",
        "f32 != 1.0",
        "0 != f32",
        "1 != vec2",
        "2 != vec3",
        "3 != vec4",
        "4 != str",
        "f32 != -1",
        "f32 != (0 / 0)",
        "f32 != \"f32\"",
        "str != \"str\"",
        "f32 != null",
        "f32 != false",
        "vec2 != [0, 0]",
        "f32 != {}",
    ] {
        assert_eq!(
            session.eval(expression).unwrap().value,
            Some(Value::Bool(true)),
            "{expression}"
        );
    }
}

#[test]
fn schema_tables_lists_and_closures_retain_tags() {
    let mut session = Runtime::standard().session(Hostless);
    session
        .eval(
            r#"
            import { f32, vec2, vec3, vec4 } from "types"
            let schema = { progress = f32, shapes = [vec2, vec3, vec4] }
            let alias = schema
            let read_type = fn() { return alias.progress }
            "#,
        )
        .unwrap();
    assert_eq!(
        session.eval("read_type()").unwrap().value,
        Some(Value::TypeTag(TypeTag::F32))
    );
    session.eval("schema.progress = vec4").unwrap();
    assert_eq!(
        session.eval("read_type()").unwrap().value,
        Some(Value::TypeTag(TypeTag::Vec4))
    );
    assert_eq!(
        session.eval("schema.shapes[1]").unwrap().value,
        Some(Value::TypeTag(TypeTag::Vec3))
    );
}

#[test]
fn type_records_resolve_aliases_and_remain_distinct_from_values() {
    let mut session = Runtime::standard().session(Hostless);
    session
        .eval(
            r#"
        import { f32 as scalar, vec2 } from "types"
        let schema = {
            progress: scalar // newline separates fields
            direction: vec2
        }
        let same = { direction: vec2, progress: scalar, }
        let values = { progress = 0.0
                       direction = [1, 0] }
        let read = fn() { return schema.progress }
    "#,
        )
        .unwrap();
    for expression in [
        "schema == same",
        "schema != values",
        "schema != {progress=scalar, direction=vec2}",
        "schema != scalar",
        "read() == scalar",
        "schema[\"direction\"] == vec2",
        "({progress: scalar}) != {progress: vec2}",
        "({progress: scalar}) != {different: scalar}",
    ] {
        assert_eq!(
            session.eval(expression).unwrap().value,
            Some(Value::Bool(true)),
            "{expression}"
        );
    }
    for invalid in [
        "schema.progress = vec2",
        "schema + 1",
        "schema + \"text\"",
        "({progress: missing})",
        "let number = 1; ({progress: number})",
        "({progress: scalar, progress: scalar})",
        "({progress: scalar, direction = [1, 0]})",
        "({progress = 0, direction: vec2})",
        "({progress: scalar = 0})",
        "({progress: scalar direction: vec2})",
    ] {
        assert!(session.eval(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn type_record_ast_preserves_roles_spans_and_unparses_without_commas() {
    use meow_meow_script::{
        Expression, MeowMeowParser, MeowMeowTokenizer, Statement, TypeExpression,
    };
    let source = "let schema = { progress: f32, direction: vec2, }";
    let parse = |source: &str| {
        MeowMeowParser::new(MeowMeowTokenizer::new(source).tokenize().unwrap())
            .parse_program()
            .unwrap()
    };
    let ast = parse(source);
    let Statement::Assignment(assignment) = &ast[0] else {
        panic!("assignment")
    };
    let Expression::Type(TypeExpression::Record { fields, span }) = &assignment.value else {
        panic!("type record")
    };
    assert_eq!(
        &source[span.start..span.end],
        "{ progress: f32, direction: vec2, }"
    );
    assert_eq!(
        &source[fields[0].span.start..fields[0].span.end],
        "progress: f32"
    );
    let canonical = meow_meow_script::unparser::unparse_program(&ast);
    assert!(canonical.contains("progress: f32\n"), "{canonical}");
    assert!(!canonical.contains(','));
    assert_eq!(
        meow_meow_script::unparser::unparse_program(&parse(&canonical)),
        canonical
    );
}

#[test]
fn tags_reject_arithmetic_ordering_and_mutation() {
    let mut session = Runtime::standard().session(Hostless);
    session.eval("import { f32 } from \"types\"").unwrap();
    for source in [
        "f32 + 1",
        "1 + f32",
        "f32 - 1",
        "1 - f32",
        "f32 * 2",
        "2 * f32",
        "f32 / 2",
        "2 / f32",
        "f32 % 2",
        "-f32",
        "f32 < 1",
        "1 >= f32",
        "f32 + \"text\"",
        "\"text\" + f32",
        "Math.sin(f32)",
        "f32.name = \"changed\"",
    ] {
        assert!(session.eval(source).is_err(), "{source}");
    }
    assert_eq!(
        session.eval("f32").unwrap().value,
        Some(Value::TypeTag(TypeTag::F32))
    );
}

#[test]
fn imports_are_explicit_with_actionable_errors_and_no_partial_bindings() {
    let mut session = Runtime::standard().session(Hostless);
    for source in ["f32 == f32", "f32 != 0.0", "vec2 == vec2"] {
        let error = session.eval(source).unwrap_err().to_string();
        assert!(
            error.contains("import") && error.contains("types"),
            "{error}"
        );
    }
    // Preserve symbolic enum comparisons outside the type-tag context.
    assert_eq!(
        session.eval("Disabled == Disabled").unwrap().value,
        Some(Value::Bool(true))
    );
    for source in [
        "import { f32, nonexistent } from \"types\"",
        "import { F32 } from \"types\"",
        "import { mat4 } from \"types\"",
        "import ast { f32 } from \"types\"",
        "import { 0 as f32 } from \"types\"",
    ] {
        assert!(session.eval(source).is_err(), "{source}");
    }
    assert!(session.eval("f32 == f32").is_err());
    session.eval("import { f32 } from \"types\"").unwrap();
    for source in ["f32 == vec2", "vec2 != f32", "f32 == typo"] {
        let error = session.eval(source).unwrap_err().to_string();
        assert!(error.contains("unbound type tag"), "{error}");
    }
}

#[derive(Default)]
struct SchemaHost {
    callback: Option<CallbackHandle>,
    loads: Vec<String>,
    calls: usize,
}

impl Host for SchemaHost {
    fn dispatch_with_context(
        &mut self,
        _: &mut HostContext,
        request: HostRequest,
    ) -> Result<HostResponse, HostError> {
        match request {
            HostRequest::CallApiById { args, .. } => {
                self.calls += 1;
                match args.into_iter().next().unwrap() {
                    TransportValue::Callback(handle) => {
                        self.callback = Some(handle);
                        Ok(HostResponse::Unit)
                    }
                    value => Ok(HostResponse::Transport(value)),
                }
            }
            HostRequest::LoadSource { specifier, .. } if specifier == "schema.mms" => {
                self.loads.push(specifier);
                Ok(HostResponse::Source(LoadedSource {
                    identity: SourceId::new("schema.mms"),
                    source: "import { f32, vec3 } from \"types\"; export let amount = f32; export let color = vec3".into(),
                }))
            }
            other => Err(HostError::unsupported(other.operation_name())),
        }
    }
}

#[test]
fn tag_schemas_cross_host_boundaries_and_callback_payloads_losslessly() {
    let mut builder = RuntimeSpec::builder::<()>();
    builder.with_standard_builtins();
    builder.namespace("test", |namespace| {
        for (name, argument) in [
            ("echo", ValueType::Any),
            ("echo_tag", ValueType::TypeTag),
            ("echo_schema", ValueType::RecordType),
            ("take_float", ValueType::F32),
            ("save_callback", ValueType::Callback),
        ] {
            namespace.api(
                name,
                ValueSignature::new(vec![argument], ValueType::Any),
                (),
            );
        }
    });
    let runtime = builder.build().unwrap();
    let mut session = runtime.runtime().session(SchemaHost::default());
    assert_eq!(
        session
            .eval(
                r#"
            import { f32, vec3 } from "types"
            let returned = test.echo({ amount = f32, nested = [{ color = vec3 }] })
            returned.amount == f32 && returned.nested[0].color == vec3
        "#
            )
            .unwrap()
            .value,
        Some(Value::Bool(true))
    );
    assert_eq!(
        session.eval("test.echo_tag(f32)").unwrap().value,
        Some(Value::TypeTag(TypeTag::F32))
    );
    let calls = session.host().calls;
    for source in [
        "test.echo_tag(0)",
        "test.echo_tag(\"f32\")",
        "test.take_float(f32)",
        "test.echo_schema({amount=f32})",
        "test.echo_schema(f32)",
    ] {
        assert!(session.eval(source).is_err(), "{source}");
    }
    assert_eq!(session.host().calls, calls);
    assert_eq!(
        session
            .eval(
                r#"
        let schema = { amount: f32, color: vec3 }
        let returned = test.echo_schema(schema)
        returned == schema && returned.color == vec3
    "#
            )
            .unwrap()
            .value,
        Some(Value::Bool(true))
    );
    session
        .eval("test.save_callback(fn(payload) { return payload.kind == f32 && payload.schema == schema })")
        .unwrap();
    let callback = session.host().callback.unwrap();
    assert_eq!(
        session
            .invoke_callback_invocation(CallbackInvocation {
                callback,
                args: vec![TransportValue::Table(vec![
                    ("kind".into(), TransportValue::TypeTag(TypeTag::F32)),
                    (
                        "schema".into(),
                        TransportValue::RecordType(meow_meow_script::RecordType(
                            [
                                ("amount".into(), TypeTag::F32),
                                ("color".into(), TypeTag::Vec3),
                            ]
                            .into()
                        )),
                    )
                ])],
            })
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn imported_modules_can_reexport_tags_without_replacing_the_types_module() {
    let mut session = Runtime::standard().session(SchemaHost::default());
    assert_eq!(
        session
            .eval(
                r#"
        import { f32, vec3 } from "types"
        import { amount, color } from "schema.mms"
        import { amount as again } from "schema.mms"
        amount == f32 && again == f32 && color == vec3
    "#
            )
            .unwrap()
            .value,
        Some(Value::Bool(true))
    );
    assert_eq!(session.host().loads, ["schema.mms", "schema.mms"]);
}
