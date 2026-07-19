use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{fs, path::PathBuf};

use crate::engine;
use crate::engine::ecs::component::style::SizeDimension;
use crate::engine::ecs::component::{LayoutComponent, StyleComponent, TransformComponent};
use crate::engine::ecs::{
    CommandQueue, ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, World,
};
use crate::engine::graphics::{RenderAssets, VisualWorld};
use crate::engine::user_input::InputState;
use crate::scripting::ast::{AssignmentStatement, Expression, ImportItem, Statement};
use crate::scripting::object::Value;
use crate::scripting::parser::MeowMeowParser;
use crate::scripting::runner::MeowMeowRunner;
use crate::scripting::tokenizer::MeowMeowTokenizer;
use crate::scripting::unparser::unparse_program;
use crate::scripting::world_evaluator::{EvalRequest, EvalResponse, MeowMeowEvaluator};

#[derive(Clone, Default)]
struct TestClockDriver {
    now_sec: Arc<Mutex<f64>>,
}

impl TestClockDriver {
    fn set_time_sec(&self, time_sec: f64) {
        *self.now_sec.lock().expect("clock mutex poisoned") = time_sec;
    }
}

impl crate::engine::ecs::system::ClockDriver for TestClockDriver {
    fn name(&self) -> &'static str {
        "test"
    }

    fn time_now_sec(&self) -> f64 {
        *self.now_sec.lock().expect("clock mutex poisoned")
    }
}

fn parse(src: &str) -> Vec<Statement> {
    let tokens = MeowMeowTokenizer::new(src).tokenize().expect("tokenize ok");
    MeowMeowParser::new(tokens)
        .parse_program()
        .expect("parse ok")
}

fn repo_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

/// Helper: extract a `ComponentExpression` from a `Statement::Expression(Expression::Component(_))`.
macro_rules! as_component {
    ($stmt:expr) => {{
        let Statement::Expression(Expression::Component(c)) = $stmt else {
            panic!("expected component expression statement")
        };
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
fn parse_music_note_as_builtin_call_not_component() {
    let prog = parse("MusicNote.e(4, 0.25, lead)");
    assert_eq!(prog.len(), 1);
    let Statement::Expression(Expression::Call(call)) = &prog[0] else {
        panic!("expected call expression statement");
    };
    let Expression::BinaryOp { op, lhs, rhs } = call.callee.as_ref() else {
        panic!("expected dot-call callee");
    };
    assert!(matches!(op, crate::scripting::ast::BinOpKind::Dot));
    assert!(matches!(lhs.as_ref(), Expression::Identifier(id) if id.0 == "MusicNote"));
    assert!(matches!(rhs.as_ref(), Expression::Identifier(id) if id.0 == "e"));
    assert_eq!(call.args.len(), 3);
}

#[test]
fn parse_math_builtin_call_not_component() {
    let prog = parse("Math.sin(1.0)");
    assert_eq!(prog.len(), 1);
    let Statement::Expression(Expression::Call(call)) = &prog[0] else {
        panic!("expected call expression statement");
    };
    let Expression::BinaryOp { op, lhs, rhs } = call.callee.as_ref() else {
        panic!("expected dot-call callee");
    };
    assert!(matches!(op, crate::scripting::ast::BinOpKind::Dot));
    assert!(matches!(lhs.as_ref(), Expression::Identifier(id) if id.0 == "Math"));
    assert!(matches!(rhs.as_ref(), Expression::Identifier(id) if id.0 == "sin"));
    assert_eq!(call.args.len(), 1);
}

#[test]
fn parse_transform_quat_constructor() {
    let prog = parse("T.quat([0.0, 0.0, 0.0, 1.0]) {}");
    assert_eq!(prog.len(), 1);
    let c = as_component!(&prog[0]);
    assert_eq!(c.component_type.0, "T");
    let hc = c.constructors.first().expect("constructor");
    assert_eq!(hc.method.0, "quat");
    assert_eq!(hc.args.len(), 1);
    let Expression::Array(items) = &hc.args[0] else {
        panic!("expected quaternion array arg");
    };
    assert_eq!(items.len(), 4);
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
    let Statement::Reassign { target, value } = &c.body.statements[0] else {
        panic!("expected Reassign")
    };
    assert!(matches!(target, Expression::Identifier(name) if name.0 == "name"));
    assert!(matches!(value, Expression::String(s) if s == "root"));
}

#[test]
fn parse_call_in_body() {
    let prog = parse("BG { with_occlusion_and_lighting() }");
    let c = as_component!(&prog[0]);
    assert_eq!(c.body.statements.len(), 1);
    let Statement::Expression(Expression::Call(call)) = &c.body.statements[0] else {
        panic!("expected Call")
    };
    let Expression::Identifier(callee_id) = call.callee.as_ref() else {
        panic!("expected Identifier callee")
    };
    assert_eq!(callee_id.0, "with_occlusion_and_lighting");
    assert!(call.args.is_empty());
}

#[test]
fn parse_positional_string() {
    let prog = parse(r#"TXT { "hello" }"#);
    let c = as_component!(&prog[0]);
    assert_eq!(c.body.statements.len(), 1);
    let Statement::Expression(Expression::String(s)) = &c.body.statements[0] else {
        panic!("expected string expr")
    };
    assert_eq!(s, "hello");
}

#[test]
fn parse_positional_ident_flag() {
    let prog = parse("R { QUAD_2D }");
    let c = as_component!(&prog[0]);
    assert_eq!(c.body.statements.len(), 1);
    let Statement::Expression(Expression::Identifier(id)) = &c.body.statements[0] else {
        panic!("expected ident expr")
    };
    assert_eq!(id.0, "QUAD_2D");
}

#[test]
fn parse_named_assignment_array() {
    let prog = parse("T { rotation = [0.0, 0.0, 3.14] }");
    let c = as_component!(&prog[0]);
    let Statement::Reassign { target, value } = &c.body.statements[0] else {
        panic!("expected Reassign")
    };
    assert!(matches!(target, Expression::Identifier(name) if name.0 == "rotation"));
    let Expression::Array(items) = value else {
        panic!()
    };
    assert_eq!(items.len(), 3);
}

#[test]
fn parse_else_if_chain() {
    let prog = parse("if false { T {} } else if true { R {} } else { C {} }");
    let Statement::If(if_stmt) = &prog[0] else {
        panic!("expected if statement")
    };
    let else_if = match if_stmt.else_branch.as_ref() {
        Some(crate::scripting::ast::ElseBranch::If(next_if)) => next_if,
        _ => panic!("expected else-if branch"),
    };
    assert!(matches!(
        else_if.else_branch.as_ref(),
        Some(crate::scripting::ast::ElseBranch::Block(_))
    ));
}

#[test]
fn unparse_roundtrip_else_if_chain() {
    let src = "if false { T {} } else if true { R {} } else { C {} }";
    let prog = parse(src);
    let reparsed = parse(&unparse_program(&prog));
    assert_eq!(reparsed, prog);
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
    assert!(matches!(
        &c.body.statements[0],
        Statement::Expression(Expression::Call(_))
    ));
    assert!(matches!(
        &c.body.statements[1],
        Statement::Expression(Expression::Component(_))
    ));
    assert!(matches!(
        &c.body.statements[2],
        Statement::Expression(Expression::Identifier(_))
    ));
}

// ---------------------------------------------------------------------------
// Nested tree (controller cube from vr-input.mms)
// ---------------------------------------------------------------------------

#[test]
fn parse_controller_cube_tree() {
    let src = r#"
CTLXR.new(true, Left, Aim) {
    T.with_scale(0.06, 0.06, 0.12) {
        TransformForkTRS {
            TransformMapTranslation {}
            TransformMapRotation {
                QuatTemporalFilter.with_smoothing_factor(220.0)
            }
            TransformMapScale {}
            T {
                R.cube() {
                    C.rgba(0.10, 0.90, 1.00, 1.0)
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

    // T → TransformForkTRS
    assert_eq!(t_scale.body.statements.len(), 1);
    let pipeline = as_component!(&t_scale.body.statements[0]);
    assert_eq!(pipeline.component_type.0, "TransformForkTRS");

    // fork root → translation, rotation, scale, downstream T
    assert_eq!(pipeline.body.statements.len(), 4);
    let fork = pipeline;

    // fork → translation, rotation, scale
    assert_eq!(fork.body.statements.len(), 4);
    let map_rot = as_component!(&fork.body.statements[1]);
    assert_eq!(map_rot.component_type.0, "TransformMapRotation");

    // rotation filter child
    assert_eq!(map_rot.body.statements.len(), 1);
    let filter = as_component!(&map_rot.body.statements[0]);
    assert_eq!(filter.component_type.0, "QuatTemporalFilter");
    assert_eq!(
        filter.constructors.first().unwrap().method.0,
        "with_smoothing_factor"
    );

    // downstream T → R.cube → C.rgba
    let out_t = as_component!(&fork.body.statements[3]);
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
    let Statement::Assignment(a) = &prog[0] else {
        panic!()
    };
    assert_eq!(a.name.0, "x");
    assert!(matches!(a.value, Expression::Number(n) if n == 42.0));
}

#[test]
fn parse_table_literal_binding() {
    let prog = parse(r#"let foo = { bar = "baz" count = 3 }"#);
    let Statement::Assignment(a) = &prog[0] else {
        panic!()
    };
    let Expression::Table(fields) = &a.value else {
        panic!("expected table literal")
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name.0, "bar");
    assert!(matches!(fields[0].value, Expression::String(ref s) if s == "baz"));
    assert_eq!(fields[1].name.0, "count");
    assert!(matches!(fields[1].value, Expression::Number(n) if n == 3.0));
}

#[test]
fn parse_nested_table_literal_binding() {
    let prog = parse(
        r#"let foo = {
    bar = {
        baz = "qux"
    }
}"#,
    );
    let Statement::Assignment(a) = &prog[0] else {
        panic!()
    };
    let Expression::Table(fields) = &a.value else {
        panic!("expected outer table")
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name.0, "bar");
    let Expression::Table(inner_fields) = &fields[0].value else {
        panic!("expected nested table")
    };
    assert_eq!(inner_fields.len(), 1);
    assert_eq!(inner_fields[0].name.0, "baz");
    assert!(matches!(inner_fields[0].value, Expression::String(ref s) if s == "qux"));
}

#[test]
fn unparse_roundtrip_table_literal() {
    let src = r#"let foo = {
    bar = {
        baz = "qux"
    }
    count = 3
}"#;
    let prog = parse(src);
    let reparsed = parse(&unparse_program(&prog));
    assert_eq!(reparsed, prog);
}

#[test]
fn parse_mms_tables_example() {
    let src = fs::read_to_string(repo_path("examples/mms-tables.mms")).expect("read example");
    let prog = parse(&src);
    assert!(!prog.is_empty());
}

#[test]
fn parse_array_index_expression() {
    let prog = parse("let x = dims[0]");
    let Statement::Assignment(a) = &prog[0] else {
        panic!()
    };
    assert!(matches!(a.value, Expression::Index { .. }));
}

#[test]
fn parse_table_field_access_expression() {
    let prog = parse("let x = settings.theme.label");
    let Statement::Assignment(a) = &prog[0] else {
        panic!()
    };
    let Expression::BinaryOp {
        op: crate::scripting::ast::BinOpKind::Dot,
        lhs,
        rhs,
    } = &a.value
    else {
        panic!("expected dot field access")
    };
    assert!(matches!(rhs.as_ref(), Expression::Identifier(id) if id.0 == "label"));
    assert!(matches!(
        lhs.as_ref(),
        Expression::BinaryOp {
            op: crate::scripting::ast::BinOpKind::Dot,
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// Error cases
// ---------------------------------------------------------------------------

#[test]
fn parse_error_unterminated_body() {
    let tokens = MeowMeowTokenizer::new("T {")
        .tokenize()
        .expect("tokenize ok");
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
            Ok(EvalResponse::SnippetComplete { .. }) => {}
            Ok(EvalResponse::NavigationComplete { .. } | EvalResponse::ReplReset) => {}
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
    let Statement::ForIn {
        binding,
        iterable,
        body,
    } = &prog[0]
    else {
        panic!()
    };
    assert_eq!(binding.0, "x");
    assert!(matches!(iterable, Expression::Array(_)));
    assert_eq!(body.statements.len(), 1);
}

#[test]
fn parse_for_in_range_call() {
    let prog = parse("for i in range(10) { T {} }");
    assert_eq!(prog.len(), 1);
    let Statement::ForIn {
        binding, iterable, ..
    } = &prog[0]
    else {
        panic!()
    };
    assert_eq!(binding.0, "i");
    let Expression::Call(call) = iterable else {
        panic!()
    };
    let Expression::Identifier(callee_id) = call.callee.as_ref() else {
        panic!("expected Identifier callee")
    };
    assert_eq!(callee_id.0, "range");
    assert_eq!(call.args.len(), 1);
}

#[test]
fn parse_break_and_continue() {
    let prog = parse("for i in range(5) { break; continue }");
    let Statement::ForIn { body, .. } = &prog[0] else {
        panic!()
    };
    assert!(matches!(body.statements[0], Statement::Break));
    assert!(matches!(body.statements[1], Statement::Continue));
}

// --- eval tests ---

fn eval(src: &str) -> crate::scripting::runner::EvalOutput {
    MeowMeowRunner::eval(src)
}

#[test]
fn live_eval_emitted_tree_is_queryable_by_next_statement() {
    let src = r##"
        T {
            name = "panel"
            T {
                name = "btn_a"
                Raycastable.enabled()
                Text { "hello" }
            }
        }

        let btn_a = query("#btn_a")
        assert(btn_a, "expected btn_a to exist after prior emit")
        on(btn_a, "Click", fn(event) {
            print("clicked")
        })
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert!(
        world
            .find_component(
                world
                    .all_components()
                    .find(|&id| world.parent_of(id).is_none())
                    .unwrap(),
                "#btn_a"
            )
            .is_some()
    );
}

#[test]
fn live_eval_reassigned_component_expr_supports_query_method_after_emit() {
    let src = r##"
        let layout_root = null

        T {
            layout_root = LayoutRoot {
                T {
                    name = "btn_a"
                    Text { "hello" }
                }
            }

            layout_root
        }

        let btn_a = layout_root.query("#btn_a")
        assert(btn_a, "expected layout_root.query('#btn_a') to work after prior emit")
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
}

#[test]
fn live_eval_let_bound_component_expr_can_mutate_before_and_after_attach() {
    let src = r##"
        let glow = Emissive.off()
        glow.set_intensity(0.2)

        T {
            R.cube() {
                glow
            }
        }

        glow.set_intensity(2.5)
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let glow = world
        .all_components()
        .find_map(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::EmissiveComponent>(id)
                .map(|glow| (id, glow.intensity))
        })
        .expect("expected EmissiveComponent");
    assert!(
        (glow.1 - 2.5).abs() < 1.0e-6,
        "expected final emissive intensity 2.5, got {} on {:?}",
        glow.1,
        glow.0
    );
}

#[test]
fn live_eval_nested_let_attached_transform_animates_via_keyframe_block() {
    let src = r##"
        Clock.bpm(60) {}

        let cube_t = T.position(0.0, 0.0, 0.0) {
            name = "cube_t"
            Transition {
                duration_beats(1.0)
                linear()
                replace_same_target()
            }
        }

        let parent_t = T.position(0.0, 0.0, 0.0) {
            name = "parent_t"
            cube_t
        }

        parent_t

        Animation.looping().length(2.0) {
            Keyframe.at(0.0) {
                cube_t.update_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            }
            Keyframe.at(1.0) {
                cube_t.update_transform([1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 1.0, 1.0])
            }
        }
    "##;

    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let input = InputState::default();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut systems.rx, &mut queue);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    for intent in out.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }

    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    let parent_t = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("parent_t"))
        .expect("parent_t exists");
    let cube_t = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("cube_t"))
        .expect("cube_t exists");
    assert_eq!(world.parent_of(cube_t), Some(parent_t));

    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &input,
        &mut queue,
        0.1,
    );
    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &input,
        &mut queue,
        1.0,
    );
    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &input,
        &mut queue,
        0.5,
    );

    let transform = world
        .get_component_by_id_as::<TransformComponent>(cube_t)
        .expect("cube_t transform exists");
    assert!(
        transform.transform.translation[0] > 0.0,
        "expected transition to begin moving cube_t, got {:?}",
        transform.transform.translation
    );
}

#[test]
fn live_eval_attached_emissive_transition_interpolates_set_intensity() {
    let src = r##"
        Clock.bpm(60) {}

        let glow = Emissive.off() {
            name = "glow"
            Transition {
                duration_beats(1.0)
                linear()
                replace_same_target()
            }
        }

        T {
            R.cube() {
                glow
            }
        }

        glow.set_intensity(2.0)
    "##;

    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let input = InputState::default();

    let driver = TestClockDriver::default();
    systems.clock.set_driver(Arc::new(driver.clone()));
    systems.clock.set_bpm(60.0);
    driver.set_time_sec(0.0);
    systems.clock.sample();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut systems.rx, &mut queue);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    for intent in out.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }

    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    let glow = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("glow"))
        .expect("glow exists");

    let initial = world
        .get_component_by_id_as::<crate::engine::ecs::component::EmissiveComponent>(glow)
        .expect("glow emissive exists");
    assert_eq!(initial.intensity, 0.0);

    driver.set_time_sec(0.5);
    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &input,
        &mut queue,
        0.0,
    );

    let halfway = world
        .get_component_by_id_as::<crate::engine::ecs::component::EmissiveComponent>(glow)
        .expect("glow emissive exists");
    assert!(
        halfway.intensity > 0.0 && halfway.intensity < 2.0,
        "expected interpolated emissive intensity, got {}",
        halfway.intensity
    );

    driver.set_time_sec(1.0);
    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &input,
        &mut queue,
        0.0,
    );

    let finished = world
        .get_component_by_id_as::<crate::engine::ecs::component::EmissiveComponent>(glow)
        .expect("glow emissive exists");
    assert!(
        (finished.intensity - 2.0).abs() < 1.0e-6,
        "expected final emissive intensity 2.0, got {}",
        finished.intensity
    );
}

