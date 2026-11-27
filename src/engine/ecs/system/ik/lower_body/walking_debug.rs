//! The selected gait's planner decisions and body-relative foot traces.
use super::{FootState, LowerBodyStatus};
use crate::engine::ecs::component::AvatarControlComponent;
use crate::engine::ecs::system::TransformSystem;
use crate::engine::ecs::system::ik::debug_draw::{self, Point, Readout};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::VisualWorld;
use crate::engine::transform::TransformTrs;
use crate::utils::math::{
    quat_conjugate, quat_rotate_vec3, vec3_add, vec3_len, vec3_scale, vec3_sub,
};
use std::collections::VecDeque;

#[derive(Debug, Default)]
pub(super) struct WalkingTrace {
    pub reason: &'static str,
    pub speed: f32,
    pub lag: f32,
    pub duration: f32,
    pub stride: f32,
    pub threshold: f32,
    pub errors: Option<[f32; 2]>,
    pub anticipated_lag: Option<f32>,
    pub turned: [bool; 2],
    pub proposal: Option<StepAttempt>,
    pub transfer: f32,
    pub planted: Option<[Point; 2]>,
    pub swing: Option<SwingTrace>,
}

#[derive(Debug)]
pub(super) struct SwingTrace {
    pub side: usize,
    pub phase: f32,
    pub duration: f32,
    pub lift: f32,
    pub path: [Point; 9],
    pub proposal: Option<StepAttempt>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StepAttempt {
    pub side: usize,
    pub landing: Point,
    pub raw_lead: f32,
    pub capped_lead: f32,
    pub stride_capped: bool,
    pub errors: [f32; 2],
    pub anticipated_lag: f32,
    pub turned: [bool; 2],
    pub reason: &'static str,
}

#[derive(Debug)]
pub(super) struct WalkingFrame {
    body: TransformTrs,
    rest: [Point; 2],
    pub trace: WalkingTrace,
}

impl WalkingFrame {
    pub fn new(body: TransformTrs, rest: [Point; 2]) -> Self {
        Self {
            body,
            rest,
            trace: WalkingTrace::default(),
        }
    }

    fn to_local(&self, point: Point) -> Point {
        quat_rotate_vec3(
            quat_conjugate(self.body.rotation_quat_xyzw),
            vec3_sub(point, self.body.translation),
        )
        .map(|value| value / self.body.scale[0])
    }

