use crate::engine::ecs::system::collidable_bvh::{SupportFace, SupportSurface};
use crate::engine::ecs::system::ik::debug_draw::{self, Point};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};

#[derive(Debug)]
pub(super) struct Outline {
    bounds: [ComponentId; 12],
    face: [(ComponentId, ComponentId); 8],
    point: ComponentId,
    last: Option<SupportSurface>,
    phase: f32,
    bright: bool,
}

impl Outline {
    pub fn spawn(world: &mut World, parent: ComponentId) -> Self {
        let anchor = world.add_component_boxed_named(
            "body_support_surface_outline",
            Box::new(crate::engine::ecs::component::TransformComponent::new()),
        );
        world
            .add_child(parent, anchor)
            .expect("new support outline");
        Self {
            bounds: std::array::from_fn(|_| {
                debug_draw::beam(world, anchor, [0.32, 0.36, 0.40, 1.]).0
            }),
            face: std::array::from_fn(|_| debug_draw::beam(world, anchor, [1.; 4])),
            point: debug_draw::beam(world, anchor, [1., 0.12, 0.80, 1.]).0,
            last: None,
            phase: 0.,
            bright: true,
        }
    }

    pub fn draw(
        &mut self,
        emit: &mut dyn SignalEmitter,
        sample: Option<SupportSurface>,
        size: f32,
        dt: f32,
    ) {
        let bounds = sample.map(|sample| sample.bounds);
        if bounds != self.last.map(|sample| sample.bounds) {
            let bounds = bounds.map(box_edges);
            for (i, id) in self.bounds.into_iter().enumerate() {
                debug_draw::segment(emit, id, bounds.map(|edges| edges[i]), size * 0.65);
            }
        }
        let face = sample.map(|sample| sample.face);
        if face != self.last.map(|sample| sample.face) {
            let edges = face.map(face_edges).unwrap_or([None; 8]);
            for ((line, _), edge) in self.face.into_iter().zip(edges) {
                debug_draw::segment(emit, line, edge, size * 1.45);
            }
        }
        if sample.map(|sample| sample.point) != self.last.map(|sample| sample.point) {
            debug_draw::segment(
                emit,
                self.point,
                sample.map(|sample| [sample.point; 2]),
                size * 2.,
            );
        }
        self.last = sample;
        if sample.is_some() {
            self.phase = (self.phase + dt.max(0.)) % 1.;
            let bright = self.phase < 0.5;
            if bright != self.bright {
                self.bright = bright;
                let value = if bright { 1. } else { 0. };
                for (_, color) in self.face {
                    emit.push_intent_now(
                        color,
                        IntentValue::SetColor {
                            component_id: color,
                            rgba: [value, value, value, 1.],
                        },
                    );
                }
            }
        }
    }
}

fn box_edges(bounds: [Point; 2]) -> [[Point; 2]; 12] {
    const EDGES: [[usize; 2]; 12] = [
        [0, 1],
        [2, 3],
        [4, 5],
        [6, 7],
        [0, 2],
        [1, 3],
        [4, 6],
        [5, 7],
        [0, 4],
        [1, 5],
        [2, 6],
        [3, 7],
    ];
    EDGES.map(|edge| {
        edge.map(|corner| std::array::from_fn(|axis| bounds[(corner >> axis) & 1][axis]))
    })
}

fn face_edges(face: SupportFace) -> [Option<[Point; 2]>; 8] {
    match face {
        SupportFace::BoxTop(corners) => std::array::from_fn(|i| {
            let start = corners[i / 2];
            let toward = corners[(i / 2 + if i % 2 == 0 { 1 } else { 3 }) % 4];
            Some([
                start,
                std::array::from_fn(|axis| start[axis] + (toward[axis] - start[axis]) * 0.22),
            ])
        }),
        SupportFace::Triangle(corners) => {
            std::array::from_fn(|i| (i < 3).then(|| [corners[i], corners[(i + 1) % 3]]))
        }
    }
}
