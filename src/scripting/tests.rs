use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::{fs, path::PathBuf};

use crate::engine;
use crate::engine::ecs::component::renderable::AuthoredRenderableShape;
use crate::engine::ecs::component::style::SizeDimension;
use crate::engine::ecs::component::{
    LayoutBoundsComponent, LayoutComponent, LayoutVisualPlacementComponent, RenderableComponent,
    StyleComponent, TransformComponent,
};
use crate::engine::ecs::system::layout::LayoutSystem;
use crate::engine::ecs::{
    CommandQueue, ComponentId, EventSignal, IntentValue, RxWorld, Signal, SignalEmitter, World,
};
use crate::engine::graphics::{RenderAssets, VisualWorld};
use crate::engine::user_input::InputState;
use crate::scripting::ast::{AssignmentStatement, Expression, ImportItem, Statement};
use crate::scripting::object::Value;
use crate::scripting::parser::MeowMeowParser;
use crate::scripting::runner::{MeowMeowRunner, RuntimeSpecSession};
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

#[test]
fn constructive_solid_geometry_example_is_valid_mms_syntax() {
    let source = fs::read_to_string(repo_path("examples/constructive-solid-geometry.mms"))
        .expect("read CSG example");
    let program = parse(&source);
    assert!(!program.is_empty(), "CSG example should author a scene");
}

#[test]
fn implicit_surface_example_is_valid_mms_syntax() {
    let path = repo_path("examples/implicit-surface.mms");
    let source = fs::read_to_string(&path).expect("read implicit-surface example");
    let program = parse(&source);
    assert!(
        !program.is_empty(),
        "implicit-surface example should author a scene"
    );

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let (_session, output) = RuntimeSpecSession::start_at_path(
        &source,
        path.to_str().expect("scene path should be UTF-8"),
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .expect("strict runtime should load microphone-speaking scene");
    assert!(
        output.errors.is_empty(),
        "implicit-surface example failed to materialize: {:?}",
        output.errors
    );
    let surfaces = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::ImplicitSurfaceComponent>(
                    id,
                )
                .is_some()
        })
        .count();
    let spheres = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::ImplicitSphereComponent>(
                    id,
                )
                .is_some()
        })
        .count();
    assert_eq!(surfaces, 2);
    assert_eq!(spheres, 149);
    assert_eq!(
        world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::GrabbableComponent>(id)
                    .is_some()
            })
            .count(),
        1,
        "implicit-surface should materialize one grabbable telemetry panel"
    );
    assert_eq!(
        world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::DraggableComponent>(id)
                    .is_some()
            })
            .count(),
        1,
        "implicit-surface should materialize one draggable telemetry title"
    );
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(id)
            .is_some_and(|text| text.text == "telemetry")
    }));
}

#[test]
fn vtuber_eye_tracking_mirror_microphone_binding_is_valid_mms_syntax() {
    let path = repo_path("examples/vtuber-eye-tracking-mirror-eye-stabilize.mms");
    let source = fs::read_to_string(&path).expect("read microphone mirror example");
    assert!(!parse(&source).is_empty());
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        &source,
        path.to_str(),
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    );
    assert!(
        output.errors.is_empty(),
        "mirror microphone scene errors: {:?}",
        output.errors
    );
    let amplitude = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::AmplitudeComponent>(id)
                .is_some()
        })
        .expect("mirror scene amplitude");
    let avc = world
        .parent_of(amplitude)
        .expect("amplitude should be attached directly to AVC");
    let avc = world
        .get_component_by_id_as::<crate::engine::ecs::component::AvatarControlComponent>(avc)
        .expect("amplitude parent should be AVC");
    let amplitude_guid = world.get_component_record(amplitude).unwrap().guid;
    assert_eq!(
        avc.mouth_open_amplitude,
        Some(crate::engine::ecs::component::ComponentRef::Guid(
            amplitude_guid
        ))
    );
}

#[test]
fn vtuber_microphone_speaking_htc_eye_tracking_scene_materializes_info_panel() {
    let path = repo_path("examples/vtuber-microphone-speaking-xr-eye-tracking-htc.mms");
    let source = fs::read_to_string(&path).expect("read microphone-speaking mirror example");
    assert!(!parse(&source).is_empty());
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let (_session, output) = RuntimeSpecSession::start_at_path(
        &source,
        path.to_str().expect("scene path should be UTF-8"),
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .expect("strict runtime should load microphone-speaking scene");
    assert!(
        output.errors.is_empty(),
        "microphone-speaking mirror scene errors: {:?}",
        output.errors
    );
    assert!(
        world
            .all_components()
            .any(|id| world.component_label(id) == Some("microphone_inputs_panel")),
        "scene should materialize the reusable microphone info panel"
    );
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(id)
            .is_some_and(|text| {
                text.text == "default audio input selected — click a device to switch"
            })
    }));
}

#[test]
fn vtuber_microphone_speaking_eye_tracking_scene_uses_standard_eye_tracking() {
    let path = repo_path("examples/vtuber-microphone-speaking-xr-eye-tracking.mms");
    let source = fs::read_to_string(&path).expect("read microphone-speaking eye-tracking example");
    assert!(!parse(&source).is_empty());
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let (_session, output) = RuntimeSpecSession::start_at_path(
        &source,
        path.to_str().expect("scene path should be UTF-8"),
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .expect("strict runtime should load microphone-speaking eye-tracking scene");
    assert!(output.errors.is_empty(), "{:#?}", output.errors);
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::VRChatOSCEyeTrackingComponent>(
                id,
            )
            .is_some()
    }));
    assert!(!world.all_components().any(|id| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::HTCEyeTrackingComponent>(id)
            .is_some()
    }));
}

#[test]
fn vtuber_microphone_speaking_eye_tracking_panel_swaps_direct_avc_tracker() {
    let path = repo_path("examples/vtuber-microphone-speaking-xr-eye-tracking.mms");
    let source = fs::read_to_string(&path).expect("read switchable eye-tracking example");
    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::new();
    systems.selection.install_handlers(&mut systems.rx);
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let (mut session, output) = RuntimeSpecSession::start_at_path(
        &source,
        path.to_str().expect("scene path should be UTF-8"),
        &mut world,
        &mut systems.rx,
        Some(&mut assets),
        &mut queue,
    )
    .expect("strict runtime should load switchable eye-tracking scene");
    assert!(output.errors.is_empty(), "{:#?}", output.errors);

    let avc = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("avatar_control"))
        .expect("named AVC");
    let standard_tracker = world
        .children_of(avc)
        .iter()
        .copied()
        .find(|&id| {
            world
                .get_component_by_id_as::<
                    crate::engine::ecs::component::VRChatOSCEyeTrackingComponent,
                >(id)
                .is_some()
        })
        .expect("initial direct VRChat OSC tracker");
    let htc_row = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("eye_tracking_option_htc_wave"))
        .expect("HTC option row");

    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            htc_row,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: htc_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        ),
    );
    let htc_row_style = world
        .children_of(htc_row)
        .iter()
        .copied()
        .find_map(|id| {
            world.get_component_by_id_as::<crate::engine::ecs::component::StyleComponent>(id)
        })
        .expect("HTC option style");
    assert_eq!(htc_row_style.background_color, Some([1.0, 0.84, 0.0, 1.0]));
    let output =
        session.service_callbacks(&mut world, &mut systems.rx, Some(&mut assets), &mut queue);
    assert!(output.errors.is_empty(), "{:#?}", output.errors);
    assert!(output.intents.iter().any(|intent| matches!(
        intent,
        IntentValue::RemoveSubtree { component_id } if *component_id == standard_tracker
    )));
    assert!(output.intents.iter().any(|intent| matches!(
        intent,
        IntentValue::Attach { parent, child }
            if *parent == avc
                && world
                    .get_component_by_id_as::<crate::engine::ecs::component::HTCEyeTrackingComponent>(*child)
                    .is_some()
    )));
    for intent in output.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    let mut visuals = VisualWorld::default();
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);

    assert!(world.get_component_record(standard_tracker).is_none());
    let direct_trackers = world
        .children_of(avc)
        .iter()
        .copied()
        .filter(|&id| {
            world
                .get_component_by_id_as::<
                    crate::engine::ecs::component::VRChatOSCEyeTrackingComponent,
                >(id)
                .is_some()
                || world
                    .get_component_by_id_as::<crate::engine::ecs::component::HTCEyeTrackingComponent>(
                        id,
                    )
                    .is_some()
        })
        .collect::<Vec<_>>();
    assert_eq!(direct_trackers.len(), 1, "one active direct eye tracker");
    assert!(
        world
            .get_component_by_id_as::<crate::engine::ecs::component::HTCEyeTrackingComponent>(
                direct_trackers[0]
            )
            .is_some()
    );

    let htc_tracker = direct_trackers[0];
    let vrchat_row = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("eye_tracking_option_vrchat_osc"))
        .expect("VRChat OSC option row");
    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            vrchat_row,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: vrchat_row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        ),
    );
    let output =
        session.service_callbacks(&mut world, &mut systems.rx, Some(&mut assets), &mut queue);
    assert!(output.errors.is_empty(), "{:#?}", output.errors);
    assert!(output.intents.iter().any(|intent| matches!(
        intent,
        IntentValue::RemoveSubtree { component_id } if *component_id == htc_tracker
    )));
    for intent in output.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);

    assert!(world.get_component_record(htc_tracker).is_none());
    let direct_trackers = world
        .children_of(avc)
        .iter()
        .copied()
        .filter(|&id| {
            world
                .get_component_by_id_as::<
                    crate::engine::ecs::component::VRChatOSCEyeTrackingComponent,
                >(id)
                .is_some()
                || world
                    .get_component_by_id_as::<crate::engine::ecs::component::HTCEyeTrackingComponent>(
                        id,
                    )
                    .is_some()
        })
        .collect::<Vec<_>>();
    assert_eq!(direct_trackers.len(), 1, "one active direct eye tracker");
    assert!(
        world
            .get_component_by_id_as::<crate::engine::ecs::component::VRChatOSCEyeTrackingComponent>(
                direct_trackers[0]
            )
            .is_some()
    );
}

#[test]
fn audio_device_static_apis_are_available_to_strict_mms() {
    let source = r#"
        let inputs = AudioInput.devices()
        let outputs = AudioOutput.devices()
        T { Text { "audio device static API smoke" } }
    "#;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let (_session, output) = RuntimeSpecSession::start_at_path(
        source,
        "examples/_mms_test_audio_device_static_apis.mms",
        &mut world,
        &mut rx,
        None,
        &mut queue,
    )
    .expect("strict runtime should start audio-device static API test");
    assert!(output.errors.is_empty(), "{:#?}", output.errors);
}

#[test]
fn audio_input_device_row_click_calls_live_selector_method() {
    let source = r#"
        let microphone = AudioInput {}
        let device_row = T {
            name = "device_row"
            Option {}
            Raycastable.enabled()
            Text { "[2] test microphone" }
        }
        on(device_row, "Click", fn(event) {
            microphone.select_device_number(2)
        })
        microphone
        device_row
    "#;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let (mut session, output) = RuntimeSpecSession::start_at_path(
        source,
        "examples/_mms_test_audio_input_device_selection.mms",
        &mut world,
        &mut rx,
        None,
        &mut queue,
    )
    .expect("strict runtime should start audio-input selection test");
    assert!(output.errors.is_empty(), "{:#?}", output.errors);

    let microphone = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::AudioInputComponent>(id)
                .is_some()
        })
        .expect("audio input");
    let row = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("device_row"))
        .expect("device row");
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            row,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: row,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        ),
    );
    let output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(output.errors.is_empty(), "{:#?}", output.errors);
    assert_eq!(
        world
            .get_component_by_id_as::<crate::engine::ecs::component::AudioInputComponent>(
                microphone
            )
            .expect("audio input after click")
            .device,
        crate::engine::ecs::component::AudioInputDeviceSelector::DeviceNumber(2),
    );
    assert_eq!(
        world
            .get_component_by_id_as::<crate::engine::ecs::component::AudioInputComponent>(
                microphone
            )
            .expect("audio input after click")
            .selection_generation,
        1,
    );
}

#[test]
fn implicit_surface_refraction_clouds_example_materializes() {
    let path = repo_path("examples/implicit-surface-refraction-clouds.mms");
    let source = fs::read_to_string(&path).expect("read implicit refraction cloud example");
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        &source,
        path.to_str(),
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    );
    assert!(
        output.errors.is_empty(),
        "implicit refraction cloud example failed to materialize: {:?}",
        output.errors
    );
    assert_eq!(
        world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::ImplicitSurfaceComponent>(
                        id,
                    )
                    .is_some()
            })
            .count(),
        1
    );
    assert_eq!(
        world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::ImplicitSphereComponent>(
                        id,
                    )
                    .is_some()
            })
            .count(),
        7
    );
    assert_eq!(
        world
            .all_components()
            .filter(|&id| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::RefractionComponent>(
                        id,
                    )
                    .is_some()
            })
            .count(),
        1
    );
}

#[test]
fn migrated_keyframe_mms_examples_materialize_in_live_worlds() {
    for scene in [
        "animation-example.mms",
        "animation-for-topology.mms",
        "audio-graph-example.mms",
        "mesh-factory-example.mms",
        "raycast-topology-animation.mms",
        "text-animation.mms",
    ] {
        let path = repo_path(&format!("examples/{scene}"));
        let source = fs::read_to_string(&path).expect("example source");
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut assets = RenderAssets::new();
        let mut queue = CommandQueue::new();
        let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
            &source,
            path.to_str(),
            &mut world,
            &mut rx,
            Some(&mut assets),
            &mut queue,
        );
        assert!(
            output.errors.is_empty(),
            "{scene} failed to materialize: {:?}",
            output.errors
        );
        assert!(
            world.all_components().any(|id| world
                .get_component_by_id_as::<crate::engine::ecs::component::AnimationComponent>(id)
                .is_some()),
            "{scene} should materialize an animation"
        );
    }
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

    let parent_t = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("parent_t"))
        .expect("parent_t exists");
    let cube_t = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("cube_t"))
        .expect("cube_t exists");
    assert_eq!(world.parent_of(cube_t), Some(parent_t));

    driver.set_time_sec(0.1);
    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &input,
        &mut queue,
        0.1,
    );
    driver.set_time_sec(1.0);
    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &input,
        &mut queue,
        1.0,
    );
    driver.set_time_sec(1.5);
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

    systems
        .animation
        .tick_with_beat(&mut world, 0.5, 60.0, &mut systems.rx);
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

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
fn ambient_eye_saccade_factory_materializes_a_32_keyframe_loop() {
    let src = r##"
        import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"

        let left_eye = T { name = "left_eye" }
        let right_eye = T { name = "right_eye" }
        left_eye
        right_eye
        ambient_eye_saccades(left_eye, right_eye, 1.0)
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let out = MeowMeowRunner::eval_with_world_at_path(
        src,
        Some("examples/_mms_test_ambient_eye_saccades.mms"),
        &mut world,
        &mut rx,
        &mut emit,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let animation = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::AnimationComponent>(*id)
                .is_some()
        })
        .expect("ambient eye factory should return an Animation");
    assert_eq!(world.children_of(animation).len(), 32);
}

#[test]
fn retained_callback_materializes_ambient_eye_animation_without_legacy_closures() {
    let source = r##"
        import { ambient_eye_saccades } from "../assets/components/animations/ambient_eye_saccades.mms"

        let avatar = T {
            name = "avatar"
            T { name = "left_eye" }
            T { name = "right_eye" }
        }
        on(avatar, "GLTFInitialized", fn(event) {
            let left_eye = event.gltf.query("#left_eye")
            let right_eye = event.gltf.query("#right_eye")
            avatar.attach(ambient_eye_saccades(left_eye, right_eye, 2.0))
        })
        avatar
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, output) = RuntimeSpecSession::start_at_path(
        source,
        "examples/_mms_test_retained_ambient_eye_saccades.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .expect("retained ambient-eye fixture should start");
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let avatar = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("avatar"))
        .expect("avatar fixture root");
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            avatar,
            EventSignal::GltfInitialized {
                gltf: avatar,
                uri: "fixture.glb".into(),
            },
        ),
    );
    let callback_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(
        callback_output.errors.is_empty(),
        "{:?}",
        callback_output.errors
    );

    let animation = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::AnimationComponent>(id)
                .is_some()
        })
        .expect("callback should attach an animation");
    let keyframes = world.children_of(animation);
    assert_eq!(keyframes.len(), 32);
    for &keyframe in keyframes {
        let keyframe = world
            .get_component_by_id_as::<crate::engine::ecs::component::KeyframeComponent>(keyframe)
            .expect("Animation children must be Keyframes");
        assert!(keyframe.callback.is_none());
        assert!(keyframe.session_callback.is_some());
    }

    let callback = world
        .get_component_by_id_as::<crate::engine::ecs::component::KeyframeComponent>(keyframes[0])
        .unwrap()
        .session_callback
        .unwrap();
    let intents = session
        .invoke_deferred_callback(
            callback,
            crate::scripting::runner::DeferredCallbackMode::VisualOnly,
            &mut world,
            &mut rx,
            Some(&mut assets),
            &mut queue,
        )
        .expect("nested pose closure should remain callable in its originating session");
    assert_eq!(
        intents
            .iter()
            .filter(|intent| matches!(intent, IntentValue::UpdateTransform { .. }))
            .count(),
        2
    );
}

#[test]
fn live_eval_local_trs_value_round_trip_is_copied_and_quaternion_preserving() {
    let src = r##"
        let source = T.position(1.0, 2.0, 3.0)
            .rotation_quat(0.0, 0.0, 0.70710677, 0.70710677)
            .scale(4.0, 5.0, 6.0) {
            name = "trs_source"
        }
        let target = T {
            name = "trs_target"
        }

        source
        target

        let pose = source.trs()
        target.trs(pose)

        source.update_transform(
            [9.0, 8.0, 7.0],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0]
        )
    "##;

    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut queue = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut systems.rx, &mut queue);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert_eq!(
        world.all_components().count(),
        2,
        "the copied TRS value must not register a component"
    );

    for intent in out.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    let source = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("trs_source"))
        .expect("source transform");
    let target = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("trs_target"))
        .expect("target transform");
    let source = world
        .get_component_by_id_as::<TransformComponent>(source)
        .expect("source TransformComponent")
        .trs();
    let target = world
        .get_component_by_id_as::<TransformComponent>(target)
        .expect("target TransformComponent")
        .trs();

    assert_eq!(source.translation, [9.0, 8.0, 7.0]);
    assert_eq!(source.rotation_quat_xyzw, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(source.scale, [1.0, 1.0, 1.0]);

    assert_eq!(target.translation, [1.0, 2.0, 3.0]);
    assert_eq!(
        target.rotation_quat_xyzw,
        [0.0, 0.0, 0.70710677, 0.70710677]
    );
    assert_eq!(target.scale, [4.0, 5.0, 6.0]);
    assert_eq!(
        world.all_components().count(),
        2,
        "applying the copied TRS must not register a component"
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
fn runtime_spec_json_host_api_round_trips_heap_backed_tables() {
    let source = r#"
        let record = JSON.parse("{\"value\":4,\"nested\":{\"value\":7},\"items\":[true, null]}")
        record.value = record.value + 1
        record.nested.value = record.nested.value + 1
        let decoded = JSON.parse(JSON.stringify(record))
        if decoded.value != 5 { json_round_trip_failed() }
        if decoded.nested.value != 8 { json_nested_table_failed() }
        if decoded.items[0] != true { json_array_failed() }
        if decoded.items[1] != null { json_null_failed() }
    "#;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let output =
        MeowMeowRunner::eval_with_runtime_spec(source, &mut world, &mut rx, None, &mut emit);
    assert!(output.errors.is_empty(), "errors: {:?}", output.errors);
}

#[test]
fn runtime_spec_file_read_text_is_a_host_api() {
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_runtime_spec(
        r#"
            let text = File.read_text("examples/data/bar-samples.json")
            let records = JSON.parse(text)
            if len(records) == 0 { file_read_text_failed() }
        "#,
        &mut world,
        &mut rx,
        None,
        &mut emit,
    );
    assert!(output.errors.is_empty(), "errors: {:?}", output.errors);
}

#[test]
fn data_viz_json_file_demo_loads_three_fixture_bars() {
    let path = repo_path("examples/data-viz-json-file.mms");
    let source = fs::read_to_string(&path).expect("JSON data-viz example source");
    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut emit = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_runtime_spec_at_path(
        &source,
        path.to_str(),
        &mut world,
        &mut systems.rx,
        Some(&mut assets),
        &mut emit,
    );
    assert!(output.errors.is_empty(), "errors: {:?}", output.errors);
    let stars = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<RenderableComponent>(id)
                .is_some_and(|renderable| {
                    matches!(
                        renderable.authored_shape,
                        Some(AuthoredRenderableShape::Star { .. })
                    )
                })
        })
        .count();
    assert!(stars > 0, "star background should materialize renderables");
    let bars = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("bar_chart"))
        .expect("bar chart container");
    let attach_count = output
        .intents
        .iter()
        .filter(|intent| matches!(intent, IntentValue::Attach { parent, .. } if *parent == bars))
        .count();
    assert_eq!(attach_count, 3, "one Attach intent per JSON record");

    for intent in output.intents {
        emit.push_intent_now(ComponentId::default(), intent);
    }
    let mut visuals = VisualWorld::default();
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut emit);
    systems.tick(
        &mut world,
        &mut visuals,
        &mut assets,
        &InputState::default(),
        &mut emit,
        1.0 / 60.0,
    );
    systems.tick(
        &mut world,
        &mut visuals,
        &mut assets,
        &InputState::default(),
        &mut emit,
        1.0 / 60.0,
    );

    let bar_items: Vec<ComponentId> = world
        .children_of(bars)
        .iter()
        .copied()
        .filter(|&child| {
            world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_some()
                && world.children_of(child).iter().any(|&metadata| {
                    world
                        .get_component_by_id_as::<StyleComponent>(metadata)
                        .is_some_and(|style| {
                            matches!(
                                style.display,
                                Some(crate::engine::ecs::component::style::Display::InlineBlock)
                            )
                        })
                })
        })
        .collect();
    assert_eq!(bar_items.len(), 3);

    let mut effective_bounds = Vec::new();
    for bar in bar_items {
        let content = world
            .children_of(bar)
            .iter()
            .find_map(|&child| world.get_component_by_id_as::<LayoutBoundsComponent>(child))
            .expect("bar content bounds")
            .content_local;
        let (visual, placement) = world
            .children_of(bar)
            .iter()
            .copied()
            .filter(|&child| {
                world
                    .get_component_by_id_as::<TransformComponent>(child)
                    .is_some()
            })
            .find_map(|visual| {
                world.children_of(visual).iter().find_map(|&child| {
                    world
                        .get_component_by_id_as::<LayoutVisualPlacementComponent>(child)
                        .copied()
                        .map(|placement| (visual, placement))
                })
            })
            .expect("bar visual placement");
        let bar_y = world
            .get_component_by_id_as::<TransformComponent>(bar)
            .expect("bar transform")
            .transform
            .translation[1];
        let source = placement.source_bounds_parent_local;
        let effective = crate::engine::graphics::bounds::Aabb {
            min: [
                source.min[0] + placement.translation_parent_local[0],
                bar_y + source.min[1] + placement.translation_parent_local[1],
                source.min[2] + placement.translation_parent_local[2],
            ],
            max: [
                source.max[0] + placement.translation_parent_local[0],
                bar_y + source.max[1] + placement.translation_parent_local[1],
                source.max[2] + placement.translation_parent_local[2],
            ],
        };
        assert!((effective.center()[0] - content.center()[0]).abs() < 1e-5);
        assert_eq!(
            world
                .get_component_by_id_as::<TransformComponent>(visual)
                .unwrap()
                .transform
                .translation,
            [0.0, 0.0, 0.0],
            "placement must not rewrite authored visual translation"
        );
        effective_bounds.push(effective);
    }

    let baseline = effective_bounds[0].min[1];
    assert!(
        effective_bounds
            .iter()
            .all(|bounds| (bounds.min[1] - baseline).abs() < 1e-5),
        "bar bounds were not bottom-aligned: {effective_bounds:?}"
    );
    assert!(effective_bounds[0].max[1] < effective_bounds[2].max[1]);
    assert!(effective_bounds[2].max[1] < effective_bounds[1].max[1]);
}

