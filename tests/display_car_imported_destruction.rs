use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, EventSignal, KeyboardEvent, Signal, World,
        component::{
            CutGeometryComponent, DestroyableComponent, MeshComponent, RenderableComponent,
            TransformComponent,
        },
        system::TransformSystem,
    },
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
};

#[derive(Default)]
struct Upload {
    next: u32,
    meshes: std::collections::HashMap<MeshHandle, CpuMesh>,
}
impl MeshUploader for Upload {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        self.next += 1;
        let handle = MeshHandle(self.next);
        self.meshes.insert(handle, mesh.clone());
        Ok(handle)
    }
}
impl TextureUploader for Upload {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.next += 1;
        Ok(TextureHandle(self.next))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        self.next += 1;
        Ok(TextureHandle(self.next))
    }
}
fn named(w: &World, label: &str) -> ComponentId {
    w.all_components()
        .find(|&id| w.component_label(id) == Some(label))
        .unwrap()
}
fn visual_mesh(scene: &Universe, component: ComponentId) -> Option<MeshHandle> {
    scene
        .visuals
        .instances()
        .iter()
        .find(|v| v.component == component)
        .map(|v| v.renderable.mesh)
}
fn query(w: &World, scope: ComponentId, selector: &str) -> ComponentId {
    w.find_component(scope, selector)
        .unwrap_or_else(|| panic!("missing {selector}"))
}
fn prepare(scene: &mut Universe, upload: &mut Upload) {
    scene.update(0.016, &Default::default());
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        upload,
        &mut scene.command_queue,
    );
}
fn world_point(m: [[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| m[0][i] * p[0] + m[1][i] * p[1] + m[2][i] * p[2] + m[3][i])
}
fn source_mesh<'a>(scene: &'a Universe, source: ComponentId) -> &'a CpuMesh {
    let mesh = scene
        .world
        .children_of(source)
        .iter()
        .find_map(|&id| scene.world.get_component_by_id_as::<MeshComponent>(id))
        .unwrap();
    scene
        .render_assets
        .cpu_mesh(scene.render_assets.imported_mesh(&mesh.key).unwrap())
        .unwrap()
}
fn target_point(scene: &Universe, source: ComponentId) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let mesh = source_mesh(scene, source);
    let mut best = None;
    for triangle in mesh.indices_u32.chunks_exact(3) {
        let p: Vec<_> = triangle
            .iter()
            .map(|&i| mesh.vertices[i as usize].pos)
            .collect();
        let local: [f32; 3] = std::array::from_fn(|i| (p[0][i] + p[1][i] + p[2][i]) / 3.0);
        let a: [f32; 3] = std::array::from_fn(|i| p[1][i] - p[0][i]);
        let b: [f32; 3] = std::array::from_fn(|i| p[2][i] - p[0][i]);
        let n = [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ];
        let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length == 0. {
            continue;
        }
        let normal = n.map(|v| v / length);
        // Aim at an exposed front face, away from the cabin and the root caps.
        if local[0].abs() > 2.5
            && normal[2] < -0.5
            && best.as_ref().is_none_or(|(area, _, _)| length > *area)
        {
            best = Some((length, local, normal));
        }
    }
    let (_, local, normal) = best.expect("wing needs an exposed target triangle");
    (
        world_point(
            TransformSystem::world_model(&scene.world, source).unwrap(),
            local,
        ),
        local,
        normal,
    )
}
fn finish_cut(scene: &mut Universe, upload: &mut Upload, before: u64, source: ComponentId) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        prepare(scene, upload);
        if scene.systems.destroyable.cuts > before && visual_mesh(scene, source).is_none() {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "imported cut was not committed"
        );
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    for _ in 0..3 {
        prepare(scene, upload);
    }
}

