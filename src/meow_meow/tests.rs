use std::time::{Duration, Instant};

use crate::engine;
use crate::meow_meow::ast::{
    AssignmentStatement, Expression, ImportItem, Statement,
};
use crate::meow_meow::evaluator::{EvalRequest, EvalResponse, MeowMeowEvaluator};
use crate::meow_meow::object::Value;
use crate::meow_meow::parser::MeowMeowParser;
use crate::meow_meow::runner::MeowMeowRunner;
use crate::meow_meow::tokenizer::MeowMeowTokenizer;

fn parse(src: &str) -> Vec<Statement> {
    let tokens = MeowMeowTokenizer::new(src).tokenize().expect("tokenize ok");
    MeowMeowParser::new(tokens).parse_program().expect("parse ok")
}

/// Helper: extract a `ComponentExpression` from a `Statement::Expression(Expression::Component(_))`.
macro_rules! as_component {
    ($stmt:expr) => {{
        let Statement::Expression(Expression::Component(c)) = $stmt else { panic!("expected component expression statement") };
        c
    }};
}

// ---------------------------------------------------------------------------
// Component expression: basic forms
// ---------------------------------------------------------------------------

#[test]
fn parse_bare_component() {
    let prog = parse("T {}");
    assert_eq!(prog.len(), 1);
    let c = as_component!(&prog[0]);
    assert_eq!(c.component_type.0, "T");
    assert!(c.constructors.is_empty());
    assert!(c.body.statements.is_empty());
}

#[test]
fn parse_constructor_no_body() {
    let prog = parse("Color.rgba(1.0, 0.0, 0.5, 1.0)");
    assert_eq!(prog.len(), 1);
    let c = as_component!(&prog[0]);
    assert_eq!(c.component_type.0, "Color");
    let hc = c.constructors.first().expect("constructor");
    assert_eq!(hc.method.0, "rgba");
    assert_eq!(hc.args.len(), 4);
    assert!(c.body.statements.is_empty());
}

#[test]
fn parse_constructor_with_body() {
    let prog = parse("T.with_scale(0.06, 0.06, 0.12) { C {} }");
    let c = as_component!(&prog[0]);
    assert_eq!(c.component_type.0, "T");
    let hc = c.constructors.first().expect("constructor");
    assert_eq!(hc.method.0, "with_scale");
    assert_eq!(hc.args.len(), 3);
    assert_eq!(c.body.statements.len(), 1);
    let child = as_component!(&c.body.statements[0]);
    assert_eq!(child.component_type.0, "C");
}