#[test]
fn planar_transparency_demo_runtime_spec_materializes_layout_loops() {
    let path = repo_path("examples/planar-auto-transparency-optimization.mms");
    let source = fs::read_to_string(&path).expect("planar transparency example source");
    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut emit = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_runtime_spec_at_path(
        &source,
        path.to_str(),
        &mut world,
        &mut systems.rx,
        Some(&mut assets),
        &mut emit,
    );
    assert!(output.errors.is_empty(), "errors: {:?}", output.errors);

    let layout = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("planar_auto_transparency_benchmark"))
        .expect("benchmark LayoutRoot");
    assert_eq!(world.children_of(layout).len(), 24, "one child per row");

    let mut rows = 0;
    let mut cells = 0;
    let mut translucent_cell_styles = 0;
    let mut opaque_cell_cubes = 0;
    for &row in world.children_of(layout) {
        if world
            .get_component_by_id_as::<TransformComponent>(row)
            .is_none()
        {
            continue;
        }
        rows += 1;
        for &child in world.children_of(row) {
            if world
                .get_component_by_id_as::<TransformComponent>(child)
                .is_none()
            {
                continue;
            }
            cells += 1;
            translucent_cell_styles += world
                .children_of(child)
                .iter()
                .filter(|&&metadata| {
                    world
                        .get_component_by_id_as::<StyleComponent>(metadata)
                        .is_some_and(|style| style.background_color.is_some())
                })
                .count();
            opaque_cell_cubes += world
                .children_of(child)
                .iter()
                .copied()
                .filter(|&visual_root| {
                    world
                        .get_component_by_id_as::<TransformComponent>(visual_root)
                        .is_some()
                })
                .flat_map(|visual_root| world.children_of(visual_root).iter().copied())
                .filter(|&renderable| {
                    world
                        .get_component_by_id_as::<RenderableComponent>(renderable)
                        .is_some_and(|component| {
                            matches!(
                                component.authored_shape,
                                Some(AuthoredRenderableShape::Builtin("cube"))
                            )
                        })
                })
                .count();
        }
    }
    assert_eq!(rows, 24);
    assert_eq!(cells, 576);
    assert_eq!(translucent_cell_styles, 576);
    assert_eq!(opaque_cell_cubes, 576);

    for intent in output.intents {
        emit.push_intent_now(ComponentId::default(), intent);
    }
    let mut visuals = VisualWorld::default();
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut emit);
    for _ in 0..2 {
        systems.tick(
            &mut world,
            &mut visuals,
            &mut assets,
            &InputState::default(),
            &mut emit,
            1.0 / 60.0,
        );
    }
    let backgrounds = world
        .all_components()
        .filter(|&id| world.component_label(id) == Some("__bg"))
        .count();
    assert_eq!(backgrounds, 576, "one layout-owned background per cell");
}

#[test]
fn runtime_spec_json_host_api_reports_parse_and_encode_errors() {
    for (source, expected) in [
        ("JSON.parse(\"{bad}\")", "JSON.parse"),
        ("JSON.stringify(fn() {})", "functions are not JSON values"),
    ] {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut emit = CommandQueue::new();
        let output =
            MeowMeowRunner::eval_with_runtime_spec(source, &mut world, &mut rx, None, &mut emit);
        assert!(
            output.errors.iter().any(|error| error.contains(expected)),
            "expected an error containing {expected:?}, got {:?}",
            output.errors
        );
    }
}

#[test]
fn runtime_spec_background_color_rgba_creates_its_color_child() {
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_runtime_spec(
        "BGC.rgba(0.1, 0.2, 0.3, 0.4)",
        &mut world,
        &mut rx,
        None,
        &mut emit,
    );
    assert!(output.errors.is_empty(), "errors: {:?}", output.errors);
    let background = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::BackgroundColorComponent>(
                    id,
                )
                .is_some()
        })
        .expect("BackgroundColor root");
    let color = *world.children_of(background).first().expect("Color child");
    assert_eq!(
        world
            .get_component_by_id_as::<crate::engine::ecs::component::ColorComponent>(color)
            .expect("Color component")
            .rgba,
        [0.1, 0.2, 0.3, 0.4]
    );
}

#[test]
fn runtime_spec_style_supports_individual_margin_sides() {
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_runtime_spec(
        "Style { margin_top(1) margin_right(2) margin_bottom(3) margin_left(4) }",
        &mut world,
        &mut rx,
        None,
        &mut emit,
    );
    assert!(output.errors.is_empty(), "errors: {:?}", output.errors);
    let style = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<StyleComponent>(id))
        .expect("Style component");
    assert_eq!(
        style.margin,
        crate::engine::ecs::component::EdgeInsets {
            top: SizeDimension::GlyphUnits(1.0),
            right: SizeDimension::GlyphUnits(2.0),
            bottom: SizeDimension::GlyphUnits(3.0),
            left: SizeDimension::GlyphUnits(4.0),
        }
    );
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
fn live_animation_step_methods_emit_directional_intents() {
    use crate::engine::ecs::component::AnimationStepDirection;

    let src = r##"
        let slides = Animation.paused() {}
        slides.next()
        slides.previous()
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let directions: Vec<_> = out
        .intents
        .iter()
        .filter_map(|intent| match intent {
            IntentValue::StepAnimation { direction, .. } => Some(*direction),
            _ => None,
        })
        .collect();
    assert_eq!(
        directions,
        vec![
            AnimationStepDirection::Next,
            AnimationStepDirection::Previous
        ]
    );
}

#[test]
fn live_animation_next_executes_through_the_command_pipeline() {
    let src = r##"
        let label = Text { "before" }
        label

        let slides = Animation.paused() {
            Keyframe.at(0) { label.set_text("first") }
            Keyframe.at(1) { label.set_text("second") }
        }
        slides
        slides.next()
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
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    let label = world
        .all_components()
        .find_map(|id| {
            world.get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(id)
        })
        .expect("label text component");
    assert_eq!(label.text, "first");
}

#[test]
fn xr_button_handler_can_step_a_paused_animation() {
    use crate::engine::ecs::component::{ControllerHand, InputXRGamepadComponent, XrButtonControl};

    let src = r##"
        let label = Text { "before" }
        label

        let slides = Animation.paused() {
            Keyframe.at(0) { label.set_text("first") }
        }
        slides

        let controls = InputXRGamepad {}
        controls
        on(controls, "XrButtonDown", fn(event) {
            if event.control == "ButtonB" {
                slides.next()
            }
        })
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

    let controls = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<InputXRGamepadComponent>(*id)
                .is_some()
        })
        .expect("XR gamepad component");
    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            controls,
            EventSignal::XrButtonDown {
                source_component: controls,
                hand: ControllerHand::Right,
                control: XrButtonControl::ButtonB,
                value: 1.0,
            },
        ),
    );
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
    systems
        .animation
        .tick_with_beat(&mut world, 0.0, 60.0, &mut systems.rx);
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    let label = world
        .all_components()
        .find_map(|id| {
            world.get_component_by_id_as::<crate::engine::ecs::component::TextComponent>(id)
        })
        .expect("label text component");
    assert_eq!(label.text, "first");
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

    let out = MeowMeowRunner::eval_with_world_and_assets_at_path(
        src,
        Some("examples/_mms_test_imported_factory_keyframe_live_handles.mms"),
        &mut world,
        &mut systems.rx,
        Some(&mut render_assets),
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
    // The factory attaches a one-beat Transition to every glow. The first
    // tick starts it; advance once more to sample an interpolated value.
    driver.set_time_sec(0.75);
    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &input,
        &mut queue,
        0.25,
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
                    component_id,
                    note,
                    beat_offset,
                    ..
                }) => Some((component_id.clone(), note.clone(), *beat_offset)),
                _ => None,
            },
        )
        .expect("expected AudioSchedulePlay from keyframe MusicNote");

    assert_eq!(audio.2, 0.0);
    let note = audio.1.expect("expected note payload");
    assert_eq!(note.pitch_name(), MusicNote::e(4, 0.25).pitch_name());
    assert_eq!(note.octave(), 4);
    assert!((note.duration_beats() - 0.25).abs() < 1.0e-6);
    assert!(world.get_component_record(audio.0).is_some());
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
fn live_handler_component_attach_emits_reparent_intent() {
    let src = r##"
        let child = T { name = "camera_rig" }
        let old_parent = T { name = "fixed_slot" child }
        let new_parent = T { name = "first_person_slot" }
        let btn = T { name = "toggle" }
        old_parent
        new_parent
        btn

        on(btn, "Click", fn(event) {
            new_parent.attach(child)
        })
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    let named = |label: &str| {
        world
            .all_components()
            .find(|id| world.component_label(*id) == Some(label))
            .unwrap_or_else(|| panic!("missing #{label}"))
    };
    let child = named("camera_rig");
    let new_parent = named("first_person_slot");
    let btn = named("toggle");

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            btn,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: btn,
                hit_point: [0.0, 0.0, 0.0],
                screen_pos_px: None,
            },
        ),
    );

    let intents = rx.drain_ready_intents();
    assert!(intents.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::Attach { parent, child: attached_child })
            if *parent == new_parent && *attached_child == child
    )));
}

#[test]
fn gltf_initialized_event_exposes_live_gltf_uri_and_scoped_query() {
    let src = r##"
        let avatar_gltf = GLTF.new("avatar.glb")
        let camera_slot = T { name = "camera_slot" }
        avatar_gltf
        camera_slot

        on(avatar_gltf, "GLTFInitialized", fn(event) {
            if event.uri == "avatar.glb" {
                let head = event.gltf.query("#J_Bip_C_Head")
                if head {
                    head.attach(camera_slot)
                }
            }
        })
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let named = |world: &World, label: &str| {
        world
            .all_components()
            .find(|id| world.component_label(*id) == Some(label))
            .unwrap_or_else(|| panic!("missing #{label}"))
    };
    let gltf = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(*id)
                .is_some()
        })
        .expect("avatar glTF");
    let head = world.add_component_boxed_named("J_Bip_C_Head", Box::new(TransformComponent::new()));
    world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::GLTFComponent>(gltf)
        .unwrap()
        .spawned_node_transforms = vec![head];

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            gltf,
            EventSignal::GltfInitialized {
                gltf,
                uri: "avatar.glb".to_string(),
            },
        ),
    );

    let camera_slot = named(&world, "camera_slot");
    let intents = rx.drain_ready_intents();
    assert!(intents.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::Attach { parent, child })
            if *parent == head && *child == camera_slot
    )));
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
fn accordion_example_gives_responder_text_distinct_line_boxes() {
    let source_path = repo_path("examples/accordion.mms");
    let source = fs::read_to_string(&source_path).expect("accordion example source");
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let out = MeowMeowRunner::eval_with_world_and_assets_at_path(
        &source,
        Some(source_path.to_str().unwrap()),
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut emit,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let panel = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("mms_accordion"))
        .expect("MMS accordion root");
    assert!(
        world.children_of(panel).iter().all(|id| world
            .get_component_by_id_as::<StyleComponent>(*id)
            .is_none()),
        "the draggable inner panel must not be repositioned by an outer layout pass"
    );
    let body_mount = world
        .find_component(panel, "#accordion_body_mount")
        .expect("accordion body mount");
    assert!(
        world.children_of(body_mount).iter().any(|id| world
            .get_component_by_id_as::<StyleComponent>(*id)
            .is_some()),
        "the stable body mount must participate in private-root flow"
    );
    let toggle = world
        .find_component(panel, "#accordion_toggle")
        .expect("accordion toggle");
    let toggle_style = world
        .children_of(toggle)
        .iter()
        .find_map(|id| world.get_component_by_id_as::<StyleComponent>(*id))
        .expect("accordion toggle style");
    assert_eq!(
        toggle_style.background_color,
        Some([0.95, 0.73, 0.16, 1.0]),
        "accordion toggle background must be caller-themeable"
    );

    let icon = world
        .find_component(panel, "#accordion_down_arrow_icon")
        .expect("font-independent accordion icon");
    assert!(world.children_of(icon).iter().any(|id| matches!(
        world
            .get_component_by_id_as::<RenderableComponent>(*id)
            .and_then(|renderable| renderable.authored_shape.as_ref()),
        Some(AuthoredRenderableShape::Polygon { mesh_key, .. })
            if mesh_key == "ui/accordion/down-chevron/v1"
    )));

    LayoutSystem::new().tick(&mut world, &mut emit);
    let mut layout_rx = RxWorld::default();
    emit.drain_into_rx(&mut layout_rx);
    let layout_intents = layout_rx.drain_ready_intents();
    let translation_y = |component_id| {
        layout_intents
            .iter()
            .filter_map(|signal| signal.intent.as_ref())
            .find_map(|intent| match &intent.value {
                IntentValue::UpdateTransform {
                    component_id: target,
                    translation,
                    ..
                } if *target == component_id => Some(translation[1]),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing layout transform for {component_id:?}"))
    };
    assert!(
        translation_y(body_mount) <= -3.5,
        "the body mount must flow below the 3.5 GU title bar"
    );

    for (name, expected_height) in [
        ("accordion_demo_card_heading", 1.5),
        ("accordion_demo_card_detail", 2.8),
    ] {
        let line_box = world
            .all_components()
            .find(|&id| world.component_label(id) == Some(name))
            .unwrap_or_else(|| panic!("missing #{name}"));
        let style = world
            .children_of(line_box)
            .iter()
            .find_map(|id| world.get_component_by_id_as::<StyleComponent>(*id))
            .unwrap_or_else(|| panic!("missing style below #{name}"));
        assert_eq!(
            style.height,
            SizeDimension::GlyphUnits(expected_height),
            "#{name} must reserve vertical layout space"
        );
    }
}

#[test]
fn accordion_factory_removes_body_and_emits_one_way_restore_request() {
    let src = r##"
        import { accordion, accordion_body } from "../assets/components/internal/ui/accordion.mms"

        let panel = accordion({
            root_name = "test_accordion"
            width_gu = 24.0
            unit_scale = 1.0
            background_color = [0.0, 0.1, 0.3, 1.0]
            children = [T { name = "test_title" Text { "test" } }]
            body = accordion_body(T { Text { "body" } })
        })
        panel
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let source_path = repo_path("examples/accordion-test.mms");
    let out = MeowMeowRunner::eval_with_world_and_assets_at_path(
        src,
        Some(source_path.to_str().unwrap()),
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut emit,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    let panel = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("test_accordion"))
        .expect("test accordion root");
    let layout_slot = world.parent_of(panel).expect("accordion layout slot");
    assert_eq!(
        world.component_label(layout_slot),
        Some("accordion_layout_slot")
    );
    let title_bar = world
        .find_component(panel, "#title_bar")
        .expect("accordion title bar");
    let toggle = world
        .find_component(panel, "#accordion_toggle")
        .expect("accordion toggle");
    let title = world
        .find_component(panel, "#test_title")
        .expect("caller-authored title child");
    let title_children = world.children_of(title_bar);
    assert!(
        title_children.iter().position(|id| *id == toggle)
            < title_children.iter().position(|id| *id == title),
        "the built-in toggle must precede caller title children"
    );
    let draggable = title_children
        .iter()
        .find_map(|id| {
            world.get_component_by_id_as::<crate::engine::ecs::component::DraggableComponent>(*id)
        })
        .expect("title bar draggable marker");
    assert!(matches!(
        &draggable.target,
        crate::engine::ecs::component::DraggableTarget::Explicit(
            crate::engine::ecs::component::ComponentRef::Query(selector)
        ) if selector == "../../#test_accordion"
    ));
    let toggle_icon = world
        .find_component(panel, "#accordion_toggle_icon")
        .expect("accordion toggle icon");
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(toggle_icon)
            .expect("accordion toggle icon transform")
            .transform
            .rotation,
        [0.0, 0.0, 0.0, 1.0],
        "expanded accordion glyph starts down-facing"
    );
    let mount = world
        .find_component(panel, "#accordion_body_mount")
        .expect("accordion body mount");
    let body = world
        .find_component(panel, "#accordion_body")
        .expect("initial accordion body");
    let click = || EventSignal::Click {
        raycaster: ComponentId::default(),
        renderable: toggle,
        hit_point: [0.0, 0.0, 0.0],
        screen_pos_px: None,
    };

    rx.dispatch_event_handlers(&mut world, &Signal::event(toggle, click()));
    let close_intents = rx.drain_ready_intents();
    assert!(close_intents.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::UpdateTransform { component_id, rotation_quat_xyzw, .. })
            if *component_id == toggle_icon
                && (rotation_quat_xyzw[2] + std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-4
                && (rotation_quat_xyzw[3] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-4
    )));
    assert!(close_intents.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::RemoveSubtree { component_id }) if *component_id == body
    )));
    world
        .remove_component_subtree(body)
        .expect("apply body removal for restore half of test");

    rx.begin_frame();
    let close_events = rx.drain_ready_events();
    assert!(close_events.iter().any(|signal| matches!(
        signal.event.as_ref(),
        Some(EventSignal::DataEvent { name, payload })
            if name == "AccordionMinimized" && *payload == Some(mount)
    )));

    rx.dispatch_event_handlers(&mut world, &Signal::event(toggle, click()));
    let open_intents = rx.drain_ready_intents();
    assert!(
        open_intents.iter().any(|signal| matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(IntentValue::UpdateTransform { component_id, rotation_quat_xyzw, .. })
                if *component_id == toggle_icon
                    && rotation_quat_xyzw[2].abs() < 1.0e-4
                    && (rotation_quat_xyzw[3] - 1.0).abs() < 1.0e-4
        )),
        "opening must update its own disclosure affordance: {open_intents:?}"
    );
    assert!(
        open_intents.iter().all(|signal| !matches!(
            signal.intent.as_ref().map(|intent| &intent.value),
            Some(IntentValue::RemoveSubtree { .. } | IntentValue::Attach { .. })
        )),
        "opening must not mutate or recreate the body: {open_intents:?}"
    );

    rx.begin_frame();
    let open_events = rx.drain_ready_events();
    assert!(open_events.iter().any(|signal| matches!(
        signal.event.as_ref(),
        Some(EventSignal::DataEvent { name, payload })
            if name == "AccordionRestoreRequested" && *payload == Some(mount)
    )));
    assert!(
        world.find_component(panel, "#accordion_body").is_none(),
        "accordion must not listen for or perform restoration itself"
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
fn transform_info_panel_updates_its_explicit_target_on_frame_tick() {
    let src = r##"
        import { transform_info_panel } from "../assets/components/ui/transform_info_panel.mms"

        let target = T.position(-1.234567, 0.0, 10.5) { name = "telemetry_target" }
        target
        transform_info_panel(target)
    "##;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();

    let out = MeowMeowRunner::eval_with_world_at_path(
        src,
        Some("examples/_mms_test_transform_info_panel.mms"),
        &mut world,
        &mut rx,
        &mut emit,
    );
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);
    assert!(rx.has_global_handlers(crate::engine::ecs::SignalKind::FrameTick));

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 0.1 },
        ),
    );
    let texts: Vec<_> = rx
        .drain_ready_intents()
        .into_iter()
        .filter_map(
            |signal| match signal.intent.as_ref().map(|intent| &intent.value) {
                Some(IntentValue::SetText { text, .. }) => Some(text.clone()),
                _ => None,
            },
        )
        .collect();
    assert_eq!(
        texts,
        [
            "x: -1.23457".to_string(),
            "y: 0.00000".to_string(),
            "z: 10.50000".to_string(),
        ]
    );
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
            world.get_component_by_id_as::<crate::engine::ecs::component::PoseCapturePoseComponent>(
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
            world.get_component_by_id_as::<crate::engine::ecs::component::PoseCapturePoseComponent>(
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
        Camera3DComponent, CameraXRComponent, CollisionComponent, CollisionMode, InputComponent,
        InputTransformModeComponent, InputXRComponent, RenderableComponent,
        SecondaryMotionComponent, SpotLightComponent, SpringBoneComponent, SpringColliderComponent,
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
        17
    );
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<SpringColliderComponent>(id)
            .is_some()),
        9
    );
    for chain in ids
        .iter()
        .filter_map(|id| world.get_component_by_id_as::<SpringBoneComponent>(*id))
    {
        let expected = if chain.stable_name.contains("Hair") {
            (
                vec![
                    "[name='bisket_collider_head']",
                    "[name='bisket_collider_neck']",
                    "[name='bisket_collider_upper_chest']",
                    "[name='bisket_collider_spine']",
                    "[name='bisket_colliders_hands']",
                    "[name='bisket_colliders_lower_arms']",
                    "[name='bisket_colliders_upper_arms']",
                ],
                0.015,
            )
        } else if chain.stable_name.contains("Bust") {
            (
                vec![
                    "[name='bisket_collider_upper_chest']",
                    "[name='bisket_collider_spine']",
                    "[name='bisket_colliders_hands']",
                    "[name='bisket_colliders_lower_arms']",
                    "[name='bisket_colliders_upper_arms']",
                ],
                0.025,
            )
        } else {
            (
                vec![
                    "[name='bisket_collider_spine']",
                    "[name='bisket_collider_hips']",
                    "[name='bisket_colliders_upper_legs']",
                ],
                0.0375,
            )
        };
        let actual: Vec<_> = chain
            .colliders
            .iter()
            .map(|reference| match reference {
                crate::engine::ecs::component::ComponentRef::Query(query) => query.as_str(),
                crate::engine::ecs::component::ComponentRef::Guid(_) => "<guid>",
            })
            .collect();
        assert_eq!(actual, expected.0, "{}", chain.stable_name);
        assert_eq!(chain.hit_radius, expected.1, "{}", chain.stable_name);
    }
    assert_eq!(
        world
            .get_component_by_id_as::<SpringColliderComponent>(named("bisket_collider_hips"))
            .unwrap()
            .radius,
        0.1375
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
                .any(|&id| world.component_label(id) == Some("tripod_light_housing"))
        );
        assert!(
            tree.iter()
                .any(|&id| world.component_label(id) == Some("tripod_light_rear_mount"))
        );
        assert!(
            tree.iter()
                .any(|&id| world.component_label(id) == Some("tripod_light_emissive_face"))
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
    let floor = world
        .get_component_by_id_as::<TransformComponent>(named("studio_floor"))
        .expect("studio floor transform");
    assert_eq!(floor.transform.translation[1], -0.05);
    assert_eq!(floor.transform.scale[1], 0.1);

    let avatar_tree = descendants(named("avatar_head_driver"));
    assert_eq!(
        avatar_tree
            .iter()
            .filter(|&&id| world
                .get_component_by_id_as::<CollisionComponent>(id)
                .is_some_and(|collision| collision.mode == CollisionMode::Kinematic))
            .count(),
        0
    );
    assert!(!source.contains("avatar_body_collider"));
    assert!(!source.contains("I.speed(0.0)"));

    assert_eq!(
        count(&|id| world.get_component_by_id_as::<InputComponent>(id).is_some()),
        1
    );
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<InputTransformModeComponent>(id)
            .is_some()),
        1
    );

    let locomotion_mode = world
        .children_of(named("desktop_avatar_input"))
        .iter()
        .find_map(|id| world.get_component_by_id_as::<InputTransformModeComponent>(*id))
        .expect("desktop locomotion mode");
    assert!(locomotion_mode.rotation_enabled && locomotion_mode.fps_rotation);
    assert!(locomotion_mode.translation_basis_source.is_none());
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

    let fixed_camera_slot = named("fixed_camera_slot");
    let fixed_camera_icon = named("fixed_camera_slot_icon");
    let hidden_camera_icon_parking = named("hidden_camera_icon_parking");
    let first_person_camera_slot = named("first_person_camera_slot");
    let desktop_camera_rig = named("desktop_camera_rig");
    let toggle = named("button_root");
    assert_eq!(world.parent_of(desktop_camera_rig), Some(fixed_camera_slot));
    assert_eq!(
        world.parent_of(fixed_camera_icon),
        Some(hidden_camera_icon_parking),
        "the fixed-camera handle starts hidden while the camera occupies its slot"
    );
    let first_person_slot_transform = world
        .get_component_by_id_as::<TransformComponent>(first_person_camera_slot)
        .expect("first-person camera slot transform");
    let camera_forward = crate::utils::math::quat_rotate_vec3(
        first_person_slot_transform.transform.rotation,
        [0.0, 0.0, -1.0],
    );
    let head_local_forward = [0.0, 0.0, 1.0];
    let forward_alignment = camera_forward[0] * head_local_forward[0]
        + camera_forward[1] * head_local_forward[1]
        + camera_forward[2] * head_local_forward[2];
    assert!(
        forward_alignment > 0.9999,
        "first-person camera must face head-local +Z: {camera_forward:?}"
    );
    assert!(first_person_slot_transform.transform.translation[2] > 0.0);

    let head = world.add_component_boxed_named("J_Bip_C_Head", Box::new(TransformComponent::new()));
    world
        .get_component_by_id_as_mut::<crate::engine::ecs::component::GLTFComponent>(avatar_gltf)
        .unwrap()
        .spawned_node_transforms = vec![head];
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            avatar_gltf,
            EventSignal::GltfInitialized {
                gltf: avatar_gltf,
                uri: "assets/models/bisket.glb".to_string(),
            },
        ),
    );
    let initialized = rx.drain_ready_intents();
    assert!(initialized.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::Attach { parent, child })
            if *parent == head && *child == first_person_camera_slot
    )));

    let click = || EventSignal::Click {
        raycaster: ComponentId::default(),
        renderable: toggle,
        hit_point: [0.0, 0.0, 0.0],
        screen_pos_px: None,
    };
    rx.dispatch_event_handlers(&mut world, &Signal::event(toggle, click()));
    let first_toggle = rx.drain_ready_intents();
    assert!(first_toggle.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::Attach { parent, child })
            if *parent == first_person_camera_slot && *child == desktop_camera_rig
    )));
    assert!(first_toggle.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::Attach { parent, child })
            if *parent == fixed_camera_slot && *child == fixed_camera_icon
    )));

    rx.dispatch_event_handlers(&mut world, &Signal::event(toggle, click()));
    let second_toggle = rx.drain_ready_intents();
    assert!(second_toggle.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::Attach { parent, child })
            if *parent == fixed_camera_slot && *child == desktop_camera_rig
    )));
    assert!(second_toggle.iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::Attach { parent, child })
            if *parent == hidden_camera_icon_parking && *child == fixed_camera_icon
    )));
}

