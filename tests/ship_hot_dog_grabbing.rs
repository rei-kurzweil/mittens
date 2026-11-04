use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, IntentValue, SignalEmitter, World,
        component::*,
        system::{TransformSystem, grabbable_system::grabbable_owner_for_hit},
    },
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
};

struct Upload(u32);
impl MeshUploader for Upload {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(MeshHandle(self.0))
    }
}
impl TextureUploader for Upload {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(TextureHandle(self.0))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.0 += 1;
        Ok(TextureHandle(self.0))
    }
}
fn named(world: &World, label: &str) -> ComponentId {
    world
        .all_components()
        .find(|&id| world.component_label(id) == Some(label))
        .unwrap()
}
fn tick(scene: &mut Universe) {
    for _ in 0..3 {
        scene.update(0.02, &Default::default());
    }
}
fn grab(scene: &mut Universe, pointer: ComponentId, target: ComponentId, renderable: ComponentId) {
    scene.command_queue.push_event(
        renderable,
        EventSignal::GrabStart {
            pointer,
            raycaster: pointer,
            renderable,
            target,
            ray_origin_world: [4., 1.4, 0.],
            ray_dir_world: [0., 0., -1.],
        },
    );
    tick(scene);
    // Settle the existing native grab easing without wall-clock waits.
    scene.systems.grabbable.tick(
        &mut scene.world,
        &scene.render_assets,
        &mut scene.command_queue,
        1.0,
    );
    tick(scene);
}

#[test]
fn sausages_and_buns_grab_independently_into_solved_hands() {
    let mut scene = Universe::new(World::default());
    scene
        .systems
        .xr
        .set_external_frame(Some(Default::default()));
    scene.load_mms_source_at_path(r##"
        import {hot_dog,hot_dog_sausage} from "../assets/components/food/hot_dog.mms"
        import {hot_dog_grill} from "../assets/components/hot_dog_grill.mms"
        T {
            name="food_test"
            hot_dog_grill()
            T.position(2,1,0) {name="served_food" hot_dog(true)}
            // Explicit solved-palm frames exercise the same native attachment
            // path that AVC supplies to the gun's XR pointers.
            T.position(0.4,1.2,0) {name="right_palm"}
            T.position(-0.4,1.2,0) {name="left_palm"}
            T.position(4,1.4,0) {name="raw_right_pointer"
                Pointer.grab_attachment("#right_palm").min_grab_distance(0.75) {name="right_pointer"}}
            T.position(-4,1.4,0) {
                Pointer.grab_attachment("#left_palm").min_grab_distance(0.75) {name="left_pointer"}}
        }
    "##,"examples/_food_grab_test.mms").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    for (_, record) in scene.visuals.materials.instances() {
        if let Some(custom) = scene
            .visuals
            .materials
            .definition(record.definition)
            .and_then(|d| d.custom.as_ref())
        {
            while custom.compiled.get().is_none() {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
    }
    tick(&mut scene);
    let mut upload = Upload(1000);
    for _ in 0..3 {
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut upload,
            &mut scene.command_queue,
        );
        tick(&mut scene);
    }
    let sausages: Vec<_> = scene
        .world
        .all_components()
        .filter(|&id| scene.world.component_label(id) == Some("hot_dog_sausage_grip"))
        .collect();
    assert_eq!(
        sausages.len(),
        7,
        "six grill sausages plus the vendor-style serving"
    );
    let served = named(&scene.world, "served_food");
    let sausage = scene
        .world
        .find_component(served, "#hot_dog_sausage_grip")
        .unwrap();
    let bun = scene
        .world
        .find_component(served, "#hot_dog_bun_grip")
        .unwrap();
    let hit = |id| {
        scene
            .visuals
            .instances()
            .iter()
            .find(|instance| grabbable_owner_for_hit(&scene.world, instance.component) == Some(id))
            .unwrap()
            .component
    };
    // Verify the actual combined render meshes still resolve to each item,
    // rather than losing their identity inside a shared grill/meal batch.
    for &part in sausages.iter().chain(std::iter::once(&bun)) {
        hit(part);
        let marker = scene
            .world
            .children_of(part)
            .iter()
            .find_map(|&id| scene.world.get_component_by_id_as::<GrabbableComponent>(id))
            .unwrap();
        assert!(marker.to_hand && !marker.move_parent);
    }
    let sausage_hit = hit(sausage);
    let bun_hit = hit(bun);
    let bun_parent = scene.world.parent_of(bun);
    let sausage_parent = scene.world.parent_of(sausage);
    let bun_before = TransformSystem::world_position(&scene.world, bun).unwrap();
    let tray_before = TransformSystem::world_position(&scene.world, served).unwrap();
    let right = named(&scene.world, "right_pointer");
    let left = named(&scene.world, "left_pointer");
    let right_palm = named(&scene.world, "right_palm");
    let left_palm = named(&scene.world, "left_palm");
    grab(&mut scene, right, sausage, sausage_hit);
    assert_eq!(scene.world.parent_of(sausage), Some(right_palm));
    assert_eq!(scene.world.parent_of(bun), bun_parent);
    assert_eq!(
        TransformSystem::world_position(&scene.world, bun),
        Some(bun_before)
    );
    grab(&mut scene, left, bun, bun_hit);
    assert_eq!(scene.world.parent_of(bun), Some(left_palm));
    for part in [sausage, bun] {
        let pose = &scene
            .world
            .get_component_by_id_as::<TransformComponent>(part)
            .unwrap()
            .transform;
        assert!(pose.translation[0].abs() < 0.001 && pose.translation[1].abs() < 0.001);
        assert!(
            (pose.translation[2] + 0.035).abs() < 0.001,
            "food belongs at the palm, not 75 cm down the ray"
        );
    }
    let held_before = TransformSystem::world_position(&scene.world, sausage).unwrap();
    scene.command_queue.push_intent_now(
        right,
        IntentValue::UpdateTransform {
            component_id: named(&scene.world, "raw_right_pointer"),
            translation: [40., 20., 30.],
            rotation_quat_xyzw: [0., 0., 0., 1.],
            scale: [1.; 3],
        },
    );
    tick(&mut scene);
    assert_eq!(scene.world.parent_of(sausage), Some(right_palm));
    assert_eq!(
        TransformSystem::world_position(&scene.world, sausage),
        Some(held_before)
    );
    assert_eq!(
        TransformSystem::world_position(&scene.world, served),
        Some(tray_before)
    );
    for (pointer, target) in [(right, sausage), (left, bun)] {
        scene
            .command_queue
            .push_event(target, EventSignal::GrabEnd { pointer, target });
    }
    tick(&mut scene);
    assert_eq!(scene.world.parent_of(sausage), sausage_parent);
    assert_eq!(scene.world.parent_of(bun), bun_parent);
}
