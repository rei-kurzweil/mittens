use std::time::{Duration, Instant};

use crate::meow_meow::ast::expression::Expression;
use crate::meow_meow::ast::statement::Statement;
use crate::meow_meow::evaluator::{EvalRequest, EvalResponse, MeowMeowEvaluator};
use crate::meow_meow::parser::MeowMeowParser;
use crate::meow_meow::tokenizer::MeowMeowTokenizer;

#[test]
fn parses_component_tree_calls_and_params() {
    let src = r#"
T {
    TXT { "kristi vs puppy", "click to start" }

    Background name="bg" {
        with_occlusion_and_lighting()
        T name="inner" {
            QUAD_2D
        }
    }
}
"#;

    let tokens = MeowMeowTokenizer::new(src).tokenize().expect("tokenize ok");
    let program = MeowMeowParser::new(tokens)
        .parse_program()
        .expect("parse ok");

    assert_eq!(program.len(), 1);

    let Statement::Expression(Expression::Component(root)) = &program[0] else {
        panic!("expected a single top-level component expression");
    };

    assert_eq!(root.component_type.0, "T");
    assert!(root.parameters.is_empty());

    // Children: TXT + Background
    assert_eq!(root.children.len(), 2);

    let txt = &root.children[0];
    assert_eq!(txt.component_type.0, "TXT");
    assert!(txt.calls.is_empty());
    assert_eq!(txt.positional.len(), 2);

    let bg = &root.children[1];
    assert_eq!(bg.component_type.0, "Background");
    assert_eq!(bg.parameters.len(), 1);
    assert_eq!(bg.parameters[0].name.0, "name");
    assert_eq!(bg.calls.len(), 1);
    assert_eq!(bg.calls[0].callee.0, "with_occlusion_and_lighting");

    // Background child: inner T
    assert_eq!(bg.children.len(), 1);
    let inner = &bg.children[0];
    assert_eq!(inner.component_type.0, "T");
    assert_eq!(inner.parameters.len(), 1);
    assert_eq!(inner.parameters[0].name.0, "name");
    assert_eq!(inner.positional.len(), 1);

    let Expression::Identifier(flag) = &inner.positional[0] else {
        panic!("expected QUAD_2D to parse as a positional identifier expression");
    };
    assert_eq!(flag.0, "QUAD_2D");
}

#[test]
fn evaluator_thread_parses_and_responds() {
    let mut handle = MeowMeowEvaluator::spawn(64);

    handle
        .requests
        .push(EvalRequest::ParseScript {
            source: "T { TXT { \"meow\" } }".to_string(),
        })
        .expect("push request");

    let deadline = Instant::now() + Duration::from_millis(250);
    let mut got_ok = false;

    while Instant::now() < deadline {
        match handle.responses.pop() {
            Ok(EvalResponse::ParsedOk { debug_ast }) => {
                assert!(debug_ast.contains("ComponentExpression"));
                got_ok = true;
                break;
            }
            Ok(EvalResponse::Error { message }) => panic!("unexpected eval error: {message}"),
            Ok(EvalResponse::ShutdownAck) => panic!("unexpected shutdown ack"),
            Err(rtrb::PopError::Empty) => {
                std::thread::yield_now();
            }
        }
    }

    assert!(got_ok, "timed out waiting for evaluator response");
    handle.shutdown_and_join();
}
