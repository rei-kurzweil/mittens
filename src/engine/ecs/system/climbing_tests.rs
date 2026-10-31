use super::*;
use crate::engine::ecs::component::{ControllerHand, ControllerPoseKind};
use crate::engine::ecs::system::{CameraSystem, LightSystem, TransformOperatorSystem};
use crate::engine::graphics::VisualWorld;

fn propagate(world: &mut World, root: ComponentId) {
    TransformSystem::new().transform_changed(
        world,
        &mut VisualWorld::default(),
        root,
        &mut TransformOperatorSystem::new(),
        &mut CameraSystem::new(),
        &mut LightSystem::new(),
    );
}
struct Fixture {
    world: World,
    system: ClimbingSystem,
    rx: RxWorld,
    root: ComponentId,
    velocity: ComponentId,
    hands: [ComponentId; 2],
    drivers: [ComponentId; 2],
    surface: ComponentId,
}
impl Fixture {
    fn new() -> Self {
        let mut world = World::default();
        let velocity = world.add_component(VelocityComponent::new());
        let root = world.add_component(TransformComponent::new());
        let input = world.add_component(InputXRComponent::on());
        world.add_child(velocity, root).unwrap();
        world.add_child(root, input).unwrap();
        let mut hands = Vec::new();
        let mut drivers = Vec::new();
        for side in [ControllerHand::Left, ControllerHand::Right] {
            let controller = world.add_component(ControllerXRComponent::new(
                true,
                side,
                ControllerPoseKind::GripAim,
            ));
            let driver = world.add_component(TransformComponent::new());
            let pointer = world.add_component(PointerComponent::new());
            world.add_child(input, controller).unwrap();
            world.add_child(controller, driver).unwrap();
            world.add_child(driver, pointer).unwrap();
            hands.push(pointer);
            drivers.push(driver);
        }
        let surface = world.add_component(TransformComponent::new());
        let marker = world.add_component(ClimbableComponent::new());
        world.add_child(surface, marker).unwrap();
        propagate(&mut world, root);
        propagate(&mut world, surface);
        Self {
            world,
            system: ClimbingSystem::default(),
            rx: RxWorld::default(),
            root,
            velocity,
            hands: [hands[0], hands[1]],
            drivers: [drivers[0], drivers[1]],
            surface,
        }
    }
    fn command(&mut self, command: Command) {
        self.system.commands.lock().unwrap().push(command);
    }
    fn tick(&mut self) {
        self.system.tick(&mut self.world, &mut self.rx);
        propagate(&mut self.world, self.root);
    }
    fn start(&mut self, side: usize) {
        self.command(Command::Start {
            pointer: self.hands[side],
            renderable: self.surface,
        });
        self.tick();
    }
    fn hand(&mut self, side: usize, p: [f32; 3]) {
        let t = self
            .world
            .get_component_by_id_as_mut::<TransformComponent>(self.drivers[side])
            .unwrap();
        t.transform.translation = p;
        t.transform.recompute_model();
        propagate(&mut self.world, self.drivers[side]);
    }
    fn position(&self) -> [f32; 3] {
        self.world
            .get_component_by_id_as::<TransformComponent>(self.root)
            .unwrap()
            .transform
            .translation
    }
}

#[test]
fn pull_keeps_pointer_fixed_and_release_stops_moving() {
    let mut f = Fixture::new();
    f.start(0);
    f.hand(0, [0.0, -0.4, 0.2]);
    f.tick();
    assert_eq!(f.position(), [0.0, 0.4, -0.2]);
    assert_eq!(
        TransformSystem::world_position(&f.world, f.hands[0]).unwrap(),
        [0.0; 3]
    );
    f.world
        .get_component_by_id_as_mut::<VelocityComponent>(f.velocity)
        .unwrap()
        .linear_local_mps = [1.0, -8.0, 0.0];
    f.system.prepare_velocity(&mut f.world);
    assert_eq!(
        f.world
            .get_component_by_id_as::<VelocityComponent>(f.velocity)
            .unwrap()
            .linear_local_mps,
        [0.0; 3]
    );
    f.command(Command::End(f.hands[0]));
    f.tick();
    f.hand(0, [0.0, -1.0, 0.0]);
    f.tick();
    assert_eq!(f.position(), [0.0, 0.4, -0.2]);
}

#[test]
fn newest_hand_takes_over_and_release_rebases_remaining_hand() {
    let mut f = Fixture::new();
    f.start(0);
    f.hand(0, [0.0, -0.3, 0.0]);
    f.tick();
    f.start(1);
    f.hand(1, [0.0, -0.2, 0.0]);
    f.tick();
    assert_eq!(f.position(), [0.0, 0.5, 0.0]);
    f.command(Command::End(f.hands[1]));
    f.tick();
    assert_eq!(f.position(), [0.0, 0.5, 0.0]);
    f.hand(0, [0.0, -0.4, 0.0]);
    f.tick();
    assert!((f.position()[1] - 0.6).abs() < 1e-5);
}

#[test]
fn scaled_parent_converts_world_pull_to_local_motion() {
    let mut f = Fixture::new();
    let parent = f
        .world
        .add_component(TransformComponent::new().with_scale(2.0, 2.0, 2.0));
    f.world.add_child(parent, f.velocity).unwrap();
    propagate(&mut f.world, parent);
    f.start(0);
    f.hand(0, [0.0, -0.25, 0.0]);
    f.tick();
    assert_eq!(f.position(), [0.0, 0.25, 0.0]);
    assert_eq!(
        TransformSystem::world_position(&f.world, f.hands[0]).unwrap(),
        [0.0; 3]
    );
}

#[test]
fn climbing_motion_is_constrained_by_static_contact() {
    use crate::engine::ecs::component::{CollidableComponent, ZoneComponent};
    use crate::engine::ecs::system::static_contact_system::StaticContactSystem;
    let mut f = Fixture::new();
    let zone = f.world.add_component(ZoneComponent::sphere(0.25));
    let mover = f.world.add_component(CollidableComponent::slide());
    f.world.add_child(f.root, zone).unwrap();
    f.world.add_child(zone, mover).unwrap();
    let wall = f
        .world
        .add_component(TransformComponent::new().with_position(0.6, 0.0, 0.0));
    let zone = f.world.add_component(ZoneComponent::cube([0.1, 2.0, 2.0]));
    let fixed = f.world.add_component(CollidableComponent::static_());
    f.world.add_child(wall, zone).unwrap();
    f.world.add_child(zone, fixed).unwrap();
    propagate(&mut f.world, wall);
    let mut contact = StaticContactSystem::default();
    contact.tick(&mut f.world);
    f.start(0);
    f.hand(0, [-0.4, 0.0, 0.0]);
    f.tick();
    for target in contact.tick(&mut f.world) {
        propagate(&mut f.world, target);
    }
    assert!((f.position()[0] - 0.25).abs() < 1e-4, "{:?}", f.position());
}

#[test]
fn climbable_registration_supplies_default_raycast_policy() {
    use crate::engine::ecs::component::RaycastableComponent;
    use crate::engine::ecs::system::GrabbableSystem;
    let mut world = World::default();
    let owner = world.add_component(TransformComponent::new());
    let marker = world.add_component(ClimbableComponent::new());
    world.add_child(owner, marker).unwrap();
    GrabbableSystem::default().register(&mut world, marker, &mut RxWorld::default());
    assert!(world.children_of(owner).iter().any(|id| {
        world
            .get_component_by_id_as::<RaycastableComponent>(*id)
            .is_some()
    }));
}