#[test]
fn parse_named_assignment_in_body() {
    let prog = parse(r#"T { name = "root" }"#);
    let c = as_component!(&prog[0]);
    assert_eq!(c.body.statements.len(), 1);
    let Statement::Reassign { name, value } = &c.body.statements[0] else { panic!("expected Reassign") };
    assert_eq!(name.0, "name");
    assert!(matches!(value, Expression::String(s) if s == "root"));
}

#[test]
fn parse_call_in_body() {
    let prog = parse("BG { with_occlusion_and_lighting() }");
    let c = as_component!(&prog[0]);
    assert_eq!(c.body.statements.len(), 1);
    let Statement::Expression(Expression::Call(call)) = &c.body.statements[0] else { panic!("expected Call") };
    assert_eq!(call.callee.0, "with_occlusion_and_lighting");
    assert!(call.args.is_empty());
}

#[test]
fn parse_positional_string() {
    let prog = parse(r#"TXT { "hello" }"#);
    let c = as_component!(&prog[0]);
    assert_eq!(c.body.statements.len(), 1);
    let Statement::Expression(Expression::String(s)) = &c.body.statements[0] else { panic!("expected string expr") };
    assert_eq!(s, "hello");
}

#[test]
fn parse_positional_ident_flag() {
    let prog = parse("R { QUAD_2D }");
    let c = as_component!(&prog[0]);
    assert_eq!(c.body.statements.len(), 1);
    let Statement::Expression(Expression::Identifier(id)) = &c.body.statements[0] else { panic!("expected ident expr") };
    assert_eq!(id.0, "QUAD_2D");
}

#[test]
fn parse_named_assignment_array() {
    let prog = parse("T { rotation = [0.0, 0.0, 3.14] }");
    let c = as_component!(&prog[0]);
    let Statement::Reassign { name, value } = &c.body.statements[0] else { panic!("expected Reassign") };
    assert_eq!(name.0, "rotation");
    let Expression::Array(items) = value else { panic!() };
    assert_eq!(items.len(), 3);
}

// ---------------------------------------------------------------------------
// Body item ordering is preserved
// ---------------------------------------------------------------------------

#[test]
fn parse_body_ordering_preserved() {
    // call, then child, then identifier — order must be preserved as Statements
    let prog = parse("T { call() C {} IDENT }");
    let c = as_component!(&prog[0]);
    assert_eq!(c.body.statements.len(), 3);
    assert!(matches!(&c.body.statements[0], Statement::Expression(Expression::Call(_))));
    assert!(matches!(&c.body.statements[1], Statement::Expression(Expression::Component(_))));
    assert!(matches!(&c.body.statements[2], Statement::Expression(Expression::Identifier(_))));
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

    let root = as_component!(&prog[0]);
    assert_eq!(root.component_type.0, "CTLXR");
    let hc = root.constructors.first().expect("constructor on CTLXR");
    assert_eq!(hc.method.0, "new");
    assert_eq!(hc.args.len(), 3);
    assert!(matches!(&hc.args[0], Expression::Bool(true)));

    // one child: T.with_scale
    assert_eq!(root.body.statements.len(), 1);
    let t_scale = as_component!(&root.body.statements[0]);
    assert_eq!(t_scale.component_type.0, "T");
    assert_eq!(t_scale.constructors.first().unwrap().method.0, "with_scale");

    // T → TransformPipeline
    assert_eq!(t_scale.body.statements.len(), 1);
    let pipeline = as_component!(&t_scale.body.statements[0]);
    assert_eq!(pipeline.component_type.0, "TransformPipeline");

    // pipeline → fork + output
    assert_eq!(pipeline.body.statements.len(), 2);
    let fork = as_component!(&pipeline.body.statements[0]);
    assert_eq!(fork.component_type.0, "TransformForkTRS");

    // fork → translation, rotation, scale
    assert_eq!(fork.body.statements.len(), 3);
    let map_rot = as_component!(&fork.body.statements[1]);
    assert_eq!(map_rot.component_type.0, "TransformMapRotation");

    // rotation filter child
    assert_eq!(map_rot.body.statements.len(), 1);
    let filter = as_component!(&map_rot.body.statements[0]);
    assert_eq!(filter.component_type.0, "QuatTemporalFilter");
    assert_eq!(filter.constructors.first().unwrap().method.0, "with_smoothing_factor");

    // output → T → R.cube → C.rgba
    let output = as_component!(&pipeline.body.statements[1]);
    let out_t = as_component!(&output.body.statements[0]);
    let cube = as_component!(&out_t.body.statements[0]);
    assert_eq!(cube.component_type.0, "R");
    assert_eq!(cube.constructors.first().unwrap().method.0, "cube");
    let color = as_component!(&cube.body.statements[0]);
    assert_eq!(color.component_type.0, "C");
    assert_eq!(color.constructors.first().unwrap().method.0, "rgba");
}

// ---------------------------------------------------------------------------
// Multiple top-level statements
// ---------------------------------------------------------------------------

#[test]
fn parse_multiple_roots() {
    let prog = parse("T {} R {} XR.on()");
    assert_eq!(prog.len(), 3);
    let c2 = as_component!(&prog[2]);
    assert_eq!(c2.component_type.0, "XR");
    assert_eq!(c2.constructors.first().unwrap().method.0, "on");
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

#[test]
fn runner_parse_errors_include_source_line_and_caret() {
    // An unterminated component body is still a parse error and should include
    // a source line + caret in the error message.
    let out = MeowMeowRunner::eval("T {\n    R.cube()\n");
    assert!(!out.errors.is_empty(), "expected parse error");
    let msg = &out.errors[0];
    assert!(msg.contains("parse error at"), "got: {msg}");
    assert!(msg.contains("^"), "got: {msg}");
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
            Ok(EvalResponse::Intent(_)) => {} // ParseScript shouldn't emit intents, skip
            Ok(EvalResponse::Error { message }) => panic!("unexpected eval error: {message}"),
            Ok(EvalResponse::ShutdownAck) => panic!("unexpected shutdown ack"),
            Ok(EvalResponse::HostCall { .. }) => {} // ParseScript never triggers HostCalls
            Err(rtrb::PopError::Empty) => std::thread::yield_now(),
        }
    }

    assert!(got_ok, "timed out waiting for evaluator response");
    handle.shutdown_and_join();
}

// ---------------------------------------------------------------------------
// Phase 5: for/in, range(), break, continue
// ---------------------------------------------------------------------------

// --- parse tests ---

#[test]
fn parse_for_in_array_literal() {
    let prog = parse("for x in [1, 2, 3] { T {} }");
    assert_eq!(prog.len(), 1);
    let Statement::ForIn { binding, iterable, body } = &prog[0] else { panic!() };
    assert_eq!(binding.0, "x");
    assert!(matches!(iterable, Expression::Array(_)));
    assert_eq!(body.statements.len(), 1);
}

#[test]
fn parse_for_in_range_call() {
    let prog = parse("for i in range(10) { T {} }");
    assert_eq!(prog.len(), 1);
    let Statement::ForIn { binding, iterable, .. } = &prog[0] else { panic!() };
    assert_eq!(binding.0, "i");
    let Expression::Call(call) = iterable else { panic!() };
    assert_eq!(call.callee.0, "range");
    assert_eq!(call.args.len(), 1);
}

#[test]
fn parse_break_and_continue() {
    let prog = parse("for i in range(5) { break; continue }");
    let Statement::ForIn { body, .. } = &prog[0] else { panic!() };
    assert!(matches!(body.statements[0], Statement::Break));
    assert!(matches!(body.statements[1], Statement::Continue));
}

// --- eval tests ---

fn eval(src: &str) -> crate::meow_meow::runner::EvalOutput {
    MeowMeowRunner::eval(src)
}

#[test]
fn eval_for_in_array_emits_correct_count() {
    // 3 elements → 3 SpawnComponentTree intents
    let out = eval("for x in [1, 2, 3] { T {} }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 3);
}

#[test]
fn eval_for_in_range_emits_correct_count() {
    let out = eval("for i in range(5) { T {} }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 5);
}

#[test]
fn eval_range_two_arg() {
    // range(2, 5) → [2, 3, 4] → 3 intents
    let out = eval("for i in range(2, 5) { T {} }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 3);
}

#[test]
fn eval_break_stops_loop_early() {
    // break after first iteration → only 1 intent despite 10-element range
    let out = eval("for i in range(10) { T {} break }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}

#[test]
fn eval_continue_skips_rest_of_body() {
    // continue before second emit → only the first emit fires each iteration
    // 3 iterations × 1 emit each = 3 intents (second T {} never reached)
    let out = eval("for i in range(3) { T {} continue T {} }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 3);
}

#[test]
fn eval_break_inside_if() {
    // break inside an if branch propagates out of the loop
    let out = eval("for i in range(10) { if i == 3.0 { break } T {} }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    // iterations 0,1,2 emit T (i==3 is the 4th iteration, 0-indexed, so 3 emits before break)
    assert_eq!(out.intents.len(), 3);
}

#[test]
fn eval_nested_for_loops() {
    // outer 3 × inner 2 = 6 intents
    let out = eval("for i in range(3) { for j in range(2) { T {} } }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 6);
}

#[test]
fn eval_break_only_exits_inner_loop() {
    // break only exits inner loop; outer loop continues
    // outer 3 iters, inner breaks after 1 → 3 intents
    let out = eval("for i in range(3) { for j in range(5) { T {} break } }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 3);
}

#[test]
fn eval_for_binding_accessible_in_body() {
    // range(0) → empty → 0 intents
    let out = eval("for i in range(0) { T {} }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 0);
}

#[test]
fn eval_return_propagates_through_for() {
    // return inside a for loop inside a function exits the function, not just the loop
    let out = eval(r#"
        fn f() {
            for i in range(10) {
                T {}
                return null
            }
        }
        f()
    "#);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}

// ---------------------------------------------------------------------------
// Export / Import
// ---------------------------------------------------------------------------

#[test]
fn parse_export_let() {
    let prog = parse("export let pi = 3.14");
    assert_eq!(prog.len(), 1);
    let Statement::Assignment(AssignmentStatement { name, exported, .. }) = &prog[0] else { panic!() };
    assert_eq!(name.0, "pi");
    assert!(*exported);
}

#[test]
fn parse_export_fn() {
    let prog = parse("export fn lerp(a, b, t) { return a + (b - a) * t }");
    assert_eq!(prog.len(), 1);
    let Statement::Assignment(AssignmentStatement { name, exported, .. }) = &prog[0] else { panic!() };
    assert_eq!(name.0, "lerp");
    assert!(*exported);
}

#[test]
fn parse_import_named() {
    let prog = parse(r#"import { pi, lerp } from "math.mms""#);
    assert_eq!(prog.len(), 1);
    let Statement::Import { items, path } = &prog[0] else { panic!() };
    assert_eq!(path, "math.mms");
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[0], ImportItem::Named(id) if id.0 == "pi"));
    assert!(matches!(&items[1], ImportItem::Named(id) if id.0 == "lerp"));
}

#[test]
fn parse_import_alias() {
    let prog = parse(r#"import { pi as PI, 0 as cube } from "parts.mms""#);
    assert_eq!(prog.len(), 1);
    let Statement::Import { items, .. } = &prog[0] else { panic!() };
    assert!(matches!(&items[0], ImportItem::NamedAlias { name, alias } if name.0 == "pi" && alias.0 == "PI"));
    assert!(matches!(&items[1], ImportItem::PositionalAlias { index: 0, alias } if alias.0 == "cube"));
}

#[test]
fn eval_export_and_import_via_files() {
    // Write a small library file that exports a value and a function.
    let tmp = std::env::temp_dir();
    let lib_path = tmp.join("_mms_test_lib.mms");
    let user_path = tmp.join("_mms_test_user.mms");

    std::fs::write(&lib_path, r#"
export let count = 3.0
export fn make_row(n) {
    for i in range(n) { T {} }
}
"#).unwrap();

    std::fs::write(&user_path, "import { count, make_row } from \"_mms_test_lib.mms\"\nmake_row(count)\n").unwrap();

    let out = MeowMeowRunner::eval_file(user_path.to_str().unwrap());
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    // count == 3 → make_row(3) → 3 T {} emits
    assert_eq!(out.intents.len(), 3, "intents: {:?}", out.intents);

    // cleanup
    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_file(&user_path);
}

#[test]
fn eval_import_positional_ce() {
    // Library emits a CE at index 0; user imports it and re-emits it.
    let tmp = std::env::temp_dir();
    let lib_path = tmp.join("_mms_test_ce_lib.mms");
    let user_path = tmp.join("_mms_test_ce_user.mms");

    std::fs::write(&lib_path, "T.position(1.0, 0.0, 0.0) {}").unwrap();
    std::fs::write(&user_path, "import { 0 as my_t } from \"_mms_test_ce_lib.mms\"\nmy_t\n").unwrap();

    let out = MeowMeowRunner::eval_file(user_path.to_str().unwrap());
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);

    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_file(&user_path);
}

// ---------------------------------------------------------------------------
// Reassignment
// ---------------------------------------------------------------------------

#[test]
fn parse_reassign() {
    let prog = parse("let x = 1\nx = 2");
    assert_eq!(prog.len(), 2);
    assert!(matches!(&prog[0], Statement::Assignment(_)));
    let Statement::Reassign { name, .. } = &prog[1] else { panic!("expected Reassign") };
    assert_eq!(name.0, "x");
}

#[test]
fn eval_reassign_basic() {
    // A number incremented via reassignment should be visible to later code.
    let src = r#"
        let x = 10
        x = 20
        let arr = [x]
    "#;
    let out = MeowMeowRunner::eval(src);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
}

#[test]
fn eval_reassign_undefined_errors() {
    let out = MeowMeowRunner::eval("x = 5");
    assert!(!out.errors.is_empty(), "expected an error for undefined reassignment");
    assert!(out.errors[0].contains("not defined"), "got: {}", out.errors[0]);
}

#[test]
fn eval_if_reassign_propagates_to_outer_scope() {
    // `y` declared in outer block, reassigned inside if-branch —
    // the emitted CE must use the updated value.
    let src = r#"
        let y = -1.0
        if (1 > 0) {
            y = 99.0
        }
        T.position(0.0, y, 0.0) {}
    "#;
    let out = MeowMeowRunner::eval(src);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
    // Verify the CE position used the updated y (second arg of the constructor call).
    let engine::ecs::IntentValue::SpawnComponentTree { root, .. } = &out.intents[0] else { panic!() };
    assert_eq!(root.ctor_method.as_deref(), Some("position"), "expected position ctor");
    let Value::Number(y_val) = &root.ctor_args[1] else { panic!("expected number arg at index 1") };
    assert!((*y_val - 99.0).abs() < 1e-6, "expected y=99.0, got {y_val}");
}

#[test]
fn eval_for_accumulator_pattern() {
    // sum = sum + i across iterations — the classic accumulator.
    let src = r#"
        let sum = 0
        for i in [1, 2, 3] {
            sum = sum + i
        }
    "#;
    // No errors means the reassignment and loop executed correctly.
    let out = MeowMeowRunner::eval(src);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
}

// ---------------------------------------------------------------------------
// While loop
// ---------------------------------------------------------------------------

#[test]
fn parse_while_loop() {
    let prog = parse("while true { T {} }");
    assert_eq!(prog.len(), 1);
    let Statement::While { condition, body } = &prog[0] else { panic!("expected While") };
    assert!(matches!(condition, Expression::Bool(true)));
    assert_eq!(body.statements.len(), 1);
}

#[test]
fn eval_while_counts_up_to_limit() {
    // Emit one T per iteration; stop when i reaches 4.
    let out = eval(r#"
        let i = 0
        while i < 4 {
            T {}
            i = i + 1
        }
    "#);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 4);
}

#[test]
fn eval_while_break_exits_early() {
    let out = eval(r#"
        let i = 0
        while true {
            if i == 3 { break }
            T {}
            i = i + 1
        }
    "#);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 3);
}

#[test]
fn eval_while_continue_skips_body_tail() {
    // Only emit T when i is even; continue skips the emit on odd iterations.
    // i goes 0..5 → 0,2,4 emit → 3 intents
    let out = eval(r#"
        let i = 0
        while i < 5 {
            i = i + 1
            if i == 2 { continue }
            if i == 4 { continue }
            T {}
        }
    "#);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 3);
}

#[test]
fn eval_while_false_never_runs() {
    let out = eval("while false { T {} }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 0);
}

// ---------------------------------------------------------------------------
// Component body: for / if / block statements
// ---------------------------------------------------------------------------

#[test]
fn body_for_expands_children() {
    // `for i in range(3)` inside a component body → 3 children under the parent
    let out = eval("T { for i in range(3) { R.cube() {} } }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    // One top-level spawn; it will have 3 children internally.
    assert_eq!(out.intents.len(), 1);
}

#[test]
fn body_for_captures_binding() {
    // The loop variable should be captured as a value in each child's constructor args.
    let out = eval(r#"T { for i in [1, 2, 3] { T.position(i, 0, 0) {} } }"#);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}

#[test]
fn body_if_true_includes_child() {
    let out = eval("T { if true { R.cube() {} } }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}

#[test]
fn body_if_false_excludes_child() {
    // When condition is false and there is no else branch, the child should be absent.
    // The parent T still spawns (1 intent) but has no children.
    let out = eval("T { if false { R.cube() {} } }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}

#[test]
fn body_if_else_picks_else_branch() {
    let out = eval("T { if false { R.cube() {} } else { R.sphere() {} } }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}

#[test]
fn body_for_nested_in_for() {
    // 3x3 grid: outer `for` produces 3 iterations each containing inner `for` of 3 → 9 children.
    let out = eval("T { for x in range(3) { for y in range(3) { R.cube() {} } } }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}

#[test]
fn body_for_with_if_inside() {
    // Only even indices: range(6) → 0,1,2,3,4,5 → 3 children (0,2,4)
    let out = eval("T { for i in range(6) { if i % 2 == 0 { R.cube() {} } } }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}
