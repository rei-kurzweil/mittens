use std::time::{Duration, Instant};

use crate::meow_meow::ast::expression::{
    ComponentBodyItem, Expression,
};
use crate::meow_meow::ast::statement::Statement;
use crate::meow_meow::evaluator::{EvalRequest, EvalResponse, MeowMeowEvaluator};
use crate::meow_meow::parser::MeowMeowParser;
use crate::meow_meow::tokenizer::MeowMeowTokenizer;

fn parse(src: &str) -> Vec<Statement> {
    let tokens = MeowMeowTokenizer::new(src).tokenize().expect("tokenize ok");
    MeowMeowParser::new(tokens).parse_program().expect("parse ok")
}

// ---------------------------------------------------------------------------
// Component expression: basic forms
// ---------------------------------------------------------------------------

#[test]
fn parse_bare_component() {
    let prog = parse("T {}");
    assert_eq!(prog.len(), 1);
    let Statement::Expression(Expression::Component(c)) = &prog[0] else { panic!() };
    assert_eq!(c.component_type.0, "T");
    assert!(c.constructor.is_none());
    assert!(c.body.is_empty());
}

#[test]
fn parse_constructor_no_body() {
    let prog = parse("Color.rgba(1.0, 0.0, 0.5, 1.0)");
    assert_eq!(prog.len(), 1);
    let Statement::Expression(Expression::Component(c)) = &prog[0] else { panic!() };
    assert_eq!(c.component_type.0, "Color");
    let hc = c.constructor.as_ref().expect("constructor");
    assert_eq!(hc.method.0, "rgba");
    assert_eq!(hc.args.len(), 4);
    assert!(c.body.is_empty());
}

#[test]
fn parse_constructor_with_body() {
    let prog = parse("T.with_scale(0.06, 0.06, 0.12) { C {} }");
    let Statement::Expression(Expression::Component(c)) = &prog[0] else { panic!() };
    assert_eq!(c.component_type.0, "T");
    let hc = c.constructor.as_ref().expect("constructor");
    assert_eq!(hc.method.0, "with_scale");
    assert_eq!(hc.args.len(), 3);
    assert_eq!(c.body.len(), 1);
    let ComponentBodyItem::Child(child) = &c.body[0] else { panic!() };
    assert_eq!(child.component_type.0, "C");
}

