use std::thread::{self, JoinHandle};

use rtrb::{Consumer, Producer, RingBuffer};

use crate::meow_meow::parser::{MeowMeowParser, ParseError};
use crate::meow_meow::tokenizer::{MeowMeowTokenizer, TokenizeError};

#[derive(Debug, Clone)]
pub enum EvalRequest {
    ParseScript { source: String },
    Shutdown,
}

#[derive(Debug, Clone)]
pub enum EvalResponse {
    ParsedOk { debug_ast: String },
    Error { message: String },
    ShutdownAck,
}

#[derive(Debug)]
pub struct MeowMeowEvaluatorHandle {
    pub requests: Producer<EvalRequest>,
    pub responses: Consumer<EvalResponse>,
    join: Option<JoinHandle<()>>,
}

impl MeowMeowEvaluatorHandle {
    pub fn shutdown_and_join(mut self) {
        let _ = self.requests.push(EvalRequest::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

pub struct MeowMeowEvaluator;

impl MeowMeowEvaluator {
    /// Spawns the evaluator on its own worker thread.
    ///
    /// Communication is intentionally lock-free SPSC (main thread <-> evaluator thread)
    /// using `rtrb` ring buffers. We use *two* queues:
    /// - requests: main -> evaluator
    /// - responses: evaluator -> main
    pub fn spawn(queue_capacity: usize) -> MeowMeowEvaluatorHandle {
        let (req_prod, req_cons) = RingBuffer::<EvalRequest>::new(queue_capacity);
        let (res_prod, res_cons) = RingBuffer::<EvalResponse>::new(queue_capacity);

        let join = thread::spawn(move || evaluator_thread(req_cons, res_prod));

        MeowMeowEvaluatorHandle {
            requests: req_prod,
            responses: res_cons,
            join: Some(join),
        }
    }
}

fn evaluator_thread(mut requests: Consumer<EvalRequest>, mut responses: Producer<EvalResponse>) {
    loop {
        match requests.pop() {
            Ok(EvalRequest::ParseScript { source }) => {
                let response = parse_only(&source)
                    .map(|dbg| EvalResponse::ParsedOk { debug_ast: dbg })
                    .unwrap_or_else(|msg| EvalResponse::Error { message: msg });

                let _ = responses.push(response);
            }
            Ok(EvalRequest::Shutdown) => {
                let _ = responses.push(EvalResponse::ShutdownAck);
                break;
            }
            Err(rtrb::PopError::Empty) => {
                // No request right now; yield so we don't spin at 100%.
                std::thread::yield_now();
            }
        }
    }
}

fn parse_only(source: &str) -> Result<String, String> {
    let tokens = MeowMeowTokenizer::new(source)
        .tokenize()
        .map_err(tokenize_err_to_string)?;

    let program = MeowMeowParser::new(tokens)
        .parse_program()
        .map_err(parse_err_to_string)?;

    Ok(format!("{program:#?}"))
}

fn tokenize_err_to_string(e: TokenizeError) -> String {
    format!("tokenize error at {}..{}: {}", e.span.start, e.span.end, e.message)
}

fn parse_err_to_string(e: ParseError) -> String {
    format!("parse error near token #{}: {}", e.token_index, e.message)
}
