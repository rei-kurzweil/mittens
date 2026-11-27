use meow_meow_script::{Hostless, Runtime, Value};

#[test]
fn math_constants_include_pi_tau_and_e() {
    let mut session = Runtime::standard().session(Hostless);
    for (source, expected) in [
        ("Math.pi", std::f64::consts::PI),
        ("Math.tau", std::f64::consts::TAU),
        ("Math.e", std::f64::consts::E),
    ] {
        assert_eq!(
            session.eval(source).unwrap().value,
            Some(Value::Number(expected)),
            "{source}"
        );
    }
    assert!(session.eval("Math.phi").is_err());
}