fn fire(scene: &mut Universe, upload: &mut Upload, point: [f32; 3], normal: [f32; 3]) {
    let muzzle = named(&scene.world, "smoke_muzzle");
    let t = scene
        .world
        .get_component_by_id_as_mut::<TransformComponent>(muzzle)
        .unwrap();
    t.transform.translation = std::array::from_fn(|i| point[i] + normal[i] * 0.6);
    let mut rotation = [-normal[1], normal[0], 0., 1. + normal[2]];
    let length = rotation.iter().map(|v| v * v).sum::<f32>().sqrt();
    if length < 1e-6 {
        rotation = [0., 1., 0., 0.];
    } else {
        rotation = rotation.map(|v| v / length);
    }
    t.transform.rotation = rotation;
    t.transform.recompute_model();
    scene
        .systems
        .transform_changed(&mut scene.world, &mut scene.visuals, muzzle);
    scene.systems.rx.dispatch_event_handlers(
        &mut scene.world,
        &Signal::event(
            ComponentId::default(),
            EventSignal::KeyDown(KeyboardEvent {
                code: Some("Space".into()),
                key: " ".into(),
            }),
        ),
    );
    for _ in 0..5 {
        prepare(scene, upload);
    }
    let hit =
        TransformSystem::world_position(&scene.world, named(&scene.world, "smoke_hit")).unwrap();
    let distance = (0..3)
        .map(|i| (hit[i] - point[i]).powi(2))
        .sum::<f32>()
        .sqrt();
    assert!(
        distance < 0.001,
        "laser must hit the intended wing triangle: {hit:?}, expected {point:?}"
    );
}