#[test]
fn live_eval_imported_factory_component_supports_top_level_update_transform() {
    let src = r##"
        import { rainbow_animated } from "../assets/components/animated.mms"

        let rainbow = rainbow_animated()
        rainbow
        rainbow.update_transform([0.0, 4.0, -4.0], [0.0, 0.0, 0.0], [1.6, 1.6, 1.6])
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world_at_path(
        src,
        Some("examples/_mms_test_top_level_component_method_dispatch.mms"),
        &mut world,
        &mut rx,
        &mut emit,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert!(
        out.intents.iter().any(|intent| matches!(
            intent,
            crate::engine::ecs::IntentValue::UpdateTransform {
                translation,
                scale,
                ..
            } if *translation == [0.0, 4.0, -4.0] && *scale == [1.6, 1.6, 1.6]
        )),
        "expected top-level imported factory method call to emit UpdateTransform, got {:?}",
        out.intents
    );
}

#[test]
fn live_eval_math_builtin_table_supports_trig_and_rounding() {
    let src = r##"
        assert(Math.abs(Math.sin(Math.pi / 2.0) - 1.0) < 0.0001, "expected sin(pi/2) ~= 1")
        assert(Math.abs(Math.cos(Math.pi) + 1.0) < 0.0001, "expected cos(pi) ~= -1")
        assert(Math.sqrt(9.0) == 3.0, "expected sqrt")
        assert(Math.floor(3.8) == 3.0, "expected floor")
        assert(Math.ceil(3.2) == 4.0, "expected ceil")
        assert(Math.round(3.6) == 4.0, "expected round")
        assert(Math.atan2(1.0, 0.0) > 1.5, "expected atan2")
        assert(Math.dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]) == 32.0, "expected dot")
        let cross = Math.cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])
        assert(cross[0] == 0.0, "expected cross.x")
        assert(cross[1] == 0.0, "expected cross.y")
        assert(cross[2] == 1.0, "expected cross.z")
        assert(Math.clamp(-1.0, 0.0, 1.0) == 0.0, "expected lower clamp")
        assert(Math.clamp(2.0, 0.0, 1.0) == 1.0, "expected upper clamp")
        assert(Math.smoothstep(-1.0, 0.0, 1.0) == 0.0, "expected smoothstep lower clamp")
        assert(Math.smoothstep(2.0, 0.0, 1.0) == 1.0, "expected smoothstep upper clamp")
        assert(Math.abs(Math.smoothstep(0.5, 0.0, 1.0) - 0.5) < 0.0001, "expected smoothstep midpoint")
        let p2 = Math.perlin(1.25, 2.5)
        let p2_again = Math.perlin(1.25, 2.5)
        let p3 = Math.perlin(1.25, 2.5, 7.0)
        assert(p2 >= -1.0, "expected perlin lower bound")
        assert(p2 <= 1.0, "expected perlin upper bound")
        assert(p2 == p2_again, "expected perlin deterministic")
        assert(Math.abs(p2 - p3) > 0.0001, "expected z slice variation")
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
}

#[test]
fn live_eval_math_builtin_table_reports_invalid_usage() {
    let cases = [
        (
            "Math.perlin(0.5)",
            "Math.perlin(): expected 2 or 3 numeric arguments",
        ),
        (
            "Math.perlin(0.5, 1.0, nope)",
            "Math.perlin(): expected 2 or 3 numeric arguments",
        ),
        (
            "Math.dot([1.0, 2.0, 3.0])",
            "Math.dot(): expected 2 array arguments",
        ),
        (
            "Math.dot([1.0, 2.0], [3.0, 4.0, 5.0])",
            "Math.dot(): expected arrays of equal length",
        ),
        (
            "Math.dot([1.0, test], [3.0, 4.0])",
            "Math.dot(): arg 0 expected numeric array element",
        ),
        (
            "Math.cross([1.0, 0.0], [0.0, 1.0])",
            "Math.cross(): arg 0 expected array of 3, got 2",
        ),
        (
            "Math.cross([1.0, 0.0, 0.0], [0.0, nope, 0.0])",
            "Math.cross(): arg 1 expected numeric array element",
        ),
        (
            "Math.clamp(0.5, 0.0)",
            "Math.clamp(): expected 3 numeric arguments",
        ),
        (
            "Math.smoothstep(0.5, 0.0)",
            "Math.smoothstep(): expected 3 numeric arguments",
        ),
        (
            "Math.smoothstep(0.5, 1.0, 1.0)",
            "Math.smoothstep(): edge0 and edge1 must be distinct",
        ),
    ];

    for (src, expected) in cases {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut emit = CommandQueue::new();
        let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
        assert!(!out.errors.is_empty(), "expected error for {src}");
        assert!(
            out.errors[0].contains(expected),
            "expected error containing {expected:?}, got {:?}",
            out.errors
        );
    }
}

#[test]
fn live_eval_imported_kawaii_background_module_emits_without_errors() {
    let src = r##"
        import { star_kawaii_background } from "../assets/components/backgrounds/star_kawaii_background.mms"

        BG {
            star_kawaii_background()
        }
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world_at_path(
        src,
        Some("examples/_mms_test_star_kawaii_background_import.mms"),
        &mut world,
        &mut rx,
        &mut emit,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert!(
        !out.intents.is_empty(),
        "expected imported background to emit content"
    );
}

#[test]
fn star_kawaii_background_derives_rotation_from_position() {
    let src = fs::read_to_string(repo_path(
        "assets/components/backgrounds/star_kawaii_background.mms",
    ))
    .expect("read star background");
    assert!(
        src.contains("let dir_x = -x / radius")
            && src.contains("let dir_y = -y / radius")
            && src.contains("let dir_z = -z / radius"),
        "expected star background to derive inward direction from position"
    );
    assert!(
        src.contains("let look_raw = [-dir_y, dir_x, 0.0, 1.0 + dir_z]")
            && src.contains("let look_inv_len = 1.0 / Math.sqrt("),
        "expected star background to build a shortest-arc look quaternion"
    );
    assert!(
        src.contains("let twist_quat = [0.0, 0.0, Math.sin(half_twist), Math.cos(half_twist)]")
            && src.contains("let rotation = quat_mul(look, twist_quat)")
            && src.contains(".quat(rotation)"),
        "expected star background to use quaternion look-at plus local twist"
    );
    assert!(
        !src.contains(".rotation("),
        "expected Euler rotation authoring to be removed from the star background"
    );
}

#[test]
fn live_eval_transform_quaternion_builder_supports_array_aliases() {
    let src = r##"
        T.quat([0.0, 0.0, 0.0, 1.0]) {
            T.quaternion([0.0, 0.0, 0.0, 1.0]) {}
        }
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert!(
        !out.intents.is_empty(),
        "expected quaternion-authored transforms to emit content"
    );
}

#[test]
fn live_eval_transform_looking_at_builder_queues_look_at_intent() {
    let src = r##"
        T.position(1.0, 2.0, 3.0).looking_at([4.0, 5.0, 6.0]) {}
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    emit.drain_into_rx(&mut rx);
    let intents = rx.drain_ready_intents();
    assert!(
        intents.iter().any(|env| matches!(
            env.intent.as_ref().map(|i| &i.value),
            Some(IntentValue::LookAt { target_world, .. }) if *target_world == [4.0, 5.0, 6.0]
        )),
        "expected builder-authored transform init to queue LookAt, got {:?}",
        intents
            .iter()
            .map(|env| env.intent.as_ref().map(|i| i.value.kind_name()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn live_eval_component_object_transform_look_at_emits_look_at_intent() {
    let src = r##"
        let t = T {}
        t.look_at([0.0, 1.0, 0.0])
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert!(
        out.intents.iter().any(|intent| matches!(
            intent,
            IntentValue::LookAt { target_world, .. } if *target_world == [0.0, 1.0, 0.0]
        )),
        "expected live component method to emit LookAt, got {:?}",
        out.intents
    );
}

#[test]
fn live_eval_imported_factory_keyframe_closure_captures_live_component_objects() {
    let src = r##"
        import { rainbow_animated } from "../assets/components/animated.mms"
        rainbow_animated()
    "##;

    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let input = InputState::default();

    let driver = TestClockDriver::default();
    systems.clock.set_driver(Arc::new(driver.clone()));
    systems.clock.set_bpm(60.0);
    driver.set_time_sec(0.0);
    systems.clock.sample();

    let out = MeowMeowRunner::eval_with_world_at_path(
        src,
        Some("examples/_mms_test_imported_factory_keyframe_live_handles.mms"),
        &mut world,
        &mut systems.rx,
        &mut queue,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    for intent in out.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }

    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    driver.set_time_sec(0.5);
    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &input,
        &mut queue,
        0.0,
    );

    let intensities: Vec<f32> = world
        .all_components()
        .filter_map(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::EmissiveComponent>(id)
                .map(|emissive| emissive.intensity)
        })
        .collect();
    assert!(
        intensities.iter().any(|intensity| *intensity > 0.2),
        "expected imported factory keyframe callback to drive emissive intensity, got {:?}",
        intensities
    );
}

#[test]
fn live_keyframe_block_music_note_emits_audio_schedule_play() {
    use crate::engine::ecs::IntentValue;
    use crate::engine::ecs::component::MusicNote;

    let src = r##"
        Clock.bpm(60) {}

        let lead = AudioOscillator.square() {
            name = "lead"
        };

        AudioOutput {
            lead;
        }

        Animation.looping() {
            Keyframe.at(0.0) {
                MusicNote.e(4, 0.25, lead)
            }
        }
    "##;

    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut queue = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut systems.rx, &mut queue);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    for intent in out.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }

    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
    systems
        .animation
        .tick_with_beat(&mut world, 0.0, 60.0, &mut systems.rx);

    let intents = systems.rx.drain_ready_intents();
    assert!(
        intents.iter().all(|signal| {
            !matches!(
                signal.intent.as_ref().map(|intent| &intent.value),
                Some(IntentValue::SpawnComponentTree { .. })
            )
        }),
        "keyframe callback should not spawn detached MusicNote trees: {:?}",
        intents
    );

    let audio = intents
        .iter()
        .find_map(
            |signal| match signal.intent.as_ref().map(|intent| &intent.value) {
                Some(IntentValue::AudioSchedulePlay {
                    component_ids,
                    note,
                    beat_offset,
                    ..
                }) => Some((component_ids.clone(), note.clone(), *beat_offset)),
                _ => None,
            },
        )
        .expect("expected AudioSchedulePlay from keyframe MusicNote");

    assert_eq!(audio.2, 0.0);
    let note = audio.1.expect("expected note payload");
    assert_eq!(note.pitch_name(), MusicNote::e(4, 0.25).pitch_name());
    assert_eq!(note.octave(), 4);
    assert!((note.duration_beats() - 0.25).abs() < 1.0e-6);
    assert_eq!(audio.0.len(), 1);
}

#[test]
fn live_handler_query_can_see_world() {
    let src = r##"
        T { name = "btn" }
        T {
            Text { "(unclicked)" name = "target" }
        }

        let btn = query("#btn")
        on(btn, "Click", fn(event) {
            let t = query("#target")
            if t {
                t.set_text("clicked")
            }
        })
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    let btn_id = world
        .all_components()
        .filter(|&id| world.parent_of(id).is_none())
        .find_map(|root| world.find_component(root, "#btn"))
        .expect("expected #btn");

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            btn_id,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: btn_id,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        ),
    );

    let intents = rx.drain_ready_intents();
    assert!(
        intents.iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(crate::engine::ecs::IntentValue::SetText { text, .. }) if text == "clicked"
        )),
        "expected handler query to resolve target and emit SetText"
    );
}

#[test]
fn mms_layoutroot_available_width_and_percent_style_width_reach_live_components() {
    let src = r##"
        LayoutRoot {
            name = "root"
            available_width(29.0)

            T {
                name = "panel"
                Style {
                    width(100%)
                }
            }
        }
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let root = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("root"))
        .expect("root component");
    let layout = world
        .get_component_by_id_as::<LayoutComponent>(root)
        .expect("layout component on root");
    assert!((layout.available_width - 29.0).abs() < 1e-6);

    let panel = world
        .find_component(root, "#panel")
        .expect("panel transform");
    let style_id = world
        .children_of(panel)
        .iter()
        .copied()
        .find(|&child| {
            world
                .get_component_by_id_as::<StyleComponent>(child)
                .is_some()
        })
        .expect("panel style");
    let style = world
        .get_component_by_id_as::<StyleComponent>(style_id)
        .expect("style component");

    assert_eq!(style.width, SizeDimension::Percent(100.0));
}