#[test]
fn every_bisket_example_uses_the_canonical_model_uri() {
    fn visit(
        directory: &std::path::Path,
        checked: &mut usize,
        anime_shaded: &mut usize,
        default_shaded: &mut usize,
    ) {
        for entry in std::fs::read_dir(directory).expect("read examples directory") {
            let path = entry.expect("read examples entry").path();
            if path.is_dir() {
                visit(&path, checked, anime_shaded, default_shaded);
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("mms") {
                continue;
            }
            let Ok(source) = std::fs::read_to_string(&path) else {
                continue;
            };
            for (index, line) in source.lines().enumerate() {
                if !line.contains("assets/models/bisket") {
                    continue;
                }
                *checked += 1;
                let remainder = line.replace("assets/models/bisket.glb", "");
                assert!(
                    !remainder.contains("assets/models/bisket"),
                    "{}:{} contains a non-canonical Bisket URI: {}",
                    path.display(),
                    index + 1,
                    line.trim()
                );
            }

            let active_source = source
                .lines()
                .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
                .collect::<Vec<_>>()
                .join("\n");
            // Comparison scenes retain the shared source outside the GLTF body.
            // Their dedicated runtime tests verify Anime/Toon selection.
            if matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("shading-models.mms" | "shading-models-xr.mms")
            ) {
                *anime_shaded += 1;
                *default_shaded += 1;
                continue;
            }
            let model = "GLTF.new(\"assets/models/bisket.glb\")";
            let mut remaining = active_source.as_str();
            while let Some(model_offset) = remaining.find(model) {
                let after_model = &remaining[model_offset + model.len()..];
                let body_start = after_model
                    .find('{')
                    .expect("active Bisket GLTF example should have a component body");
                let mut depth = 0usize;
                let mut body_end = None;
                for (offset, character) in after_model[body_start..].char_indices() {
                    match character {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                body_end = Some(body_start + offset);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let body_end = body_end.expect("active Bisket GLTF body should be balanced");
                let body = &after_model[body_start + 1..body_end];
                if body.contains("bisket_anime_shading()") {
                    *anime_shaded += 1;
                } else {
                    assert_eq!(
                        path.file_name().and_then(|name| name.to_str()),
                        Some("shading-models.mms"),
                        "{} contains a Bisket GLTF without bisket_anime_shading()",
                        path.display()
                    );
                    *default_shaded += 1;
                }
                remaining = &after_model[body_end + 1..];
            }
        }
    }

    let mut checked = 0;
    let mut anime_shaded = 0;
    let mut default_shaded = 0;
    visit(
        std::path::Path::new("examples"),
        &mut checked,
        &mut anime_shaded,
        &mut default_shaded,
    );
    assert!(checked > 0, "expected Bisket example references");
    assert!(anime_shaded > 0, "expected anime-shaded Bisket examples");
    assert_eq!(default_shaded, 2);
}

#[test]
fn secondary_motion_desktop_head_camera_survives_gltf_and_avatar_initialization() {
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
    systems.tick(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &InputState::default(),
        &mut queue,
        1.0 / 60.0,
    );

    let named = |world: &World, label: &str| {
        world
            .all_components()
            .find(|id| world.component_label(*id) == Some(label))
            .unwrap_or_else(|| panic!("missing named scene node {label}"))
    };
    let head = named(&world, "J_Bip_C_Head");
    let camera_slot = named(&world, "first_person_camera_slot");
    let camera_rig = named(&world, "desktop_camera_rig");
    let avatar_driver = named(&world, "avatar_head_driver");
    let toggle = named(&world, "button_root");
    assert_eq!(world.parent_of(camera_slot), Some(head));

    systems.rx.push_event(
        toggle,
        EventSignal::Click {
            raycaster: ComponentId::default(),
            renderable: toggle,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    systems.process_signals(
        &mut world,
        &mut visuals,
        &mut render_assets,
        &mut queue,
        100_000,
    );
    queue.flush(&mut world, &mut systems, &mut visuals, &mut render_assets);
    assert_eq!(world.parent_of(camera_rig), Some(camera_slot));

    let camera_world = world
        .get_component_by_id_as::<TransformComponent>(camera_rig)
        .unwrap()
        .transform
        .matrix_world;
    let driver_world = world
        .get_component_by_id_as::<TransformComponent>(avatar_driver)
        .unwrap()
        .transform
        .matrix_world;
    let camera_forward = crate::utils::math::mat4_mul_vec4(camera_world, [0.0, 0.0, -1.0, 0.0]);
    let avatar_forward = crate::utils::math::mat4_mul_vec4(driver_world, [0.0, 0.0, -1.0, 0.0]);
    let dot = camera_forward[0] * avatar_forward[0]
        + camera_forward[1] * avatar_forward[1]
        + camera_forward[2] * avatar_forward[2];
    let lengths = (camera_forward[0] * camera_forward[0]
        + camera_forward[1] * camera_forward[1]
        + camera_forward[2] * camera_forward[2])
        .sqrt()
        * (avatar_forward[0] * avatar_forward[0]
            + avatar_forward[1] * avatar_forward[1]
            + avatar_forward[2] * avatar_forward[2])
            .sqrt();
    assert!(
        dot / lengths > 0.99,
        "head camera and avatar forward diverged: camera={camera_forward:?} avatar={avatar_forward:?}"
    );
}

#[test]
fn lights_example_materializes_all_light_types_and_labeled_targets() {
    use crate::engine::ecs::component::{
        AmbientLightComponent, Camera3DComponent, DirectionalLightComponent, EmissiveComponent,
        InputComponent, InputTransformModeComponent, PointLightComponent, SpotLightComponent,
        TextComponent,
    };

    let source = include_str!("../../examples/lights.mms");
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let mut render_assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        source,
        Some("examples/lights.mms"),
        &mut world,
        &mut rx,
        Some(&mut render_assets),
        &mut emit,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let ids: Vec<_> = world.all_components().collect();
    assert_eq!(
        ids.iter()
            .filter(|&&id| world
                .get_component_by_id_as::<AmbientLightComponent>(id)
                .is_some())
            .count(),
        1
    );
    assert_eq!(
        ids.iter()
            .filter(|&&id| world
                .get_component_by_id_as::<DirectionalLightComponent>(id)
                .is_some())
            .count(),
        1
    );
    assert_eq!(
        ids.iter()
            .filter(|&&id| world
                .get_component_by_id_as::<PointLightComponent>(id)
                .is_some())
            .count(),
        1
    );
    assert_eq!(
        ids.iter()
            .filter(|&&id| world
                .get_component_by_id_as::<SpotLightComponent>(id)
                .is_some())
            .count(),
        1
    );
    assert_eq!(
        ids.iter()
            .filter(|&&id| world
                .get_component_by_id_as::<Camera3DComponent>(id)
                .is_some())
            .count(),
        1
    );
    assert_eq!(
        ids.iter()
            .filter(|&&id| world.get_component_by_id_as::<InputComponent>(id).is_some())
            .count(),
        1
    );
    assert_eq!(
        ids.iter()
            .filter(|&&id| world
                .get_component_by_id_as::<InputTransformModeComponent>(id)
                .is_some())
            .count(),
        1
    );

    for name in [
        "ambient_target",
        "directional_target",
        "point_target",
        "spot_target",
        "ambient_fixture",
        "directional_fixture",
        "point_fixture",
        "spot_fixture",
        "lights_camera_input",
        "lights_camera_rig",
        "lights_camera",
    ] {
        assert!(
            ids.iter()
                .any(|&id| world.component_label(id) == Some(name)),
            "missing {name}"
        );
    }

    let labels: Vec<_> = ids
        .iter()
        .filter_map(|&id| world.get_component_by_id_as::<TextComponent>(id))
        .collect();
    assert_eq!(labels.len(), 4);
    for expected in [
        "AmbientLight",
        "DirectionalLight",
        "PointLight",
        "SpotLight",
    ] {
        assert!(labels.iter().any(|label| label.text == expected));
    }
    assert!(
        ids.iter()
            .filter(|&&id| world
                .get_component_by_id_as::<EmissiveComponent>(id)
                .is_some())
            .count()
            >= 12
    );
}

#[test]
fn shading_models_example_materializes_comparison_models_and_spotlights() {
    use crate::engine::ecs::component::{
        AnimeShadingComponent, GLTFComponent, GrabbableComponent, SpotLightComponent,
    };

    let source = include_str!("../../examples/shading-models.mms");
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let mut render_assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        source,
        Some("examples/shading-models.mms"),
        &mut world,
        &mut rx,
        Some(&mut render_assets),
        &mut emit,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let ids: Vec<_> = world.all_components().collect();
    assert_eq!(
        ids.iter()
            .filter(|&&id| world.get_component_by_id_as::<GLTFComponent>(id).is_some())
            .count(),
        2
    );
    for (label, expected) in [
        (
            "bisket_default_shading",
            crate::engine::ecs::component::ShadingModel::Toon,
        ),
        (
            "bisket_anime_shading",
            crate::engine::ecs::component::ShadingModel::Anime,
        ),
    ] {
        let root = ids
            .iter()
            .copied()
            .find(|&id| world.component_label(id) == Some(label))
            .unwrap();
        let gltf = world
            .children_of(root)
            .iter()
            .copied()
            .find(|&id| world.get_component_by_id_as::<GLTFComponent>(id).is_some())
            .unwrap();
        let shading = world
            .children_of(gltf)
            .iter()
            .find_map(|&id| world.get_component_by_id_as::<AnimeShadingComponent>(id))
            .unwrap();
        assert_eq!(shading.model, expected, "{label}");
    }
    assert_eq!(
        ids.iter()
            .filter(|&&id| world
                .get_component_by_id_as::<SpotLightComponent>(id)
                .is_some())
            .count(),
        2
    );
    assert_eq!(
        ids.iter()
            .filter(|&&id| world
                .get_component_by_id_as::<GrabbableComponent>(id)
                .is_some())
            .count(),
        2
    );
}

#[test]
fn tripod_light_without_a_mounted_light_has_no_emissive_face() {
    use crate::engine::ecs::component::{
        EmissiveComponent, GrabbableComponent, TransformComponent,
    };

    let source = r#"
        import { tripod_light } from "../assets/components/tripod_light.mms"
        tripod_light("empty_fixture", [0.0, 0.0, 0.0], [0.0, 1.0, -2.0])
    "#;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let mut render_assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        source,
        Some("examples/tripod-light-empty-test.mms"),
        &mut world,
        &mut rx,
        Some(&mut render_assets),
        &mut emit,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    assert!(
        world
            .all_components()
            .all(|id| world.component_label(id) != Some("tripod_light_emissive_face"))
    );
    let fixture = world
        .all_components()
        .find(|id| world.component_label(*id) == Some("empty_fixture"))
        .expect("tripod fixture");
    assert!(world.children_of(fixture).iter().any(|id| {
        world
            .get_component_by_id_as::<GrabbableComponent>(*id)
            .is_some()
    }));
    assert!(world.all_components().all(|id| {
        world
            .get_component_by_id_as::<EmissiveComponent>(id)
            .is_none()
    }));

    let leg_transforms: Vec<_> = world
        .all_components()
        .filter(|id| {
            world
                .get_component_by_id_as::<TransformComponent>(*id)
                .is_some_and(|transform| transform.transform.scale == [0.09, 0.78, 0.09])
        })
        .collect();
    assert_eq!(leg_transforms.len(), 3);
    for leg in leg_transforms {
        let model = world
            .get_component_by_id_as::<TransformComponent>(leg)
            .expect("leg transform")
            .transform
            .model;
        let mut minimum_y = f32::INFINITY;
        for x in [-0.5, 0.5] {
            for y in [-0.5, 0.5] {
                for z in [-0.5, 0.5] {
                    minimum_y =
                        minimum_y.min(crate::utils::math::mat4_mul_vec4(model, [x, y, z, 1.0])[1]);
                }
            }
        }
        assert!(minimum_y.abs() < 1e-5, "tripod foot is at y={minimum_y}");
    }

    let leg_height = 0.78 * 0.62_f32.cos() + 0.09 * 0.62_f32.sin();
    let shaft = world
        .all_components()
        .find(|id| world.component_label(*id) == Some("tripod_light_shaft"))
        .and_then(|id| world.get_component_by_id_as::<TransformComponent>(id))
        .expect("tripod shaft transform");
    assert!((shaft.transform.translation[1] - (leg_height + 2.25 / 2.0)).abs() < 1e-5);
    assert!((shaft.transform.translation[1] - 2.25 / 2.0 - leg_height).abs() < 1e-5);

    let rear_mount = world
        .all_components()
        .find(|id| world.component_label(*id) == Some("tripod_light_rear_mount"))
        .expect("tripod rear mount");
    let head = world.parent_of(rear_mount).expect("tripod rotating head");
    let head = world
        .get_component_by_id_as::<TransformComponent>(head)
        .expect("tripod head transform");
    assert!((head.transform.translation[1] - (leg_height + 2.25)).abs() < 1e-5);
}

#[test]
fn tripod_light_factory_marks_fixture_grabbable() {
    use crate::engine::ecs::component::GrabbableComponent;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let mut render_assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        r#"
            import { tripod_light } from "../assets/components/tripod_light.mms"
            tripod_light("grabbable_fixture", [0, 0, 0], [0, 1, -2])
        "#,
        Some("examples/tripod-light-grabbable-test.mms"),
        &mut world,
        &mut rx,
        Some(&mut render_assets),
        &mut emit,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    let fixture = world
        .all_components()
        .find(|id| world.component_label(*id) == Some("grabbable_fixture"))
        .expect("tripod fixture");
    assert_eq!(
        world
            .children_of(fixture)
            .iter()
            .filter(|id| world
                .get_component_by_id_as::<GrabbableComponent>(**id)
                .is_some())
            .count(),
        1
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
            1.25 - light.position_ws[1],
            -light.position_ws[2],
        ];
        let to_target_len = (to_target[0] * to_target[0]
            + to_target[1] * to_target[1]
            + to_target[2] * to_target[2])
            .sqrt();
        let alignment = (light.direction_ws[0] * to_target[0]
            + light.direction_ws[1] * to_target[1]
            + light.direction_ws[2] * to_target[2])
            / to_target_len;
        assert!(
            alignment > 0.999,
            "spotlight misses studio target: {alignment}"
        );
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

    systems.collision_response.tick_with_queue(
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
fn mms_mount_lifecycle_handler_receives_component_payload() {
    let src = r##"
        let target = Text { "idle" name = "target" }
        let rider = Rider {}
        let mountable = Mountable {}
        T { target rider mountable }

        on(mountable, "MountStarted", fn(event) {
            target.set_text("mounted")
        })
        on(mountable, "MountEnded", fn(event) {
            target.set_text("dismounted")
        })
    "##;

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let out = MeowMeowRunner::eval_with_world(src, &mut world, &mut rx, &mut emit);
    assert!(out.errors.is_empty(), "errors: {:?}", out.errors);

    let rider = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::RiderComponent>(id)
                .is_some()
        })
        .unwrap();
    let mountable = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::MountableComponent>(id)
                .is_some()
        })
        .unwrap();

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(mountable, EventSignal::MountStarted { rider, mountable }),
    );
    assert!(rx.drain_ready_intents().iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::SetText { text, .. }) if text == "mounted"
    )));

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(mountable, EventSignal::MountEnded { rider, mountable }),
    );
    assert!(rx.drain_ready_intents().iter().any(|signal| matches!(
        signal.intent.as_ref().map(|intent| &intent.value),
        Some(IntentValue::SetText { text, .. }) if text == "dismounted"
    )));
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
    let Statement::Import { ast, items, path } = &prog[0] else {
        panic!()
    };
    assert_eq!(path, "math.mms");
    assert!(!ast);
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
fn parse_import_ast_named_and_positional() {
    let prog = parse(r#"import ast { avatar, 0 as root } from "avatar.mms""#);
    let Statement::Import { ast, items, path } = &prog[0] else {
        panic!()
    };
    assert!(*ast);
    assert_eq!(path, "avatar.mms");
    assert!(matches!(&items[0], ImportItem::Named(id) if id.0 == "avatar"));
    assert!(
        matches!(&items[1], ImportItem::PositionalAlias { index: 0, alias } if alias.0 == "root")
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
import { world_panel } from "../assets/components/internal/panels.mms"
import { inspector_panel } from "../assets/components/internal/panels.mms"

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
    let module_path = workspace_root.join("assets/components/internal/panels.mms");

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
fn moved_editor_internal_modules_materialize_from_their_runtime_paths() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let rgba = Value::Array(vec![
        Value::Number(0.2),
        Value::Number(0.3),
        Value::Number(0.4),
        Value::Number(1.0),
    ]);
    let modules = [
        (
            "assets/components/internal/panels.mms",
            "color_panel",
            vec![
                Value::String("Color".to_string()),
                rgba.clone(),
                rgba.clone(),
            ],
        ),
        (
            "assets/components/internal/panel_items.mms",
            "world_panel_status",
            vec![Value::String("ready".to_string())],
        ),
        (
            "assets/components/internal/assets_content.mms",
            "assets_content",
            vec![Value::Array(Vec::new()), rgba.clone()],
        ),
        (
            "assets/components/internal/asset_item.mms",
            "asset_item",
            vec![
                Value::String("asset".to_string()),
                Value::String("asset-key".to_string()),
                rgba.clone(),
            ],
        ),
        (
            "assets/components/internal/asset_module_header.mms",
            "asset_module_header",
            vec![Value::String("module".to_string())],
        ),
        (
            "assets/components/internal/inspector_details.mms",
            "inspector_details",
            vec![
                Value::String("Transform".to_string()),
                Value::String("component-id".to_string()),
                Value::String("guid".to_string()),
            ],
        ),
    ];

    for (relative_path, export_name, args) in modules {
        let path = workspace_root.join(relative_path);
        let component = MeowMeowRunner::materialize_mms_module_component_from_file(
            path.to_str().expect("UTF-8 module path"),
            export_name,
            args,
            None,
            None,
        )
        .unwrap_or_else(|error| panic!("{relative_path}::{export_name} must materialize: {error}"));
        assert!(!component.component_type.is_empty());
    }
}

#[test]
fn toggle_icon_factories_accept_default_and_custom_colors() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let module_path = workspace_root.join("assets/components/icons.mms");
    let module = MeowMeowRunner::load_module_file(module_path.to_str().unwrap())
        .expect("expected icon module to load");

    let on = MeowMeowRunner::call_mms_module_fn(&module, "on_icon", vec![], None, None, None)
        .expect("default on icon should materialize");
    let off = MeowMeowRunner::call_mms_module_fn(
        &module,
        "off_icon",
        vec![
            Value::Array(vec![Value::Number(0.2); 4]),
            Value::Array(vec![Value::Number(0.8); 4]),
        ],
        None,
        None,
        None,
    )
    .expect("custom-color off icon should materialize");

    assert!(matches!(on, Value::ComponentExpr(_)));
    assert!(matches!(off, Value::ComponentExpr(_)));
    let camera =
        MeowMeowRunner::call_mms_module_fn(&module, "camera_icon", vec![], None, None, None)
            .expect("camera icon should materialize");
    assert!(matches!(camera, Value::ComponentExpr(_)));
}

#[test]
fn primitives_module_spawns_wireframe_square_through_the_renderable_registry() {
    use crate::engine::ecs::component::RenderableComponent;
    use crate::engine::ecs::component::renderable::AuthoredRenderableShape;

    let module_path = repo_path("assets/components/primitives.mms");
    let module = MeowMeowRunner::load_module_file(module_path.to_str().unwrap())
        .expect("expected primitives module to load");
    let mut world = World::default();
    let mut render_assets = RenderAssets::new();
    let mut emit = CommandQueue::new();
    let root = MeowMeowRunner::spawn_mms_module_component_uninitialized_with_assets(
        &module,
        "wireframe_square",
        vec![],
        &mut world,
        Some(&mut render_assets),
        &mut emit,
    )
    .expect("wireframe square primitive should spawn");

    let renderable = world
        .children_of(root)
        .iter()
        .find_map(|child| world.get_component_by_id_as::<RenderableComponent>(*child))
        .expect("primitive should contain a renderable");
    assert_eq!(
        renderable.authored_shape,
        Some(AuthoredRenderableShape::WireframeSquare { thickness: 0.1 })
    );
}

#[test]
fn voxel_terrain_cube_xz_boundaries_land_on_whole_local_units() {
    use std::collections::HashMap;

    let module_path = repo_path("assets/components/floors/voxel_terrain.mms");
    let module = MeowMeowRunner::load_module_file(module_path.to_str().unwrap())
        .expect("expected voxel terrain module to load");
    let mut world = World::default();
    let mut emit = CommandQueue::new();
    let config = Value::Map(HashMap::from([
        ("length".to_string(), Value::Number(1.0)),
        ("width".to_string(), Value::Number(3.0)),
    ]));

    MeowMeowRunner::spawn_mms_module_component_uninitialized(
        &module,
        "voxel_terrain",
        vec![config],
        &mut world,
        &mut emit,
    )
    .expect("voxel terrain should spawn");

    let mut cube_centers: Vec<[f32; 3]> = world
        .all_components()
        .filter_map(|component_id| {
            let transform = world.get_component_by_id_as::<TransformComponent>(component_id)?;
            (transform.transform.scale == [3.0, 3.0, 3.0])
                .then_some(transform.transform.translation)
        })
        .collect();
    cube_centers.sort_by(|a, b| a[0].total_cmp(&b[0]));

    assert_eq!(cube_centers.len(), 3);
    assert_eq!(
        cube_centers
            .iter()
            .map(|center| center[0])
            .collect::<Vec<_>>(),
        vec![-2.5, 0.5, 3.5]
    );
    for center in cube_centers {
        for axis in [0, 2] {
            let cell_min = center[axis] - 1.5;
            let cell_max = center[axis] + 1.5;
            assert!(
                (cell_min - cell_min.round()).abs() < 1e-5,
                "cube axis {axis} minimum {cell_min} should be a whole local unit"
            );
            assert!(
                (cell_max - cell_max.round()).abs() < 1e-5,
                "cube axis {axis} maximum {cell_max} should be a whole local unit"
            );
        }
    }
}

#[test]
fn voxel_terrain_custom_palette_tracks_cube_height_layers() {
    use std::collections::HashMap;

    use crate::engine::ecs::component::ColorComponent;

    fn descendant_color(world: &World, root: ComponentId) -> Option<[f32; 4]> {
        let mut pending = vec![root];
        while let Some(component) = pending.pop() {
            if let Some(color) = world.get_component_by_id_as::<ColorComponent>(component) {
                return Some(color.rgba);
            }
            pending.extend(world.children_of(component).iter().copied());
        }
        None
    }

    let module_path = repo_path("assets/components/floors/voxel_terrain.mms");
    let module = MeowMeowRunner::load_module_file(module_path.to_str().unwrap())
        .expect("expected voxel terrain module to load");
    let mut world = World::default();
    let mut emit = CommandQueue::new();
    let palette: [[f32; 4]; 4] = [
        [0.11, 0.12, 0.13, 1.0],
        [0.21, 0.22, 0.23, 1.0],
        [0.31, 0.32, 0.33, 1.0],
        [0.41, 0.42, 0.43, 1.0],
    ];
    let palette_value = Value::Array(
        palette
            .iter()
            .map(|color| {
                Value::Array(
                    color
                        .iter()
                        .map(|channel| Value::Number((*channel).into()))
                        .collect(),
                )
            })
            .collect(),
    );
    let config = Value::Map(HashMap::from([
        ("length".to_string(), Value::Number(24.0)),
        ("width".to_string(), Value::Number(24.0)),
        ("palette".to_string(), palette_value),
    ]));

    MeowMeowRunner::spawn_mms_module_component_uninitialized(
        &module,
        "voxel_terrain",
        vec![config],
        &mut world,
        &mut emit,
    )
    .expect("voxel terrain should accept a custom palette");

    let cubes: Vec<_> = world
        .all_components()
        .filter_map(|component_id| {
            let transform = world.get_component_by_id_as::<TransformComponent>(component_id)?;
            (transform.transform.scale == [3.0, 3.0, 3.0])
                .then_some((component_id, transform.transform.translation[1]))
        })
        .collect();
    assert_eq!(cubes.len(), 24 * 24);

    let mut layer_counts = [0usize; 4];
    let mut grass_has_surface_variation = false;
    for (cube, center_y) in cubes {
        let cube_min_y = center_y - 1.5;
        let raw_level = (cube_min_y + 3.15) / 3.0;
        let level = raw_level.round() as usize;
        let layer = level.min(3);
        let surface_offset = (raw_level - raw_level.round()) * 3.0;
        layer_counts[layer] += 1;
        if layer == 3 {
            assert!(
                surface_offset.abs() <= 0.0501,
                "grass offset {surface_offset} should stay within ±0.05",
            );
            grass_has_surface_variation |= surface_offset.abs() > 0.0001;
        } else {
            assert!(
                surface_offset.abs() < 0.0001,
                "only grass should receive surface variation, got {surface_offset}",
            );
        }
        assert_eq!(
            descendant_color(&world, cube),
            Some(palette[layer]),
            "cube at y={center_y} should use palette layer {layer}",
        );
    }
    assert!(
        layer_counts.iter().all(|count| *count > 0),
        "expected all four terrain layers, got {layer_counts:?}",
    );
    assert!(
        grass_has_surface_variation,
        "expected subtle vertical variation on the grass plateau",
    );
}

#[test]
fn call_mms_module_fn_invokes_exported_factory_function() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let module_path = workspace_root.join("assets/components/internal/panels.mms");

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
fn humanoid_bone_map_factories_load_and_return_components() {
    let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (file, factory) in [
        ("vroid.mms", "vroid_humanoid_bone_map"),
        ("bisket.mms", "bisket_humanoid_bone_map"),
        ("pc-rei.mms", "pc_rei_humanoid_bone_map"),
    ] {
        let module_path = workspace_root
            .join("assets/components/humanoid_bone_maps")
            .join(file);
        let module = MeowMeowRunner::load_module_file(module_path.to_str().unwrap())
            .unwrap_or_else(|error| panic!("failed to load {file}: {error}"));
        let value = MeowMeowRunner::call_mms_module_fn(&module, factory, vec![], None, None, None)
            .unwrap_or_else(|error| panic!("failed to call {factory}: {error}"));
        assert!(
            matches!(value, Value::ComponentExpr(_)),
            "{factory} returned {value:?}"
        );
    }
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
        R.wireframe_sphere() {}
        R.wireframe_sphere(5, 9, 0.03) {}
        R.wireframe_icosahedron() {}
        R.wireframe_icosahedron(2, 0.75, 0.04) {}
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
    assert_eq!(world.children_of(root).len(), 9);
    use crate::engine::ecs::component::RenderableComponent;
    use crate::engine::ecs::component::renderable::AuthoredRenderableShape;
    let authored: Vec<_> = world
        .children_of(root)
        .iter()
        .filter_map(|id| {
            world
                .get_component_by_id_as::<RenderableComponent>(*id)
                .and_then(|renderable| renderable.authored_shape.clone())
        })
        .collect();
    assert!(
        authored.contains(&AuthoredRenderableShape::WireframeSphere {
            latitude_segments: 16,
            longitude_segments: 32,
            thickness: 0.02,
        })
    );
    assert!(
        authored.contains(&AuthoredRenderableShape::WireframeSphere {
            latitude_segments: 5,
            longitude_segments: 9,
            thickness: 0.03,
        })
    );
    assert!(
        authored.contains(&AuthoredRenderableShape::WireframeIcosahedron {
            tessellations: 0,
            sphericalness: 0.0,
            thickness: 0.02,
        })
    );
    assert!(
        authored.contains(&AuthoredRenderableShape::WireframeIcosahedron {
            tessellations: 2,
            sphericalness: 0.75,
            thickness: 0.04,
        })
    );
}

#[test]
fn renderable_polygon_constructs_from_nested_arrays_and_serializes_losslessly() {
    use crate::engine::ecs::component::RenderableComponent;
    use crate::engine::ecs::component::renderable::AuthoredRenderableShape;

    let points = vec![
        [-0.5, 0.25],
        [0.0, -0.25],
        [0.5, 0.25],
        [0.35, 0.4],
        [0.0, 0.05],
        [-0.35, 0.4],
    ];
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_world_and_assets(
        r#"R.polygon(
            "ui/test/chevron/v1",
            [[-0.5, 0.25], [0.0, -0.25], [0.5, 0.25], [0.35, 0.4], [0.0, 0.05], [-0.35, 0.4]],
        )"#,
        &mut world,
        &mut rx,
        &mut assets,
        &mut queue,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    let original = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<RenderableComponent>(id))
        .expect("polygon renderable");
    assert_eq!(
        original.authored_shape,
        Some(AuthoredRenderableShape::Polygon {
            mesh_key: "ui/test/chevron/v1".into(),
            points: points.clone(),
        })
    );

    let text = crate::scripting::unparser::unparse_component(&ComponentTrait::to_mms_ast(
        original, &world,
    ));
    assert!(text.contains("ui/test/chevron/v1"), "{text}");
    let parsed = parse(&text);
    let ce = as_component!(parsed.into_iter().next().unwrap());
    let materialized = crate::scripting::component_registry::ce_ast_to_materialized(&ce).unwrap();
    let mut reparsed_world = World::default();
    let id = crate::scripting::component_registry::with_live_render_assets(&mut assets, || {
        crate::scripting::component_registry::spawn_tree_uninitialized(
            &materialized,
            &mut reparsed_world,
            &mut queue,
        )
    })
    .unwrap();
    let reparsed = reparsed_world
        .get_component_by_id_as::<RenderableComponent>(id)
        .unwrap();
    assert_eq!(reparsed.authored_shape, original.authored_shape);
}

