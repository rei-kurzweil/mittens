use crate::engine::ecs::ComponentId;
use crate::engine::ecs::component::{
    TransitionComponent, TransitionEasing, TransitionReplacePolicy,
};
use crate::engine::graphics::VisualWorld;
use crate::engine::graphics::primitives::Transform;
use crate::engine::user_input::InputState;

use super::System;
use crate::engine::ecs::World;

#[derive(Debug, Clone, Copy)]
struct ActiveTransformTransition {
    component: ComponentId,
    from: Transform,
    to: Transform,
    start_beat: f64,
    duration_beats: f64,
    easing: TransitionEasing,
    last_sampled_beat: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
struct ActiveEmissiveTransition {
    component: ComponentId,
    from: f32,
    to: f32,
    start_beat: f64,
    duration_beats: f64,
    easing: TransitionEasing,
    last_sampled_beat: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub struct TransformTransitionUpdate {
    pub component: ComponentId,
    pub transform: Transform,
}

#[derive(Debug, Clone, Copy)]
pub struct EmissiveTransitionUpdate {
    pub component: ComponentId,
    pub intensity: f32,
}

#[derive(Debug, Default)]
pub struct TransitionSystem {
    active_transforms: Vec<ActiveTransformTransition>,
    active_emissives: Vec<ActiveEmissiveTransition>,
}

impl TransitionSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_transform_transition(
        &mut self,
        component: ComponentId,
        current: Transform,
        target: Transform,
        transition: TransitionComponent,
        start_beat: f64,
    ) -> bool {
        if !transition.enabled || transition.duration_beats <= 0.0 {
            return false;
        }

        match transition.replace {
            TransitionReplacePolicy::ReplaceSameTarget | TransitionReplacePolicy::AllowParallel => {
                self.active_transforms
                    .retain(|active| active.component != component);
            }
        }

        self.active_transforms.push(ActiveTransformTransition {
            component,
            from: current,
            to: target,
            start_beat,
            duration_beats: transition.duration_beats,
            easing: transition.easing,
            last_sampled_beat: None,
        });

        true
    }

    pub fn start_emissive_transition(
        &mut self,
        component: ComponentId,
        current: f32,
        target: f32,
        transition: TransitionComponent,
        start_beat: f64,
    ) -> bool {
        if !transition.enabled || transition.duration_beats <= 0.0 {
            return false;
        }

        match transition.replace {
            TransitionReplacePolicy::ReplaceSameTarget | TransitionReplacePolicy::AllowParallel => {
                self.active_emissives
                    .retain(|active| active.component != component);
            }
        }

        self.active_emissives.push(ActiveEmissiveTransition {
            component,
            from: current,
            to: target,
            start_beat,
            duration_beats: transition.duration_beats,
            easing: transition.easing,
            last_sampled_beat: None,
        });

        true
    }

    pub fn cancel_transform_transitions(&mut self, component: ComponentId) {
        self.active_transforms
            .retain(|active| active.component != component);
    }

    pub fn cancel_emissive_transitions(&mut self, component: ComponentId) {
        self.active_emissives
            .retain(|active| active.component != component);
    }

    pub fn sample_transform_updates(&mut self, beat_now: f64) -> Vec<TransformTransitionUpdate> {
        let mut updates = Vec::new();
        let mut completed = Vec::new();

        for (index, active) in self.active_transforms.iter_mut().enumerate() {
            if active.last_sampled_beat == Some(beat_now) {
                continue;
            }

            let raw_progress = if active.duration_beats <= f64::EPSILON {
                1.0
            } else {
                ((beat_now - active.start_beat) / active.duration_beats).clamp(0.0, 1.0)
            };
            let eased_progress = apply_easing(active.easing, raw_progress as f32);
            let transform = interpolate_transform(active.from, active.to, eased_progress);

            active.last_sampled_beat = Some(beat_now);
            updates.push(TransformTransitionUpdate {
                component: active.component,
                transform: if raw_progress >= 1.0 {
                    active.to
                } else {
                    transform
                },
            });

            if raw_progress >= 1.0 {
                completed.push(index);
            }
        }

        for index in completed.into_iter().rev() {
            self.active_transforms.remove(index);
        }

        updates
    }