#[test]
fn mms_dimension_arrays_feed_transform_and_layout_boundaries() {
    let src = r##"
        let meme_dimensions = [2.85188wu, 4wu]

        T.position(-meme_dimensions[0] / 2.0, -meme_dimensions[1] / 2.0, 0.0).scale(meme_dimensions[0], meme_dimensions[1], 1.0) {
            name = "panel"
            LayoutRoot {
                name = "layout"
                available_width(meme_dimensions[0])
                available_height(meme_dimensions[1])
            }
        }
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let panel = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("panel"))
        .expect("panel transform");
    let transform = world
        .get_component_by_id_as::<TransformComponent>(panel)
        .expect("transform component");
    assert!((transform.transform.translation[0] + 1.42594).abs() < 1e-5);
    assert!((transform.transform.translation[1] + 2.0).abs() < 1e-6);
    assert!((transform.transform.scale[0] - 2.85188).abs() < 1e-5);
    assert!((transform.transform.scale[1] - 4.0).abs() < 1e-6);

    let layout = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("layout"))
        .and_then(|id| world.get_component_by_id_as::<LayoutComponent>(id))
        .expect("layout component");
    assert_eq!(
        layout.authored_available_width,
        SizeDimension::WorldUnits(2.85188)
    );
    assert_eq!(
        layout.authored_available_height,
        Some(SizeDimension::WorldUnits(4.0))
    );
}

#[test]
fn mms_layoutroot_available_size_accepts_gu_and_wu_independent_of_unit_scale_order() {
    let src = r##"
        LayoutRoot {
            name = "gu_root"
            available_width(34gu)
            available_height(24gu)
        }

        LayoutRoot {
            name = "wu_after"
            available_width(4.0380833wu)
            available_height(2.851wu)
            unit_scale(2.851 / 24.0)
        }

        LayoutRoot {
            name = "wu_before"
            unit_scale(2.851 / 24.0)
            available_width(4.0380833wu)
            available_height(2.851wu)
        }
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    for root_name in ["gu_root", "wu_after", "wu_before"] {
        let root = world
            .all_components()
            .find(|&id| world.component_label(id) == Some(root_name))
            .unwrap_or_else(|| panic!("missing root {root_name}"));
        let layout = world
            .get_component_by_id_as::<LayoutComponent>(root)
            .expect("layout component on root");

        assert!(
            (layout.available_width - 34.0).abs() < 1e-4,
            "wrong width for {root_name}"
        );
        assert!(
            (layout.available_height.unwrap_or_default() - 24.0).abs() < 1e-4,
            "wrong height for {root_name}"
        );
    }
}

#[test]
fn handler_registered_inside_function_body_fires() {
    // Regression for: function-call EvalContext used to hard-code
    // `channels: None` / `host_world: None`, so `on(...)` inside a
    // factory function silently no-op'd. After forwarding the caller's
    // channels + host_world through, the handler must actually register.
    let src = r##"
        T { name = "btn" }
        T {
            Text { "(unclicked)" name = "target" }
        }

        fn wire_click(target_handle) {
            on(target_handle, "Click", fn(event) {
                let t = query("#target")
                if t {
                    t.set_text("clicked-from-fn")
                }
            })
        }

        let btn = query("#btn")
        wire_click(btn)
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let btn_id = world
        .all_components()
        .filter(|&id| world.parent_of(id).is_none())
        .find_map(|root| world.find_component(root, "#btn"))
        .expect("expected #btn");

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            btn_id,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: btn_id,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        ),
    );

    let intents = rx.drain_ready_intents();
    assert!(
        intents.iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(crate::engine::ecs::IntentValue::SetText { text, .. }) if text == "clicked-from-fn"
        )),
        "expected on(...) registered inside fn body to fire and emit SetText"
    );
}

#[test]
fn global_frame_tick_handler_reads_translation_and_dt() {
    let src = r##"
        let driven = T.position(2.0, 3.0, 4.0) { name = "driven" }
        driven
        let initial_position = driven.translation()
        Text { "waiting" name = "frame_status" }

        on_global("FrameTick", fn(event) {
            let position = driven.translation()
            if position[0] == 2.0 && event.dt_sec == 0.25 {
                let status = query("#frame_status")
                status.set_text("frame-observed")
            }
        })
    "##;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert!(rx.has_global_handlers(crate::engine::ecs::SignalKind::FrameTick));

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 0.25 },
        ),
    );
    assert!(rx.drain_ready_intents().iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(crate::engine::ecs::IntentValue::SetText { text, .. }) if text == "frame-observed"
    )));
}

#[test]
fn live_pose_handles_expose_replace_overlay_and_clamped_blend() {
    let src = r##"
        let target = T {}
        let pose = PoseCapturePose.new("run")
        pose.apply(target)
        pose.overlay(target)
        pose.apply_blended(target, 4.0)
    "##;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 3);
    assert!(matches!(
        out.intents[0],
        IntentValue::PoseApply {
            mode: crate::engine::ecs::PoseApplyMode::Replace,
            ..
        }
    ));
    assert!(matches!(
        out.intents[1],
        IntentValue::PoseApply {
            mode: crate::engine::ecs::PoseApplyMode::Overlay,
            ..
        }
    ));
    assert!(matches!(
        out.intents[2],
        IntentValue::PoseApply {
            mode: crate::engine::ecs::PoseApplyMode::RestBlend { amount: 1.0 },
            ..
        }
    ));
}

#[test]
fn gltf_pose_animation_example_imports_named_pose_factories() {
    let source = include_str!("../../examples/gltf-pose-animation.mms");
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let mut render_assets = RenderAssets::new();
    let out = MeowMeowRunner::eval_with_world_and_assets_at_path(
        source,
        Some("examples/gltf-pose-animation.mms"),
        &mut world,
        &mut rx,
        Some(&mut render_assets),
        &mut emit,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert!(rx.has_global_handlers(crate::engine::ecs::SignalKind::FrameTick));
    let poses: Vec<_> = world
        .all_components()
        .filter_map(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::PoseCapturePoseComponent>(
                    id,
                )
        })
        .collect();
    assert_eq!(poses.len(), 3);
    let pose_sizes: std::collections::HashMap<_, _> = poses
        .iter()
        .map(|pose| (pose.name.as_str(), pose.entries.len()))
        .collect();
    assert_eq!(pose_sizes.get("relaxed"), Some(&6));
    assert_eq!(pose_sizes.get("running_1"), Some(&7));
    assert_eq!(pose_sizes.get("running_2"), Some(&8));
    assert!(poses.iter().all(|pose| pose.entries.iter().all(|entry| {
        !entry.query.contains("J_Bip_C_Head") && !entry.query.contains("J_Sec_")
    })));
    assert_eq!(source.matches(".overlay(avatar_gltf)").count(), 5);
    assert_eq!(source.matches(".apply(avatar_gltf)").count(), 0);
    let avatar_gltf = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(*id)
                .is_some()
        })
        .expect("avatar glTF");
    let direct_startup_poses: Vec<_> = world
        .children_of(avatar_gltf)
        .iter()
        .filter_map(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::PoseCapturePoseComponent>(
                    *id,
                )
        })
        .collect();
    assert_eq!(direct_startup_poses.len(), 1);
    assert_eq!(direct_startup_poses[0].name, "relaxed");
    assert_eq!(
        world
            .all_components()
            .filter(|&id| world
                .get_component_by_id_as::<crate::engine::ecs::component::SecondaryMotionComponent>(
                    id
                )
                .is_some())
            .count(),
        1
    );

    let animations: Vec<_> = world
        .all_components()
        .filter_map(|id| {
            world.get_component_by_id_as::<crate::engine::ecs::component::AnimationComponent>(id)
        })
        .collect();
    assert_eq!(animations.len(), 1);
    assert_eq!(animations[0].length_beats, Some(1.0));
    let mut keyframe_beats: Vec<_> = world
        .all_components()
        .filter_map(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::KeyframeComponent>(id)
                .map(|keyframe| keyframe.beat)
        })
        .collect();
    keyframe_beats.sort_by(f64::total_cmp);
    assert_eq!(keyframe_beats, vec![0.0, 0.4_f32 as f64, 0.5, 0.75]);
}

#[test]
fn secondary_motion_desktop_example_has_studio_collision_and_no_xr() {
    use crate::engine::ecs::component::{
        Camera3DComponent, CameraXRComponent, CollisionComponent, CollisionMode,
        InputComponent, InputTransformModeComponent, InputXRComponent, SpotLightComponent,
        KineticResponseComponent, KineticResponseMode, RenderableComponent,
        SecondaryMotionComponent, SpringBoneComponent,
    };
    let source = include_str!("../../examples/secondary-motion-desktop.mms");
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let mut render_assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        source,
        Some("examples/secondary-motion-desktop.mms"),
        &mut world,
        &mut rx,
        Some(&mut render_assets),
        &mut emit,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    assert!(!source.contains(".overlay(avatar_gltf)"));

    let avatar_gltf = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(*id)
                .is_some()
        })
        .expect("avatar glTF");
    assert_eq!(
        world
            .children_of(avatar_gltf)
            .iter()
            .filter(|id| {
                world
                    .get_component_by_id_as::<
                        crate::engine::ecs::component::PoseCapturePoseComponent,
                    >(**id)
                    .is_some()
            })
            .count(),
        1
    );

    let ids: Vec<_> = world.all_components().collect();
    let count = |predicate: &dyn Fn(crate::engine::ecs::ComponentId) -> bool| {
        ids.iter().copied().filter(|id| predicate(*id)).count()
    };
    let named = |label: &str| {
        ids.iter()
            .copied()
            .find(|id| world.component_label(*id) == Some(label))
            .unwrap_or_else(|| panic!("missing named scene node {label}"))
    };
    let descendants = |root| {
        let mut found = Vec::new();
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            found.push(id);
            pending.extend(world.children_of(id).iter().copied());
        }
        found
    };

    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<SecondaryMotionComponent>(id)
            .is_some()),
        1
    );
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<SpringBoneComponent>(id)
            .is_some()),
        16
    );

    for light_name in ["studio_key_light", "studio_fill_light", "studio_rim_light"] {
        let tree = descendants(named(light_name));
        assert_eq!(
            tree.iter()
                .filter(|&&id| world
                    .get_component_by_id_as::<SpotLightComponent>(id)
                    .is_some())
                .count(),
            1
        );
        assert!(
            tree.iter()
                .filter(|&&id| world
                    .get_component_by_id_as::<RenderableComponent>(id)
                    .is_some())
                .count()
                >= 6
        );
        assert!(
            tree.iter()
                .any(|&id| world.component_label(id) == Some("studio_light_housing"))
        );
        assert!(
            tree.iter()
                .any(|&id| world.component_label(id) == Some("studio_light_emissive_face"))
        );
    }

    let scenery = [
        "studio_floor",
        "pile_a_base_left",
        "pile_a_base_right",
        "pile_a_top",
        "pile_b_base_left",
        "pile_b_base_right",
        "pile_b_top",
        "pile_c_base",
        "pile_c_top",
    ];
    for name in scenery {
        let tree = descendants(named(name));
        assert_eq!(
            tree.iter()
                .filter(|&&id| world
                    .get_component_by_id_as::<CollisionComponent>(id)
                    .is_some_and(|collision| collision.mode == CollisionMode::Static))
                .count(),
            1,
            "{name}"
        );
    }

    let avatar_tree = descendants(named("avatar_driver"));
    assert_eq!(
        avatar_tree
            .iter()
            .filter(|&&id| world
                .get_component_by_id_as::<CollisionComponent>(id)
                .is_some_and(|collision| collision.mode == CollisionMode::Kinematic))
            .count(),
        1
    );
    assert_eq!(
        avatar_tree
            .iter()
            .filter(|&&id| world
                .get_component_by_id_as::<KineticResponseComponent>(id)
                .is_some_and(|response| response.mode == KineticResponseMode::Slide))
            .count(),
        1
    );

    assert_eq!(
        count(&|id| world.get_component_by_id_as::<InputComponent>(id).is_some()),
        2
    );
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<InputTransformModeComponent>(id)
            .is_some()),
        2
    );

    let locomotion_mode = world
        .children_of(named("desktop_avatar_input"))
        .iter()
        .find_map(|id| world.get_component_by_id_as::<InputTransformModeComponent>(*id))
        .expect("desktop locomotion mode");
    assert!(!locomotion_mode.rotation_enabled);
    assert!(matches!(
        locomotion_mode.translation_basis_source.as_ref(),
        Some(crate::engine::ecs::component::ComponentRef::Query(query))
            if query == "../#avatar_head_driver"
    ));

    let head_mode = world
        .children_of(named("desktop_head_input"))
        .iter()
        .find_map(|id| world.get_component_by_id_as::<InputTransformModeComponent>(*id))
        .expect("desktop head mode");
    assert!(head_mode.rotation_enabled && head_mode.fps_rotation);
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<Camera3DComponent>(id)
            .is_some()),
        1
    );
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<InputXRComponent>(id)
            .is_some()),
        0
    );
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<CameraXRComponent>(id)
            .is_some()),
        0
    );
}

#[test]
fn secondary_motion_desktop_avatar_separates_from_named_pile_cube() {
    use crate::engine::ecs::component::{CollisionComponent, TransformComponent};
    use winit::event::MouseButton;

    let source = include_str!("../../examples/secondary-motion-desktop.mms");
    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        source,
        Some("examples/secondary-motion-desktop.mms"),
        &mut world,
        &mut systems.rx,
        Some(&mut render_assets),
        &mut queue,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    for intent in output.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    let studio_spots: Vec<_> = visuals
        .lights()
        .iter()
        .filter(|light| light.light_type == 3)
        .collect();
    assert_eq!(studio_spots.len(), 3);
    for light in studio_spots {
        let to_target = [
            -light.position_ws[0],
            -0.35 - light.position_ws[1],
            -light.position_ws[2],
        ];
        let to_target_len =
            (to_target[0] * to_target[0] + to_target[1] * to_target[1] + to_target[2] * to_target[2])
                .sqrt();
        let alignment = (light.direction_ws[0] * to_target[0]
            + light.direction_ws[1] * to_target[1]
            + light.direction_ws[2] * to_target[2])
            / to_target_len;
        assert!(alignment > 0.999, "spotlight misses studio target: {alignment}");
        assert!((light.angle - 0.62).abs() < 1e-6);
        assert!((light.penumbra - 0.35).abs() < 1e-6);
    }

    let named = |world: &World, label: &str| {
        world
            .all_components()
            .find(|id| world.component_label(*id) == Some(label))
            .unwrap_or_else(|| panic!("missing named scene node {label}"))
    };
    let avatar_driver = named(&world, "avatar_driver");
    let avatar_head_driver = named(&world, "avatar_head_driver");
    let obstacle_transform = named(&world, "pile_a_base_left");
    let avatar_collider = world
        .children_of(avatar_driver)
        .iter()
        .copied()
        .find(|id| {
            world
                .get_component_by_id_as::<CollisionComponent>(*id)
                .is_some()
        })
        .expect("avatar collider directly under driver");
    let obstacle_collider = world
        .children_of(obstacle_transform)
        .iter()
        .copied()
        .find(|id| {
            world
                .get_component_by_id_as::<CollisionComponent>(*id)
                .is_some()
        })
        .expect("pile cube collider");

    // Right-drag rotates only the head-level driver. The body/collider root
    // must neither rotate nor translate around the 0.8-unit head offset.
    let body_before = world
        .get_component_by_id_as::<TransformComponent>(avatar_driver)
        .unwrap()
        .transform;
    let mut mouse_input = InputState::default();
    mouse_input.cursor_pos = Some((0.0, 0.0));
    mouse_input.start_frame();
    mouse_input.mouse_down.insert(MouseButton::Right);
    mouse_input.cursor_pos = Some((40.0, 20.0));
    mouse_input.start_frame();
    systems
        .input
        .process_input(&mut world, &mouse_input, &mut queue, 1.0 / 60.0);
    queue.flush(&mut world, &mut systems, &mut visuals, &mut render_assets);

    let body_after_mouse = world
        .get_component_by_id_as::<TransformComponent>(avatar_driver)
        .unwrap()
        .transform;
    let head_after_mouse = world
        .get_component_by_id_as::<TransformComponent>(avatar_head_driver)
        .unwrap()
        .transform;
    assert_eq!(body_after_mouse.translation, body_before.translation);
    assert_eq!(body_after_mouse.rotation, body_before.rotation);
    assert_ne!(head_after_mouse.rotation, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(head_after_mouse.translation, [0.0, 0.8, 0.0]);

    let obstacle_position = world
        .get_component_by_id_as::<TransformComponent>(obstacle_transform)
        .unwrap()
        .transform
        .translation;
    {
        let avatar = world
            .get_component_by_id_as_mut::<TransformComponent>(avatar_driver)
            .unwrap();
        avatar.transform.translation = obstacle_position;
        avatar.transform.recompute_model();
    }

    let input = InputState::default();
    systems.transform.transform_changed(
        &mut world,
        &mut visuals,
        avatar_driver,
        &mut systems.transform_stream,
        &mut systems.camera,
        &mut systems.light,
        &mut systems.collision,
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        systems.collision.tick_with_rx(
            &mut world,
            &mut visuals,
            &input,
            1.0 / 60.0,
            &mut systems.rx,
        );
        if systems
            .collision
            .active_pairs_snapshot()
            .iter()
            .any(|&(a, b)| {
                (a == avatar_collider && b == obstacle_collider)
                    || (a == obstacle_collider && b == avatar_collider)
            })
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "collision worker did not report overlap"
        );
        std::thread::yield_now();
    }

    systems.kinetic_response.tick_with_queue(
        &mut world,
        &mut visuals,
        &input,
        1.0 / 60.0,
        &mut queue,
        &systems.collision,
    );
    queue.flush(&mut world, &mut systems, &mut visuals, &mut render_assets);
    systems.transform.transform_changed(
        &mut world,
        &mut visuals,
        avatar_driver,
        &mut systems.transform_stream,
        &mut systems.camera,
        &mut systems.light,
        &mut systems.collision,
    );

    let separated = world
        .get_component_by_id_as::<TransformComponent>(avatar_driver)
        .unwrap()
        .transform
        .translation;
    let delta = [
        (separated[0] - obstacle_position[0]).abs(),
        (separated[1] - obstacle_position[1]).abs(),
        (separated[2] - obstacle_position[2]).abs(),
    ];
    assert!(
        delta[0] >= 0.34 + 0.425 || delta[1] >= 0.8 + 0.4 || delta[2] >= 0.28 + 0.425,
        "avatar remained inside pile_a_base_left: delta={delta:?}"
    );
}