#[test]
fn renderable_polygon_reports_argument_shapes_clearly() {
    let cases = [
        (
            "R.polygon(7, [[0, 0], [1, 0], [0, 1]])",
            "mesh_key must be a string",
        ),
        (
            "R.polygon(\"ui/test/bad/v1\", [0, 1, 2])",
            "point 0 must be a [x, y] array",
        ),
        (
            "R.polygon(\"ui/test/bad/v2\", [[0, 0], [1, \"x\"], [0, 1]])",
            "point 1 y coordinate",
        ),
    ];
    for (source, expected) in cases {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut queue = CommandQueue::new();
        let mut assets = RenderAssets::new();
        let output = MeowMeowRunner::eval_with_world_and_assets(
            source,
            &mut world,
            &mut rx,
            &mut assets,
            &mut queue,
        );
        let errors = format!("{:?}", output.errors);
        assert!(
            errors.contains(expected),
            "{errors} did not contain {expected:?}"
        );
    }
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
import { world_panel_content } from "../assets/components/internal/panel_items.mms"

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
fn live_eval_table_index_returns_null_for_missing_optional_field() {
    use crate::engine::ecs::component::ColorComponent;

    let source = r#"
        let config = {
            palette = [[0.2, 0.4, 0.6, 1.0]]
        }
        let palette = config["palette"]
        let optional_width = config["width"]
        if optional_width {
            T {}
        }
        R.cube() {
            C.rgba(palette[0][0], palette[0][1], palette[0][2], palette[0][3])
        }
    "#;
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world(source, &mut world, &mut rx, &mut emit);
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    let colors: Vec<_> = world
        .all_components()
        .filter_map(|id| {
            world
                .get_component_by_id_as::<ColorComponent>(id)
                .map(|color| color.rgba)
        })
        .collect();
    assert_eq!(colors, vec![[0.2, 0.4, 0.6, 1.0]]);
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
fn roundtrip_refraction_preserves_authored_options() {
    use crate::engine::ecs::component::RefractionComponent;

    let mut original = RefractionComponent::new();
    original.apply_builder("ior", 1.45).unwrap();
    original.apply_builder("thickness", 0.08).unwrap();
    original.apply_builder("strength", 0.9).unwrap();
    original.apply_builder("edge_fade", 0.03).unwrap();

    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RefractionComponent>(id)
        .unwrap();
    assert_eq!(*got, original);
}

#[test]
fn roundtrip_rough_transmission_preserves_authored_options() {
    use crate::engine::ecs::component::RoughTransmissionComponent;

    let mut original = RoughTransmissionComponent::new();
    original.apply_builder("ior", 1.33).unwrap();
    original.apply_builder("thickness", 0.2).unwrap();
    original.apply_builder("strength", 0.75).unwrap();
    original.apply_builder("edge_fade", 0.04).unwrap();
    original.apply_builder("roughness", 0.6).unwrap();

    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RoughTransmissionComponent>(id)
        .unwrap();
    assert_eq!(*got, original);
}

#[test]
fn transmissive_examples_evaluate_with_expected_materials_and_camera_paths() {
    use crate::engine::ecs::component::{
        Camera3DComponent, CameraXRComponent, EditorPanel, EditorUIComponent, GrabbableComponent,
        RefractionComponent, RoughTransmissionComponent,
    };

    for (
        path,
        source,
        refractions,
        rough_transmissions,
        cameras_3d,
        cameras_xr,
        verify_transmissive_grabbables,
        editor_panels,
    ) in [
        (
            "examples/refraction.mms",
            include_str!("../../examples/refraction.mms"),
            5,
            0,
            1,
            0,
            true,
            &[EditorPanel::Settings, EditorPanel::Grid][..],
        ),
        (
            "examples/rough-transmission.mms",
            include_str!("../../examples/rough-transmission.mms"),
            0,
            7,
            1,
            0,
            false,
            &[][..],
        ),
        (
            "examples/transmissive-xr.mms",
            include_str!("../../examples/transmissive-xr.mms"),
            2,
            2,
            0,
            1,
            true,
            &[EditorPanel::Settings, EditorPanel::Grid][..],
        ),
        (
            "examples/rough-transmission-xr.mms",
            include_str!("../../examples/rough-transmission-xr.mms"),
            0,
            5,
            0,
            1,
            true,
            &[][..],
        ),
    ] {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut emit = CommandQueue::new();
        let mut render_assets = RenderAssets::new();
        let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
            source,
            Some(path),
            &mut world,
            &mut rx,
            Some(&mut render_assets),
            &mut emit,
        );
        assert!(output.errors.is_empty(), "{path}: {:?}", output.errors);

        assert_eq!(
            world
                .all_components()
                .filter(|id| {
                    world
                        .get_component_by_id_as::<RefractionComponent>(*id)
                        .is_some()
                })
                .count(),
            refractions,
            "{path}: refraction count",
        );
        assert_eq!(
            world
                .all_components()
                .filter(|id| {
                    world
                        .get_component_by_id_as::<RoughTransmissionComponent>(*id)
                        .is_some()
                })
                .count(),
            rough_transmissions,
            "{path}: rough transmission count",
        );
        if path == "examples/rough-transmission-xr.mms" {
            let mut roughnesses: Vec<_> = world
                .all_components()
                .filter_map(|id| {
                    world
                        .get_component_by_id_as::<RoughTransmissionComponent>(id)
                        .map(|component| component.roughness)
                })
                .collect();
            roughnesses.sort_by(f32::total_cmp);
            assert_eq!(roughnesses, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
        }
        assert_eq!(
            world
                .all_components()
                .filter(|id| {
                    world
                        .get_component_by_id_as::<Camera3DComponent>(*id)
                        .is_some()
                })
                .count(),
            cameras_3d,
            "{path}: Camera3D count",
        );
        assert_eq!(
            world
                .all_components()
                .filter(|id| {
                    world
                        .get_component_by_id_as::<CameraXRComponent>(*id)
                        .is_some()
                })
                .count(),
            cameras_xr,
            "{path}: CameraXR count",
        );
        if verify_transmissive_grabbables {
            for material in world.all_components().filter(|id| {
                world
                    .get_component_by_id_as::<RefractionComponent>(*id)
                    .is_some()
                    || world
                        .get_component_by_id_as::<RoughTransmissionComponent>(*id)
                        .is_some()
            }) {
                let mut ancestor = world.parent_of(material);
                let mut has_grabbable_ancestor = false;
                while let Some(id) = ancestor {
                    if world.children_of(id).iter().any(|child| {
                        world
                            .get_component_by_id_as::<GrabbableComponent>(*child)
                            .is_some()
                    }) {
                        has_grabbable_ancestor = true;
                        break;
                    }
                    ancestor = world.parent_of(id);
                }
                assert!(
                    has_grabbable_ancestor,
                    "{path}: transmissive material {material:?} should have a grabbable ancestor",
                );
            }
        }
        let authored_editor_panels = world.all_components().find_map(|id| {
            world
                .get_component_by_id_as::<EditorUIComponent>(id)
                .map(EditorUIComponent::panels)
        });
        if editor_panels.is_empty() {
            assert_eq!(authored_editor_panels, None, "{path}: unexpected EditorUI");
        } else {
            assert_eq!(
                authored_editor_panels.as_deref(),
                Some(editor_panels),
                "{path}: EditorUI panels",
            );
        }
    }
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
    let original = XRHandComponent::new(true, ControllerHand::Right, ControllerPoseKind::GripAim);
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<XRHandComponent>(id).unwrap();
    assert!(got.enabled);
    assert_eq!(got.hand, ControllerHand::Right);
    assert_eq!(got.pose, ControllerPoseKind::GripAim);
}

#[test]
fn roundtrip_joint_retarget_basis_preserves_five_references() {
    use crate::engine::ecs::component::{ComponentRef, JointRetargetBasisComponent};
    let query = |value: &str| ComponentRef::Query(value.into());
    let original = JointRetargetBasisComponent::new(
        query("#hand"),
        query("#middle1"),
        query("#middle3"),
        query("#little1"),
        query("#index1"),
    );
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<JointRetargetBasisComponent>(id)
        .unwrap();
    assert_eq!(got.target, query("#hand"));
    assert_eq!(got.forward_start, query("#middle1"));
    assert_eq!(got.forward_end, query("#middle3"));
    assert_eq!(got.up_start, query("#little1"));
    assert_eq!(got.up_end, query("#index1"));
}

#[test]
fn roundtrip_xr_hand_laser() {
    use crate::engine::ecs::component::{ControllerHand, ControllerPoseKind, XRHandComponent};
    let (world, id) = roundtrip_component(
        XRHandComponent::new(true, ControllerHand::Left, ControllerPoseKind::Aim).laser(),
    );
    assert!(
        world
            .get_component_by_id_as::<XRHandComponent>(id)
            .is_some_and(|hand| hand.laser)
    );
}

#[test]
fn roundtrip_rest_attachment() {
    use crate::engine::ecs::component::{ComponentRef, RestAttachmentComponent};
    let original = RestAttachmentComponent::new(
        ComponentRef::Query("[name='hand']".into()),
        ComponentRef::Query("[name='tip']".into()),
    );
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RestAttachmentComponent>(id)
        .unwrap();
    assert_eq!(got.anchor, ComponentRef::Query("[name='hand']".into()));
    assert_eq!(got.target, ComponentRef::Query("[name='tip']".into()));
}

#[test]
fn roundtrip_spring_bone_from_root_preserves_chain_tuning() {
    use crate::engine::ecs::component::{ComponentRef, SpringBoneComponent};
    let original = SpringBoneComponent::from_root(ComponentRef::Query("[name='tail']".into()))
        .virtual_end_length_ratio(1.0)
        .stiffness(2.0)
        .drag_force(0.35)
        .gravity(3.0, [0.0, -1.0, 0.0]);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<SpringBoneComponent>(id)
        .unwrap();
    assert!(matches!(
        &got.root,
        Some(ComponentRef::Query(root)) if root == "[name='tail']"
    ));
    assert_eq!(got.virtual_end_length_ratio, Some(1.0));
    assert_eq!(got.stiffness, 2.0);
    assert_eq!(got.drag_force, 0.35);
    assert_eq!(got.gravity_power, 3.0);
    assert_eq!(got.gravity_dir, [0.0, -1.0, 0.0]);
}

#[test]
fn roundtrip_spring_collision_configuration() {
    use crate::engine::ecs::component::{
        ComponentRef, SpringBoneComponent, SpringColliderComponent,
    };
    let target_a = ComponentRef::Query("[name='head']".into());
    let target_b = ComponentRef::Query("[name='neck']".into());
    let (world, id) = roundtrip_component(SpringColliderComponent::spheres(
        vec![target_a.clone(), target_b.clone()],
        0.11,
    ));
    let collider = world
        .get_component_by_id_as::<SpringColliderComponent>(id)
        .unwrap();
    assert_eq!(collider.radius, 0.11);
    assert_eq!(collider.targets, vec![target_a, target_b]);

    let original = SpringBoneComponent::from_root(ComponentRef::Query("[name='tail']".into()))
        .colliders(vec![
            ComponentRef::Query("[name='torso_colliders']".into()),
            ComponentRef::Query("[name='hips_collider']".into()),
        ])
        .hit_radius(0.03);
    let (world, id) = roundtrip_component(original);
    let chain = world
        .get_component_by_id_as::<SpringBoneComponent>(id)
        .unwrap();
    assert_eq!(chain.hit_radius, 0.03);
    assert_eq!(chain.colliders.len(), 2);
}

fn assert_xr_gamepad_locomotion_targets(world: &World, path: &str) {
    use crate::engine::ecs::component::{InputXRComponent, InputXRGamepadComponent};
    use crate::engine::ecs::system::input_xr_gamepad_system::xr_locomotion_target_transform;

    let gamepads: Vec<_> = world
        .all_components()
        .filter(|id| {
            world
                .get_component_by_id_as::<InputXRGamepadComponent>(*id)
                .is_some()
        })
        .collect();
    assert!(!gamepads.is_empty(), "{path}: missing InputXRGamepad");

    for gamepad in gamepads {
        let config = world
            .get_component_by_id_as::<InputXRGamepadComponent>(gamepad)
            .unwrap();
        assert!(config.locomotion, "{path}: gamepad locomotion is disabled");

        let mut ancestor = world.parent_of(gamepad);
        let input_xr = loop {
            let Some(component) = ancestor else {
                panic!("{path}: InputXRGamepad has no InputXR ancestor");
            };
            if world
                .get_component_by_id_as::<InputXRComponent>(component)
                .is_some()
            {
                break component;
            }
            ancestor = world.parent_of(component);
        };
        assert!(
            xr_locomotion_target_transform(world, input_xr).is_some(),
            "{path}: InputXRGamepad has no locomotion Transform above InputXR"
        );
    }
}

#[test]
fn all_bisket_secondary_motion_examples_evaluate_with_explicit_colliders() {
    use crate::engine::ecs::component::{
        ControllerXRComponent, PointerComponent, SpringBoneComponent, SpringColliderComponent,
        SpringCollidersComponent,
    };
    for (path, source, expected_chains, expected_pointer_hands) in [
        (
            "examples/secondary-motion-desktop.mms",
            include_str!("../../examples/secondary-motion-desktop.mms"),
            17,
            0,
        ),
        (
            "examples/gltf-pose-animation.mms",
            include_str!("../../examples/gltf-pose-animation.mms"),
            17,
            0,
        ),
        (
            "examples/vtuber-secondary-motion.mms",
            include_str!("../../examples/vtuber-secondary-motion.mms"),
            23,
            2,
        ),
        (
            "examples/xr-grab-demo.mms",
            include_str!("../../examples/xr-grab-demo.mms"),
            23,
            2,
        ),
        (
            "examples/vtuber-mirror-example.mms",
            include_str!("../../examples/vtuber-mirror-example.mms"),
            23,
            2,
        ),
        (
            "examples/bisket-vr-demo.mms",
            include_str!("../../examples/bisket-vr-demo.mms"),
            23,
            2,
        ),
        (
            "examples/bisket-vr-only-example.mms",
            include_str!("../../examples/bisket-vr-only-example.mms"),
            23,
            2,
        ),
        (
            "examples/input-xr-gamepad.mms",
            include_str!("../../examples/input-xr-gamepad.mms"),
            23,
            2,
        ),
        (
            "examples/vtuber-editor-example.mms",
            include_str!("../../examples/vtuber-editor-example.mms"),
            23,
            2,
        ),
    ] {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut emit = CommandQueue::new();
        let mut render_assets = RenderAssets::new();
        let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
            source,
            Some(path),
            &mut world,
            &mut rx,
            Some(&mut render_assets),
            &mut emit,
        );
        assert!(output.errors.is_empty(), "{path}: {:?}", output.errors);
        if expected_pointer_hands > 0 {
            assert_xr_gamepad_locomotion_targets(&world, path);
        }
        assert_eq!(
            world
                .all_components()
                .filter(|id| world
                    .get_component_by_id_as::<SpringBoneComponent>(*id)
                    .is_some())
                .count(),
            expected_chains,
            "{path}"
        );
        let laser_hands: Vec<_> = world
            .all_components()
            .filter(|id| {
                world
                    .get_component_by_id_as::<ControllerXRComponent>(*id)
                    .is_some_and(|controller| controller.laser)
            })
            .collect();
        assert_eq!(laser_hands.len(), expected_pointer_hands, "{path}");
        for hand in laser_hands {
            let mut pending = vec![hand];
            let mut has_pointer = false;
            while let Some(component) = pending.pop() {
                if world
                    .get_component_by_id_as::<PointerComponent>(component)
                    .is_some()
                {
                    has_pointer = true;
                    break;
                }
                pending.extend(world.children_of(component).iter().copied());
            }
            assert!(has_pointer, "{path}: XR hand is missing a Pointer child");
        }
        assert_eq!(
            world
                .all_components()
                .filter(|id| world
                    .get_component_by_id_as::<SpringColliderComponent>(*id)
                    .is_some())
                .count(),
            9,
            "{path}"
        );
        assert_eq!(
            world
                .all_components()
                .filter(|id| world
                    .get_component_by_id_as::<SpringCollidersComponent>(*id)
                    .is_some())
                .count(),
            1,
            "{path}"
        );
    }
}

#[test]
fn pc_rei_xr_examples_evaluate_with_secondary_motion_and_hand_pointers() {
    use crate::engine::ecs::component::{
        ControllerXRComponent, PointerComponent, SpringBoneComponent, SpringColliderComponent,
    };
    for (path, source) in [
        (
            "examples/vr-input.mms",
            include_str!("../../examples/vr-input.mms"),
        ),
        (
            "examples/pc-rei-mirror-example.mms",
            include_str!("../../examples/pc-rei-mirror-example.mms"),
        ),
    ] {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut emit = CommandQueue::new();
        let mut render_assets = RenderAssets::new();
        let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
            source,
            Some(path),
            &mut world,
            &mut rx,
            Some(&mut render_assets),
            &mut emit,
        );
        assert!(output.errors.is_empty(), "{path}: {:?}", output.errors);
        assert_xr_gamepad_locomotion_targets(&world, path);
        assert_eq!(
            world
                .all_components()
                .filter(|id| world
                    .get_component_by_id_as::<SpringBoneComponent>(*id)
                    .is_some())
                .count(),
            10,
            "{path}"
        );
        assert_eq!(
            world
                .all_components()
                .filter(|id| world
                    .get_component_by_id_as::<SpringColliderComponent>(*id)
                    .is_some())
                .count(),
            7,
            "{path}"
        );
        let laser_hands: Vec<_> = world
            .all_components()
            .filter(|id| {
                world
                    .get_component_by_id_as::<ControllerXRComponent>(*id)
                    .is_some_and(|controller| controller.laser)
            })
            .collect();
        assert_eq!(laser_hands.len(), 2, "{path}");
        for hand in laser_hands {
            let mut pending = vec![hand];
            assert!(
                loop {
                    let Some(component) = pending.pop() else {
                        break false;
                    };
                    if world
                        .get_component_by_id_as::<PointerComponent>(component)
                        .is_some()
                    {
                        break true;
                    }
                    pending.extend(world.children_of(component).iter().copied());
                },
                "{path}: laser-enabled PC-Rei hand is missing a Pointer child"
            );
        }
    }
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
fn roundtrip_keyframe() {
    use crate::engine::ecs::component::KeyframeComponent;
    let (world, id) = roundtrip_component(KeyframeComponent::new(4.25));
    let got = world
        .get_component_by_id_as::<KeyframeComponent>(id)
        .unwrap();
    assert!((got.beat - 4.25).abs() < 1e-9);
}

#[test]
fn roundtrip_input_speed_and_mapping_state() {
    use crate::engine::ecs::component::InputComponent;
    let (world, id) = roundtrip_component(
        InputComponent::new()
            .with_speed(0.25)
            .enabled(false)
            .with_translation_enabled(false)
            .with_rotation_enabled(false),
    );
    let got = world.get_component_by_id_as::<InputComponent>(id).unwrap();
    assert!((got.speed - 0.25).abs() < 1e-6);
    assert!(!got.enabled);
    assert!(!got.translation_enabled);
    assert!(!got.rotation_enabled);
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
fn mms_input_transform_mode_accepts_explicit_roll_axis_z() {
    use crate::engine::ecs::component::{InputTransformModeComponent, RollAxis};

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world(
        "InputTransformMode.forward_z() { roll_axis_z() }",
        &mut world,
        &mut rx,
        &mut emit,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let mode = world
        .all_components()
        .find_map(|id| {
            world
                .get_component_by_id_as::<InputTransformModeComponent>(id)
                .cloned()
        })
        .expect("materialized InputTransformModeComponent");
    assert_eq!(mode.roll_axis, RollAxis::Z);
}

#[test]
fn roundtrip_editor() {
    use crate::engine::ecs::component::{
        EditorComponent, EditorInteractionMode, TransformGizmoCoordSpace,
    };
    let original = EditorComponent::new()
        .with_interaction_mode(EditorInteractionMode::Paint)
        .with_transform_gizmo_translation_space(TransformGizmoCoordSpace::Local)
        .with_transform_gizmo_rotation_space(TransformGizmoCoordSpace::World)
        .with_panels(false)
        .with_serialize_editor_panels(true)
        .with_asset_dir("../custom-assets");
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<EditorComponent>(id).unwrap();
    assert_eq!(got.interaction_mode, EditorInteractionMode::Paint);
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
fn roundtrip_editor_ui_default_and_settings_only() {
    use crate::engine::ecs::component::{EditorPanel, EditorUIComponent, EditorUIPanelSpec};

    let (world, id) = roundtrip_component(EditorUIComponent::new());
    let got = world
        .get_component_by_id_as::<EditorUIComponent>(id)
        .unwrap();
    assert_eq!(got.panels(), EditorPanel::ALL.to_vec());

    let (world, id) = roundtrip_component(
        EditorUIComponent::new().with_panels([EditorPanel::Settings, EditorPanel::Settings]),
    );
    let got = world
        .get_component_by_id_as::<EditorUIComponent>(id)
        .unwrap();
    assert_eq!(got.panels(), vec![EditorPanel::Settings]);

    let expected = EditorUIPanelSpec::new(EditorPanel::Settings)
        .with_show_armature(false)
        .with_show_bounds(true)
        .with_show_cameras(false)
        .with_show_colliders(false)
        .with_show_gltf_colliders(true)
        .with_show_zones(true);
    let (world, id) =
        roundtrip_component(EditorUIComponent::new().with_panel_specs([expected.clone()]));
    let got = world
        .get_component_by_id_as::<EditorUIComponent>(id)
        .unwrap();
    assert_eq!(got.panel_specs(), &[expected]);
}

#[test]
fn editor_ui_panel_specs_validate_tables_configs_and_types_strictly() {
    let invalid = [
        ("EditorUI { panels([\"settings\"]) }", "must be a table"),
        ("EditorUI { panels([{}]) }", "missing required 'panel'"),
        (
            "EditorUI { panels([{ panel = \"settings\" }, { panel = \"settings\" }]) }",
            "duplicate EditorUI panel",
        ),
        (
            "EditorUI { panels([{ panel = \"settings\" nope = true }]) }",
            "unknown EditorUI panel-spec key",
        ),
        (
            "EditorUI { panels([{ panel = \"paint\" config = { nope = true } }]) }",
            "does not accept config keys",
        ),
        (
            "EditorUI { panels([{ panel = \"settings\" config = false }]) }",
            "config must be a table",
        ),
        (
            "EditorUI { panels([{ panel = \"settings\" config = { show_bounds = 1 } }]) }",
            "expects a boolean",
        ),
    ];
    for (source, expected) in invalid {
        let mut world = World::default();
        let mut rx = RxWorld::default();
        let mut emit = CommandQueue::new();
        let output = MeowMeowRunner::eval_with_world(source, &mut world, &mut rx, &mut emit);
        assert!(
            output.errors.iter().any(|error| error.contains(expected)),
            "{source}: {:?}",
            output.errors
        );
    }
}

#[test]
fn editor_ui_rejects_unknown_panel_names() {
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut emit = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world(
        "EditorUI { panels([{ panel = \"wat\" }]) }",
        &mut world,
        &mut rx,
        &mut emit,
    );
    assert!(
        output
            .errors
            .iter()
            .any(|error| error.contains("unknown EditorUI panel 'wat'")),
        "{:?}",
        output.errors
    );
}

#[test]
fn editor_ui_settings_only_materializes_under_authored_transform() {
    let source = r#"
        Editor.active() { T { name = "editable_scene" } }
        T.position(-2.25, 1.25, 0.0) {
            name = "authored_ui_position"
            EditorUI { panels([{ panel = "settings" }]) }
        }
    "#;
    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world_and_assets(
        source,
        &mut world,
        &mut systems.rx,
        &mut render_assets,
        &mut queue,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    for intent in output.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);

    let editor_ui = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::EditorUIComponent>(id)
                .is_some()
        })
        .expect("authored EditorUI");
    let mount = world
        .find_component(editor_ui, "#editor_panel_layout_mount")
        .expect("layout mount");
    let transform = world
        .get_component_by_id_as::<TransformComponent>(mount)
        .unwrap();
    assert_eq!(transform.transform.translation, [0.0, 0.0, 0.0]);
    assert_eq!(
        world.component_label(world.parent_of(editor_ui).unwrap()),
        Some("authored_ui_position")
    );
    let selectable_off_count = |world: &World| {
        world
            .children_of(editor_ui)
            .iter()
            .copied()
            .filter(|&child| {
                world
                    .get_component_by_id_as::<crate::engine::ecs::component::SelectableComponent>(
                        child,
                    )
                    .is_some_and(|selectable| !selectable.enabled)
            })
            .count()
    };
    assert_eq!(selectable_off_count(&world), 1);
    systems.register_editor_ui(
        &mut world,
        &mut visuals,
        &mut render_assets,
        editor_ui,
        &mut queue,
    );
    assert_eq!(
        selectable_off_count(&world),
        1,
        "re-registering EditorUI must not duplicate its selection marker"
    );
    assert!(
        world
            .find_component(editor_ui, "#editor_panel_layout_root")
            .is_some()
    );
    assert!(
        world
            .find_component(editor_ui, "#editor_panel_layout_selection")
            .is_some()
    );
    assert!(
        world
            .find_component(editor_ui, "#editor_settings_panel_root")
            .is_some()
    );
    for default_row in [
        "#editor_settings_armature_visibility",
        "#editor_settings_bounds_visibility",
        "#editor_settings_colliders_visibility",
        "#editor_settings_gltf_colliders_visibility",
        "#editor_settings_spring_bones_visibility",
    ] {
        assert!(
            world.find_component(editor_ui, default_row).is_some(),
            "missing default Settings row {default_row}"
        );
    }
    for omitted in [
        "#paint_panel_root",
        "#color_panel_root",
        "#grid_panel_root",
        "#pose_capture_panel_root",
        "#assets_root",
        "#world_panel_root",
        "#inspector_panel_root",
    ] {
        assert!(
            world.find_component(editor_ui, omitted).is_none(),
            "unexpected {omitted}"
        );
    }

    let panel_renderable = world
        .all_components()
        .find(|&id| {
            crate::engine::ecs::system::panel_system::is_descendant_or_self(&world, editor_ui, id)
                && world
                    .get_component_by_id_as::<crate::engine::ecs::component::RenderableComponent>(
                        id,
                    )
                    .is_some()
        })
        .expect("renderable in authored EditorUI");
    assert!(
        crate::engine::ecs::system::editor_scene_hit::resolve_world_scene_hit(
            &world,
            panel_renderable,
        )
        .is_none(),
        "authored EditorUI renderables must not resolve as editable scene hits"
    );
}

#[test]
fn editor_ui_settings_config_conditionally_authors_diagnostic_rows() {
    let source = r#"
        Editor.active() {
            T {
                name = "editable_scene"
                Zone.cube([1, 1, 1])
            }
        }
        T {
            EditorUI {
                panels([{
                    panel = "settings"
                    config = {
                        show_armature = false
                        show_bounds = true
                        show_cameras = false
                        show_colliders = false
                        show_gltf_colliders = false
                        show_spring_bones = false
                        show_zones = true
                    }
                }])
            }
        }
    "#;
    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut render_assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world_and_assets(
        source,
        &mut world,
        &mut systems.rx,
        &mut render_assets,
        &mut queue,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    for intent in output.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
    let editor_ui = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<crate::engine::ecs::component::EditorUIComponent>(*id)
                .is_some()
        })
        .unwrap();
    assert!(
        world
            .find_component(editor_ui, "#editor_settings_bounds_visibility")
            .is_some()
    );
    assert!(
        world
            .find_component(editor_ui, "#editor_settings_zones_visibility")
            .is_some()
    );
    assert!(
        !systems
            .editor_context
            .shared_state()
            .lock()
            .unwrap()
            .zones_visible,
        "show_zones authors the control; it does not force the diagnostic on"
    );
    assert!(systems.zone_visualization.requests().is_empty());
    let zones_row = world
        .find_component(editor_ui, "#editor_settings_zones_visibility")
        .expect("authored zones visibility row");
    let zones_row_renderable = world
        .all_components()
        .find(|&id| {
            crate::engine::ecs::system::panel_system::is_descendant_or_self(&world, zones_row, id)
                && world
                    .get_component_by_id_as::<crate::engine::ecs::component::RenderableComponent>(
                        id,
                    )
                    .is_some()
        })
        .expect("renderable hit target in authored zones row");
    systems.rx.push_event(
        zones_row_renderable,
        EventSignal::Click {
            raycaster: ComponentId::default(),
            renderable: zones_row_renderable,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
    assert!(
        systems
            .editor_context
            .shared_state()
            .lock()
            .unwrap()
            .zones_visible,
        "the first click should turn zone visualization on"
    );
    assert!(
        systems
            .zone_visualization
            .requests()
            .contains_key(&editor_ui),
        "the toggle should create a request owned by the authored EditorUI"
    );
    systems
        .zone_visualization
        .tick_with_queue(&mut world, &mut render_assets, &mut queue);
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
    let zone_marker = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("zone_visualization_marker"))
        .expect("marker after enabling zones");
    systems.rx.push_event(
        zones_row_renderable,
        EventSignal::Click {
            raycaster: ComponentId::default(),
            renderable: zones_row_renderable,
            hit_point: [0.0; 3],
            screen_pos_px: None,
        },
    );
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
    assert!(
        !systems
            .editor_context
            .shared_state()
            .lock()
            .unwrap()
            .zones_visible,
        "the second click should turn zone visualization off"
    );
    assert!(
        systems.zone_visualization.requests().is_empty(),
        "turning zones off must remove the authored EditorUI request"
    );
    systems
        .zone_visualization
        .tick_with_queue(&mut world, &mut render_assets, &mut queue);
    systems.process_commands(&mut world, &mut visuals, &mut render_assets, &mut queue);
    assert!(
        world.get_component_record(zone_marker).is_none(),
        "turning zones off must remove an already materialized marker"
    );
    let title_bar = world
        .find_component(editor_ui, "#title_bar")
        .expect("settings panel title bar");
    assert!(world.children_of(title_bar).iter().any(|child| {
        world
            .get_component_by_id_as::<crate::engine::ecs::component::DraggableComponent>(*child)
            .is_some_and(|draggable| {
                draggable.enabled
                    && matches!(
                        &draggable.target,
                        crate::engine::ecs::component::DraggableTarget::Explicit(_)
                    )
            })
    }));
    for omitted in [
        "#editor_settings_armature_visibility",
        "#editor_settings_cameras_visibility",
        "#editor_settings_colliders_visibility",
        "#editor_settings_gltf_colliders_visibility",
        "#editor_settings_spring_bones_visibility",
    ] {
        assert!(
            world.find_component(editor_ui, omitted).is_none(),
            "unexpected {omitted}"
        );
    }
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
        ..RaycastableComponent::default()
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
fn raycastable_drag_policy_defaults_and_roundtrip() {
    use crate::engine::ecs::component::{
        DragContinuationPolicy, DragMappingPolicy, RaycastableComponent,
    };
    let defaults = RaycastableComponent::enabled();
    assert_eq!(defaults.drag_continuation, DragContinuationPolicy::Auto);
    assert_eq!(defaults.drag_mapping, DragMappingPolicy::Auto);

    let original = defaults
        .with_drag_continuation(DragContinuationPolicy::Captured)
        .with_drag_mapping(DragMappingPolicy::StartRayPlane);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RaycastableComponent>(id)
        .unwrap();
    assert_eq!(got.drag_continuation, DragContinuationPolicy::Captured);
    assert_eq!(got.drag_mapping, DragMappingPolicy::StartRayPlane);
}

#[test]
fn raycastable_drag_policy_mms_parses_and_rejects_invalid_values() {
    use crate::engine::ecs::component::{
        DragContinuationPolicy, DragMappingPolicy, RaycastableComponent,
    };

    let spawn = |source: &str| {
        let mut program = parse(source);
        let ce = as_component!(program.remove(0));
        let materialized = crate::scripting::component_registry::ce_ast_to_materialized(&ce)
            .expect("materialize policy component");
        let mut world = World::default();
        let mut emit = CommandQueue::new();
        let result = crate::scripting::component_registry::spawn_tree_uninitialized(
            &materialized,
            &mut world,
            &mut emit,
        );
        (world, result)
    };

    let (world, result) = spawn(
        "Raycastable.drag_continuation(\"require_target_contact\").drag_mapping(\"contact_hit\") {}",
    );
    let component = world
        .get_component_by_id_as::<RaycastableComponent>(result.unwrap())
        .unwrap();
    assert_eq!(
        component.drag_continuation,
        DragContinuationPolicy::RequireTargetContact
    );
    assert_eq!(component.drag_mapping, DragMappingPolicy::ContactHit);

    assert!(
        spawn("Raycastable.drag_continuation(\"sticky\") {}")
            .1
            .is_err()
    );
    assert!(spawn("Raycastable.drag_mapping(\"screen\") {}").1.is_err());
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
    use crate::engine::ecs::component::{GridComponent, GridVisualSpace};
    let original = GridComponent::new(0.5)
        .with_size_x(24)
        .with_size_z(12)
        .with_enabled(false)
        .with_hidden(true)
        .with_selectable(false)
        .with_visual_space(GridVisualSpace::World);
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<GridComponent>(id).unwrap();
    assert!((got.spacing - 0.5).abs() < 1e-6);
    assert_eq!(got.size_x, 24);
    assert_eq!(got.size_z, 12);
    assert!(!got.enabled);
    assert!(got.hidden);
    assert!(!got.selectable);
    assert_eq!(got.visual_space, GridVisualSpace::World);
}

#[test]
fn grid_binding_live_handle_materializes_as_guid_reference() {
    use crate::engine::ecs::component::{ComponentRef, GridBindingComponent, GridComponent};

    let module = MeowMeowRunner::load_module_source(
        r#"
export fn scene() {
    let grid_transform = T {
        Grid.spacing(0.5)
    }
    return T {
        grid_transform
        T {
            GridBinding.grid(grid_transform)
        }
    }
}
"#,
        None,
    )
    .expect("load binding module");
    let mut world = World::default();
    let mut emit = CommandQueue::new();
    let root = MeowMeowRunner::spawn_mms_module_component_uninitialized(
        &module,
        "scene",
        vec![],
        &mut world,
        &mut emit,
    )
    .expect("spawn binding scene");
    let grid_component = find_first::<GridComponent>(&world, root).expect("grid component");
    let grid_transform = world.parent_of(grid_component).expect("grid transform");
    let binding_id = find_first::<GridBindingComponent>(&world, root).expect("binding");
    let binding = world
        .get_component_by_id_as::<GridBindingComponent>(binding_id)
        .unwrap();
    assert_eq!(
        binding.grid,
        ComponentRef::Guid(world.get_component_record(grid_transform).unwrap().guid)
    );
}

#[test]
fn grid_binding_roundtrip_preserves_referenced_grid_guid() {
    use crate::engine::ecs::component::{ComponentRef, GridBindingComponent, GridComponent};

    let mut world = World::default();
    let root = world.add_component(TransformComponent::new());
    let grid_transform = world.add_component(TransformComponent::new());
    let grid = world.add_component(GridComponent::new(0.5));
    world.add_child(root, grid_transform).unwrap();
    world.add_child(grid_transform, grid).unwrap();
    let grid_guid = world.get_component_record(grid_transform).unwrap().guid;
    let target = world.add_component(TransformComponent::new());
    let binding = world.add_component(GridBindingComponent::new(ComponentRef::Guid(grid_guid)));
    world.add_child(root, target).unwrap();
    world.add_child(target, binding).unwrap();

    let ce = crate::scripting::component_registry::subtree_to_ce_ast(&world, root)
        .expect("serialize binding subtree");
    let text = crate::scripting::unparser::unparse_component(&ce);
    let parsed = parse(&text);
    let parsed_ce = as_component!(parsed.into_iter().next().unwrap());
    let materialized = crate::scripting::component_registry::ce_ast_to_materialized(&parsed_ce)
        .expect("materialize binding subtree");
    let mut loaded = World::default();
    let mut emit = CommandQueue::new();
    let loaded_root = crate::scripting::component_registry::spawn_tree_uninitialized(
        &materialized,
        &mut loaded,
        &mut emit,
    )
    .expect("load binding subtree");
    let loaded_binding_id =
        find_first::<GridBindingComponent>(&loaded, loaded_root).expect("loaded binding");
    let loaded_binding = loaded
        .get_component_by_id_as::<GridBindingComponent>(loaded_binding_id)
        .unwrap();
    assert_eq!(loaded_binding.grid, ComponentRef::Guid(grid_guid));
    assert!(loaded.component_id_by_guid(grid_guid).is_some());
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
fn roundtrip_anime_shading() {
    use crate::engine::ecs::component::AnimeShadingComponent;
    let original = AnimeShadingComponent::new()
        .with_shade_color([0.61, 0.42, 0.53])
        .with_shade_strength(0.44)
        .with_shade_threshold(0.22)
        .with_lit_threshold(0.66)
        .with_rim_color([0.9, 0.8, 1.0])
        .with_rim_strength(0.2)
        .with_rim_power(3.5);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<AnimeShadingComponent>(id)
        .unwrap();
    assert_eq!(got.shade_color, [0.61, 0.42, 0.53]);
    assert!((got.shade_strength - 0.44).abs() < 1e-6);
    assert!((got.shade_threshold - 0.22).abs() < 1e-6);
    assert!((got.lit_threshold - 0.66).abs() < 1e-6);
    assert_eq!(got.rim_color, [0.9, 0.8, 1.0]);
    assert!((got.rim_strength - 0.2).abs() < 1e-6);
    assert!((got.rim_power - 3.5).abs() < 1e-6);
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
    let original = RendererSettingsComponent::msaa_off()
        .with_window_size(1920, 1080)
        .with_transmission_depth_compare(false);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RendererSettingsComponent>(id)
        .unwrap();
    assert!(!got.msaa4x);
    assert_eq!(got.window_size, Some([1920, 1080]));
    assert!(!got.transmission_depth_compare);
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
fn roundtrip_collision_shape_capsule_clamps_dimensions() {
    use crate::engine::ecs::component::{CollisionShape, CollisionShapeComponent};
    let original = CollisionShapeComponent::capsule_y(-1.5, 2.25);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<CollisionShapeComponent>(id)
        .unwrap();
    assert_eq!(got.shape, CollisionShape::capsule_y(0.0, 2.25));
}

#[test]
fn roundtrip_zone_preserves_shape_frame_roles_and_enabled_state() {
    use crate::engine::ecs::component::{CollisionShape, ComponentRef, ZoneComponent};
    let original = ZoneComponent::capsule_y(0.25, 0.75)
        .at(ComponentRef::Query("../[name='head']".to_string()))
        .role("spring_exclusion")
        .role("head")
        .enabled(false);
    let (world, id) = roundtrip_component(original);
    let got = world.get_component_by_id_as::<ZoneComponent>(id).unwrap();
    assert_eq!(got.shape, CollisionShape::capsule_y(0.25, 0.75));
    assert_eq!(
        got.frame_source,
        Some(ComponentRef::Query("../[name='head']".to_string()))
    );
    assert_eq!(got.roles, vec!["spring_exclusion", "head"]);
    assert!(!got.enabled);
}

#[test]
fn roundtrip_rider_preserves_attachment_references() {
    use crate::engine::ecs::component::{ComponentRef, RiderComponent};
    let original = RiderComponent::new()
        .anchor(ComponentRef::Query("[name='head']".into()))
        .movement_root(ComponentRef::Query("[name='root']".into()))
        .input(ComponentRef::Query("[name='locomotion']".into()))
        .enabled(false);
    let (world, id) = roundtrip_component(original.clone());
    assert_eq!(
        world.get_component_by_id_as::<RiderComponent>(id),
        Some(&original)
    );
}

#[test]
fn roundtrip_mountable_preserves_attachment_references() {
    use crate::engine::ecs::component::{ComponentRef, MountableComponent};
    let original = MountableComponent::new()
        .entry_zone(ComponentRef::Query("[name='entry']".into()))
        .mount_anchor(ComponentRef::Query("[name='seat']".into()))
        .dismount_anchor(ComponentRef::Query("[name='exit']".into()))
        .on_grip()
        .enabled(false);
    let (world, id) = roundtrip_component(original.clone());
    assert_eq!(
        world.get_component_by_id_as::<MountableComponent>(id),
        Some(&original)
    );
}

#[test]
fn zone_at_accepts_a_live_mms_component_object_as_a_durable_guid_ref() {
    use crate::engine::ecs::component::{ComponentRef, ZoneComponent};

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world_and_assets(
        r#"
            let target = T { name = "zone_target" }
            let zone = Zone.sphere(0.5).at(target) { name = "test_zone" }
            T { target zone }
        "#,
        &mut world,
        &mut rx,
        &mut assets,
        &mut queue,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let target = world
        .all_components()
        .find(|id| {
            world
                .get_component_record(*id)
                .is_some_and(|node| node.name == "zone_target")
        })
        .expect("target transform");
    let zone = world
        .all_components()
        .find_map(|id| {
            world
                .get_component_by_id_as::<ZoneComponent>(id)
                .map(|zone| (id, zone))
        })
        .expect("zone component")
        .1;
    assert_eq!(
        zone.frame_source,
        Some(ComponentRef::Guid(
            world.get_component_record(target).unwrap().guid
        ))
    );
}

#[test]
fn live_input_handles_can_relinquish_automatic_locomotion() {
    use crate::engine::ecs::component::{InputComponent, InputXRGamepadComponent};

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world_and_assets(
        r#"
            let desktop_input = I {}
            let xr_input = InputXRGamepad {}
            T { desktop_input xr_input }
            desktop_input.disable()
            xr_input.disable()
        "#,
        &mut world,
        &mut rx,
        &mut assets,
        &mut queue,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let desktop = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<InputComponent>(id))
        .expect("desktop input");
    let xr = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<InputXRGamepadComponent>(id))
        .expect("XR gamepad input");
    assert!(!desktop.enabled);
    assert!(xr.enabled, "raw XR controls stay observable");
    assert!(
        !xr.locomotion,
        "only the automatic XR movement map is disabled"
    );
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
fn roundtrip_pointer_min_grab_distance() {
    use crate::engine::ecs::component::PointerComponent;
    let (world, id) = roundtrip_component(PointerComponent::new().min_grab_distance(0.125));
    assert_eq!(
        world
            .get_component_by_id_as::<PointerComponent>(id)
            .and_then(|p| p.min_grab_distance),
        Some(0.125)
    );
}

#[test]
fn roundtrip_pointer_click_thresholds() {
    use crate::engine::ecs::component::PointerComponent;
    let pointer = PointerComponent::new()
        .click_max_screen_distance_px(14.0)
        .click_max_ray_angle_deg(4.5)
        .click_max_origin_distance(0.08);
    let (world, id) = roundtrip_component(pointer);
    let got = world
        .get_component_by_id_as::<PointerComponent>(id)
        .unwrap();
    assert_eq!(got.click_max_screen_distance_px, 14.0);
    assert_eq!(got.click_max_ray_angle_deg, 4.5);
    assert_eq!(got.click_max_origin_distance, 0.08);
}

#[test]
fn pointer_click_threshold_defaults() {
    use crate::engine::ecs::component::PointerComponent;
    let pointer = PointerComponent::default();
    assert_eq!(pointer.click_max_screen_distance_px, 8.0);
    assert_eq!(pointer.click_max_ray_angle_deg, 2.0);
    assert_eq!(pointer.click_max_origin_distance, 0.03);
    assert!(!pointer.debug_enabled);
}

#[test]
fn roundtrip_pointer_debug_enable() {
    use crate::engine::ecs::component::PointerComponent;
    let stub = World::default();
    let default_text =
        crate::scripting::unparser::unparse_component(&PointerComponent::new().to_mms_ast(&stub));
    assert!(!default_text.contains("debug_enable"));

    let (world, id) = roundtrip_component(PointerComponent::new().debug_enable(true));
    assert!(
        world
            .get_component_by_id_as::<PointerComponent>(id)
            .unwrap()
            .debug_enabled
    );
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
fn roundtrip_transform_apply_inverse_local() {
    use crate::engine::ecs::component::{ComponentRef, TransformApplyInverseLocalComponent};
    let original = TransformApplyInverseLocalComponent::new(ComponentRef::Query(
        "#cockpit_camera_offset".to_string(),
    ));
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<TransformApplyInverseLocalComponent>(id)
        .unwrap();
    assert_eq!(
        got.source,
        ComponentRef::Query("#cockpit_camera_offset".to_string())
    );
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
    let original = RayCastComponent::continuous()
        .with_min_distance(0.75)
        .with_max_distance(75.0);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<RayCastComponent>(id)
        .unwrap();
    assert_eq!(got.mode, RayCastMode::Continuous);
    assert!((got.min_distance - 0.75).abs() < 1e-6);
    assert!((got.max_distance - 75.0).abs() < 1e-6);
}

#[test]
fn roundtrip_avatar_control() {
    use crate::engine::ecs::component::{AvatarControlComponent, ComponentRef};
    let original = AvatarControlComponent::new()
        .with_forward_plus_z()
        .with_hand_rotation_smoothing(220.0)
        .with_avatar_height(1.7)
        .with_capsule_radius(0.31)
        .without_neck_pin()
        .with_mouth_open_from_amplitude(ComponentRef::Query("#voice_level".into()))
        .with_mouth_open_rms_floor(0.02)
        .unwrap()
        .with_mouth_open_rms_ceiling(0.2)
        .unwrap()
        .with_mouth_open_smoothing(12.0)
        .unwrap()
        .with_collision_disabled();
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<AvatarControlComponent>(id)
        .unwrap();
    assert!(got.forward_plus_z);
    assert_eq!(got.hand_rotation_smoothing, Some(220.0));
    assert_eq!(got.avatar_height, Some(1.7));
    assert_eq!(got.capsule_radius, 0.31);
    assert!(!got.neck_pin_enabled);
    assert!(!got.collision_enabled);
    assert_eq!(
        got.mouth_open_amplitude,
        Some(ComponentRef::Query("#voice_level".into()))
    );
    assert_eq!(got.mouth_open_rms_floor, 0.02);
    assert_eq!(got.mouth_open_rms_ceiling, 0.2);
    assert_eq!(got.mouth_open_smoothing, 12.0);
}

#[test]
fn roundtrip_amplitude_preserves_authored_source_and_window_only() {
    use crate::engine::ecs::component::{AmplitudeComponent, ComponentRef};
    let original = AmplitudeComponent::rolling_window(0.25)
        .unwrap()
        .with_source(ComponentRef::Query("#microphone".into()))
        .with_enabled(false);
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<AmplitudeComponent>(id)
        .expect("amplitude component");
    assert_eq!(got.window_sec, 0.25);
    assert!(!got.enabled);
    assert_eq!(got.source, Some(ComponentRef::Query("#microphone".into())));
    assert!(!got.retained.is_live(), "runtime sample must not serialize");
}

#[test]
fn roundtrip_humanoid_bone_map_preserves_authored_policy() {
    use crate::engine::ecs::component::{
        AuthoredSlot, ComponentRef, HumanoidBoneMapComponent, HumanoidSlot,
    };
    let original = HumanoidBoneMapComponent::new()
        .with_slot(
            HumanoidSlot::LeftHand,
            ComponentRef::Query("[name='hand_l']".into()),
        )
        .with_absent(HumanoidSlot::Neck)
        .with_automap_disabled();
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<HumanoidBoneMapComponent>(id)
        .unwrap();
    assert!(!got.automap);
    assert!(matches!(
        got.authored(HumanoidSlot::Neck),
        AuthoredSlot::Absent
    ));
    assert_eq!(
        got.authored(HumanoidSlot::LeftHand),
        AuthoredSlot::Reference(ComponentRef::Query("[name='hand_l']".into()))
    );
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
fn roundtrip_transform_gizmo_translate_plane() {
    use crate::engine::ecs::component::{
        TransformGizmoPlane, TransformGizmoTranslatePlaneComponent,
    };
    let (world, id) = roundtrip_component(TransformGizmoTranslatePlaneComponent::new(
        TransformGizmoPlane::XZ,
    ));
    let got = world
        .get_component_by_id_as::<TransformGizmoTranslatePlaneComponent>(id)
        .expect("TransformGizmoTranslatePlane component");
    assert_eq!(got.plane, TransformGizmoPlane::XZ);
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
fn roundtrip_collision_response() {
    use crate::engine::ecs::component::{
        CollisionResponseComponent, CollisionResponseMode, ComponentRef,
    };
    let original = CollisionResponseComponent::push()
        .with_push_strength(8.0)
        .with_friction(0.5)
        .with_friction_y(0.25)
        .movement_target(ComponentRef::Query("/#locomotion".to_string()));
    let (world, id) = roundtrip_component(original);
    let got = world
        .get_component_by_id_as::<CollisionResponseComponent>(id)
        .unwrap();
    assert_eq!(got.mode, CollisionResponseMode::Push);
    assert!((got.push_strength - 8.0).abs() < 1e-6);
    assert!((got.friction - 0.5).abs() < 1e-6);
    assert!((got.friction_y - 0.25).abs() < 1e-6);
    assert_eq!(
        got.movement_target_source,
        Some(ComponentRef::Query("/#locomotion".to_string()))
    );
}

#[test]
fn roundtrip_grabbable() {
    use crate::engine::ecs::component::GrabbableComponent;
    let (world, id) = roundtrip_component(GrabbableComponent::new());
    assert!(
        world
            .get_component_by_id_as::<GrabbableComponent>(id)
            .is_some()
    );

    let (world, id) = roundtrip_component(GrabbableComponent::parent());
    assert!(
        world
            .get_component_by_id_as::<GrabbableComponent>(id)
            .is_some_and(|grabbable| grabbable.move_parent)
    );

    let (world, id) = roundtrip_component(GrabbableComponent::off());
    assert!(
        world
            .get_component_by_id_as::<GrabbableComponent>(id)
            .is_some_and(|grabbable| !grabbable.enabled)
    );
}

#[test]
fn roundtrip_draggable() {
    use crate::engine::ecs::component::{
        ComponentRef, DraggableComponent, DraggablePlane, DraggableTarget,
    };
    for plane in [
        DraggablePlane::Camera,
        DraggablePlane::WorldAxes([[1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]),
    ] {
        let (world, id) = roundtrip_component(DraggableComponent::new().with_plane(plane));
        assert_eq!(
            world
                .get_component_by_id_as::<DraggableComponent>(id)
                .map(|draggable| draggable.plane),
            Some(plane)
        );
    }
    let explicit = DraggableComponent::explicit(ComponentRef::Query("../#panel".to_string()))
        .with_plane(DraggablePlane::Camera);
    let (world, id) = roundtrip_component(explicit);
    assert!(matches!(
        world.get_component_by_id_as::<DraggableComponent>(id),
        Some(DraggableComponent {
            target: DraggableTarget::Explicit(ComponentRef::Query(query)),
            plane: DraggablePlane::Camera,
            ..
        }) if query == "../#panel"
    ));
}

#[test]
fn draggable_plane_builder_accepts_object_camera_and_world_axes() {
    use crate::engine::ecs::component::{DraggableComponent, DraggablePlane};
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_world_and_assets(
        r#"
            T { Draggable.plane("object") }
            T { Draggable.plane("camera") }
            T { Draggable.plane([[1, 0, 0], [0, 0, 1]]) }
        "#,
        &mut world,
        &mut rx,
        &mut assets,
        &mut queue,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    let mut planes = world.all_components().filter_map(|id| {
        world
            .get_component_by_id_as::<DraggableComponent>(id)
            .map(|draggable| draggable.plane)
    });
    assert_eq!(planes.next(), Some(DraggablePlane::Object));
    assert_eq!(planes.next(), Some(DraggablePlane::Camera));
    assert_eq!(
        planes.next(),
        Some(DraggablePlane::WorldAxes([
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0]
        ]))
    );
}

#[test]
fn mittens_corp_desktop_evaluates_with_a_desktop_camera_and_no_xr_player_components() {
    use crate::engine::ecs::component::{
        AvatarControlComponent, Camera3DComponent, CameraXRComponent, ControllerXRComponent,
        EditorComponent, GLTFComponent, HTCEyeTrackingComponent, HumanoidBoneMapComponent,
        InputComponent, InputXRComponent, InputXRGamepadComponent, MountableComponent,
        RayCastComponent, RiderComponent, VRChatOSCEyeTrackingComponent, XREyeTrackingComponent,
        XrComponent, ZoneComponent,
    };

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (_session, output) = RuntimeSpecSession::start_at_path(
        include_str!("../../examples/mittens-corp-desktop.mms"),
        "examples/mittens-corp-desktop.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .expect("mittens-corp desktop scene should start");
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let count = |predicate: &dyn Fn(crate::engine::ecs::ComponentId) -> bool| {
        world.all_components().filter(|id| predicate(*id)).count()
    };
    assert_eq!(
        count(&|id| world.get_component_by_id_as::<InputComponent>(id).is_some()),
        1
    );
    assert_eq!(
        count(&|id| world
            .get_component_by_id_as::<Camera3DComponent>(id)
            .is_some()),
        1
    );
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<AvatarControlComponent>(id)
            .is_some()
    }));
    for absent in [
        count(&|id| world.get_component_by_id_as::<XrComponent>(id).is_some()),
        count(&|id| {
            world
                .get_component_by_id_as::<InputXRComponent>(id)
                .is_some()
        }),
        count(&|id| {
            world
                .get_component_by_id_as::<InputXRGamepadComponent>(id)
                .is_some()
        }),
        count(&|id| {
            world
                .get_component_by_id_as::<CameraXRComponent>(id)
                .is_some()
        }),
        count(&|id| {
            world
                .get_component_by_id_as::<ControllerXRComponent>(id)
                .is_some()
        }),
        count(&|id| {
            world
                .get_component_by_id_as::<XREyeTrackingComponent>(id)
                .is_some()
        }),
        count(&|id| {
            world
                .get_component_by_id_as::<VRChatOSCEyeTrackingComponent>(id)
                .is_some()
        }),
        count(&|id| {
            world
                .get_component_by_id_as::<HTCEyeTrackingComponent>(id)
                .is_some()
        }),
    ] {
        assert_eq!(absent, 0);
    }
    assert_eq!(
        count(&|id| world.get_component_by_id_as::<RiderComponent>(id).is_some()),
        1
    );
    assert_eq!(
        count(&|id| {
            world
                .get_component_by_id_as::<MountableComponent>(id)
                .is_some()
        }),
        1
    );

    let bisket = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<GLTFComponent>(id)
                .is_some_and(|gltf| gltf.uri == "assets/models/bisket.glb")
        })
        .expect("desktop scene should load Bisket");
    assert!(world.children_of(bisket).iter().any(|id| {
        world
            .get_component_by_id_as::<HumanoidBoneMapComponent>(*id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<GLTFComponent>(id)
            .is_some_and(|gltf| gltf.uri == "assets/models/car.glb")
    }));
    let car_zone = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_entry_zone"))
        .expect("desktop car prefab should expose its entry zone");
    assert!(
        world
            .get_component_by_id_as::<ZoneComponent>(car_zone)
            .is_some()
    );
    let mut ancestor = world.parent_of(car_zone);
    while ancestor.is_some_and(|id| {
        !world
            .get_component_by_id_as::<EditorComponent>(id)
            .is_some_and(|editor| editor.active)
    }) {
        ancestor = ancestor.and_then(|id| world.parent_of(id));
    }
    assert!(
        ancestor.is_some(),
        "the desktop car must be under the active editor so Show Zones can render it"
    );
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<RayCastComponent>(id)
            .is_some_and(|raycast| (raycast.min_distance - 0.75).abs() < 1e-6)
    }));
}

#[test]
fn mittens_corp_desktop_wasd_moves_its_input_driver() {
    use crate::engine::ecs::component::{InputComponent, TransformComponent};
    use winit::keyboard::Key;

    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        include_str!("../../examples/mittens-corp-desktop.mms"),
        Some("examples/mittens-corp-desktop.mms"),
        &mut world,
        &mut systems.rx,
        Some(&mut assets),
        &mut queue,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    for intent in output.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    let desktop_input = world
        .all_components()
        .find(|&id| world.get_component_by_id_as::<InputComponent>(id).is_some())
        .expect("desktop scene should create Input");
    world.init_component_tree(desktop_input, &mut queue);
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);

    let driver = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("bisket_desktop_driver"))
        .expect("desktop scene should expose an Input-controlled driver");
    assert_eq!(
        world.parent_of(driver),
        Some(desktop_input),
        "Input must directly own the transform it drives; input children: {:?}",
        world.children_of(desktop_input)
    );
    let before = world
        .get_component_by_id_as::<TransformComponent>(driver)
        .expect("desktop driver transform")
        .transform
        .translation;
    let mut input = InputState::default();
    input.keys_down.insert(Key::Character("w".into()));
    systems
        .input
        .process_input(&mut world, &input, &mut queue, 1.0);
    queue.flush(&mut world, &mut systems, &mut visuals, &mut assets);
    let after = world
        .get_component_by_id_as::<TransformComponent>(driver)
        .expect("desktop driver transform after WASD")
        .transform
        .translation;
    assert_ne!(after, before, "WASD should move the desktop driver");
}

#[test]
fn mittens_corp_evaluates_with_bisket_player_and_car_mount_fixture() {
    use crate::engine::ecs::component::{
        AmplitudeComponent, AudioInputComponent, AvatarControlComponent, CameraXRComponent,
        CollisionShape, ComponentRef, ControllerXRComponent, EditorComponent, EditorPanel,
        EditorUIComponent, GLTFComponent, HTCEyeTrackingComponent, HumanoidBoneMapComponent,
        InputXRComponent, InputXRGamepadComponent, MountableComponent, PointerComponent,
        PoseCaptureComponent, RiderComponent, SecondaryMotionComponent, ShadingComponent,
        ShadingModel, SpringColliderComponent, TransformComponent, XrAxisControl, XrButtonControl,
        ZoneComponent,
    };

    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let (mut session, output) = RuntimeSpecSession::start_at_path(
        include_str!("../../examples/mittens-corp.mms"),
        "examples/mittens-corp.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .expect("mittens-corp retained runtime should start");
    assert!(output.errors.is_empty(), "{:?}", output.errors);

    let driver = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("bisket_xr_driver"))
        .expect("mittens-corp should name its Bisket XR driver");
    let input_xr = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<InputXRComponent>(id)
                .is_some()
        })
        .expect("mittens-corp should author InputXR");
    let locomotion_root =
        crate::engine::ecs::system::input_xr_gamepad_system::xr_locomotion_target_transform(
            &world, input_xr,
        )
        .expect("InputXRGamepad should resolve the outer locomotion transform");
    assert_eq!(
        world.component_label(locomotion_root),
        Some("bisket_locomotion_root")
    );
    let rider = world
        .all_components()
        .find_map(|id| world.get_component_by_id_as::<RiderComponent>(id))
        .expect("mittens-corp should declare its Bisket Rider");
    assert!(rider.anchor.is_some());
    assert!(rider.movement_root.is_some());
    assert!(rider.input.is_some());

    let rider_anchor = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("bisket_rider_cxr_anchor"))
        .expect("Bisket should expose a rider-side CXR anchor");
    let mut rider_ancestor = world.parent_of(rider_anchor);
    while rider_ancestor.is_some() && rider_ancestor != Some(driver) {
        rider_ancestor = rider_ancestor.and_then(|id| world.parent_of(id));
    }
    assert_eq!(rider_ancestor, Some(driver));
    assert!(world.children_of(rider_anchor).iter().any(|id| {
        world
            .get_component_by_id_as::<CameraXRComponent>(*id)
            .is_some()
    }));

    let car_mount = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("left_display_car_cxr_mount"))
        .expect("car should expose its old cockpit CXR offset as a mount target");
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(car_mount)
            .unwrap()
            .translation(),
        [0.0, 4.5, -1.0]
    );
    let mountable_id = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<MountableComponent>(id)
                .is_some()
        })
        .expect("the independent car should be Mountable");
    let mountable = world
        .get_component_by_id_as::<MountableComponent>(mountable_id)
        .unwrap();
    assert!(mountable.entry_zone.is_some());
    assert!(mountable.mount_anchor.is_some());
    assert!(mountable.dismount_anchor.is_some());

    let car_zone = world
        .all_components()
        .find_map(|id| {
            world
                .get_component_by_id_as::<ZoneComponent>(id)
                .filter(|_| world.component_label(id) == Some("car_entry_zone"))
        })
        .expect("car should expose a detection-only front entry zone");
    assert_eq!(
        car_zone.shape,
        CollisionShape::cube_half_extents([4.6, 1.4, 1.4])
    );
    assert_eq!(car_zone.roles, vec!["vehicle_entry"]);
    let zone_frame = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_entry_zone_frame"))
        .unwrap();
    assert_eq!(
        car_zone.frame_source,
        Some(ComponentRef::Guid(
            world.get_component_record(zone_frame).unwrap().guid
        ))
    );
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(zone_frame)
            .unwrap()
            .translation(),
        [0.0, 1.0, -1.4],
        "the entry zone should overlap the lower front of the car while projecting along local -Z"
    );
    let dismount_anchor = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_dismount"))
        .unwrap();
    assert_eq!(
        world
            .get_component_by_id_as::<TransformComponent>(dismount_anchor)
            .unwrap()
            .translation(),
        [0.0, 2.4, -4.6]
    );

    assert_eq!(
        world
            .all_components()
            .filter(|id| world
                .get_component_by_id_as::<CameraXRComponent>(*id)
                .is_some())
            .count(),
        1
    );
    let vehicle_controls = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<InputXRGamepadComponent>(id)
                .is_some_and(|gamepad| gamepad.locomotion && gamepad.speed == 1.5)
        })
        .expect("Bisket should expose canonical vehicle controls");

    let hands: Vec<_> = world
        .all_components()
        .filter(|id| {
            world
                .get_component_by_id_as::<ControllerXRComponent>(*id)
                .is_some_and(|controller| controller.laser)
        })
        .collect();
    assert_eq!(
        hands.len(),
        2,
        "the Bisket player should retain both tracked laser hands"
    );
    for hand in hands {
        let mut pending = vec![hand];
        assert!(
            loop {
                let Some(component) = pending.pop() else {
                    break false;
                };
                if world
                    .get_component_by_id_as::<PointerComponent>(component)
                    .is_some()
                {
                    break true;
                }
                pending.extend(world.children_of(component).iter().copied());
            },
            "each XR controller must retain a pointer without model hand bones"
        );
    }

    let editor = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<EditorComponent>(*id)
                .is_some_and(|editor| editor.active)
        })
        .expect("mittens-corp should author an active editor");
    let bisket = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<GLTFComponent>(*id)
                .is_some_and(|gltf| gltf.uri == "assets/models/bisket.glb")
        })
        .expect("mittens-corp should load Bisket");
    let mut bisket_ancestor = world.parent_of(bisket);
    while bisket_ancestor.is_some() && bisket_ancestor != Some(editor) {
        bisket_ancestor = bisket_ancestor.and_then(|id| world.parent_of(id));
    }
    assert_eq!(bisket_ancestor, Some(editor));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<AvatarControlComponent>(id)
            .is_some()
    }));
    let eye_tracker = world
        .all_components()
        .find_map(|id| {
            world
                .get_component_by_id_as::<HTCEyeTrackingComponent>(id)
                .map(|tracker| (id, tracker))
        })
        .expect("mittens-corp should retain HTC eye tracking for blink closure");
    assert!(
        !eye_tracker.1.enable_pupil_direction_tracking,
        "the ambient animation, not live gaze, should own Bisket eye direction"
    );
    assert!(world.children_of(bisket).iter().any(|id| {
        world
            .get_component_by_id_as::<HumanoidBoneMapComponent>(*id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<AudioInputComponent>(id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<AmplitudeComponent>(id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<SecondaryMotionComponent>(id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<SpringColliderComponent>(id)
            .is_some()
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<ShadingComponent>(id)
            .is_some_and(|shading| shading.model == ShadingModel::Anime)
    }));
    assert!(world.children_of(bisket).iter().any(|id| {
        world
            .get_component_by_id_as::<PoseCaptureComponent>(*id)
            .is_some_and(|capture| capture.asset_name.as_deref() == Some("bisket"))
    }));

    let editor_panels = world
        .all_components()
        .find_map(|id| {
            world
                .get_component_by_id_as::<EditorUIComponent>(id)
                .map(EditorUIComponent::panels)
        })
        .expect("mittens-corp should author EditorUI");
    assert_eq!(
        editor_panels,
        vec![EditorPanel::Settings, EditorPanel::Pose]
    );

    let car_root = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("left_display_car"))
        .unwrap();
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            mountable_id,
            EventSignal::MountStarted {
                rider: ComponentId::default(),
                mountable: mountable_id,
            },
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            vehicle_controls,
            EventSignal::XrAxisChanged {
                source_component: vehicle_controls,
                hand: crate::engine::ecs::component::ControllerHand::Left,
                control: XrAxisControl::LeftStick,
                value: [0.0, 1.0],
            },
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 1.0 },
        ),
    );
    let movement_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(
        movement_output.errors.is_empty(),
        "{:?}",
        movement_output.errors
    );
    assert!(movement_output.intents.iter().any(|intent| matches!(
        intent,
        IntentValue::UpdateTransform {
            component_id,
            translation,
            ..
        } if *component_id == car_root
            && translation[1] == -0.75
            && (translation[0] - -19.0).abs() > 0.01
            && (translation[2] - -1.5).abs() > 0.01
    )));

    // Import only the car and publish its bounds without starting audio or XR.
    let car_model = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_model"))
        .unwrap();
    let car_gltf = world
        .children_of(car_model)
        .iter()
        .copied()
        .find(|&id| world.get_component_by_id_as::<GLTFComponent>(id).is_some())
        .unwrap();
    let mut gltf_system = crate::engine::ecs::system::GLTFSystem::new();
    let mut renderable_system = crate::engine::ecs::system::RenderableSystem::default();
    let mut skinned = crate::engine::ecs::system::SkinnedMeshSystem::new();
    let mut visuals = VisualWorld::default();
    gltf_system.register_component(car_gltf);
    gltf_system.tick_with_queue(
        &mut world,
        &mut visuals,
        &mut skinned,
        &mut renderable_system,
        &mut queue,
        0.0,
    );
    gltf_system.flush_mesh_imports_only(&mut assets);
    let mut pending = vec![car_model];
    while let Some(node) = pending.pop() {
        pending.extend(world.children_of(node).iter().copied());
        if world
            .get_component_by_id_as::<RenderableComponent>(node)
            .is_some()
        {
            renderable_system.register_renderable_from_world(&mut world, &mut visuals, node);
        }
    }
    struct CarUploader(u32);
    impl crate::engine::graphics::MeshUploader for CarUploader {
        fn upload_mesh(
            &mut self,
            _: &crate::engine::graphics::CpuMesh,
        ) -> Result<crate::engine::graphics::MeshHandle, Box<dyn std::error::Error>> {
            self.0 += 1;
            Ok(crate::engine::graphics::MeshHandle(self.0))
        }
    }
    renderable_system.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut CarUploader(0),
        &mut queue,
    );
    let model_bounds = crate::engine::ecs::system::bounds_system::BoundsSystem::measure_cached_renderable_subtree_bounds(&world, car_model, |_| false).unwrap();
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 0.0 },
        ),
    );
    let placement_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(
        placement_output.errors.is_empty(),
        "{:?}",
        placement_output.errors
    );
    let origin = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_laser_origin"))
        .unwrap();
    let origin_translation = placement_output
        .intents
        .iter()
        .find_map(|intent| match intent {
            IntentValue::UpdateTransform {
                component_id,
                translation,
                ..
            } if *component_id == origin => Some(*translation),
            _ => None,
        })
        .expect("model readiness should position the shared muzzle frame");
    assert!((origin_translation[2] - (model_bounds.min[2] - 0.10)).abs() < 1e-4);
    assert!(
        (origin_translation[1]
            - (model_bounds.min[1] + (model_bounds.max[1] - model_bounds.min[1]) * 0.56))
            .abs()
            < 1e-4
    );

    for control in [XrButtonControl::RightGrip, XrButtonControl::RightTrigger] {
        rx.dispatch_event_handlers(
            &mut world,
            &Signal::event(
                vehicle_controls,
                EventSignal::XrButtonDown {
                    source_component: vehicle_controls,
                    hand: crate::engine::ecs::component::ControllerHand::Right,
                    control,
                    value: 1.0,
                },
            ),
        );
    }
    let fire_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(fire_output.errors.is_empty(), "{:?}", fire_output.errors);
    assert!(fire_output.intents.iter().any(|intent| matches!(
        intent,
        IntentValue::SetAnimationState {
            state: crate::engine::ecs::component::AnimationState::Playing,
            ..
        }
    )));

    let animation_id = fire_output
        .intents
        .iter()
        .find_map(|intent| match intent {
            IntentValue::SetAnimationState { component_id, .. } => Some(*component_id),
            _ => None,
        })
        .unwrap();
    let mut animation = crate::engine::ecs::system::AnimationSystem::new();
    animation.register_animation(&mut world, animation_id);
    for keyframe in world.children_of(animation_id).to_vec() {
        if world
            .get_component_by_id_as::<crate::engine::ecs::component::KeyframeComponent>(keyframe)
            .is_some()
        {
            animation.register_keyframe(&mut world, keyframe);
        }
    }
    animation.set_animation_state(
        animation_id,
        crate::engine::ecs::component::AnimationState::Playing,
    );
    let mut animation_executor =
        crate::engine::ecs::system::animation_keyframe_evaluator::SessionCallbackExecutor {
            session: &mut session,
            render_assets: &mut assets,
            emit: &mut queue,
        };
    animation.tick_with_beat_and_executor(
        &mut world,
        0.0,
        60.0,
        &mut rx,
        Some(&mut animation_executor),
    );
    let shot_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(shot_output.errors.is_empty(), "{:?}", shot_output.errors);
    let mut shot_intents = shot_output.intents;
    shot_intents.extend(
        rx.drain_ready_intents()
            .into_iter()
            .filter_map(|signal| signal.intent.map(|intent| intent.value)),
    );
    // A square is centered in its local XY plane. After the beam root rotates
    // its Y axis onto local -Z, its 40-unit scale needs a -20-unit center
    // offset for the near edge to coincide with the shared muzzle origin.
    for (name, offset) in [("car_laser_muzzle_flash", 0.0), ("laser_beam_glow", -20.0)] {
        let effect = world
            .all_components()
            .find(|&id| world.component_label(id) == Some(name))
            .unwrap();
        let translation = shot_intents
            .iter()
            .find_map(|intent| match intent {
                IntentValue::UpdateTransform {
                    component_id,
                    translation,
                    ..
                } if *component_id == effect => Some(*translation),
                _ => None,
            })
            .expect("shot should position its effects using the live bounds-derived state");
        assert!(
            (origin_translation[2] + translation[2] - (model_bounds.min[2] - 0.10 + offset)).abs()
                < 1e-4
        );
        assert_eq!(translation[1], 0.0);
    }

    let flash = world
        .all_components()
        .find(|&id| world.component_label(id) == Some("car_laser_muzzle_flash"))
        .unwrap();
    let flash_rotation = shot_intents
        .iter()
        .find_map(|intent| match intent {
            IntentValue::UpdateTransform {
                component_id,
                rotation_quat_xyzw,
                ..
            } if *component_id == flash => Some(*rotation_quat_xyzw),
            _ => None,
        })
        .expect("shot should preserve the flash frame's firing-axis orientation");
    assert!(flash_rotation[0].abs() < 1.0e-4);
    assert!((flash_rotation[1] - 1.0).abs() < 1.0e-4);
    assert!(flash_rotation[2].abs() < 1.0e-4);
    assert!(flash_rotation[3].abs() < 1.0e-4);

    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            mountable_id,
            EventSignal::MountEnded {
                rider: ComponentId::default(),
                mountable: mountable_id,
            },
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::FrameTick { dt_sec: 1.0 },
        ),
    );
    rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            vehicle_controls,
            EventSignal::XrButtonDown {
                source_component: vehicle_controls,
                hand: crate::engine::ecs::component::ControllerHand::Right,
                control: XrButtonControl::RightTrigger,
                value: 1.0,
            },
        ),
    );
    let dismounted_output = session.service_callbacks(&mut world, &mut rx, None, &mut queue);
    assert!(
        dismounted_output.errors.is_empty(),
        "{:?}",
        dismounted_output.errors
    );
    assert!(dismounted_output.intents.iter().all(|intent| !matches!(
        intent,
        IntentValue::UpdateTransform { component_id, .. } if *component_id == car_root
    )));
    assert!(dismounted_output.intents.iter().all(|intent| !matches!(
        intent,
        IntentValue::SetAnimationState {
            state: crate::engine::ecs::component::AnimationState::Playing,
            ..
        }
    )));
}