    fn to_world(&self, point: Point) -> Point {
        vec3_add(
            self.body.translation,
            quat_rotate_vec3(
                self.body.rotation_quat_xyzw,
                vec3_scale(point, self.body.scale[0]),
            ),
        )
    }
}

const PATH_HISTORY: usize = 12;
const TARGET: [f32; 4] = [0.18, 0.88, 1., 1.];
const SOLVED: [f32; 4] = [0.36, 1., 0.42, 1.];
const PLANNED: [f32; 4] = [1., 0.74, 0.18, 1.];

#[derive(Debug, Default)]
pub(super) struct WalkingOverlay {
    visuals: Option<Visuals>,
    history: VecDeque<[Point; 2]>,
    previous: Option<(ComponentId, usize, f32)>,
}

#[derive(Debug)]
struct Visuals {
    root: ComponentId,
    markers: [ComponentId; 10],
    planned: [ComponentId; 8],
    history: [[ComponentId; PATH_HISTORY - 1]; 2],
    readout: Readout,
}

impl WalkingOverlay {
    pub fn draw(
        &mut self,
        world: &mut World,
        visuals: &VisualWorld,
        emit: &mut dyn SignalEmitter,
        target: Option<(ComponentId, LowerBodyStatus, Option<[FootState; 2]>)>,
        frame: Option<&WalkingFrame>,
        dt: f32,
    ) {
        let Some((avatar, status, feet)) = target else {
            if let Some(active) = self.visuals.take() {
                emit.push_intent_now(
                    active.root,
                    IntentValue::RemoveSubtree {
                        component_id: active.root,
                    },
                );
            }
            self.history.clear();
            self.previous = None;
            return;
        };
        let active = self
            .visuals
            .get_or_insert_with(|| Visuals::spawn(world, emit));
        let size = frame.map_or(0.009, |f| (f.body.scale[0] * 0.009).max(0.0005));
        let facing = debug_draw::facing(visuals);
        let anchor = world
            .get_component_by_id_as::<AvatarControlComponent>(avatar)
            .and_then(|avc| avc.head_mount.or(avc.model_root_id))
            .and_then(|id| TransformSystem::world_position(world, id))
            .unwrap_or([0.; 3]);
        let label = vec3_add(
            vec3_sub(anchor, quat_rotate_vec3(facing, [size * 52., 0., 0.])),
            [0., size * 28., 0.],
        );
        let trace = frame.map(|frame| &frame.trace);
        let swing = trace.and_then(|trace| trace.swing.as_ref());
        if let (Some(frame), Some(swing), Some(feet)) = (frame, swing, feet) {
            let previous = self.previous.filter(|&(id, side, phase)| {
                id == avatar && side == swing.side && phase <= swing.phase
            });
            if previous.is_none() {
                self.history.clear();
            }
            if previous.is_none_or(|(_, _, phase)| swing.phase - phase >= 0.07) {
                if self.history.len() == PATH_HISTORY {
                    self.history.pop_front();
                }
                let foot = feet[swing.side];
                self.history.push_back([
                    frame.to_local(foot.target_sole),
                    frame.to_local(foot.solved_sole),
                ]);
                self.previous = Some((avatar, swing.side, swing.phase));
            }
        } else if trace.is_none_or(|trace| trace.planted.is_none())
            || self.previous.is_some_and(|(id, _, _)| id != avatar)
        {
            self.history.clear();
            self.previous = None;
        }

        let points = |side: usize| {
            [
                frame.map(|frame| frame.rest[side]),
                trace.and_then(|trace| trace.planted.map(|feet| feet[side])),
                feet.map(|feet| feet[side].target_sole),
                feet.map(|feet| feet[side].solved_sole),
            ]
        };
        for (id, point) in active.markers[..8]
            .iter()
            .zip(points(0).into_iter().chain(points(1)))
        {
            debug_draw::segment(emit, *id, point.map(|p| [p; 2]), size * 2.);
        }
        debug_draw::segment(
            emit,
            active.markers[8],
            frame.map(|frame| [frame.body.translation, frame.to_world([0., 0., 0.45])]),
            size * 1.5,
        );
        let attempt = trace
            .and_then(|trace| trace.proposal)
            .or_else(|| swing.and_then(|swing| swing.proposal));
        let rejected = trace
            .filter(|trace| trace.reason == "landing has no support")
            .and_then(|trace| trace.proposal)
            .map(|attempt| [attempt.landing; 2]);
        debug_draw::segment(emit, active.markers[9], rejected, size * 3.);
        for (i, &id) in active.planned.iter().enumerate() {
            debug_draw::segment(
                emit,
                id,
                swing.map(|swing| [swing.path[i], swing.path[i + 1]]),
                size,
            );
        }
        for (path, lines) in active.history.iter().enumerate() {
            for (i, &id) in lines.iter().enumerate() {
                let endpoints = frame.and_then(|frame| {
                    Some([
                        frame.to_world(self.history.get(i)?[path]),
                        frame.to_world(self.history.get(i + 1)?[path]),
                    ])
                });
                debug_draw::segment(emit, id, endpoints, size * 0.8);
            }
        }
        active.readout.draw(emit, label, facing, size * 3.2, dt, || {
            let avatar = world.component_label(avatar).filter(|s| !s.is_empty()).unwrap_or("Avatar");
            let Some(frame) = frame else { return format!("Walking gait | {avatar}\n{status:?}"); };
            let trace = &frame.trace;
            let mut text = format!("Walking gait | {avatar}\n{} | {:.2} m/s | {status:?}", trace.reason, trace.speed);
            if let Some(errors) = trace.errors.or_else(|| attempt.map(|attempt| attempt.errors)) {
                let anticipated = trace.anticipated_lag.or_else(|| attempt.map(|attempt| attempt.anticipated_lag));
                let turned = attempt.map_or(trace.turned, |attempt| attempt.turned);
                text += &format!("\nStance L {:.2} / R {:.2} | limit {:.2} m\nLag {:.2} -> {:.2} m | turn L {} R {}",
                    errors[0], errors[1], trace.threshold, trace.lag, anticipated.unwrap_or(trace.lag), turned[0], turned[1]);
            }
            if let Some(swing) = swing {
                let start = swing.proposal.map_or("step in progress", |attempt| attempt.reason);
                text += &format!("\n{} {:.0}% | {start}\n{:.0}/{:.0} ms effective/planned | {:.2} m lift\nTravel {:.2} / {:.2} m stride",
                    if swing.side == 0 { "Left" } else { "Right" }, swing.phase * 100.,
                    swing.duration.min(trace.duration) * 1000., swing.duration * 1000., swing.lift,
                    vec3_len(super::walking::horizontal(vec3_sub(swing.path[8], swing.path[0]))), trace.stride);
            } else if trace.transfer > 0. {
                text += &format!("\nTransfer {:.0} ms", trace.transfer * 1000.);
            }
            if let Some(attempt) = attempt {
                text += &format!("\n{} lead {:.2} -> {:.2} m{}", if attempt.side == 0 { "L" } else { "R" }, attempt.raw_lead, attempt.capped_lead, if attempt.stride_capped { " | stride capped" } else { "" });
            }
            if let Some(feet) = feet {
                let [left, right] = feet.map(|foot| (frame.to_local(foot.solved_sole)[2], foot.target_error));
                text += &format!("\nBody Z L {:+.2} / R {:+.2} model m\nIK error L {:.3} / R {:.3} m", left.0, right.0, left.1, right.1);
            }
            text + "\nGold plan | cyan target | green solved"
        });
    }
}

impl Visuals {
    fn spawn(world: &mut World, emit: &mut dyn SignalEmitter) -> Self {
        let (root, parent) = debug_draw::canvas(world, "walking_gait_debug");
        let rest = [0.95, 0.95, 0.95, 1.];
        let planted = [0.9, 0.45, 0.72, 1.];
        let markers = [
            rest,
            planted,
            TARGET,
            SOLVED,
            rest,
            planted,
            TARGET,
            SOLVED,
            [0.4, 0.58, 1., 1.],
            [1., 0.3, 0.2, 1.],
        ]
        .map(|color| debug_draw::beam(world, parent, color).0);
        let planned = std::array::from_fn(|_| debug_draw::beam(world, parent, PLANNED).0);
        let history = [TARGET, SOLVED]
            .map(|color| std::array::from_fn(|_| debug_draw::beam(world, parent, color).0));
        let readout = Readout::spawn(world, parent, "walking_gait_debug_readout");
        world.init_component_tree(root, emit);
        Self {
            root,
            markers,
            planned,
            history,
            readout,
        }
    }
}