#[test]
fn parse_named_assignment_in_body() {
    let prog = parse(r#"T { name = "root" }"#);
    let Statement::Expression(Expression::Component(c)) = &prog[0] else { panic!() };
    assert_eq!(c.body.len(), 1);
    let ComponentBodyItem::NamedAssignment { name, value } = &c.body[0] else { panic!() };
    assert_eq!(name.0, "name");
    assert!(matches!(value, Expression::String(s) if s == "root"));
}

#[test]
fn parse_call_in_body() {
    let prog = parse("BG { with_occlusion_and_lighting() }");
    let Statement::Expression(Expression::Component(c)) = &prog[0] else { panic!() };
    assert_eq!(c.body.len(), 1);
    let ComponentBodyItem::Call(call) = &c.body[0] else { panic!() };
    assert_eq!(call.callee.0, "with_occlusion_and_lighting");
    assert!(call.args.is_empty());
}

#[test]
fn parse_positional_string() {
    let prog = parse(r#"TXT { "hello" }"#);
    let Statement::Expression(Expression::Component(c)) = &prog[0] else { panic!() };
    assert_eq!(c.body.len(), 1);
    let ComponentBodyItem::Positional(Expression::String(s)) = &c.body[0] else { panic!() };
    assert_eq!(s, "hello");
}

#[test]
fn parse_positional_ident_flag() {
    let prog = parse("R { QUAD_2D }");
    let Statement::Expression(Expression::Component(c)) = &prog[0] else { panic!() };
    assert_eq!(c.body.len(), 1);
    let ComponentBodyItem::Positional(Expression::Identifier(id)) = &c.body[0] else { panic!() };
    assert_eq!(id.0, "QUAD_2D");
}

#[test]
fn parse_named_assignment_array() {
    let prog = parse("T { rotation = [0.0, 0.0, 3.14] }");
    let Statement::Expression(Expression::Component(c)) = &prog[0] else { panic!() };
    let ComponentBodyItem::NamedAssignment { name, value } = &c.body[0] else { panic!() };
    assert_eq!(name.0, "rotation");
    let Expression::Array(items) = value else { panic!() };
    assert_eq!(items.len(), 3);
}

// ---------------------------------------------------------------------------
// Body item ordering is preserved
// ---------------------------------------------------------------------------

#[test]
fn parse_body_ordering_preserved() {
    // call, then child, then positional — order must be preserved
    let prog = parse("T { call() C {} IDENT }");
    let Statement::Expression(Expression::Component(c)) = &prog[0] else { panic!() };
    assert_eq!(c.body.len(), 3);
    assert!(matches!(&c.body[0], ComponentBodyItem::Call(_)));
    assert!(matches!(&c.body[1], ComponentBodyItem::Child(_)));
    assert!(matches!(&c.body[2], ComponentBodyItem::Positional(_)));
}

// ---------------------------------------------------------------------------
// Nested tree (controller cube from vr-input.mms)
// ---------------------------------------------------------------------------

#[test]
fn parse_controller_cube_tree() {
    let src = r#"
CTLXR.new(true, Left, Aim) {
    T.with_scale(0.06, 0.06, 0.12) {
        TransformPipeline {
            TransformForkTRS {
                TransformMapTranslation {}
                TransformMapRotation {
                    QuatTemporalFilter.with_smoothing_factor(220.0)
                }
                TransformMapScale {}
                TransformMergeTRS {}
            }
            TransformPipelineOutput {
                T {
                    R.cube() {
                        C.rgba(0.10, 0.90, 1.00, 1.0)
                    }
                }
            }
        }
    }
}
"#;
    let prog = parse(src);
    assert_eq!(prog.len(), 1);

    let Statement::Expression(Expression::Component(root)) = &prog[0] else { panic!() };
    assert_eq!(root.component_type.0, "CTLXR");
    let hc = root.constructor.as_ref().expect("constructor on CTLXR");
    assert_eq!(hc.method.0, "new");
    assert_eq!(hc.args.len(), 3);
    assert!(matches!(&hc.args[0], Expression::Bool(true)));

    // one child: T.with_scale
    assert_eq!(root.body.len(), 1);
    let ComponentBodyItem::Child(t_scale) = &root.body[0] else { panic!() };
    assert_eq!(t_scale.component_type.0, "T");
    assert_eq!(t_scale.constructor.as_ref().unwrap().method.0, "with_scale");

    // T → TransformPipeline
    assert_eq!(t_scale.body.len(), 1);
    let ComponentBodyItem::Child(pipeline) = &t_scale.body[0] else { panic!() };
    assert_eq!(pipeline.component_type.0, "TransformPipeline");

    // pipeline → fork + output
    assert_eq!(pipeline.body.len(), 2);
    let ComponentBodyItem::Child(fork) = &pipeline.body[0] else { panic!() };
    assert_eq!(fork.component_type.0, "TransformForkTRS");

    // fork → translation, rotation, scale, merge
    assert_eq!(fork.body.len(), 4);
    let ComponentBodyItem::Child(map_rot) = &fork.body[1] else { panic!() };
    assert_eq!(map_rot.component_type.0, "TransformMapRotation");

    // rotation filter child
    assert_eq!(map_rot.body.len(), 1);
    let ComponentBodyItem::Child(filter) = &map_rot.body[0] else { panic!() };
    assert_eq!(filter.component_type.0, "QuatTemporalFilter");
    assert_eq!(filter.constructor.as_ref().unwrap().method.0, "with_smoothing_factor");

    // output → T → R.cube → C.rgba
    let ComponentBodyItem::Child(output) = &pipeline.body[1] else { panic!() };
    let ComponentBodyItem::Child(out_t) = &output.body[0] else { panic!() };
    let ComponentBodyItem::Child(cube) = &out_t.body[0] else { panic!() };
    assert_eq!(cube.component_type.0, "R");
    assert_eq!(cube.constructor.as_ref().unwrap().method.0, "cube");
    let ComponentBodyItem::Child(color) = &cube.body[0] else { panic!() };
    assert_eq!(color.component_type.0, "C");
    assert_eq!(color.constructor.as_ref().unwrap().method.0, "rgba");
}

// ---------------------------------------------------------------------------
// Multiple top-level statements
// ---------------------------------------------------------------------------

#[test]
fn parse_multiple_roots() {
    let prog = parse("T {} R {} XR.on()");
    assert_eq!(prog.len(), 3);
    let Statement::Expression(Expression::Component(c2)) = &prog[2] else { panic!() };
    assert_eq!(c2.component_type.0, "XR");
    assert_eq!(c2.constructor.as_ref().unwrap().method.0, "on");
}

// ---------------------------------------------------------------------------
// Let binding
// ---------------------------------------------------------------------------

#[test]
fn parse_let_binding() {
    let prog = parse("let x = 42");
    assert_eq!(prog.len(), 1);
    let Statement::Assignment(a) = &prog[0] else { panic!() };
    assert_eq!(a.name.0, "x");
    assert!(matches!(a.value, Expression::Number(n) if n == 42.0));
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn parse_error_unterminated_body() {
    let tokens = MeowMeowTokenizer::new("T {").tokenize().expect("tokenize ok");
    let err = MeowMeowParser::new(tokens).parse_program().unwrap_err();
    assert!(err.message.contains("Unterminated"));
}

// ---------------------------------------------------------------------------
// Evaluator thread smoke test
// ---------------------------------------------------------------------------

#[test]
fn evaluator_thread_parses_and_responds() {
    let mut handle = MeowMeowEvaluator::spawn(64);

    handle
        .requests
        .push(EvalRequest::ParseScript {
            source: "T.with_scale(1.0, 2.0, 3.0) { R.cube() { C.rgba(1,0,0,1) } }".to_string(),
        })
        .expect("push request");

    let deadline = Instant::now() + Duration::from_millis(250);
    let mut got_ok = false;

    while Instant::now() < deadline {
        match handle.responses.pop() {
            Ok(EvalResponse::ParsedOk { debug_ast }) => {
                assert!(debug_ast.contains("ComponentExpression"));
                assert!(debug_ast.contains("with_scale"));
                got_ok = true;
                break;
            }
            Ok(EvalResponse::Error { message }) => panic!("unexpected eval error: {message}"),
            Ok(EvalResponse::ShutdownAck) => panic!("unexpected shutdown ack"),
            Err(rtrb::PopError::Empty) => std::thread::yield_now(),
        }
    }

    assert!(got_ok, "timed out waiting for evaluator response");
    handle.shutdown_and_join();
}