#[test]
fn mms_named_handler_is_filtered_by_observer_router() {
    let src = r##"
        let router = ObserverRouter {}
        let root = T {
            name = "router_root"
            router
        }
        T {
            Text { "(idle)" name = "target" }
        }

        on(root, "DataEvent", "named_light", fn(event) {
            if event == "pulse_on" {
                let t = query("#target")
                if t {
                    t.set_text("allowed")
                }
            }
        })
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let root_id = world
        .all_components()
        .filter(|&id| world.parent_of(id).is_none())
        .find_map(|root| world.find_component(root, "#router_root"))
        .expect("expected #router_root");
    let router_id = world
        .children_of(root_id)
        .iter()
        .copied()
        .find(|&child| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::SignalObserverRouterComponent>(
                    child,
                )
                .is_some()
        })
        .expect("expected ObserverRouter child");

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            root_id,
            EventSignal::DataEvent {
                name: "pulse_on".to_string(),
                payload: None,
            },
        ),
    );

    let intents = rx.drain_ready_intents();
    assert!(
        intents.iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(crate::engine::ecs::IntentValue::SetText { text, .. }) if text == "allowed"
        )),
        "expected named handler to run before router blacklist"
    );

    world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::SignalObserverRouterComponent>(
            router_id,
        )
        .expect("expected mutable router")
        .blacklist = vec!["named_light".to_string()];

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            root_id,
            EventSignal::DataEvent {
                name: "pulse_on".to_string(),
                payload: None,
            },
        ),
    );

    let intents = rx.drain_ready_intents();
    assert!(
        !intents.iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(crate::engine::ecs::IntentValue::SetText { text, .. }) if text == "allowed"
        )),
        "expected router blacklist to suppress named handler"
    );
}

#[test]
fn mms_click_handler_can_emit_data_event() {
    let src = r##"
        let root = T { name = "router_root" }
        let btn = T { name = "btn" }
        T {
            Text { "(idle)" name = "target" }
        }

        on(root, "DataEvent", "named_light", fn(event) {
            if event == "pulse_on" {
                let t = query("#target")
                if t {
                    t.set_text("emitted")
                }
            }
        })

        on(btn, "Click", fn(event) {
            emit_data(root, "pulse_on")
        })
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let btn_id = world
        .all_components()
        .filter(|&id| world.parent_of(id).is_none())
        .find_map(|root| world.find_component(root, "#btn"))
        .expect("expected #btn");

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            btn_id,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: btn_id,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        ),
    );

    rx.begin_frame();
    for signal in rx.drain_ready_events() {
        rx.dispatch_event_handlers(&mut world, &signal);
    }

    let intents = rx.drain_ready_intents();
    assert!(
        intents.iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(crate::engine::ecs::IntentValue::SetText { text, .. }) if text == "emitted"
        )),
        "expected emit_data() inside MMS handler to produce a follow-up DataEvent"
    );
}

#[test]
fn mms_xr_axis_handler_receives_table_payload() {
    let src = r##"
        let root = T { name = "root" }
        let target = Text { "(idle)" name = "target" }

        on(root, "XrAxisChanged", fn(event) {
            target.set_text("" + event.hand + ":" + event.control + ":" + event.value[0])
        })
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let root_id = world
        .all_components()
        .filter(|&id| world.parent_of(id).is_none())
        .find_map(|root| world.find_component(root, "#root"))
        .expect("expected #root");

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            root_id,
            EventSignal::XrAxisChanged {
                source_component: root_id,
                hand: crate::engine::ecs::component::ControllerHand::Left,
                control: crate::engine::ecs::component::XrAxisControl::LeftStick,
                value: [0.25, -0.5],
            },
        ),
    );

    let intents = rx.drain_ready_intents();
    assert!(
        intents.iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(crate::engine::ecs::IntentValue::SetText { text, .. }) if text == "Left:LeftStick:0.25"
        )),
        "expected XR axis handler payload to reach MMS"
    );
}

#[test]
fn xr_input_gamepad_example_parses() {
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let source = std::fs::read_to_string("examples/input-xr-gamepad.mms").unwrap();

    let out = MeowMeowRunner::eval_with_world_at_path(
        &source,
        Some("examples/input-xr-gamepad.mms"),
        &mut world,
        &mut rx,
        &mut emit,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
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
    let out = eval(
        r#"
        fn f() {
            for i in range(10) {
                T {}
                return null
            }
        }
        f()
    "#,
    );
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
    let Statement::Assignment(AssignmentStatement { name, exported, .. }) = &prog[0] else {
        panic!()
    };
    assert_eq!(name.0, "pi");
    assert!(*exported);
}

#[test]
fn parse_export_fn() {
    let prog = parse("export fn lerp(a, b, t) { return a + (b - a) * t }");
    assert_eq!(prog.len(), 1);
    let Statement::Assignment(AssignmentStatement { name, exported, .. }) = &prog[0] else {
        panic!()
    };
    assert_eq!(name.0, "lerp");
    assert!(*exported);
}

#[test]
fn parse_import_named() {
    let prog = parse(r#"import { pi, lerp } from "math.mms""#);
    assert_eq!(prog.len(), 1);
    let Statement::Import { items, path } = &prog[0] else {
        panic!()
    };
    assert_eq!(path, "math.mms");
    assert_eq!(items.len(), 2);
    assert!(matches!(&items[0], ImportItem::Named(id) if id.0 == "pi"));
    assert!(matches!(&items[1], ImportItem::Named(id) if id.0 == "lerp"));
}

#[test]
fn parse_import_alias() {
    let prog = parse(r#"import { pi as PI, 0 as cube } from "parts.mms""#);
    assert_eq!(prog.len(), 1);
    let Statement::Import { items, .. } = &prog[0] else {
        panic!()
    };
    assert!(
        matches!(&items[0], ImportItem::NamedAlias { name, alias } if name.0 == "pi" && alias.0 == "PI")
    );
    assert!(
        matches!(&items[1], ImportItem::PositionalAlias { index: 0, alias } if alias.0 == "cube")
    );
}

#[test]
fn eval_export_and_import_via_files() {
    // Write a small library file that exports a value and a function.
    let tmp = std::env::temp_dir();
    let lib_path = tmp.join("_mms_test_lib.mms");
    let user_path = tmp.join("_mms_test_user.mms");

    std::fs::write(
        &lib_path,
        r#"
export let count = 3.0
export fn make_row(n) {
    for i in range(n) { T {} }
}
"#,
    )
    .unwrap();

    std::fs::write(
        &user_path,
        "import { count, make_row } from \"_mms_test_lib.mms\"\nmake_row(count)\n",
    )
    .unwrap();

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
    std::fs::write(
        &user_path,
        "import { 0 as my_t } from \"_mms_test_ce_lib.mms\"\nmy_t\n",
    )
    .unwrap();

    let out = MeowMeowRunner::eval_file(user_path.to_str().unwrap());
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);

    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_file(&user_path);
}

#[test]
fn eval_panel_component_factories_from_assets() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let user_path = workspace_root.join("target/_mms_test_panel_factories_user.mms");

    std::fs::write(
        &user_path,
        r#"
import { world_panel } from "../assets/components/panels.mms"
import { inspector_panel } from "../assets/components/panels.mms"

let world_items = ["Root", "Camera", "Light"]
let inspector_items = ["Transform {}", "Style {}"]

world_panel("World", world_items)
inspector_panel("Inspector", inspector_items)
"#,
    )
    .unwrap();

    let out = MeowMeowRunner::eval_file(user_path.to_str().unwrap());
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 2, "intents: {:?}", out.intents);

    let _ = std::fs::remove_file(&user_path);
}

#[test]
fn load_module_file_exposes_named_exports_as_evaluated_values() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let module_path = workspace_root.join("assets/components/panels.mms");

    let module = MeowMeowRunner::load_module_file(module_path.to_str().unwrap())
        .expect("expected module to load");

    assert!(matches!(
        module.named_export("world_panel"),
        Some(Value::Function { .. })
    ));
    assert!(matches!(
        module.named_export("inspector_panel"),
        Some(Value::Function { .. })
    ));
    assert!(matches!(
        module.named_export("paint_panel"),
        Some(Value::Function { .. })
    ));
}

#[test]
fn call_mms_module_fn_invokes_exported_factory_function() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let module_path = workspace_root.join("assets/components/panels.mms");

    let module = MeowMeowRunner::load_module_file(module_path.to_str().unwrap())
        .expect("expected module to load");

    let value = MeowMeowRunner::call_mms_module_fn(
        &module,
        "world_panel",
        vec![
            Value::String("World".to_string()),
            Value::Array(vec![
                Value::String("Root".to_string()),
                Value::String("Camera".to_string()),
            ]),
        ],
        None,
        None,
        None,
    )
    .expect("expected exported factory call to succeed");

    assert!(matches!(value, Value::ComponentExpr(_)));
}

#[test]
fn materialize_mms_module_component_keeps_factory_return_as_component_expr_in_live_mode() {
    let module = MeowMeowRunner::load_module_source(
        r#"
export fn example() {
    let root = T {}
    return root
}
"#,
        None,
    )
    .expect("load inline module");

    let mut world = World::default();
    let mut emit = CommandQueue::new();
    let value = MeowMeowRunner::materialize_mms_module_component(
        &module,
        "example",
        vec![],
        Some(&mut world),
        Some(&mut emit),
    )
    .expect("materialize live module component");

    assert_eq!(value.component_type, "T");
    assert!(world.all_components().next().is_none());
}

#[test]
fn omitted_function_args_bind_to_null() {
    let tmp = std::env::temp_dir();
    let lib_path = tmp.join("_mms_test_optional_args_lib.mms");
    let user_path = tmp.join("_mms_test_optional_args_user.mms");

    std::fs::write(
        &lib_path,
        r#"
export fn maybe_label(label, options) {
    if options == null {
        return label + ":none"
    }
    return label + ":some"
}
"#,
    )
    .unwrap();

    std::fs::write(
        &user_path,
        r#"
import { maybe_label } from "_mms_test_optional_args_lib.mms"
let result = maybe_label("ok")
if result != "ok:none" {
    print("unexpected: " + result)
}
"#,
    )
    .unwrap();

    let out = MeowMeowRunner::eval_file(user_path.to_str().unwrap());
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let _ = std::fs::remove_file(&lib_path);
    let _ = std::fs::remove_file(&user_path);
}

#[test]
fn renderable_constructors_accept_omitted_default_args() {
    let module = MeowMeowRunner::load_module_source(
        r#"
export fn procedural_defaults() {
    return T {
        R.cone() {}
        R.icosahedron() {}
        R.heart() {}
        R.star() {}
        R.partial_annulus_2d() {}
    }
}
"#,
        None,
    )
    .expect("load inline module");

    let mut world = World::default();
    let mut render_assets = RenderAssets::new();
    let mut emit = CommandQueue::new();
    let root = MeowMeowRunner::spawn_mms_module_component_uninitialized_with_assets(
        &module,
        "procedural_defaults",
        vec![],
        &mut world,
        Some(&mut render_assets),
        &mut emit,
    )
    .expect("spawn procedural defaults");

    assert!(world.get_component_record(root).is_some());
    assert_eq!(world.children_of(root).len(), 5);
}

#[test]
fn renderable_cone_accepts_explicit_detail_arg() {
    let out = eval("T { R.cone(12) {} }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}

#[test]
fn renderable_icosahedron_accepts_explicit_args() {
    let out = eval("T { R.icosahedron(2, 1.0) {} }");
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 1);
}

#[test]
fn spawn_mms_module_component_initialises_live_root() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "mms_spawn_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&tmp_dir).expect("create temp dir");
    let path = tmp_dir.join("test_asset.mms");
    std::fs::write(
        &path,
        r#"
            export fn example() {
                let root = T {}
                return root
            }
        "#,
    )
    .expect("write asset file");

    let module = MeowMeowRunner::load_module_file(path.to_str().unwrap()).expect("load module");
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let root_id = MeowMeowRunner::spawn_mms_module_component(
        &module,
        "example",
        vec![],
        None,
        &mut world,
        &mut emit,
    )
    .expect("spawn module component");

    assert!(world.component_name(root_id).is_some());
    assert!(world.is_initialized(root_id));
}

#[test]
fn spawn_mms_module_component_uninitialized_captures_live_component_objects_in_keyframes() {
    let module = MeowMeowRunner::load_module_source(
        r#"
export fn animated_preview() {
    let glow = Emissive.off() {
        name = "glow"
    }

    return T {
        glow
        Animation.looping().length(2.0) {
            Keyframe.at(0.0) {
                glow.set_intensity(2.5)
            }
        }
    }
}
"#,
        None,
    )
    .expect("load inline module");

    let mut world = World::default();
    let mut emit = CommandQueue::new();
    let root_id = MeowMeowRunner::spawn_mms_module_component_uninitialized(
        &module,
        "animated_preview",
        vec![],
        &mut world,
        &mut emit,
    )
    .expect("spawn live preview root");

    assert!(world.get_component_record(root_id).is_some());
    assert!(!world.is_initialized(root_id));

    let keyframe_id = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::KeyframeComponent>(id)
                .is_some()
        })
        .expect("keyframe exists");
    let keyframe = world
        .get_component_by_id_as::<crate::engine::ecs::component::KeyframeComponent>(keyframe_id)
        .expect("keyframe component exists");
    let callback = keyframe
        .callback
        .as_ref()
        .expect("keyframe callback exists");
    let captured = callback
        .captured_env
        .get("glow")
        .expect("captured glow binding exists");

    match captured {
        Value::ComponentObject { id, component_type } => {
            assert_eq!(component_type, "Emissive");
            assert!(world.get_component_record(*id).is_some());
        }
        other => panic!("expected live ComponentObject capture, got {other:?}"),
    }
}