#[test]
fn xr_grab_demo_evaluates_with_editor_settings_and_grabbable_playground() {
    use crate::engine::ecs::component::{
        ControllerXRComponent, EditorComponent, EditorPanel, EditorUIComponent, GLTFComponent,
        GrabbableComponent, InputXRGamepadComponent, PoseCapturePoseComponent,
        SecondaryMotionComponent, SpotLightComponent, SpringColliderComponent,
    };
    let mut world = World::default();
    let mut systems = crate::engine::ecs::system::SystemWorld::default();
    let mut queue = CommandQueue::new();
    let mut assets = RenderAssets::new();
    let output = MeowMeowRunner::eval_with_world_and_assets_at_path(
        include_str!("../../examples/xr-grab-demo.mms"),
        Some("examples/xr-grab-demo.mms"),
        &mut world,
        &mut systems.rx,
        Some(&mut assets),
        &mut queue,
    );
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    assert!(
        world
            .all_components()
            .filter(|id| {
                world
                    .get_component_by_id_as::<GrabbableComponent>(*id)
                    .is_some()
            })
            .count()
            >= 3
    );
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<GLTFComponent>(id)
            .is_some_and(|gltf| gltf.uri == "assets/models/bisket.glb")
    }));
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<PoseCapturePoseComponent>(id)
            .is_some_and(|pose| pose.name == "relaxed")
    }));
    assert_eq!(
        world
            .all_components()
            .filter(|id| world
                .get_component_by_id_as::<SecondaryMotionComponent>(*id)
                .is_some())
            .count(),
        1
    );
    assert_eq!(
        world
            .all_components()
            .filter(|id| world
                .get_component_by_id_as::<crate::engine::ecs::component::SpringBoneComponent>(*id)
                .is_some())
            .count(),
        23
    );
    assert_eq!(
        world
            .all_components()
            .filter(|id| world
                .get_component_by_id_as::<SpringColliderComponent>(*id)
                .is_some())
            .count(),
        9
    );
    let shirt_chains: Vec<_> = world
        .all_components()
        .filter_map(|id| {
            world.get_component_by_id_as::<crate::engine::ecs::component::SpringBoneComponent>(id)
        })
        .filter(|chain| chain.stable_name.contains("TopsUpperLeg"))
        .collect();
    assert_eq!(shirt_chains.len(), 6);
    assert!(shirt_chains.iter().all(|chain| {
        let groups: Vec<_> = chain
            .colliders
            .iter()
            .filter_map(|reference| match reference {
                crate::engine::ecs::component::ComponentRef::Query(query) => Some(query.as_str()),
                crate::engine::ecs::component::ComponentRef::Guid(_) => None,
            })
            .collect();
        groups
            == [
                "[name='bisket_collider_upper_chest']",
                "[name='bisket_collider_spine']",
                "[name='bisket_collider_hips']",
                "[name='bisket_colliders_upper_legs']",
            ]
            && chain.hit_radius == 0.02
    }));
    assert_eq!(
        world
            .all_components()
            .filter(|id| world
                .get_component_by_id_as::<SpotLightComponent>(*id)
                .is_some())
            .count(),
        3
    );
    assert!(world.all_components().any(|id| {
        world
            .get_component_by_id_as::<InputXRGamepadComponent>(id)
            .is_some_and(|gamepad| gamepad.locomotion && gamepad.speed == 1.5)
    }));
    assert_eq!(
        world
            .all_components()
            .filter(|id| world
                .get_component_by_id_as::<ControllerXRComponent>(*id)
                .is_some_and(|controller| controller.laser))
            .count(),
        2
    );
    let editor_root = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<EditorComponent>(*id)
                .is_some_and(|editor| editor.active)
        })
        .expect("xr-grab demo should author an active editor root");
    for name in ["pile_a_top", "pile_b_top", "pile_c_top"] {
        let cube = world
            .all_components()
            .find(|id| world.component_label(*id) == Some(name))
            .unwrap_or_else(|| panic!("xr-grab demo should author {name}"));
        let mut ancestor = world.parent_of(cube);
        while ancestor.is_some() && ancestor != Some(editor_root) {
            ancestor = ancestor.and_then(|id| world.parent_of(id));
        }
        assert_eq!(ancestor, Some(editor_root));
    }
    let editor_ui = world
        .all_components()
        .find(|id| {
            world
                .get_component_by_id_as::<EditorUIComponent>(*id)
                .is_some()
        })
        .expect("xr-grab demo should author its settings panel");
    let editor_ui_component = world
        .get_component_by_id_as::<EditorUIComponent>(editor_ui)
        .unwrap();
    assert_eq!(editor_ui_component.panels().len(), 1);
    assert_eq!(editor_ui_component.panels()[0], EditorPanel::Settings);
    for intent in output.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    let mut visuals = VisualWorld::default();
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
    systems.tick(
        &mut world,
        &mut visuals,
        &mut assets,
        &InputState::default(),
        &mut queue,
        1.0 / 60.0,
    );
    assert!(
        world
            .find_component(editor_ui, "#editor_panel_layout_mount")
            .is_some(),
        "active editor should materialize the authored settings panel"
    );
    assert_eq!(systems.secondary_motion.runtime_counts(), (1, 23, 23, 0, 0));
}

