use crate::{EvalError, Evaluation, Evaluator, Host, Hostless};

/// Convenience entry point for ordinary scripts that require no host powers.
pub struct HostlessRunner;

impl HostlessRunner {
    pub fn eval(source: &str) -> Result<Evaluation, EvalError> {
        let mut host = Hostless;
        Evaluator::new(&mut host).evaluate(source)
    }
}

/// Run a script with an application-provided synchronous host.
pub fn eval_with_host(source: &str, host: &mut impl Host) -> Result<Evaluation, EvalError> {
    Evaluator::new(host).evaluate(source)
}

/// Compatibility name for the language crate's pure runner. This does not
/// expose the old engine façade; engine-aware helpers live in
/// `mittens_engine::scripting`.
pub type MeowMeowRunner = HostlessRunner;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ComponentHandle, HostError, HostErrorKind, HostRequest, HostResponse, Value};

    #[derive(Default)]
    struct FakeHost {
        operations: Vec<String>,
        fail_methods: bool,
    }

    impl Host for FakeHost {
        fn dispatch(&mut self, request: HostRequest) -> Result<HostResponse, HostError> {
            self.operations.push(request.operation_name().to_owned());
            match request {
                HostRequest::Query { .. } => Ok(HostResponse::Component {
                    handle: ComponentHandle::from_raw(7),
                    component_type: "Fake".into(),
                }),
                HostRequest::InvokeComponentMethod { method, .. } if self.fail_methods => {
                    Err(HostError::failure(method, "fake host rejected method"))
                }
                HostRequest::InvokeComponentMethod { .. } => {
                    Ok(HostResponse::Value(Value::Number(42.0)))
                }
                _ => Ok(HostResponse::Unit),
            }
        }
    }

    #[test]
    fn evaluates_pure_arithmetic() {
        let result = HostlessRunner::eval("1 + 2 * 3").unwrap();
        assert_eq!(result.value, Some(Value::Number(7.0)));
    }

    #[test]
    fn engine_expression_is_a_typed_host_error() {
        let error = HostlessRunner::eval("Text { \"hello\" }").unwrap_err();
        let EvalError::Host(error) = error else {
            panic!("expected host error")
        };
        assert_eq!(error.kind, HostErrorKind::UnsupportedHostOperation);
        assert_eq!(error.operation, "spawn");
    }

    #[test]
    fn queries_and_methods_dispatch_through_the_host() {
        let mut host = FakeHost::default();
        let result = eval_with_host("query(\"#target\").answer()", &mut host).unwrap();
        assert_eq!(result.value, Some(Value::Number(42.0)));
        assert_eq!(host.operations, ["query", "invoke_component_method"]);
    }

    #[test]
    fn host_failures_propagate_without_panicking() {
        let mut host = FakeHost {
            fail_methods: true,
            ..FakeHost::default()
        };
        let error = eval_with_host("query(\"#target\").explode()", &mut host).unwrap_err();
        let EvalError::Host(error) = error else {
            panic!("expected host error")
        };
        assert_eq!(error.kind, HostErrorKind::HostFailure);
        assert_eq!(error.operation, "explode");
    }
}