#[test]
fn car_wings_take_laser_damage_independently_and_cut_geometry_follows_vehicle() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r##"
        import {display_car} from "../assets/components/vehicles/display_car.mms"
        import {laser} from "../assets/components/laser.mms"
        display_car("first_car",[0,0,0],0,"first_mount",[])
        display_car("second_car",[14,0,0],0,"second_mount",[])
        query("#first_car_gravity").set_enabled(false)
        query("#second_car_gravity").set_enabled(false)
        T {name="smoke_muzzle"}
        let hit_marker=T.position(100,100,100) {name="smoke_hit"}
        let weapon=laser({name="smoke_laser" length=4.0})
        on(world,"KeyDown",fn(event) {
            weapon.launch({pose=query("#smoke_muzzle").world_trs() source_tool=null source_user=null
                on_hit=fn(hit) {
                    hit_marker.update_transform(hit.hit_point,[0,0,0],[1,1,1])
                    Damage.at(hit.hit_point[0],hit.hit_point[1],hit.hit_point[2],0.28,1.0)
                }
            })
        })
    "##,
            "examples/_car_wing_damage.mms",
        )
        .unwrap();
    // Native material compilation must finish before this short-lived fixture exits.
    let definitions: Vec<_> = scene
        .visuals
        .materials
        .instances()
        .filter_map(|(_, instance)| {
            scene
                .visuals
                .materials
                .definition(instance.definition)?
                .custom
                .clone()
        })
        .collect();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while definitions
        .iter()
        .any(|definition| definition.compiled.get().is_none())
    {
        assert!(
            std::time::Instant::now() < deadline,
            "material compilation timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let mut upload = Upload::default();
    for _ in 0..5 {
        prepare(&mut scene, &mut upload);
    }
    let first = named(&scene.world, "first_car");
    let second = named(&scene.world, "second_car");
    for root in [first, second] {
        let markers = scene.world.find_all_components(root, "destroyable");
        assert_eq!(
            markers.len(),
            2,
            "only the two validated wing primitives get damage markers"
        );
        for marker in markers {
            let render = scene.world.parent_of(marker).unwrap();
            assert!(
                scene
                    .world
                    .get_component_by_id_as::<RenderableComponent>(render)
                    .is_some()
            );
            let part = scene.world.parent_of(render).unwrap();
            assert!(matches!(
                scene.world.component_label(part),
                Some("vehicle_body_wing_left" | "vehicle_body_wing_right")
            ));
            assert_eq!(
                scene
                    .world
                    .get_component_by_id_as::<DestroyableComponent>(marker)
                    .unwrap()
                    .threshold,
                3.
            );
        }
    }
    let left = query(&scene.world, first, "#vehicle_body_wing_left > renderable");
    let right = query(&scene.world, first, "#vehicle_body_wing_right > renderable");
    let other_left = query(&scene.world, second, "#vehicle_body_wing_left > renderable");
    let untouched_left = visual_mesh(&scene, other_left).unwrap();
    let untouched_right = visual_mesh(&scene, right).unwrap();
    let right_original = source_mesh(&scene, right).clone();
    let left_original = source_mesh(&scene, left).clone();
    let wing_part = query(&scene.world, first, "#vehicle_body_wing_left");
    let original = scene
        .world
        .children_of(left)
        .iter()
        .find_map(|&id| {
            scene
                .world
                .get_component_by_id_as::<CutGeometryComponent>(id)
        })
        .unwrap()
        .mesh
        .clone();
    let (point, local, normal) = target_point(&scene, left);
    for _ in 0..2 {
        fire(&mut scene, &mut upload, point, normal);
    }
    assert_eq!(
        scene.systems.destroyable.cuts, 0,
        "two shots remain below threshold"
    );
    fire(&mut scene, &mut upload, point, normal);
    finish_cut(&mut scene, &mut upload, 0, left);
    assert_eq!(
        visual_mesh(&scene, right),
        Some(untouched_right),
        "left damage must not change the right wing"
    );
    assert_eq!(
        visual_mesh(&scene, other_left),
        Some(untouched_left),
        "damage must not leak across instances"
    );
    let precise: Vec<_> = scene
        .world
        .find_all_components(wing_part, "cut_geometry")
        .into_iter()
        .filter_map(|id| {
            scene
                .world
                .get_component_by_id_as::<CutGeometryComponent>(id)
        })
        .filter(|g| !std::sync::Arc::ptr_eq(&g.mesh, &original))
        .map(|g| g.mesh.clone())
        .collect();
    assert!(
        !precise.is_empty(),
        "a successful cut must publish replacement geometry"
    );
    let ray_origin = std::array::from_fn(|i| local[i] + normal[i] * 0.04);
    assert!(original.ray(ray_origin, normal.map(|v| -v)).is_some());
    assert!(
        precise.iter().all(|g| g
            .ray(ray_origin, normal.map(|v| -v))
            .is_none_or(|t| t > 0.08)),
        "rays should pass through the actual cut hole"
    );
    let replacement = scene
        .world
        .find_all_components(wing_part, "renderable")
        .into_iter()
        .find(|&id| id != left && visual_mesh(&scene, id).is_some())
        .unwrap();
    let handle = visual_mesh(&scene, replacement).unwrap();
    let damaged = upload.meshes.get(&handle).unwrap();
    assert_surviving_uvs(&left_original, damaged, local);
    eprintln!(
        "wing cut worker time (including attribute interpolation): {:?}",
        scene.systems.destroyable.last_cut_time
    );
    let before = TransformSystem::world_position(&scene.world, replacement).unwrap();
    let t = scene
        .world
        .get_component_by_id_as_mut::<TransformComponent>(first)
        .unwrap();
    t.transform.translation[0] += 2.;
    t.transform.recompute_model();
    scene
        .systems
        .transform_changed(&mut scene.world, &mut scene.visuals, first);
    for _ in 0..3 {
        prepare(&mut scene, &mut upload);
    }
    let after = TransformSystem::world_position(&scene.world, replacement).unwrap();
    assert!(
        (after[0] - before[0] - 2.).abs() < 0.001,
        "cut mesh must follow the car"
    );
    // Smoke the other wing after motion too, using a fresh exposed surface.
    let (point, _, normal) = target_point(&scene, right);
    let before_cuts = scene.systems.destroyable.cuts;
    for _ in 0..3 {
        fire(&mut scene, &mut upload, point, normal);
    }
    finish_cut(&mut scene, &mut upload, before_cuts, right);
    assert!(
        source_mesh(&scene, right).indices_u32 == right_original.indices_u32,
        "shared source asset remains immutable"
    );
    assert_eq!(visual_mesh(&scene, other_left), Some(untouched_left));
}

fn assert_surviving_uvs(source: &CpuMesh, damaged: &CpuMesh, hit: [f32; 3]) {
    let mut originals = std::collections::HashMap::<[u32; 3], Vec<[f32; 2]>>::new();
    for v in &source.vertices {
        originals
            .entry(v.pos.map(f32::to_bits))
            .or_default()
            .push(v.uv);
    }
    let mut checked = 0;
    for v in &damaged.vertices {
        if (0..3).map(|i| (v.pos[i] - hit[i]).powi(2)).sum::<f32>() < 0.4 * 0.4 {
            continue;
        }
        if let Some(uvs) = originals.get(&v.pos.map(f32::to_bits)) {
            assert!(
                uvs.iter()
                    .any(|uv| (0..2).all(|i| (uv[i] - v.uv[i]).abs() < 1e-5)),
                "undamaged wing vertex changed its atlas UV: {:?}, {:?}, expected {:?}",
                v.pos,
                v.uv,
                uvs
            );
            checked += 1;
        }
    }
    assert!(
        checked > 100,
        "verify a substantial area of the original wing surface"
    );
}
