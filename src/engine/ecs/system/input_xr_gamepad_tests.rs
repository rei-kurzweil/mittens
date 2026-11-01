use super::*;
use crate::engine::ecs::component::{GravityComponent, VelocityComponent};
use crate::engine::ecs::system::{
    CameraSystem, LightSystem, TransformOperatorSystem, VelocitySystem,
};
use crate::engine::ecs::{CommandQueue, RxWorld};
fn attach<T: crate::engine::ecs::component::Component>(
    w: &mut World,
    parent: ComponentId,
    value: T,
) -> ComponentId {
    let id = w.add_component(value);
    w.add_child(parent, id).unwrap();
    id
}
struct Fixture {
    world: World,
    visuals: VisualWorld,
    system: InputXRGamepadSystem,
    velocity: ComponentId,
    target: ComponentId,
    input: ComponentId,
    xr: XrGamepadState,
}
impl Fixture {
    fn new(enable: bool) -> Self {
        let mut world = World::default();
        let gravity = world.add_component(GravityComponent::new());
        let velocity = attach(&mut world, gravity, VelocityComponent::new());
        let target = attach(&mut world, velocity, TransformComponent::new());
        let input_xr = attach(&mut world, target, InputXRComponent::on());
        let input = attach(
            &mut world,
            input_xr,
            InputXRGamepadComponent::new()
                .speed(1.4)
                .enable_flying(enable),
        );
        let mut system = InputXRGamepadSystem::default();
        system.register_input_xr_gamepad(input);
        let mut f = Self {
            world,
            visuals: VisualWorld::default(),
            system,
            velocity,
            target,
            input,
            xr: XrGamepadState {
                active: true,
                ..Default::default()
            },
        };
        f.propagate();
        f
    }
    fn propagate(&mut self) {
        TransformSystem::new().transform_changed(
            &mut self.world,
            &mut self.visuals,
            self.target,
            &mut TransformOperatorSystem::new(),
            &mut CameraSystem::new(),
            &mut LightSystem::new(),
        );
    }
    fn tick(&mut self, dt: f32, down: bool) {
        self.xr.hands[1].button_b = Some((down, if down { 1. } else { 0. }));
        self.system.tick_with_state(
            &mut self.world,
            &mut self.visuals,
            self.xr,
            &mut CommandQueue::new(),
            dt,
        );
        self.propagate();
    }
    fn double_tap(&mut self) {
        self.tick(0.01, true);
        self.tick(0.05, false);
        self.tick(0.05, true);
        self.tick(0.01, false);
    }
    fn flying(&self) -> bool {
        self.world
            .get_component_by_id_as::<InputXRGamepadComponent>(self.input)
            .unwrap()
            .flying
    }
    fn position(&self) -> [f32; 3] {
        self.world
            .get_component_by_id_as::<TransformComponent>(self.target)
            .unwrap()
            .transform
            .translation
    }
}
#[test]
fn flight_double_tap_is_opt_in_edge_triggered_and_has_a_strict_window() {
    let mut f = Fixture::new(false);
    f.double_tap();
    assert!(!f.flying());
    f.world
        .get_component_by_id_as_mut::<InputXRGamepadComponent>(f.input)
        .unwrap()
        .enable_flying = true;
    f.tick(0.5, false);
    f.tick(0.01, true);
    f.tick(0.1, true);
    assert!(!f.flying(), "holding is not two taps");
    f.tick(0.01, false);
    f.tick(0.3, true);
    assert!(!f.flying(), "late taps do not activate flight");
    f.tick(0.05, false);
    f.tick(0.05, true);
    assert!(f.flying());
    f.tick(0.4, false);
    f.double_tap();
    assert!(!f.flying(), "another double tap restores walking/gravity");
}
#[test]
fn flight_moves_at_double_speed_can_rise_descend_and_suspends_gravity() {
    let mut walking = Fixture::new(false);
    walking.xr.hands[0].thumbstick = Some([1., 0.]);
    walking.tick(0.1, false);
    let mut flying = Fixture::new(true);
    flying.double_tap();
    flying
        .world
        .get_component_by_id_as_mut::<TransformComponent>(flying.target)
        .unwrap()
        .transform
        .translation = [0.; 3];
    flying.propagate();
    flying.xr.hands[0].thumbstick = Some([1., 0.]);
    flying.tick(0.1, false);
    assert!((flying.position()[0] - 2. * walking.position()[0]).abs() < 1e-5);
    flying.xr.hands[0].thumbstick = Some([0.; 2]);
    let y = flying.position()[1];
    flying.tick(0.4, true);
    assert!(flying.position()[1] > y);
    flying.xr.hands[1].button_a = Some((true, 1.));
    flying.tick(0.1, false);
    let y = flying.position()[1];
    flying.tick(0.1, false);
    assert!(flying.position()[1] < y);
    flying.xr.hands[1].button_a = None;
    let before = flying.position();
    VelocitySystem::default().step(&mut flying.world, &mut RxWorld::default());
    assert_eq!(
        flying.position(),
        before,
        "gravity must not pull a flying avatar down"
    );
    flying
        .world
        .get_component_by_id_as_mut::<InputXRGamepadComponent>(flying.input)
        .unwrap()
        .enable_flying = false;
    flying.tick(0.1, false);
    assert!(!flying.flying());
    VelocitySystem::default().step(&mut flying.world, &mut RxWorld::default());
    assert!(flying.position()[1] < before[1]);
}
#[test]
fn single_tap_jumps_and_controller_removal_restores_gravity() {
    let mut f = Fixture::new(true);
    f.world
        .get_component_by_id_as_mut::<VelocityComponent>(f.velocity)
        .unwrap()
        .grounded = true;
    f.tick(0.01, true);
    assert_eq!(
        f.world
            .get_component_by_id_as::<VelocityComponent>(f.velocity)
            .unwrap()
            .linear_local_mps,
        [0., 4.5, 0.]
    );
    f.tick(0.05, false);
    f.tick(0.05, true);
    assert!(f.flying());
    f.system.remove_input_xr_gamepad(&mut f.world, f.input);
    assert!(
        !f.world
            .get_component_by_id_as::<VelocityComponent>(f.velocity)
            .unwrap()
            .flying
    );
}