#[test]
fn eval_world_panel_content_rows_are_queryable_by_index_name() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source_path = workspace_root.join("target/_mms_test_world_panel_content_names.mms");
    let source = r##"
import { world_panel_content } from "../assets/components/panel_items.mms"

let root = world_panel_content(["Root", "Camera", "Light"])
let rows_mount = root.query("#rows_mount")
let row_0 = root.query("#item_0")
let row_1 = root.query("#item_1")
let row_2 = root.query("#item_2")

assert(rows_mount, "expected rows_mount to exist")
assert(row_0, "expected item_0 row to exist")
assert(row_1, "expected item_1 row to exist")
assert(row_2, "expected item_2 row to exist")
"##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world_at_path(
        source,
        Some(source_path.to_str().unwrap()),
        &mut world,
        &mut rx,
        &mut emit,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
}

// ---------------------------------------------------------------------------
// Reassignment
// ---------------------------------------------------------------------------

#[test]
fn parse_reassign() {
    let prog = parse("let x = 1\nx = 2");
    assert_eq!(prog.len(), 2);
    assert!(matches!(&prog[0], Statement::Assignment(_)));
    let Statement::Reassign { target, .. } = &prog[1] else {
        panic!("expected Reassign")
    };
    assert!(matches!(target, Expression::Identifier(name) if name.0 == "x"));
}