    pub fn sample_emissive_updates(&mut self, beat_now: f64) -> Vec<EmissiveTransitionUpdate> {
        let mut updates = Vec::new();
        let mut completed = Vec::new();

        for (index, active) in self.active_emissives.iter_mut().enumerate() {
            if active.last_sampled_beat == Some(beat_now) {
                continue;
            }

            let raw_progress = if active.duration_beats <= f64::EPSILON {
                1.0
            } else {
                ((beat_now - active.start_beat) / active.duration_beats).clamp(0.0, 1.0)
            };
            let eased_progress = apply_easing(active.easing, raw_progress as f32);
            let intensity = active.from + (active.to - active.from) * eased_progress;

            active.last_sampled_beat = Some(beat_now);
            updates.push(EmissiveTransitionUpdate {
                component: active.component,
                intensity: if raw_progress >= 1.0 {
                    active.to
                } else {
                    intensity
                },
            });

            if raw_progress >= 1.0 {
                completed.push(index);
            }
        }

        for index in completed.into_iter().rev() {
            self.active_emissives.remove(index);
        }

        updates
    }

    #[cfg(test)]
    pub(crate) fn active_transform_count(&self) -> usize {
        self.active_transforms.len()
    }

    #[cfg(test)]
    pub(crate) fn active_emissive_count(&self) -> usize {
        self.active_emissives.len()
    }
}

impl System for TransitionSystem {
    fn tick(
        &mut self,
        _world: &mut World,
        _visuals: &mut VisualWorld,
        _input: &InputState,
        _dt_sec: f32,
    ) {
    }
}

fn apply_easing(easing: TransitionEasing, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match easing {
        TransitionEasing::Step => {
            if t >= 1.0 {
                1.0
            } else {
                0.0
            }
        }
        TransitionEasing::Linear => t,
        TransitionEasing::EaseInQuad => t * t,
        TransitionEasing::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
        TransitionEasing::EaseInOutQuad => {
            if t < 0.5 {
                2.0 * t * t
            } else {
                1.0 - ((-2.0 * t + 2.0).powi(2) / 2.0)
            }
        }
        TransitionEasing::EaseInCubic => t * t * t,
        TransitionEasing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
        TransitionEasing::EaseInOutCubic => {
            if t < 0.5 {
                4.0 * t * t * t
            } else {
                1.0 - ((-2.0 * t + 2.0).powi(3) / 2.0)
            }
        }
        TransitionEasing::EaseInOutSine => -(std::f32::consts::PI * t).cos() * 0.5 + 0.5,
    }
}

fn interpolate_transform(from: Transform, to: Transform, t: f32) -> Transform {
    let mut out = Transform::default();
    out.translation = lerp3(from.translation, to.translation, t);
    out.scale = lerp3(from.scale, to.scale, t);
    out.rotation = nlerp_quat(from.rotation, to.rotation, t);
    out.recompute_model();
    out
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn nlerp_quat(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut target = b;
    let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
    if dot < 0.0 {
        target = [-b[0], -b[1], -b[2], -b[3]];
    }

    let blended = [
        a[0] + (target[0] - a[0]) * t,
        a[1] + (target[1] - a[1]) * t,
        a[2] + (target[2] - a[2]) * t,
        a[3] + (target[3] - a[3]) * t,
    ];

    normalize_quat(blended)
}

fn normalize_quat(q: [f32; 4]) -> [f32; 4] {
    let len2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
    if len2 <= f32::EPSILON {
        [0.0, 0.0, 0.0, 1.0]
    } else {
        let inv_len = len2.sqrt().recip();
        [
            q[0] * inv_len,
            q[1] * inv_len,
            q[2] * inv_len,
            q[3] * inv_len,
        ]
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::TransitionSystem;
    use crate::engine::ecs::World;
    use crate::engine::ecs::CommandQueue;
    use crate::engine::ecs::component::{
        EmissiveComponent, RenderableComponent, TransitionComponent, TransitionEasing,
        TransformComponent,
    };
    use crate::engine::ecs::system::{ClockDriver, SystemWorld};
    use crate::engine::graphics::primitives::{MaterialHandle, Renderable};
    use crate::engine::graphics::{CpuMesh, MeshUploader, RenderAssets, VisualWorld};
    use crate::engine::graphics::primitives::Transform;

    #[derive(Clone, Default)]
    struct TestClockDriver {
        now_sec: Arc<Mutex<f64>>,
    }

    impl TestClockDriver {
        fn set_time_sec(&self, time_sec: f64) {
            *self.now_sec.lock().expect("clock mutex poisoned") = time_sec;
        }
    }

    impl ClockDriver for TestClockDriver {
        fn name(&self) -> &'static str {
            "test"
        }

        fn time_now_sec(&self) -> f64 {
            *self.now_sec.lock().expect("clock mutex poisoned")
        }
    }

    #[derive(Default)]
    struct TestUploader {
        next_mesh: u32,
    }

    impl MeshUploader for TestUploader {
        fn upload_mesh(
            &mut self,
            _mesh: &CpuMesh,
        ) -> Result<crate::engine::graphics::primitives::MeshHandle, Box<dyn std::error::Error>>
        {
            let handle = crate::engine::graphics::primitives::MeshHandle(self.next_mesh);
            self.next_mesh += 1;
            Ok(handle)
        }
    }

    #[test]
    fn transform_transition_reaches_exact_final_value() {
        let mut world = World::default();
        let component =
            world.add_component(crate::engine::ecs::component::TransformComponent::new());

        let mut system = TransitionSystem::new();
        let from = Transform::default();
        let mut to = Transform::default();
        to.translation = [10.0, -2.0, 3.0];
        to.scale = [2.0, 3.0, 4.0];
        to.recompute_model();

        let started = system.start_transform_transition(
            component,
            from,
            to,
            TransitionComponent::new()
                .with_duration_beats(2.0)
                .with_easing(TransitionEasing::Linear),
            0.0,
        );

        assert!(started);
        let halfway = system.sample_transform_updates(1.0);
        assert_eq!(halfway.len(), 1);
        assert!((halfway[0].transform.translation[0] - 5.0).abs() < 1e-5);
        assert_eq!(system.active_transform_count(), 1);

        let finished = system.sample_transform_updates(2.0);
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].transform.translation, to.translation);
        assert_eq!(finished[0].transform.scale, to.scale);
        assert_eq!(system.active_transform_count(), 0);
    }

