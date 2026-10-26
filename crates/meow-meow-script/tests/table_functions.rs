use meow_meow_script::{Hostless, Runtime, Value};

#[test]
fn table_functions_run_with_arguments_captures_and_shared_state() {
    let runtime = Runtime::standard();
    let mut session = runtime.session(Hostless);
    session
        .eval(
            r#"
        let state = { total = 0.0 }
        let api = { add = fn(value) {
            state.total = state.total + value
            return state.total
        } }
        let alias = api
        let nested = { api = alias }
    "#,
        )
        .unwrap();
    for (source, expected) in [
        ("api.add(2.0)", 2.0),
        ("alias.add(3.0)", 5.0),
        ("nested.api.add(4.0)", 9.0),
    ] {
        assert_eq!(
            session.eval(source).unwrap().value,
            Some(Value::Number(expected))
        );
    }
    assert_eq!(
        session.eval("state.total").unwrap().value,
        Some(Value::Number(9.0))
    );
}

#[test]
fn table_call_requires_a_callable_member() {
    let runtime = Runtime::standard();
    let mut session = runtime.session(Hostless);
    session.eval("let data = { value = 7.0 }").unwrap();
    for source in ["data.value()", "data.missing()"] {
        assert!(
            session
                .eval(source)
                .unwrap_err()
                .to_string()
                .contains("not callable")
        );
    }
}
