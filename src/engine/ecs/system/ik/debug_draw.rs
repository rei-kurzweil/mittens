//! Retained, non-interactive IK diagnostic geometry.
use crate::engine::ecs::component::{
    ColorComponent, OverlayComponent, RaycastableComponent, RenderableComponent,
    SelectableComponent, SerializeComponent, TextComponent, TransformComponent, UnlitComponent,
};
use crate::engine::ecs::{ComponentId, IntentValue, SignalEmitter, World};
use crate::engine::graphics::{CameraTarget, VisualWorld};
use crate::utils::math::{shortest_arc_quat, vec3_len, vec3_sub};

pub(super) type Point = [f32; 3];
const IDENTITY: [f32; 4] = [0., 0., 0., 1.];

pub(super) fn canvas(world: &mut World, name: &str) -> (ComponentId, ComponentId) {
    let root = world.add_component_boxed_named(name, Box::new(OverlayComponent::new()));
    let transient = world.add_component(SerializeComponent::off());
    let unselectable = world.add_component(SelectableComponent::off());
    let raycast = world.add_component(RaycastableComponent::disabled());
    let unlit = world.add_component(UnlitComponent);
    for child in [transient, unselectable, raycast, unlit] {
        world.add_child(root, child).expect("new diagnostic root");
    }
    (root, raycast)
}

pub(super) fn beam(
    world: &mut World,
    parent: ComponentId,
    rgba: [f32; 4],
) -> (ComponentId, ComponentId) {
    let anchor = world.add_component(TransformComponent::new().with_scale(0., 0., 0.));
    let cube = world.add_component(RenderableComponent::cube());
    let [r, g, b, a] = rgba;
    let color = world.add_component(ColorComponent::rgba(r, g, b, a));
    world.add_child(parent, anchor).expect("new overlay beam");
    world.add_child(anchor, cube).expect("new beam mesh");
    world.add_child(cube, color).expect("new beam color");
    (anchor, color)
}

pub(super) fn segment(
    emit: &mut dyn SignalEmitter,
    id: ComponentId,
    endpoints: Option<[Point; 2]>,
    thickness: f32,
) {
    let Some([a, b]) = endpoints else {
        transform(emit, id, [0.; 3], IDENTITY, [0.; 3]);
        return;
    };
    let delta = vec3_sub(b, a);
    let length = vec3_len(delta);
    let direction = if length > f32::EPSILON {
        delta.map(|v| v / length)
    } else {
        [0., 0., 1.]
    };
    transform(
        emit,
        id,
        std::array::from_fn(|i| (a[i] + b[i]) * 0.5),
        shortest_arc_quat([0., 0., 1.], direction),
        [thickness, thickness, length.max(thickness)],
    );
}

pub(super) fn transform(
    emit: &mut dyn SignalEmitter,
    id: ComponentId,
    translation: Point,
    rotation_quat_xyzw: [f32; 4],
    scale: Point,
) {
    emit.push_intent_now(
        id,
        IntentValue::UpdateTransform {
            component_id: id,
            translation,
            rotation_quat_xyzw,
            scale,
        },
    );
}

pub(super) fn facing(visuals: &VisualWorld) -> [f32; 4] {
    visuals
        .visual_camera(CameraTarget::Xr)
        .or_else(|| visuals.visual_camera(CameraTarget::Window))
        .and_then(|view| view.eyes.first())
        .map_or(IDENTITY, |eye| eye.transform.rotation)
}

#[derive(Debug)]
pub(super) struct Readout {
    anchor: ComponentId,
    label: ComponentId,
    text: String,
    refresh: f32,
}

impl Readout {
    pub fn spawn(world: &mut World, parent: ComponentId, name: &str) -> Self {
        let anchor = world.add_component(TransformComponent::new());
        let label = world.add_component_boxed_named(name, Box::new(TextComponent::new("")));
        world.add_child(parent, anchor).expect("new readout anchor");
        world.add_child(anchor, label).expect("new readout label");
        Self {
            anchor,
            label,
            text: String::new(),
            refresh: 0.,
        }
    }

    pub fn draw(
        &mut self,
        emit: &mut dyn SignalEmitter,
        position: Point,
        rotation: [f32; 4],
        size: f32,
        dt: f32,
        text: impl FnOnce() -> String,
    ) {
        transform(emit, self.anchor, position, rotation, [size; 3]);
        self.refresh -= dt.max(0.);
        if self.refresh <= 0. {
            self.refresh = 0.20;
            let text = text();
            if self.text != text {
                self.text.clone_from(&text);
                emit.push_intent_now(
                    self.label,
                    IntentValue::SetText {
                        component_id: self.label,
                        text,
                    },
                );
            }
        }
    }
}