    #[test]
    fn transform_transition_dedupes_same_beat_sampling() {
        let mut world = World::default();
        let component =
            world.add_component(crate::engine::ecs::component::TransformComponent::new());

        let mut system = TransitionSystem::new();
        let from = Transform::default();
        let mut to = Transform::default();
        to.translation = [4.0, 0.0, 0.0];
        to.recompute_model();

        assert!(system.start_transform_transition(
            component,
            from,
            to,
            TransitionComponent::new().with_duration_beats(1.0),
            0.0,
        ));

        assert_eq!(system.sample_transform_updates(0.0).len(), 1);
        assert!(system.sample_transform_updates(0.0).is_empty());
    }

    #[test]
    fn replace_same_target_keeps_only_latest_transition() {
        let mut world = World::default();
        let component =
            world.add_component(crate::engine::ecs::component::TransformComponent::new());

        let mut system = TransitionSystem::new();
        let from = Transform::default();

        let mut first_target = Transform::default();
        first_target.translation = [10.0, 0.0, 0.0];
        first_target.recompute_model();
        assert!(system.start_transform_transition(
            component,
            from,
            first_target,
            TransitionComponent::new().with_duration_beats(2.0),
            0.0,
        ));

        let mut current = Transform::default();
        current.translation = [2.5, 0.0, 0.0];
        current.recompute_model();
        let mut second_target = Transform::default();
        second_target.translation = [20.0, 0.0, 0.0];
        second_target.recompute_model();
        assert!(system.start_transform_transition(
            component,
            current,
            second_target,
            TransitionComponent::new().with_duration_beats(1.0),
            0.25,
        ));

        assert_eq!(system.active_transform_count(), 1);
        let finished = system.sample_transform_updates(1.25);
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].transform.translation, second_target.translation);
        assert_eq!(system.active_transform_count(), 0);
    }

    #[test]
    fn emissive_transition_reaches_exact_final_value() {
        let mut world = World::default();
        let component = world.add_component(EmissiveComponent::off());

        let mut system = TransitionSystem::new();
        let started = system.start_emissive_transition(
            component,
            0.0,
            2.5,
            TransitionComponent::new()
                .with_duration_beats(2.0)
                .with_easing(TransitionEasing::Linear),
            0.0,
        );

        assert!(started);
        let halfway = system.sample_emissive_updates(1.0);
        assert_eq!(halfway.len(), 1);
        assert!((halfway[0].intensity - 1.25).abs() < 1e-5);
        assert_eq!(system.active_emissive_count(), 1);

        let finished = system.sample_emissive_updates(2.0);
        assert_eq!(finished.len(), 1);
        assert!((finished[0].intensity - 2.5).abs() < 1e-6);
        assert_eq!(system.active_emissive_count(), 0);
    }

    #[test]
    fn replace_same_target_keeps_only_latest_emissive_transition() {
        let mut world = World::default();
        let component = world.add_component(EmissiveComponent::off());

        let mut system = TransitionSystem::new();
        assert!(system.start_emissive_transition(
            component,
            0.0,
            1.0,
            TransitionComponent::new().with_duration_beats(2.0),
            0.0,
        ));

        assert!(system.start_emissive_transition(
            component,
            0.25,
            3.0,
            TransitionComponent::new().with_duration_beats(1.0),
            0.25,
        ));

        assert_eq!(system.active_emissive_count(), 1);
        let finished = system.sample_emissive_updates(1.25);
        assert_eq!(finished.len(), 1);
        assert!((finished[0].intensity - 3.0).abs() < 1e-6);
        assert_eq!(system.active_emissive_count(), 0);
    }

    #[test]
    fn system_world_update_transform_is_immediate_without_transition_child() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let component =
            world.add_component(crate::engine::ecs::component::TransformComponent::new());

        let mut target = Transform::default();
        target.translation = [3.0, 4.0, 5.0];
        target.recompute_model();

        systems.update_transform(&mut world, &mut visuals, component, target);

        let current = world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(component)
            .unwrap();
        assert_eq!(current.transform.translation, target.translation);
        assert_eq!(systems.transition.active_transform_count(), 0);
    }

    #[test]
    fn system_world_update_transform_starts_runtime_when_transition_child_exists() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let component =
            world.add_component(crate::engine::ecs::component::TransformComponent::new());
        let transition = world.add_component(
            TransitionComponent::new()
                .with_duration_beats(1.0)
                .with_easing(TransitionEasing::Linear),
        );
        world.add_child(component, transition).unwrap();

        let initial = world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(component)
            .unwrap()
            .transform
            .translation;

        let mut target = Transform::default();
        target.translation = [9.0, 0.0, 0.0];
        target.recompute_model();

        systems.update_transform(&mut world, &mut visuals, component, target);

        let current = world
            .get_component_by_id_as::<crate::engine::ecs::component::TransformComponent>(component)
            .unwrap();
        assert_eq!(current.transform.translation, initial);
        assert_eq!(systems.transition.active_transform_count(), 1);
    }

    #[test]
    fn system_world_update_emissive_is_immediate_without_transition_child() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let component = world.add_component(EmissiveComponent::off());

        systems.update_emissive_intensity(&mut world, &mut visuals, component, 2.5);

        let current = world
            .get_component_by_id_as::<EmissiveComponent>(component)
            .unwrap();
        assert!((current.intensity - 2.5).abs() < 1.0e-6);
        assert_eq!(systems.transition.active_emissive_count(), 0);
    }

    #[test]
    fn system_world_update_emissive_starts_runtime_when_transition_child_exists() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let component = world.add_component(EmissiveComponent::off());
        let transition = world.add_component(
            TransitionComponent::new()
                .with_duration_beats(1.0)
                .with_easing(TransitionEasing::Linear),
        );
        world.add_child(component, transition).unwrap();

        systems.update_emissive_intensity(&mut world, &mut visuals, component, 2.5);

        let current = world
            .get_component_by_id_as::<EmissiveComponent>(component)
            .unwrap();
        assert_eq!(current.intensity, 0.0);
        assert_eq!(systems.transition.active_emissive_count(), 1);
    }

    #[test]
    fn system_world_tick_advances_emissive_transition_and_reaches_final_value() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let component = world.add_component(EmissiveComponent::off());
        let transition = world.add_component(
            TransitionComponent::new()
                .with_duration_beats(1.0)
                .with_easing(TransitionEasing::Linear),
        );
        world.add_child(component, transition).unwrap();

        let driver = TestClockDriver::default();
        systems.clock.set_driver(Arc::new(driver.clone()));
        systems.clock.set_bpm(60.0);
        driver.set_time_sec(0.0);
        systems.clock.sample();

        systems.update_emissive_intensity(&mut world, &mut visuals, component, 2.0);

        driver.set_time_sec(0.5);
        systems.clock.sample();
        systems.tick_transition_runtime(&mut world, &mut visuals);
        let halfway = world
            .get_component_by_id_as::<EmissiveComponent>(component)
            .unwrap();
        assert!((halfway.intensity - 1.0).abs() < 1.0e-6);
        assert_eq!(systems.transition.active_emissive_count(), 1);

        driver.set_time_sec(1.0);
        systems.clock.sample();
        systems.tick_transition_runtime(&mut world, &mut visuals);
        let finished = world
            .get_component_by_id_as::<EmissiveComponent>(component)
            .unwrap();
        assert!((finished.intensity - 2.0).abs() < 1.0e-6);
        assert_eq!(systems.transition.active_emissive_count(), 0);
    }

    #[test]
    fn direct_parent_emissive_transition_updates_renderable_visual_over_time() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut queue = CommandQueue::new();

        let renderable = world.add_component(RenderableComponent::new(Renderable::new(
            crate::engine::graphics::primitives::CpuMeshHandle::CUBE,
            MaterialHandle::TOON_MESH,
        )));
        let emissive = world.add_component(EmissiveComponent::off());
        let transition = world.add_component(
            TransitionComponent::new()
                .with_duration_beats(1.0)
                .with_easing(TransitionEasing::Linear),
        );
        world.add_child(renderable, emissive).unwrap();
        world.add_child(emissive, transition).unwrap();

        systems
            .renderable
            .register_renderable_from_world(&mut world, &mut visuals, renderable);
        systems.renderable.flush_pending(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut uploader,
            &mut queue,
        );
        let handle = world
            .get_component_by_id_as::<RenderableComponent>(renderable)
            .and_then(|r| r.get_handle())
            .expect("renderable handle");

        let driver = TestClockDriver::default();
        systems.clock.set_driver(Arc::new(driver.clone()));
        systems.clock.set_bpm(60.0);
        driver.set_time_sec(0.0);
        systems.clock.sample();

        systems.update_emissive_intensity(&mut world, &mut visuals, emissive, 2.0);
        driver.set_time_sec(0.5);
        systems.clock.sample();
        systems.tick_transition_runtime(&mut world, &mut visuals);
        systems.renderable.flush_pending(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut uploader,
            &mut queue,
        );
        let halfway = visuals.instance(handle).expect("visual instance");
        assert!((halfway.emissive - 1.0).abs() < 1.0e-6);

        driver.set_time_sec(1.0);
        systems.clock.sample();
        systems.tick_transition_runtime(&mut world, &mut visuals);
        systems.renderable.flush_pending(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut uploader,
            &mut queue,
        );
        let finished = visuals.instance(handle).expect("visual instance");
        assert!((finished.emissive - 2.0).abs() < 1.0e-6);
    }

    #[test]
    fn inherited_emissive_transition_updates_descendant_renderables_over_time() {
        let mut world = World::default();
        let mut systems = SystemWorld::default();
        let mut visuals = VisualWorld::default();
        let mut render_assets = RenderAssets::new();
        let mut uploader = TestUploader::default();
        let mut queue = CommandQueue::new();

        let container = world.add_component(TransformComponent::new());
        let emissive = world.add_component(EmissiveComponent::off());
        let transition = world.add_component(
            TransitionComponent::new()
                .with_duration_beats(1.0)
                .with_easing(TransitionEasing::Linear),
        );
        let child_t = world.add_component(TransformComponent::new());
        let child_r = world.add_component(RenderableComponent::new(Renderable::new(
            crate::engine::graphics::primitives::CpuMeshHandle::CUBE,
            MaterialHandle::TOON_MESH,
        )));
        world.add_child(container, emissive).unwrap();
        world.add_child(emissive, transition).unwrap();
        world.add_child(container, child_t).unwrap();
        world.add_child(child_t, child_r).unwrap();

        systems
            .renderable
            .register_renderable_from_world(&mut world, &mut visuals, child_r);
        systems.renderable.flush_pending(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut uploader,
            &mut queue,
        );
        let handle = world
            .get_component_by_id_as::<RenderableComponent>(child_r)
            .and_then(|r| r.get_handle())
            .expect("renderable handle");

        let driver = TestClockDriver::default();
        systems.clock.set_driver(Arc::new(driver.clone()));
        systems.clock.set_bpm(60.0);
        driver.set_time_sec(0.0);
        systems.clock.sample();

        systems.update_emissive_intensity(&mut world, &mut visuals, emissive, 3.0);
        driver.set_time_sec(0.5);
        systems.clock.sample();
        systems.tick_transition_runtime(&mut world, &mut visuals);
        systems.renderable.flush_pending(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut uploader,
            &mut queue,
        );
        let halfway = visuals.instance(handle).expect("visual instance");
        assert!((halfway.emissive - 1.5).abs() < 1.0e-6);

        driver.set_time_sec(1.0);
        systems.clock.sample();
        systems.tick_transition_runtime(&mut world, &mut visuals);
        systems.renderable.flush_pending(
            &mut world,
            &mut visuals,
            &mut render_assets,
            &mut uploader,
            &mut queue,
        );
        let finished = visuals.instance(handle).expect("visual instance");
        assert!((finished.emissive - 3.0).abs() < 1.0e-6);
    }
}
