use meow_meow_script::{Hostless, Runtime, TypeTag, Value};

#[test]
fn from_as_and_ast_are_names_outside_imports() {
    let mut session = Runtime::standard().session(Hostless);
    let source = "
        fn sum(from, as, ast) { return from + as + ast }
        let from = 1
        let to = 2
        let table = { from = from as = to }
        sum(table.from, table.as, 3)
    ";
    assert_eq!(session.eval(source).unwrap().value, Some(Value::Number(6.0)));
}

#[test]
fn imports_still_read_from_and_as() {
    let mut session = Runtime::standard().session(Hostless);
    let value = session
        .eval("import { f32 as scalar } from \"types\"\nscalar")
        .unwrap()
        .value;
    assert_eq!(value, Some(Value::TypeTag(TypeTag::F32)));
    assert!(session.eval("import { f32 } \"types\"").is_err());
}

#[test]
fn parse_errors_name_the_line_and_column() {
    let mut session = Runtime::standard().session(Hostless);
    let message = session.eval("let a = 1\nlet = 2").unwrap_err().to_string();
    assert!(message.contains("line 2, column 5"), "{message}");
    assert!(message.contains("let = 2"), "{message}");
}