#[test]
fn parse_table_field_reassign() {
    let prog = parse("app_state.text = \"sent\"");
    let Statement::Reassign { target, value } = &prog[0] else {
        panic!("expected Reassign");
    };
    assert!(matches!(value, Expression::String(s) if s == "sent"));
    let Expression::BinaryOp { op, lhs, rhs } = target else {
        panic!("expected dot target");
    };
    assert!(matches!(op, crate::scripting::ast::BinOpKind::Dot));
    assert!(matches!(lhs.as_ref(), Expression::Identifier(name) if name.0 == "app_state"));
    assert!(matches!(rhs.as_ref(), Expression::Identifier(name) if name.0 == "text"));
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
fn eval_table_field_reassign_basic() {
    let module = MeowMeowRunner::load_module_source(
        r#"
export let result = {
    text = "before"
    count = 0
}
result.text = "after"
result.count = result.count + 1
"#,
        None,
    )
    .expect("module eval");
    let Value::Object(result) = module
        .named_exports
        .get("result")
        .cloned()
        .expect("result export")
    else {
        panic!("expected object-backed table");
    };
    let Some(()) = result.with_map(|result| {
        assert!(matches!(result.get("text"), Some(Value::String(text)) if text == "after"));
        assert!(
            matches!(result.get("count"), Some(Value::Number(count)) if (*count - 1.0).abs() < 1e-6)
        );
    }) else {
        panic!("expected live table object");
    };
}

#[test]
fn eval_table_field_read_inside_function() {
    let module = MeowMeowRunner::load_module_source(
        r#"
fn pick_text(state) {
    return state.text
}

export let app_state = {
    text = "hello table fields"
    count = 1
}

export let result = pick_text(app_state)
"#,
        None,
    )
    .expect("module eval");
    assert!(matches!(
        module.named_exports.get("result"),
        Some(Value::String(text)) if text == "hello table fields"
    ));
}

#[test]
fn exported_functions_share_object_backed_table_state() {
    let module = MeowMeowRunner::load_module_source(
        r#"
export let app_state = {
    text = "before"
    count = 0
}

export fn write_state() {
    app_state.text = "after"
    app_state.count = app_state.count + 1
}

export fn read_state() {
    return app_state.text
}

export fn read_count() {
    return app_state.count
}
"#,
        None,
    )
    .expect("module eval");

    MeowMeowRunner::call_mms_module_fn(&module, "write_state", vec![], None, None, None)
        .expect("write_state");

    let text = MeowMeowRunner::call_mms_module_fn(&module, "read_state", vec![], None, None, None)
        .expect("read_state");
    assert!(matches!(text, Value::String(text) if text == "after"));

    let count = MeowMeowRunner::call_mms_module_fn(&module, "read_count", vec![], None, None, None)
        .expect("read_count");
    assert!(matches!(count, Value::Number(count) if (count - 1.0).abs() < 1e-6));
}

#[test]
fn eval_reassign_undefined_errors() {
    let out = MeowMeowRunner::eval("x = 5");
    assert!(
        !out.errors.is_empty(),
        "expected an error for undefined reassignment"
    );
    assert!(
        out.errors[0].contains("not defined"),
        "got: {}",
        out.errors[0]
    );
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
    let engine::ecs::IntentValue::SpawnComponentTree { root, .. } = &out.intents[0] else {
        panic!()
    };
    assert_eq!(
        root.ctor_method.as_deref(),
        Some("position"),
        "expected position ctor"
    );
    let Value::Number(y_val) = &root.ctor_args[1] else {
        panic!("expected number arg at index 1")
    };
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

#[test]
fn eval_for_accumulator_propagates_after_loop_exit() {
    // After the frame-stack refactor, reassignment to an outer-declared variable
    // inside a loop body should walk up to the declaring frame — so `sum` is 6
    // *after* the loop, not 0. Observable here via a conditional emit.
    //
    // Pre-refactor: `loop_env = env.clone()` sandboxes the loop; sum stays 0;
    // the `if sum == 6` branch never fires; intents.len() == 0.
    let out = eval(
        r#"
        let sum = 0
        for i in [1, 2, 3] {
            sum = sum + i
        }
        if sum == 6 { T {} }
    "#,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(
        out.intents.len(),
        1,
        "expected sum to propagate out of the loop and equal 6"
    );
}

// ---------------------------------------------------------------------------
// While loop
// ---------------------------------------------------------------------------

#[test]
fn parse_while_loop() {
    let prog = parse("while true { T {} }");
    assert_eq!(prog.len(), 1);
    let Statement::While { condition, body } = &prog[0] else {
        panic!("expected While")
    };
    assert!(matches!(condition, Expression::Bool(true)));
    assert_eq!(body.statements.len(), 1);
}

#[test]
fn eval_while_counts_up_to_limit() {
    // Emit one T per iteration; stop when i reaches 4.
    let out = eval(
        r#"
        let i = 0
        while i < 4 {
            T {}
            i = i + 1
        }
    "#,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 4);
}

#[test]
fn eval_while_break_exits_early() {
    let out = eval(
        r#"
        let i = 0
        while true {
            if i == 3 { break }
            T {}
            i = i + 1
        }
    "#,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(out.intents.len(), 3);
}

#[test]
fn eval_while_continue_skips_body_tail() {
    // Only emit T when i is even; continue skips the emit on odd iterations.
    // i goes 0..5 → 0,2,4 emit → 3 intents
    let out = eval(
        r#"
        let i = 0
        while i < 5 {
            i = i + 1
            if i == 2 { continue }
            if i == 4 { continue }
            T {}
        }
    "#,
    );
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
fn body_else_if_picks_first_matching_branch() {
    let out =
        eval("T { if false { R.cube() {} } else if true { R.sphere() {} } else { R.cone() {} } }");
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

// ---------------------------------------------------------------------------
// to_mms_ast round-trip tests
//
// Build a live component → emit MMS via `to_mms_ast` → unparse → parse →
// materialize → `spawn_tree_uninitialized` → downcast and assert fields.
// This is the path scene save/load and `attach_clone` actually follow.
// ---------------------------------------------------------------------------

use crate::engine::ecs::component::Component as ComponentTrait;

fn roundtrip_component<C: ComponentTrait + 'static>(original: C) -> (World, ComponentId) {
    // Round-trip helper assumes the component encodes losslessly without
    // needing live world context (no ComponentId refs). Use an empty world
    // as a stand-in.
    let world_stub = World::default();
    let ce_ast = original.to_mms_ast(&world_stub);
    let text = crate::scripting::unparser::unparse_component(&ce_ast);
    let prog = parse(&text);
    assert_eq!(prog.len(), 1, "unparsed `{text}` did not produce one stmt");
    let parsed_ce = as_component!(prog.into_iter().next().unwrap());
    let mat = crate::scripting::component_registry::ce_ast_to_materialized(&parsed_ce)
        .expect("materialize");
    let mut world = World::default();
    let mut emit = CommandQueue::new();
    let id =
        crate::scripting::component_registry::spawn_tree_uninitialized(&mat, &mut world, &mut emit)
            .expect("spawn");
    (world, id)
}

#[test]
fn roundtrip_pose_capture_pose_preserves_all_ordered_joints() {
    use crate::engine::ecs::component::{PoseBoneEntry, PoseCapturePoseComponent, PoseTargetRef};

    let entries: Vec<_> = (0..12)
        .map(|i| PoseBoneEntry {
            query: format!("#joint_{i}"),
            translation: [i as f32, i as f32 + 0.25, -(i as f32)],
            rotation: [0.01 * i as f32, 0.02 * i as f32, 0.03 * i as f32, 1.0],
            scale: [1.0, 1.0 + 0.01 * i as f32, 1.0],
        })
        .collect();
    let original = PoseCapturePoseComponent::new(
        "many joints",
        PoseTargetRef::Query("#avatar".into()),
        entries.clone(),
    );

    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<PoseCapturePoseComponent>(id)
        .expect("PoseCapturePose downcast");
    assert_eq!(got.name, "many joints");
    assert_eq!(got.entries.len(), entries.len());
    for (got, expected) in got.entries.iter().zip(&entries) {
        assert_eq!(got.query, expected.query);
        assert_eq!(got.translation, expected.translation);
        assert_eq!(got.rotation, expected.rotation);
        assert_eq!(got.scale, expected.scale);
    }
}

#[test]
fn roundtrip_pose_capture_preserves_asset_name() {
    use crate::engine::ecs::component::PoseCaptureComponent;

    let original = PoseCaptureComponent::new()
        .with_label("Avatar")
        .with_asset_name("bisket_v2");
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<PoseCaptureComponent>(id)
        .expect("PoseCapture downcast");
    assert_eq!(got.label.as_deref(), Some("Avatar"));
    assert_eq!(got.asset_name.as_deref(), Some("bisket_v2"));
}

#[test]
fn pose_capture_rejects_invalid_asset_name_from_mms() {
    let prog = parse(r#"PoseCapture { asset_name("../escape") }"#);
    let parsed_ce = as_component!(prog.into_iter().next().unwrap());
    let mat = crate::scripting::component_registry::ce_ast_to_materialized(&parsed_ce)
        .expect("materialize");
    let mut world = World::default();
    let mut emit = CommandQueue::new();
    let error =
        crate::scripting::component_registry::spawn_tree_uninitialized(&mat, &mut world, &mut emit)
            .unwrap_err();
    assert!(error.contains("asset_name"), "{error}");
}

#[test]
fn pose_capture_pose_joint_appends_and_rejects_duplicates() {
    use crate::engine::ecs::component::{PoseBoneEntry, PoseCapturePoseComponent, PoseTargetRef};
    let entry = |query: &str| PoseBoneEntry {
        query: query.into(),
        translation: [0.0; 3],
        rotation: [0.0, 0.0, 0.0, 1.0],
        scale: [1.0; 3],
    };
    let mut pose = PoseCapturePoseComponent::new(
        "partial",
        PoseTargetRef::Query("#avatar".into()),
        Vec::new(),
    );
    pose.push_joint(entry("#hips")).unwrap();
    pose.push_joint(entry("#head")).unwrap();
    assert_eq!(pose.entries.len(), 2);
    assert_eq!(pose.entries[0].query, "#hips");
    assert_eq!(pose.entries[1].query, "#head");
    assert!(pose.push_joint(entry("#hips")).is_err());
    assert_eq!(pose.entries.len(), 2);
}

#[test]
fn roundtrip_opacity() {
    use crate::engine::ecs::component::OpacityComponent;
    let original = OpacityComponent::new()
        .with_opacity(0.42)
        .with_multiple_layers();
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<OpacityComponent>(id)
        .expect("Opacity downcast");
    assert!(
        (got.opacity - 0.42).abs() < 1e-6,
        "opacity: {}",
        got.opacity
    );
    assert!(got.multiple_layers);
}

#[test]
fn roundtrip_opacity_default_multiple_layers_omitted() {
    use crate::engine::ecs::component::OpacityComponent;
    // multiple_layers=false should not emit the toggle call.
    let original = OpacityComponent::new().with_opacity(0.75);
    let text = crate::scripting::unparser::unparse_component(&ComponentTrait::to_mms_ast(
        &original,
        &World::default(),
    ));
    assert!(
        !text.contains("multiple_layers"),
        "expected `multiple_layers` not emitted when false: {text}"
    );
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<OpacityComponent>(id)
        .unwrap();
    assert!((got.opacity - 0.75).abs() < 1e-6);
    assert!(!got.multiple_layers);
}

#[test]
fn roundtrip_emissive_on() {
    use crate::engine::ecs::component::EmissiveComponent;
    let (world, id) = roundtrip_component(EmissiveComponent::on());
    let got = world
        .get_component_by_id_as::<EmissiveComponent>(id)
        .unwrap();
    assert_eq!(got.intensity, 1.0);
}

#[test]
fn roundtrip_emissive_off() {
    use crate::engine::ecs::component::EmissiveComponent;
    let (world, id) = roundtrip_component(EmissiveComponent::off());
    let got = world
        .get_component_by_id_as::<EmissiveComponent>(id)
        .unwrap();
    assert_eq!(got.intensity, 0.0);
}

#[test]
fn roundtrip_emissive_custom_intensity() {
    use crate::engine::ecs::component::EmissiveComponent;
    let (world, id) = roundtrip_component(EmissiveComponent::new(2.5));
    let got = world
        .get_component_by_id_as::<EmissiveComponent>(id)
        .unwrap();
    assert!(
        (got.intensity - 2.5).abs() < 1e-6,
        "intensity: {}",
        got.intensity
    );
}

#[test]
fn roundtrip_ambient_light() {
    use crate::engine::ecs::component::AmbientLightComponent;
    let (world, id) = roundtrip_component(AmbientLightComponent::rgb(0.1, 0.5, 0.9));
    let got = world
        .get_component_by_id_as::<AmbientLightComponent>(id)
        .unwrap();
    assert!((got.rgb[0] - 0.1).abs() < 1e-6);
    assert!((got.rgb[1] - 0.5).abs() < 1e-6);
    assert!((got.rgb[2] - 0.9).abs() < 1e-6);
}

#[test]
fn roundtrip_directional_light() {
    use crate::engine::ecs::component::DirectionalLightComponent;
    let original = DirectionalLightComponent::new()
        .with_intensity(2.0)
        .with_color(0.5, 0.6, 0.7);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<DirectionalLightComponent>(id)
        .unwrap();
    assert!((got.intensity - 2.0).abs() < 1e-6);
    assert!((got.color[0] - 0.5).abs() < 1e-6);
    assert!((got.color[1] - 0.6).abs() < 1e-6);
    assert!((got.color[2] - 0.7).abs() < 1e-6);
}

#[test]
fn roundtrip_point_light() {
    use crate::engine::ecs::component::PointLightComponent;
    let original = PointLightComponent::new()
        .with_intensity(3.0)
        .with_distance(15.0)
        .with_color(0.25, 0.5, 0.75);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<PointLightComponent>(id)
        .unwrap();
    assert!((got.intensity - 3.0).abs() < 1e-6);
    assert!((got.distance - 15.0).abs() < 1e-6);
    assert!((got.color[0] - 0.25).abs() < 1e-6);
    assert!((got.color[1] - 0.5).abs() < 1e-6);
    assert!((got.color[2] - 0.75).abs() < 1e-6);
}

#[test]
fn roundtrip_spot_light() {
    use crate::engine::ecs::component::SpotLightComponent;
    let original = SpotLightComponent::new()
        .with_intensity(4.0)
        .with_distance(12.0)
        .with_angle(0.6)
        .with_penumbra(0.3)
        .with_color(0.25, 0.5, 0.75);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<SpotLightComponent>(id)
        .unwrap();
    assert!((got.intensity - 4.0).abs() < 1e-6);
    assert!((got.distance - 12.0).abs() < 1e-6);
    assert!((got.angle - 0.6).abs() < 1e-6);
    assert!((got.penumbra - 0.3).abs() < 1e-6);
    assert_eq!(got.color, [0.25, 0.5, 0.75]);
}

#[test]
fn roundtrip_gltf() {
    use crate::engine::ecs::component::GLTFComponent;
    let original = GLTFComponent::new("models/cat.glb").with_visualized_transforms(true);
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<GLTFComponent>(id).unwrap();
    assert_eq!(got.uri, "models/cat.glb");
    assert!(got.with_visualized_transforms);
}

#[test]
fn roundtrip_gltf_no_visualized_transforms_omits_call() {
    use crate::engine::ecs::component::GLTFComponent;
    let original = GLTFComponent::new("models/cat.glb");
    let text = crate::scripting::unparser::unparse_component(&ComponentTrait::to_mms_ast(
        &original,
        &World::default(),
    ));
    assert!(
        !text.contains("with_visualized_transforms"),
        "expected `with_visualized_transforms` omitted when false: {text}"
    );
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<GLTFComponent>(id).unwrap();
    assert_eq!(got.uri, "models/cat.glb");
    assert!(!got.with_visualized_transforms);
}

#[test]
fn roundtrip_texture_with_uri() {
    use crate::engine::ecs::component::texture::TextureSource;
    use crate::engine::ecs::component::{CatEngineTextureFormat, TextureComponent};
    let (world, id) = roundtrip_component(TextureComponent::with_uri("textures/cat.png"));
    let got = world
        .get_component_by_id_as::<TextureComponent>(id)
        .unwrap();
    match &got.source {
        TextureSource::Uri(u) => assert_eq!(u, "textures/cat.png"),
        _ => panic!("expected URI source"),
    }
    assert_eq!(got.format, CatEngineTextureFormat::Rgba8);
    assert!(got.render_image.is_none());
}

#[test]
fn roundtrip_texture_from_dds() {
    use crate::engine::ecs::component::{CatEngineTextureFormat, TextureComponent};
    let (world, id) = roundtrip_component(TextureComponent::from_dds("textures/cat.dds"));
    let got = world
        .get_component_by_id_as::<TextureComponent>(id)
        .unwrap();
    assert_eq!(got.format, CatEngineTextureFormat::DdsBc7);
}

#[test]
fn roundtrip_texture_render_image() {
    use crate::engine::ecs::component::TextureComponent;
    let (world, id) = roundtrip_component(TextureComponent::render_image("#main"));
    let got = world
        .get_component_by_id_as::<TextureComponent>(id)
        .unwrap();
    assert_eq!(got.render_image.as_deref(), Some("#main"));
}

#[test]
fn roundtrip_camera_3d() {
    use crate::engine::ecs::component::Camera3DComponent;
    use crate::engine::graphics::CameraTarget;
    let original = Camera3DComponent::new()
        .with_fov(75.0)
        .with_near(0.5)
        .with_far(200.0);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<Camera3DComponent>(id)
        .unwrap();
    assert!((got.fov_y_degrees - 75.0).abs() < 1e-6);
    assert!((got.z_near - 0.5).abs() < 1e-6);
    assert!((got.z_far - 200.0).abs() < 1e-6);
    assert!(matches!(got.target, CameraTarget::Window));
}

#[test]
fn roundtrip_camera_2d() {
    use crate::engine::ecs::component::Camera2DComponent;
    use crate::engine::graphics::CameraTarget;
    let mut original = Camera2DComponent::new();
    original.target = CameraTarget::Xr;
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<Camera2DComponent>(id)
        .unwrap();
    assert!(matches!(got.target, CameraTarget::Xr));
}

#[test]
fn roundtrip_camera_xr_off() {
    use crate::engine::ecs::component::CameraXRComponent;
    let (world, id) = roundtrip_component(CameraXRComponent::off());
    let got = world
        .get_component_by_id_as::<CameraXRComponent>(id)
        .unwrap();
    assert!(!got.enabled);
}

#[test]
fn roundtrip_xr_off() {
    use crate::engine::ecs::component::XrComponent;
    let (world, id) = roundtrip_component(XrComponent::off());
    let got = world.get_component_by_id_as::<XrComponent>(id).unwrap();
    assert!(!got.enabled);
}

#[test]
fn roundtrip_xr_hand() {
    use crate::engine::ecs::component::{ControllerHand, ControllerPoseKind, XRHandComponent};
    let original = XRHandComponent::new(true, ControllerHand::Right, ControllerPoseKind::Grip);
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<XRHandComponent>(id).unwrap();
    assert!(got.enabled);
    assert_eq!(got.hand, ControllerHand::Right);
    assert_eq!(got.pose, ControllerPoseKind::Grip);
}

#[test]
fn roundtrip_input_xr_off() {
    use crate::engine::ecs::component::InputXRComponent;
    let (world, id) = roundtrip_component(InputXRComponent::off());
    let got = world
        .get_component_by_id_as::<InputXRComponent>(id)
        .unwrap();
    assert!(!got.enabled);
}

#[test]
fn roundtrip_input_xr_gamepad() {
    use crate::engine::ecs::component::{InputXRGamepadComponent, XrHandPreference};
    let original = InputXRGamepadComponent::new()
        .hand(XrHandPreference::Either)
        .locomotion()
        .speed(2.25)
        .deadzone(0.15);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<InputXRGamepadComponent>(id)
        .unwrap();
    assert!(got.enabled);
    assert_eq!(got.hand, XrHandPreference::Either);
    assert!(got.locomotion);
    assert!((got.speed - 2.25).abs() < 1e-6);
    assert!((got.deadzone - 0.15).abs() < 1e-6);
}

#[test]
fn roundtrip_animation_paused() {
    use crate::engine::ecs::component::{AnimationComponent, AnimationState};
    let (world, id) =
        roundtrip_component(AnimationComponent::new().with_state(AnimationState::Paused));
    let got = world
        .get_component_by_id_as::<AnimationComponent>(id)
        .unwrap();
    assert_eq!(got.state, AnimationState::Paused);
}

#[test]
fn roundtrip_animation_resolve_targets_on_play() {
    use crate::engine::ecs::component::{AnimationComponent, ResolveTargetsMode};
    let (world, id) = roundtrip_component(
        AnimationComponent::new().with_resolve_targets(ResolveTargetsMode::OnPlay),
    );
    let got = world
        .get_component_by_id_as::<AnimationComponent>(id)
        .unwrap();
    assert_eq!(got.resolve_targets, ResolveTargetsMode::OnPlay);
}

// ---------------------------------------------------------------------------
// Action round-trip tests
//
// These exercise the subtree dump path (`subtree_to_ce_ast`) because Action
// references other components by ComponentId — the dump needs the surrounding
// world to derive `@uuid:` strings and the guid-preservation pre-pass needs
// the full subtree to know which targets are referenced.
// ---------------------------------------------------------------------------

/// Build a live source world, dump the subtree rooted at `root` via
/// `subtree_to_ce_ast`, unparse → parse → spawn into a fresh world, and
/// return the fresh world plus the respawned root id.
fn roundtrip_subtree(source_world: &World, root: ComponentId) -> (World, ComponentId) {
    let ce_ast = crate::scripting::component_registry::subtree_to_ce_ast(source_world, root)
        .expect("subtree_to_ce_ast");
    let text = crate::scripting::unparser::unparse_component(&ce_ast);
    let prog = parse(&text);
    assert_eq!(prog.len(), 1, "unparsed `{text}` did not produce one stmt");
    let parsed_ce = as_component!(prog.into_iter().next().unwrap());
    let mat = crate::scripting::component_registry::ce_ast_to_materialized(&parsed_ce)
        .expect("materialize");
    let mut world = World::default();
    let mut emit = CommandQueue::new();
    let id =
        crate::scripting::component_registry::spawn_tree_uninitialized(&mat, &mut world, &mut emit)
            .expect("spawn");
    (world, id)
}

#[test]
fn roundtrip_action_query_selector_preserved_verbatim() {
    use crate::engine::ecs::IntentValue;
    use crate::engine::ecs::component::{
        ActionComponent, ComponentRef, KeyframeComponent, TransformComponent,
    };
    use slotmap::Key;

    let mut w = World::default();
    let root = w.add_component(TransformComponent::new());
    let target = w.add_component_boxed_named("hero", Box::new(TransformComponent::new()));
    w.add_child(root, target).unwrap();
    let kf = w.add_component(KeyframeComponent::new(0.0));
    w.add_child(root, kf).unwrap();
    let signal = IntentValue::SetColor {
        component_ids: vec![ComponentId::null()],
        rgba: [1.0, 0.0, 0.0, 1.0],
    };
    let action = w.add_component(ActionComponent::new_authored(
        signal,
        vec![ComponentRef::Query("#hero".to_string())],
    ));
    w.add_child(kf, action).unwrap();

    let (new_world, new_root) = roundtrip_subtree(&w, root);
    // Find the respawned Action by walking.
    let new_action = find_first::<ActionComponent>(&new_world, new_root).expect("action exists");
    let comp = new_world
        .get_component_by_id_as::<ActionComponent>(new_action)
        .unwrap();
    assert_eq!(comp.target_sources.len(), 1);
    match &comp.target_sources[0] {
        ComponentRef::Query(s) => assert_eq!(s, "#hero"),
        other => panic!("expected Query selector, got {other:?}"),
    }
}

#[test]
fn roundtrip_action_handle_becomes_guid_and_target_keeps_guid() {
    use crate::engine::ecs::IntentValue;
    use crate::engine::ecs::component::{
        ActionComponent, ComponentRef, KeyframeComponent, TransformComponent,
    };
    use slotmap::Key;

    let mut w = World::default();
    let root = w.add_component(TransformComponent::new());
    // Unnamed target; only the GUID identifies it.
    let target = w.add_component(TransformComponent::new());
    w.add_child(root, target).unwrap();
    let target_guid = w.get_component_record(target).unwrap().guid;

    let kf = w.add_component(KeyframeComponent::new(0.0));
    w.add_child(root, kf).unwrap();

    let signal = IntentValue::SetPosition {
        component_ids: vec![ComponentId::null()],
        position: [1.0, 2.0, 3.0],
    };
    let action = w.add_component(ActionComponent::new_authored(
        signal,
        vec![ComponentRef::Guid(target_guid)],
    ));
    w.add_child(kf, action).unwrap();

    let (new_world, new_root) = roundtrip_subtree(&w, root);

    // The action's target_sources should preserve the same guid.
    let new_action = find_first::<ActionComponent>(&new_world, new_root).unwrap();
    let comp = new_world
        .get_component_by_id_as::<ActionComponent>(new_action)
        .unwrap();
    match &comp.target_sources[0] {
        ComponentRef::Guid(u) => assert_eq!(*u, target_guid),
        other => panic!("expected Guid, got {other:?}"),
    }

    // The target component should have its guid restored on the new
    // world's guid_index — otherwise OnAttach/OnPlay resolution would
    // fail to find it.
    assert!(
        new_world.component_id_by_guid(target_guid).is_some(),
        "target guid not restored across round-trip"
    );
}

#[test]
fn roundtrip_action_named_and_guid_referenced_target_emits_both() {
    use crate::engine::ecs::IntentValue;
    use crate::engine::ecs::component::{
        ActionComponent, ComponentRef, KeyframeComponent, TransformComponent,
    };
    use slotmap::Key;

    let mut w = World::default();
    let root = w.add_component(TransformComponent::new());
    // Target has BOTH a name and gets referenced by guid (author wrote
    // `let hero = T { name = "hero" }; Action.set_color(hero, ...)`).
    let target = w.add_component_boxed_named("hero", Box::new(TransformComponent::new()));
    w.add_child(root, target).unwrap();
    let target_guid = w.get_component_record(target).unwrap().guid;

    let kf = w.add_component(KeyframeComponent::new(0.0));
    w.add_child(root, kf).unwrap();
    let signal = IntentValue::SetColor {
        component_ids: vec![ComponentId::null()],
        rgba: [0.0, 1.0, 0.0, 1.0],
    };
    let action = w.add_component(ActionComponent::new_authored(
        signal,
        vec![ComponentRef::Guid(target_guid)],
    ));
    w.add_child(kf, action).unwrap();

    // Inspect the dump text directly: both `name = "hero"` and
    // `guid = "<uuid>"` should be present on the target's CE.
    let ce = crate::scripting::component_registry::subtree_to_ce_ast(&w, root)
        .expect("subtree_to_ce_ast");
    let text = crate::scripting::unparser::unparse_component(&ce);
    assert!(
        text.contains("name = \"hero\""),
        "expected name emit: {text}"
    );
    assert!(
        text.contains(&format!("guid = \"{target_guid}\"")),
        "expected guid emit: {text}"
    );

    // And both round-trip live.
    let (new_world, new_root) = roundtrip_subtree(&w, root);
    let new_target = new_world
        .component_id_by_guid(target_guid)
        .expect("target guid restored");
    let new_target_node = new_world.get_component_record(new_target).unwrap();
    assert_eq!(new_target_node.name, "hero");
    assert_eq!(new_target_node.guid, target_guid);

    // The action's target_sources still resolves to the same guid.
    let new_action = find_first::<ActionComponent>(&new_world, new_root).unwrap();
    let comp = new_world
        .get_component_by_id_as::<ActionComponent>(new_action)
        .unwrap();
    match &comp.target_sources[0] {
        ComponentRef::Guid(u) => assert_eq!(*u, target_guid),
        other => panic!("expected Guid, got {other:?}"),
    }
}

#[test]
fn roundtrip_action_unreferenced_component_does_not_get_guid_emit() {
    use crate::engine::ecs::IntentValue;
    use crate::engine::ecs::component::{ActionComponent, TransformComponent};

    let mut w = World::default();
    let root = w.add_component(TransformComponent::new());
    let bystander = w.add_component(TransformComponent::new());
    w.add_child(root, bystander).unwrap();
    // No Action references `bystander`.
    let action = w.add_component(ActionComponent::new(IntentValue::Print {
        message: "hi".into(),
    }));
    w.add_child(root, action).unwrap();

    let ce = crate::scripting::component_registry::subtree_to_ce_ast(&w, root)
        .expect("subtree_to_ce_ast");
    let text = crate::scripting::unparser::unparse_component(&ce);
    let bystander_guid = w.get_component_record(bystander).unwrap().guid;
    assert!(
        !text.contains(&format!("guid = \"{bystander_guid}\"")),
        "unreferenced component should not get guid emit: {text}"
    );
}

// Temporarily gated: see docs/bugs/ik-solver-api-drift-breaks-tests.md.
#[cfg(any())]
#[test]
fn roundtrip_ikchain_target_and_end_effector_via_selectors() {
    use crate::engine::ecs::component::{
        ComponentRef, IKChainComponent, IKSolver, TransformComponent,
    };

    let mut w = World::default();
    let root = w.add_component(TransformComponent::new());
    let target = w.add_component_boxed_named("hand_target", Box::new(TransformComponent::new()));
    w.add_child(root, target).unwrap();
    let ee = w.add_component_boxed_named("end_effector", Box::new(TransformComponent::new()));
    w.add_child(root, ee).unwrap();
    let mut ik = IKChainComponent::new(
        IKSolver::TwoBoneIK {
            pole_direction: [0.0, 1.0, 0.0],
            copy_end_rotation: false,
        },
        target,
        ee,
    );
    ik = ik
        .with_target_source(ComponentRef::Query("#hand_target".to_string()))
        .with_end_effector_source(ComponentRef::Query("#end_effector".to_string()));
    let ik_id = w.add_component(ik);
    w.add_child(root, ik_id).unwrap();

    let (new_world, new_root) = roundtrip_subtree(&w, root);
    let new_ik_id = find_first::<IKChainComponent>(&new_world, new_root).unwrap();
    let new_ik = new_world
        .get_component_by_id_as::<IKChainComponent>(new_ik_id)
        .unwrap();
    match &new_ik.target_source {
        Some(ComponentRef::Query(s)) => assert_eq!(s, "#hand_target"),
        other => panic!("expected Query target_source, got {other:?}"),
    }
    match &new_ik.end_effector_source {
        Some(ComponentRef::Query(s)) => assert_eq!(s, "#end_effector"),
        other => panic!("expected Query end_effector_source, got {other:?}"),
    }
    // Registry should have resolved them too since the named targets
    // exist in the same subtree.
    assert_ne!(
        new_ik.target_id, ee,
        "target_id should not have been mis-resolved"
    );
    assert!(
        new_world.get_component_record(new_ik.target_id).is_some(),
        "target_id should resolve to a live component"
    );
    assert!(
        new_world
            .get_component_record(new_ik.end_effector_id)
            .is_some(),
        "end_effector_id should resolve to a live component"
    );
}

// Temporarily gated: see docs/bugs/ik-solver-api-drift-breaks-tests.md.
#[cfg(any())]
#[test]
fn roundtrip_ikchain_guid_handle_preserves_target_guid() {
    use crate::engine::ecs::component::{
        ComponentRef, IKChainComponent, IKSolver, TransformComponent,
    };

    let mut w = World::default();
    let root = w.add_component(TransformComponent::new());
    let target = w.add_component(TransformComponent::new()); // unnamed
    w.add_child(root, target).unwrap();
    let ee = w.add_component(TransformComponent::new());
    w.add_child(root, ee).unwrap();
    let target_guid = w.get_component_record(target).unwrap().guid;
    let ee_guid = w.get_component_record(ee).unwrap().guid;

    let ik = IKChainComponent::new(IKSolver::AimConstraint { offset_yaw: 0.0 }, target, ee)
        .with_target_source(ComponentRef::Guid(target_guid))
        .with_end_effector_source(ComponentRef::Guid(ee_guid));
    let ik_id = w.add_component(ik);
    w.add_child(root, ik_id).unwrap();

    let (new_world, _new_root) = roundtrip_subtree(&w, root);
    assert!(
        new_world.component_id_by_guid(target_guid).is_some(),
        "target guid not preserved"
    );
    assert!(
        new_world.component_id_by_guid(ee_guid).is_some(),
        "end_effector guid not preserved"
    );
}

/// DFS lookup of the first component of type `C` under `root`.
fn find_first<C: ComponentTrait + 'static>(
    world: &World,
    root: ComponentId,
) -> Option<ComponentId> {
    if world.get_component_by_id_as::<C>(root).is_some() {
        return Some(root);
    }
    let children: Vec<ComponentId> = world
        .get_component_record(root)
        .map(|n| n.children.clone())
        .unwrap_or_default();
    for child in children {
        if let Some(hit) = find_first::<C>(world, child) {
            return Some(hit);
        }
    }
    None
}

#[test]
fn roundtrip_animation_default_omits_resolve_targets_in_text() {
    use crate::engine::ecs::component::AnimationComponent;
    let original = AnimationComponent::new();
    let world_stub = crate::engine::ecs::World::default();
    let text = crate::scripting::unparser::unparse_component(&ComponentTrait::to_mms_ast(
        &original,
        &world_stub,
    ));
    assert!(
        !text.contains("resolve_targets"),
        "default mode should not emit resolve_targets call: {text}"
    );
}

#[test]
fn roundtrip_keyframe() {
    use crate::engine::ecs::component::KeyframeComponent;
    let (world, id) = roundtrip_component(KeyframeComponent::new(4.25));
    let got = world
        .get_component_by_id_as::<KeyframeComponent>(id)
        .unwrap();
    assert!((got.beat - 4.25).abs() < 1e-9);
}

#[test]
fn roundtrip_input_speed() {
    use crate::engine::ecs::component::InputComponent;
    let (world, id) = roundtrip_component(InputComponent::new().with_speed(0.25));
    let got = world.get_component_by_id_as::<InputComponent>(id).unwrap();
    assert!((got.speed - 0.25).abs() < 1e-6);
}

#[test]
fn roundtrip_input_transform_mode() {
    use crate::engine::ecs::component::{
        ComponentRef, ForwardAxis, InputTransformModeComponent, RollAxis,
    };
    let original = InputTransformModeComponent::forward_y()
        .with_roll_axis_y()
        .with_rotation_disabled()
        .with_translation_basis_source(ComponentRef::Query("../#xr_pose".to_string()))
        .with_fps_rotation();
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<InputTransformModeComponent>(id)
        .unwrap();
    assert_eq!(got.forward_axis, ForwardAxis::Y);
    assert_eq!(got.roll_axis, RollAxis::Y);
    assert!(!got.rotation_enabled);
    assert!(got.fps_rotation);
    match got.translation_basis_source.as_ref() {
        Some(ComponentRef::Query(s)) => assert_eq!(s, "../#xr_pose"),
        other => panic!("unexpected translation_basis_source: {other:?}"),
    }
}

#[test]
fn roundtrip_editor() {
    use crate::engine::ecs::component::{EditorComponent, TransformGizmoCoordSpace};
    let original = EditorComponent::new()
        .with_transform_gizmo_translation_space(TransformGizmoCoordSpace::Local)
        .with_transform_gizmo_rotation_space(TransformGizmoCoordSpace::World)
        .with_panels(false)
        .with_serialize_editor_panels(true)
        .with_asset_dir("../custom-assets");
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<EditorComponent>(id).unwrap();
    assert_eq!(
        got.transform_gizmo_translation_space,
        TransformGizmoCoordSpace::Local
    );
    assert_eq!(
        got.transform_gizmo_rotation_space,
        TransformGizmoCoordSpace::World
    );
    assert!(!got.spawn_panels);
    assert!(got.serialize_editor_panels);
    assert_eq!(got.asset_dir.as_deref(), Some("../custom-assets"));
}

#[test]
fn roundtrip_background() {
    use crate::engine::ecs::component::BackgroundComponent;
    let original = BackgroundComponent::new()
        .with_occlusion_and_lighting()
        .with_ray_casting();
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<BackgroundComponent>(id)
        .unwrap();
    assert!(got.occlusion_and_lighting);
    assert!(got.ray_casting);
}

#[test]
fn roundtrip_background_color() {
    use crate::engine::ecs::component::BackgroundColorComponent;
    let (_world, _id) = roundtrip_component(BackgroundColorComponent::new());
}

#[test]
fn roundtrip_raycastable_drag_only() {
    use crate::engine::ecs::component::{PointerEvents, RaycastableComponent};
    let (world, id) = roundtrip_component(RaycastableComponent::drag_only());
    let got = world
        .get_component_by_id_as::<RaycastableComponent>(id)
        .unwrap();
    assert!(got.enable);
    assert_eq!(got.pointer_events, PointerEvents::DragOnly);
}

#[test]
fn roundtrip_raycastable_disabled() {
    use crate::engine::ecs::component::RaycastableComponent;
    let (world, id) = roundtrip_component(RaycastableComponent::disabled());
    let got = world
        .get_component_by_id_as::<RaycastableComponent>(id)
        .unwrap();
    assert!(!got.enable);
}

#[test]
fn roundtrip_raycastable_pass_through() {
    use crate::engine::ecs::component::{PointerEvents, RaycastableComponent};
    let original = RaycastableComponent {
        enable: true,
        pointer_events: PointerEvents::PassThrough,
        interaction_priority: 0,
    };
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RaycastableComponent>(id)
        .unwrap();
    assert!(got.enable);
    assert_eq!(got.pointer_events, PointerEvents::PassThrough);
}

#[test]
fn roundtrip_raycastable_interaction_priority() {
    use crate::engine::ecs::component::RaycastableComponent;
    let original = RaycastableComponent::enabled().with_interaction_priority(3);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RaycastableComponent>(id)
        .unwrap();
    assert!(got.enable);
    assert_eq!(got.interaction_priority, 3);
}

#[test]
fn roundtrip_selectable_off() {
    use crate::engine::ecs::component::SelectableComponent;
    let (world, id) = roundtrip_component(SelectableComponent::off());
    let got = world
        .get_component_by_id_as::<SelectableComponent>(id)
        .unwrap();
    assert!(!got.enabled);
}

#[test]
fn roundtrip_html_element_h1() {
    use crate::engine::ecs::component::{ElementType, HtmlElementComponent};
    let (world, id) = roundtrip_component(HtmlElementComponent::new(ElementType::H1));
    let got = world
        .get_component_by_id_as::<HtmlElementComponent>(id)
        .unwrap();
    assert_eq!(got.element_type, ElementType::H1);
}

// --- Medium value round-trip tests ---

#[test]
fn roundtrip_transparent_cutout_disabled() {
    use crate::engine::ecs::component::TransparentCutoutComponent;
    let (world, id) = roundtrip_component(TransparentCutoutComponent::new().with_enabled(false));
    let got = world
        .get_component_by_id_as::<TransparentCutoutComponent>(id)
        .unwrap();
    assert!(!got.enabled);
}

#[test]
fn roundtrip_texture_filtering_nearest() {
    use crate::engine::ecs::component::TextureFilteringComponent;
    use crate::engine::graphics::TextureFiltering;
    let (world, id) = roundtrip_component(TextureFilteringComponent::nearest());
    let got = world
        .get_component_by_id_as::<TextureFilteringComponent>(id)
        .unwrap();
    assert_eq!(got.filtering, TextureFiltering::Nearest);
}

#[test]
fn roundtrip_emissive_pass() {
    use crate::engine::ecs::component::EmissivePassComponent;
    let (_world, _id) = roundtrip_component(EmissivePassComponent::new());
}

#[test]
fn roundtrip_grid_component_with_dimensions() {
    use crate::engine::ecs::component::GridComponent;
    let original = GridComponent::new(0.5)
        .with_size_x(24)
        .with_size_z(12)
        .with_enabled(false)
        .with_hidden(true)
        .with_selectable(false);
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<GridComponent>(id).unwrap();
    assert!((got.spacing - 0.5).abs() < 1e-6);
    assert_eq!(got.size_x, 24);
    assert_eq!(got.size_z, 12);
    assert!(!got.enabled);
    assert!(got.hidden);
    assert!(!got.selectable);
}

#[test]
fn roundtrip_bloom() {
    use crate::engine::ecs::component::BloomComponent;
    let original = BloomComponent::new()
        .with_enabled(false)
        .with_intensity(0.75)
        .with_radius_ndc(0.125)
        .with_emissive_scale(1.5)
        .with_half_res(true)
        .with_output_texture("scene_bloom");
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<BloomComponent>(id).unwrap();
    assert!(!got.enabled);
    assert!((got.intensity - 0.75).abs() < 1e-6);
    assert!((got.radius_ndc - 0.125).abs() < 1e-6);
    assert!((got.emissive_scale - 1.5).abs() < 1e-6);
    assert!(got.half_res);
    assert_eq!(got.output_texture.as_deref(), Some("scene_bloom"));
}

#[test]
fn roundtrip_blur_pass() {
    use crate::engine::ecs::component::BlurPassComponent;
    let original = BlurPassComponent::new()
        .with_enabled(false)
        .with_radius_ndc(0.25)
        .with_half_res(true);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<BlurPassComponent>(id)
        .unwrap();
    assert!(!got.enabled);
    assert!((got.radius_ndc - 0.25).abs() < 1e-6);
    assert!(got.half_res);
}

#[test]
fn roundtrip_render_graph_off() {
    use crate::engine::ecs::component::RenderGraphComponent;
    let (world, id) = roundtrip_component(RenderGraphComponent::off());
    let got = world
        .get_component_by_id_as::<RenderGraphComponent>(id)
        .unwrap();
    assert!(!got.enabled);
}

#[test]
fn roundtrip_light_quantization() {
    use crate::engine::ecs::component::LightQuantizationComponent;
    let (world, id) = roundtrip_component(LightQuantizationComponent::steps(5.0));
    let got = world
        .get_component_by_id_as::<LightQuantizationComponent>(id)
        .unwrap();
    assert!((got.quant_steps - 5.0).abs() < 1e-6);
}

#[test]
fn roundtrip_normal_visualisation() {
    use crate::engine::ecs::component::NormalVisualisationComponent;
    let (world, id) = roundtrip_component(NormalVisualisationComponent::new().with_thickness(0.05));
    let got = world
        .get_component_by_id_as::<NormalVisualisationComponent>(id)
        .unwrap();
    assert!((got.thickness - 0.05).abs() < 1e-6);
}

#[test]
fn roundtrip_uv() {
    use crate::engine::ecs::component::UVComponent;
    let original = UVComponent::new()
        .with_uv(0.0, 0.0)
        .with_uv(1.0, 0.0)
        .with_uv(0.5, 1.0);
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<UVComponent>(id).unwrap();
    assert_eq!(got.uvs.len(), 3);
    assert!((got.uvs[2][0] - 0.5).abs() < 1e-6);
    assert!((got.uvs[2][1] - 1.0).abs() < 1e-6);
}

#[test]
fn roundtrip_scrolling() {
    use crate::engine::ecs::component::ScrollingComponent;
    let (world, id) = roundtrip_component(ScrollingComponent::new(2.0, 8.0));
    let got = world
        .get_component_by_id_as::<ScrollingComponent>(id)
        .unwrap();
    assert!((got.viewport_height - 2.0).abs() < 1e-6);
    assert!((got.content_height - 8.0).abs() < 1e-6);
}

#[test]
fn roundtrip_clock() {
    use crate::engine::ecs::component::ClockComponent;
    let (world, id) = roundtrip_component(ClockComponent::new().with_bpm(140.0));
    let got = world.get_component_by_id_as::<ClockComponent>(id).unwrap();
    assert!((got.bpm - 140.0).abs() < 1e-9);
}

#[test]
fn roundtrip_router() {
    use crate::engine::ecs::component::RouterComponent;
    let original = RouterComponent::new()
        .with_target_name("content")
        .with_ignored_names(["a", "b", "c"]);
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<RouterComponent>(id).unwrap();
    assert_eq!(got.target_name.as_deref(), Some("content"));
    assert_eq!(got.ignore_names, vec!["a", "b", "c"]);
}

#[test]
fn roundtrip_transition() {
    use crate::engine::ecs::component::{
        TransitionComponent, TransitionEasing, TransitionReplacePolicy,
    };
    let original = TransitionComponent::new()
        .enabled(true)
        .with_duration_beats(2.0)
        .with_capture_from_current(false)
        .with_easing(TransitionEasing::EaseInOutCubic)
        .with_replace(TransitionReplacePolicy::AllowParallel);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<TransitionComponent>(id)
        .unwrap();
    assert!(got.enabled);
    assert!((got.duration_beats - 2.0).abs() < 1e-9);
    assert!(!got.capture_from_current);
    assert_eq!(got.easing, TransitionEasing::EaseInOutCubic);
    assert_eq!(got.replace, TransitionReplacePolicy::AllowParallel);
}

#[test]
fn roundtrip_text_shadow() {
    use crate::engine::ecs::component::TextShadowComponent;
    let original = TextShadowComponent::new()
        .with_rgba([0.1, 0.2, 0.3, 0.5])
        .with_scale(1.5)
        .with_offset([0.25, -0.5, 0.001]);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<TextShadowComponent>(id)
        .unwrap();
    assert!((got.rgba[0] - 0.1).abs() < 1e-6);
    assert!((got.rgba[3] - 0.5).abs() < 1e-6);
    assert!((got.scale - 1.5).abs() < 1e-6);
    assert!((got.offset[0] - 0.25).abs() < 1e-6);
    assert!((got.offset[1] + 0.5).abs() < 1e-6);
    assert!((got.offset[2] - 0.001).abs() < 1e-6);
}

#[test]
fn roundtrip_renderer_settings_msaa_off() {
    use crate::engine::ecs::component::RendererSettingsComponent;
    let original = RendererSettingsComponent::msaa_off().with_window_size(1920, 1080);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RendererSettingsComponent>(id)
        .unwrap();
    assert!(!got.msaa4x);
    assert_eq!(got.window_size, Some([1920, 1080]));
}

// --- Low value round-trip tests ---

#[test]
fn roundtrip_stencil_clip() {
    use crate::engine::ecs::component::StencilClipComponent;
    let mut original = StencilClipComponent::new();
    original.stencil_ref = 3;
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<StencilClipComponent>(id)
        .unwrap();
    assert_eq!(got.stencil_ref, 3);
}

#[test]
fn roundtrip_bounds() {
    use crate::engine::ecs::component::BoundsComponent;
    use crate::engine::graphics::bounds::Aabb;
    let original = BoundsComponent::new(Aabb {
        min: [-1.0, -2.0, -3.0],
        max: [1.0, 2.0, 3.0],
    });
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<BoundsComponent>(id).unwrap();
    assert_eq!(got.local.min, [-1.0, -2.0, -3.0]);
    assert_eq!(got.local.max, [1.0, 2.0, 3.0]);
}

#[test]
fn roundtrip_mesh() {
    use crate::engine::ecs::component::MeshComponent;
    let (world, id) = roundtrip_component(MeshComponent::new("scene.glb:body:0"));
    let got = world.get_component_by_id_as::<MeshComponent>(id).unwrap();
    assert_eq!(got.key, "scene.glb:body:0");
}

#[test]
fn roundtrip_gesture_coord_type() {
    use crate::engine::ecs::component::{GestureCoordType, GestureCoordTypeComponent};
    let (world, id) = roundtrip_component(GestureCoordTypeComponent::screen_space_1d_slider());
    let got = world
        .get_component_by_id_as::<GestureCoordTypeComponent>(id)
        .unwrap();
    assert_eq!(got.coord_type, GestureCoordType::ScreenSpace1DSlider);
}

#[test]
fn roundtrip_collision_shape_cube() {
    use crate::engine::ecs::component::{CollisionShape, CollisionShapeComponent};
    let original = CollisionShapeComponent::new(CollisionShape::cube_half_extents([2.0, 3.0, 4.0]));
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<CollisionShapeComponent>(id)
        .unwrap();
    match got.shape {
        CollisionShape::Cube { half_extents } => assert_eq!(half_extents, [2.0, 3.0, 4.0]),
        _ => panic!("expected Cube"),
    }
}

#[test]
fn roundtrip_collision_shape_sphere() {
    use crate::engine::ecs::component::{CollisionShape, CollisionShapeComponent};
    let original = CollisionShapeComponent::new(CollisionShape::sphere_radius(1.5));
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<CollisionShapeComponent>(id)
        .unwrap();
    match got.shape {
        CollisionShape::Sphere { radius } => assert!((radius - 1.5).abs() < 1e-6),
        _ => panic!("expected Sphere"),
    }
}

#[test]
fn roundtrip_raycastable_shape() {
    use crate::engine::ecs::component::{RaycastableShapeComponent, RaycastableShapeType};
    let (world, id) = roundtrip_component(RaycastableShapeComponent::cone());
    let got = world
        .get_component_by_id_as::<RaycastableShapeComponent>(id)
        .unwrap();
    assert_eq!(got.shape, RaycastableShapeType::Cone);
}

#[test]
fn roundtrip_collision() {
    use crate::engine::ecs::component::{CollisionComponent, CollisionMode};
    let (world, id) = roundtrip_component(CollisionComponent::KINEMATIC());
    let got = world
        .get_component_by_id_as::<CollisionComponent>(id)
        .unwrap();
    assert_eq!(got.mode, CollisionMode::Kinematic);
}

#[test]
fn roundtrip_gravity() {
    use crate::engine::ecs::component::GravityComponent;
    let (world, id) = roundtrip_component(GravityComponent::new().with_coefficient(0.5));
    let got = world
        .get_component_by_id_as::<GravityComponent>(id)
        .unwrap();
    assert!(got.enabled);
    assert!((got.coefficient - 0.5).abs() < 1e-6);
}

#[test]
fn roundtrip_pointer_disabled() {
    use crate::engine::ecs::component::PointerComponent;
    let (world, id) = roundtrip_component(PointerComponent::disabled());
    let got = world
        .get_component_by_id_as::<PointerComponent>(id)
        .unwrap();
    assert!(!got.enabled);
}

#[test]
fn roundtrip_skinned_mesh() {
    use crate::engine::ecs::component::SkinnedMeshComponent;
    let (world, id) = roundtrip_component(SkinnedMeshComponent::new(7));
    let got = world
        .get_component_by_id_as::<SkinnedMeshComponent>(id)
        .unwrap();
    assert_eq!(got.skin_index, 7);
}

#[test]
fn roundtrip_transform_sample_ancestor() {
    use crate::engine::ecs::component::TransformSampleAncestorComponent;
    let (world, id) = roundtrip_component(TransformSampleAncestorComponent::new().with_skip(3));
    let got = world
        .get_component_by_id_as::<TransformSampleAncestorComponent>(id)
        .unwrap();
    assert_eq!(got.skip, 3);
}

#[test]
fn roundtrip_transform_parent() {
    use crate::engine::ecs::component::{ComponentRef, TransformParentComponent};
    let original = TransformParentComponent::new()
        .with_target_source(ComponentRef::Query("#hero".to_string()))
        .with_root_source(ComponentRef::Query("#scene".to_string()));
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<TransformParentComponent>(id)
        .unwrap();
    match got.target_source.as_ref() {
        Some(ComponentRef::Query(s)) => assert_eq!(s, "#hero"),
        other => panic!("unexpected target_source: {other:?}"),
    }
    match got.root_source.as_ref() {
        Some(ComponentRef::Query(s)) => assert_eq!(s, "#scene"),
        other => panic!("unexpected root_source: {other:?}"),
    }
}

#[test]
fn roundtrip_quat_temporal_filter() {
    use crate::engine::ecs::component::QuatTemporalFilterComponent;
    let (world, id) =
        roundtrip_component(QuatTemporalFilterComponent::new().with_smoothing_factor(220.0));
    let got = world
        .get_component_by_id_as::<QuatTemporalFilterComponent>(id)
        .unwrap();
    assert!((got.smoothing_factor - 220.0).abs() < 1e-6);
}

#[test]
fn roundtrip_vector3_temporal_filter() {
    use crate::engine::ecs::component::Vector3TemporalFilterComponent;
    let (world, id) =
        roundtrip_component(Vector3TemporalFilterComponent::new().with_smoothing_factor(15.0));
    let got = world
        .get_component_by_id_as::<Vector3TemporalFilterComponent>(id)
        .unwrap();
    assert!((got.smoothing_factor - 15.0).abs() < 1e-6);
}

#[test]
fn roundtrip_quat_yaw_follow() {
    use crate::engine::ecs::component::QuatYawFollowComponent;
    let original = QuatYawFollowComponent::new(0.5, 2.0)
        .with_forward_plus_z()
        .with_initial_yaw(std::f32::consts::PI);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<QuatYawFollowComponent>(id)
        .unwrap();
    assert!((got.threshold - 0.5).abs() < 1e-6);
    assert!((got.rate - 2.0).abs() < 1e-6);
    assert!(got.forward_plus_z);
    assert!((got.initial_yaw - std::f32::consts::PI).abs() < 1e-6);
}

#[test]
fn roundtrip_signal_route_upward() {
    use crate::engine::ecs::component::SignalRouteUpwardComponent;
    let original = SignalRouteUpwardComponent::new("UpdateTransform", "transform_pipeline");
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<SignalRouteUpwardComponent>(id)
        .unwrap();
    assert_eq!(got.intent_kind, "UpdateTransform");
    assert_eq!(got.parent_type, "transform_pipeline");
}

#[test]
fn roundtrip_avatar_body_yaw() {
    use crate::engine::ecs::component::AvatarBodyYawComponent;
    let original = AvatarBodyYawComponent::new()
        .with_threshold(0.5)
        .with_rate(2.0)
        .with_forward_plus_z();
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<AvatarBodyYawComponent>(id)
        .unwrap();
    assert!((got.threshold - 0.5).abs() < 1e-6);
    assert!((got.rate - 2.0).abs() < 1e-6);
    assert!(got.forward_plus_z);
}

#[test]
fn roundtrip_raycast() {
    use crate::engine::ecs::component::{RayCastComponent, RayCastMode};
    let original = RayCastComponent::continuous().with_max_distance(75.0);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RayCastComponent>(id)
        .unwrap();
    assert_eq!(got.mode, RayCastMode::Continuous);
    assert!((got.max_distance - 75.0).abs() < 1e-6);
}

#[test]
fn roundtrip_avatar_control() {
    use crate::engine::ecs::component::AvatarControlComponent;
    let original = AvatarControlComponent::new()
        .with_head_bone("J_Bip_C_Neck")
        .with_left_hand_bone("J_Bip_L_Hand")
        .with_right_hand_bone("J_Bip_R_Hand")
        .with_forward_plus_z()
        .with_hand_rotation_smoothing(220.0)
        .with_camera_bone("J_Bip_C_Head")
        .with_avatar_height(1.7);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<AvatarControlComponent>(id)
        .unwrap();
    assert_eq!(got.head_bone, "J_Bip_C_Neck");
    assert_eq!(got.left_hand_bone.as_deref(), Some("J_Bip_L_Hand"));
    assert_eq!(got.right_hand_bone.as_deref(), Some("J_Bip_R_Hand"));
    assert!(got.forward_plus_z);
    assert_eq!(got.hand_rotation_smoothing, Some(220.0));
    assert_eq!(got.camera_bone.as_deref(), Some("J_Bip_C_Head"));
    assert_eq!(got.avatar_height, Some(1.7));
}

#[test]
fn roundtrip_music_note_c5() {
    use crate::engine::ecs::component::{MusicNote, MusicNoteComponent};
    let note = MusicNote::c(5, 0.25).with_velocity(0.8);
    let (world, id) = roundtrip_component(MusicNoteComponent::new(note));
    let got = world
        .get_component_by_id_as::<MusicNoteComponent>(id)
        .unwrap();
    assert_eq!(got.note.pitch_name(), "c");
    assert_eq!(got.note.octave(), 5);
    assert!((got.note.duration_beats() - 0.25).abs() < 1e-6);
    assert!((got.note.velocity() - 0.8).abs() < 1e-6);
}

// Temporarily gated: see docs/bugs/ik-solver-api-drift-breaks-tests.md.
#[cfg(any())]
#[test]
fn roundtrip_ik_chain_aim() {
    use crate::engine::ecs::ComponentId;
    use crate::engine::ecs::component::{IKChainComponent, IKSolver};
    use slotmap::Key;
    let sentinel = ComponentId::null();
    let original = IKChainComponent::new(
        IKSolver::AimConstraint {
            offset_yaw: std::f32::consts::PI,
        },
        sentinel,
        sentinel,
    )
    .with_weight(0.5);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<IKChainComponent>(id)
        .unwrap();
    match got.solver {
        IKSolver::AimConstraint { offset_yaw } => {
            assert!((offset_yaw - std::f32::consts::PI).abs() < 1e-6);
        }
        _ => panic!("expected AimConstraint"),
    }
    assert!((got.weight - 0.5).abs() < 1e-6);
}

#[test]
fn roundtrip_transform_gizmo_translate() {
    use crate::engine::ecs::component::{TransformGizmoAxis, TransformGizmoTranslateComponent};
    let (world, id) =
        roundtrip_component(TransformGizmoTranslateComponent::new(TransformGizmoAxis::Y));
    let got = world
        .get_component_by_id_as::<TransformGizmoTranslateComponent>(id)
        .unwrap();
    assert_eq!(got.axis, TransformGizmoAxis::Y);
}

#[test]
fn roundtrip_transform_gizmo() {
    use crate::engine::ecs::component::TransformGizmoComponent;
    let (world, id) = roundtrip_component(TransformGizmoComponent::new().with_scale(0.5));
    let got = world
        .get_component_by_id_as::<TransformGizmoComponent>(id)
        .unwrap();
    assert!((got.scale - 0.5).abs() < 1e-6);
}

#[test]
fn roundtrip_transform_camera_specific_modes() {
    use crate::engine::ecs::component::{
        TransformCameraSpecificComponent, TransformCameraSpecificMode,
    };
    for (component, expected) in [
        (
            TransformCameraSpecificComponent::active_monoscopic(),
            TransformCameraSpecificMode::Monoscopic,
        ),
        (
            TransformCameraSpecificComponent::active_stereoscopic(),
            TransformCameraSpecificMode::Stereoscopic,
        ),
    ] {
        let (world, id) = roundtrip_component(component);
        assert_eq!(
            world
                .get_component_by_id_as::<TransformCameraSpecificComponent>(id)
                .unwrap()
                .mode,
            expected
        );
    }
}

#[test]
fn roundtrip_renderer_stats() {
    use crate::engine::ecs::component::RendererStatsComponent;
    use crate::engine::graphics::CameraTarget;
    let mut original = RendererStatsComponent::new();
    original.enabled = false;
    original.target = CameraTarget::Xr;
    original.update_interval_sec = 0.5;
    original.smoothing = 0.8;
    original.color = [0.5, 0.6, 0.7, 1.0];
    original.emissive = false;
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RendererStatsComponent>(id)
        .unwrap();
    assert!(!got.enabled);
    assert!(matches!(got.target, CameraTarget::Xr));
    assert!((got.update_interval_sec - 0.5).abs() < 1e-6);
    assert!((got.smoothing - 0.8).abs() < 1e-6);
    assert_eq!(got.color, [0.5, 0.6, 0.7, 1.0]);
    assert!(!got.emissive);
}

#[test]
fn roundtrip_kinetic_response() {
    use crate::engine::ecs::component::{KineticResponseComponent, KineticResponseMode};
    let original = KineticResponseComponent::push()
        .with_push_strength(8.0)
        .with_friction(0.5)
        .with_friction_y(0.25);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<KineticResponseComponent>(id)
        .unwrap();
    assert_eq!(got.mode, KineticResponseMode::Push);
    assert!((got.push_strength - 8.0).abs() < 1e-6);
    assert!((got.friction - 0.5).abs() < 1e-6);
    assert!((got.friction_y - 0.25).abs() < 1e-6);
}