#[test]
fn anime_shading_controls_live_runtime_reaches_gltf_visuals_and_restores() {
    use crate::engine::ecs::component::{
        ShadingComponent, ShadingModel, SliderComponent, TextComponent,
    };
    use crate::engine::ecs::system::SystemWorld;
    use crate::engine::graphics::{CpuMesh, MaterialHandle, MeshHandle, MeshUploader};

    #[derive(Default)]
    struct Uploader(u32);
    impl MeshUploader for Uploader {
        fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
            self.0 += 1;
            Ok(MeshHandle(self.0))
        }
    }
    fn label(world: &World, name: &str) -> ComponentId {
        world
            .all_components()
            .find(|&id| world.component_label(id) == Some(name))
            .unwrap()
    }
    fn service(
        session: &mut RuntimeSpecSession,
        world: &mut World,
        systems: &mut SystemWorld,
        visuals: &mut VisualWorld,
        assets: &mut RenderAssets,
        queue: &mut CommandQueue,
    ) -> usize {
        let output = session.service_callbacks(world, &mut systems.rx, Some(assets), queue);
        assert!(output.errors.is_empty(), "{:?}", output.errors);
        let edits = output
            .intents
            .iter()
            .filter(|i| matches!(i, IntentValue::RegisterAnimeShading { .. }))
            .count();
        for intent in output.intents {
            queue.push_intent_now(ComponentId::default(), intent);
        }
        systems.process_commands(world, visuals, assets, queue);
        edits
    }

    let source = format!(
        "{}\nT {{ R.cube() {{ name = \"isolated_anime\" Shading.anime().shade_strength(0.23) }} }}",
        include_str!("../../examples/shading-models.mms")
    );
    let mut world = World::default();
    let mut systems = SystemWorld::default();
    let mut visuals = VisualWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let (mut session, output) = RuntimeSpecSession::start_at_path(
        &source,
        "examples/shading-models.mms",
        &mut world,
        &mut systems.rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    for intent in output.intents {
        queue.push_intent_now(ComponentId::default(), intent);
    }
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
    let source_id = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<ShadingComponent>(id)
                .is_some_and(|s| s.model == ShadingModel::Anime && s.shade_strength == 0.5)
        })
        .unwrap();
    let slider = label(&world, "anime_shade_strength_slider");

    // Update via the actual prefab callback before GLTF creates its projections.
    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(slider, EventSignal::SliderChanged { slider, value: 0.8 }),
    );
    assert_eq!(
        service(
            &mut session,
            &mut world,
            &mut systems,
            &mut visuals,
            &mut assets,
            &mut queue
        ),
        1
    );
    assert_eq!(
        world
            .get_component_by_id_as::<ShadingComponent>(source_id)
            .unwrap()
            .shade_strength,
        0.8
    );
    systems.gltf.tick_with_queue(
        &mut world,
        &mut visuals,
        &mut systems.skinned_mesh,
        &mut systems.renderable,
        &mut queue,
        0.0,
    );
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
    systems.gltf.flush_mesh_imports_only(&mut assets);
    systems.renderable.flush_pending(
        &mut world,
        &mut visuals,
        &mut assets,
        &mut Uploader::default(),
        &mut queue,
    );
    let projections: Vec<_> = world
        .all_components()
        .filter(|&id| {
            world
                .get_component_by_id_as::<ShadingComponent>(id)
                .is_some_and(|s| s.source_component() == Some(source_id))
        })
        .collect();
    assert!(projections.len() > 1);
    let handles: Vec<_> = projections
        .iter()
        .map(|&id| {
            world
                .get_component_by_id_as::<RenderableComponent>(world.parent_of(id).unwrap())
                .unwrap()
                .get_handle()
                .unwrap()
        })
        .collect();
    assert!(
        handles
            .iter()
            .any(|&h| visuals.instance(h).unwrap().renderable.material
                == MaterialHandle::SKINNED_ANIME_MESH)
    );
    for &h in &handles {
        assert_eq!(
            visuals
                .instance(h)
                .unwrap()
                .anime_shading
                .shade_color_strength[3],
            0.8
        );
    }
    let isolated = label(&world, "isolated_anime");
    let isolated_handle = world
        .get_component_by_id_as::<RenderableComponent>(isolated)
        .unwrap()
        .get_handle()
        .unwrap();

    for value in [0.0, 1.0, 2.0, -1.0, 0.73] {
        systems.rx.dispatch_event_handlers(
            &mut world,
            &Signal::event(slider, EventSignal::SliderChanged { slider, value }),
        );
        assert_eq!(
            service(
                &mut session,
                &mut world,
                &mut systems,
                &mut visuals,
                &mut assets,
                &mut queue
            ),
            1
        );
        let effective = value.clamp(0.0, 1.0);
        for &id in &projections {
            assert_eq!(
                world
                    .get_component_by_id_as::<ShadingComponent>(id)
                    .unwrap()
                    .shade_strength,
                effective
            );
        }
        for &h in &handles {
            assert_eq!(
                visuals
                    .instance(h)
                    .unwrap()
                    .anime_shading
                    .shade_color_strength[3],
                effective
            );
        }
        assert_eq!(
            visuals
                .instance(isolated_handle)
                .unwrap()
                .anime_shading
                .shade_color_strength[3],
            0.23
        );
    }
    let shade_threshold_slider = label(&world, "anime_shade_threshold_slider");
    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            shade_threshold_slider,
            EventSignal::SliderChanged {
                slider: shade_threshold_slider,
                value: 1.2,
            },
        ),
    );
    assert_eq!(
        service(
            &mut session,
            &mut world,
            &mut systems,
            &mut visuals,
            &mut assets,
            &mut queue
        ),
        1
    );
    let shading = world
        .get_component_by_id_as::<ShadingComponent>(source_id)
        .unwrap();
    assert!((shading.shade_threshold - 1.2).abs() < 1e-6);
    assert!((shading.lit_threshold - 1.2).abs() < 1e-6);
    let lit_threshold_slider = label(&world, "anime_lit_threshold_slider");
    assert!(
        (world
            .get_component_by_id_as::<SliderComponent>(lit_threshold_slider)
            .unwrap()
            .value()
            - 1.2)
            .abs()
            < 1e-6
    );
    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            lit_threshold_slider,
            EventSignal::SliderChanged {
                slider: lit_threshold_slider,
                value: 0.25,
            },
        ),
    );
    assert_eq!(
        service(
            &mut session,
            &mut world,
            &mut systems,
            &mut visuals,
            &mut assets,
            &mut queue
        ),
        1
    );
    let shading = world
        .get_component_by_id_as::<ShadingComponent>(source_id)
        .unwrap();
    assert!((shading.shade_threshold - 0.25).abs() < 1e-6);
    assert!((shading.lit_threshold - 0.25).abs() < 1e-6);
    for &h in &handles {
        assert_eq!(
            visuals.instance(h).unwrap().anime_shading.controls[0..2],
            [0.25, 0.25]
        );
    }
    let rim_power_slider = label(&world, "anime_rim_power_slider");
    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            rim_power_slider,
            EventSignal::SliderChanged {
                slider: rim_power_slider,
                value: 9.5,
            },
        ),
    );
    assert_eq!(
        service(
            &mut session,
            &mut world,
            &mut systems,
            &mut visuals,
            &mut assets,
            &mut queue
        ),
        1
    );
    for &h in &handles {
        assert_eq!(visuals.instance(h).unwrap().anime_shading.controls[3], 9.5);
    }
    let readout = label(&world, "anime_shade_strength_readout");
    assert_eq!(
        world
            .get_component_by_id_as::<TextComponent>(readout)
            .unwrap()
            .text,
        "0.73"
    );

    // Delete the removable body through the engine lifecycle, then prove old
    // callbacks cannot mutate the material and restore from current source state.
    let body = label(&world, "accordion_body");
    queue.push_intent_now(body, IntentValue::RemoveSubtree { component_id: body });
    systems.process_commands(&mut world, &mut visuals, &mut assets, &mut queue);
    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(slider, EventSignal::SliderChanged { slider, value: 0.1 }),
    );
    assert_eq!(
        service(
            &mut session,
            &mut world,
            &mut systems,
            &mut visuals,
            &mut assets,
            &mut queue
        ),
        0
    );
    let panel = label(&world, "anime_shading_panel");
    let slot = world.parent_of(panel).unwrap();
    let mount = label(&world, "accordion_body_mount");
    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            slot,
            EventSignal::DataEvent {
                name: "AccordionRestoreRequested".into(),
                payload: Some(mount),
            },
        ),
    );
    service(
        &mut session,
        &mut world,
        &mut systems,
        &mut visuals,
        &mut assets,
        &mut queue,
    );
    let restored_slider = label(&world, "anime_shade_strength_slider");
    assert_ne!(restored_slider, slider);
    assert!(
        (world
            .get_component_by_id_as::<SliderComponent>(restored_slider)
            .unwrap()
            .value()
            - 0.73)
            .abs()
            < 1e-6
    );
    let reset = label(&world, "anime_shading_reset");
    systems.rx.dispatch_event_handlers(
        &mut world,
        &Signal::event(
            reset,
            EventSignal::Click {
                raycaster: ComponentId::default(),
                renderable: reset,
                hit_point: [0.0; 3],
                screen_pos_px: None,
            },
        ),
    );
    assert_eq!(
        service(
            &mut session,
            &mut world,
            &mut systems,
            &mut visuals,
            &mut assets,
            &mut queue
        ),
        5
    );
    assert_eq!(
        world
            .get_component_by_id_as::<ShadingComponent>(source_id)
            .unwrap()
            .shade_strength,
        0.5
    );
    let reset_shading = world
        .get_component_by_id_as::<ShadingComponent>(source_id)
        .unwrap();
    assert_eq!(reset_shading.shade_threshold, 0.4);
    assert_eq!(reset_shading.lit_threshold, 0.55);
    assert_eq!(reset_shading.rim_strength, 0.38);
    assert_eq!(reset_shading.rim_power, 4.0);
    for &h in &handles {
        let gpu = visuals.instance(h).unwrap().anime_shading;
        assert_eq!(gpu.shade_color_strength[3], 0.5);
        assert_eq!(gpu.controls, [0.4, 0.55, 0.38, 4.0]);
    }
}

