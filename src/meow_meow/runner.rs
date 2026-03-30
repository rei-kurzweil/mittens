use std::time::{Duration, Instant};

use crate::engine::ecs::IntentValue;
use crate::meow_meow::evaluator::{EvalRequest, EvalResponse, MeowMeowEvaluator};

/// The result of evaluating an MMS script: collected intents and any errors.
#[derive(Debug, Default)]
pub struct EvalOutput {
    pub intents: Vec<IntentValue>,
    pub errors: Vec<String>,
}

/// Synchronous wrapper around [`MeowMeowEvaluator`].
///
/// Spawns an evaluator thread, sends a script, drains all responses to
/// completion, and returns the collected [`EvalOutput`]. The thread is shut
/// down and joined before returning.
pub struct MeowMeowRunner;

impl MeowMeowRunner {
    /// Evaluate `source`, collecting all emitted intents and errors.
    /// Times out after 2 seconds if the evaluator stalls.
    pub fn eval(source: &str) -> EvalOutput {
        Self::eval_impl(source, None, Duration::from_secs(2))
    }

    /// Evaluate `source` with a caller-provided timeout.
    pub fn eval_with_timeout(source: &str, timeout: Duration) -> EvalOutput {
        Self::eval_impl(source, None, timeout)
    }

    /// Evaluate `source` knowing it came from `path` (enables relative imports).
    pub fn eval_with_path(source: &str, path: &str) -> EvalOutput {
        Self::eval_impl(source, Some(path), Duration::from_secs(2))
    }

    /// Read `path` from disk and evaluate it (enables relative imports).
    pub fn eval_file(path: &str) -> EvalOutput {
        match std::fs::read_to_string(path) {
            Ok(source) => Self::eval_impl(&source, Some(path), Duration::from_secs(2)),
            Err(e) => {
                let mut output = EvalOutput::default();
                output.errors.push(format!("cannot read file '{}': {}", path, e));
                output
            }
        }
    }

    fn eval_impl(source: &str, source_path: Option<&str>, timeout: Duration) -> EvalOutput {
        let mut handle = MeowMeowEvaluator::spawn(64);

        handle
            .requests
            .push(EvalRequest::EvalScript {
                source: source.to_string(),
                source_path: source_path.map(|s| s.to_string()),
            })
            .expect("MeowMeowRunner: push EvalScript");
        handle
            .requests
            .push(EvalRequest::Shutdown)
            .expect("MeowMeowRunner: push Shutdown");

        let mut output = EvalOutput::default();
        let deadline = Instant::now() + timeout;

        loop {
            match handle.responses.pop() {
                Ok(EvalResponse::Intent(iv)) => output.intents.push(iv),
                Ok(EvalResponse::Error { message }) => output.errors.push(message),
                Ok(EvalResponse::ParsedOk { .. }) => {}
                Ok(EvalResponse::ShutdownAck) => break,
                Err(rtrb::PopError::Empty) => {
                    if Instant::now() > deadline {
                        output.errors.push("MeowMeowRunner: timed out waiting for evaluator".into());
                        break;
                    }
                    std::thread::yield_now();
                }
            }
        }

        handle.shutdown_and_join();
        output
    }
}