#[test]
fn flying_still_hits_static_collidable_walls() {
    use crate::engine::ecs::component::{CollidableComponent, ZoneComponent};
    use crate::engine::ecs::system::static_contact_system::StaticContactSystem;
    let mut f = Fixture::new(true);
    f.double_tap();
    let zone = attach(&mut f.world, f.target, ZoneComponent::capsule_y(0.2, 0.4));
    attach(
        &mut f.world,
        zone,
        CollidableComponent::slide().with_runtime_movement_target(Some(f.target)),
    );
    let wall = f
        .world
        .add_component(TransformComponent::new().with_position(1., 0., 0.));
    let zone = attach(&mut f.world, wall, ZoneComponent::cube([0.1, 5., 5.]));
    attach(&mut f.world, zone, CollidableComponent::static_());
    f.propagate();
    TransformSystem::new().transform_changed(
        &mut f.world,
        &mut f.visuals,
        wall,
        &mut TransformOperatorSystem::new(),
        &mut CameraSystem::new(),
        &mut LightSystem::new(),
    );
    let mut contacts = StaticContactSystem::default();
    contacts.tick(&mut f.world);
    f.xr.hands[0].thumbstick = Some([1., 0.]);
    for _ in 0..20 {
        f.tick(0.05, false);
        contacts.tick(&mut f.world);
        f.propagate();
    }
    assert!(
        f.position()[0] > 0.6 && f.position()[0] < 0.71,
        "flight must stop at the wall: {:?}",
        f.position()
    );
    assert!(f.flying());
}

#[test]
fn flight_handles_exact_window_and_disabling_during_input_loss() {
    let mut f = Fixture::new(true);
    f.tick(0.01, true);
    f.tick(0.125, false);
    f.tick(0.125, true);
    assert!(
        !f.flying(),
        "exactly 250 ms is outside the strict double-tap window"
    );
    f.tick(0.4, false);
    f.double_tap();
    assert!(f.flying());
    f.xr.active = false;
    f.tick(0.05, false);
    assert!(f.flying(), "input loss should hover");
    f.world
        .get_component_by_id_as_mut::<InputXRGamepadComponent>(f.input)
        .unwrap()
        .enabled = false;
    f.tick(0.05, false);
    assert!(!f.flying());
    assert!(
        !f.world
            .get_component_by_id_as::<VelocityComponent>(f.velocity)
            .unwrap()
            .flying
    );
}