#[test]
fn anime_shading_xr_fixture_materializes_shared_source_and_controls() {
    use crate::engine::ecs::component::{
        CameraXRComponent, ShadingComponent, ShadingModel, SliderComponent,
    };
    let mut world = World::default();
    let mut rx = RxWorld::default();
    let mut assets = RenderAssets::new();
    let mut queue = CommandQueue::new();
    let (_session, output) = RuntimeSpecSession::start_at_path(
        include_str!("../../examples/shading-models-xr.mms"),
        "examples/shading-models-xr.mms",
        &mut world,
        &mut rx,
        Some(&mut assets),
        &mut queue,
    )
    .unwrap();
    assert!(output.errors.is_empty(), "{:?}", output.errors);
    assert_eq!(
        world
            .all_components()
            .filter(|&id| world
                .get_component_by_id_as::<CameraXRComponent>(id)
                .is_some())
            .count(),
        1
    );
    assert_eq!(
        world
            .all_components()
            .filter(|&id| world
                .get_component_by_id_as::<SliderComponent>(id)
                .is_some())
            .count(),
        5
    );
    let anime = world
        .all_components()
        .find(|&id| {
            world
                .get_component_by_id_as::<ShadingComponent>(id)
                .is_some_and(|s| s.model == ShadingModel::Anime)
        })
        .unwrap();
    let consumers = world
        .all_components()
        .filter(|&id| {
            (world
                .get_component_by_id_as::<RenderableComponent>(id)
                .is_some()
                || world
                    .get_component_by_id_as::<crate::engine::ecs::component::GLTFComponent>(id)
                    .is_some())
                && crate::engine::ecs::system::RenderableSystem::resolve_anime_shading(&world, id)
                    .is_some_and(|(source, _)| source == anime)
        })
        .count();
    assert_eq!(
        consumers, 2,
        "one ordinary mesh and one GLTF share the Anime source"
    );
}
