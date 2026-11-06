use mittens_engine::engine::{
    Universe,
    ecs::{
        ComponentId, World,
        component::{
            CutGeometryComponent, DamageComponent, DestroyableComponent, EditorComponent,
            EditorUIComponent, MeshComponent, RenderableComponent, TransformComponent,
        },
        system::TransformSystem,
    },
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
};

#[derive(Default)]
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
    let points: Vec<_> = mesh.indices_u32[..3]
        .iter()
        .map(|&i| mesh.vertices[i as usize].pos)
        .collect();
    let local: [f32; 3] =
        std::array::from_fn(|i| (points[0][i] + points[1][i] + points[2][i]) / 3.0);
    let a: [f32; 3] = std::array::from_fn(|i| points[1][i] - points[0][i]);
    let b: [f32; 3] = std::array::from_fn(|i| points[2][i] - points[0][i]);
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let length = n.iter().map(|v| v * v).sum::<f32>().sqrt();
    let normal = n.map(|v| v / length);
    (
        world_point(
            TransformSystem::world_model(&scene.world, source).unwrap(),
            local,
        ),
        local,
        normal,
    )
}
fn damage(scene: &mut Universe, point: [f32; 3], amount: f32) {
    scene.world.add_component(DamageComponent {
        position: point,
        radius: 0.16,
        amount,
    });
}
fn finish_cut(scene: &mut Universe, upload: &mut Upload, before: u64) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        prepare(scene, upload);
        if scene.systems.destroyable.cuts > before {
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

#[test]
fn trucks_cut_imported_parts_keep_cabin_and_other_instance_and_follow_motion() {
    let mut scene = Universe::new(World::default());
    scene
        .load_mms_source_at_path(
            r#"
        import {pickup_truck} from "../assets/components/vehicles/pickup_truck.mms"
        T {name="first_truck" pickup_truck(null,null,false)}
        T.position(12,0,0) {name="second_truck" pickup_truck(null,null,false)}
    "#,
            "examples/_pickup_imported_damage.mms",
        )
        .unwrap();
    let mut upload = Upload::default();
    for _ in 0..5 {
        prepare(&mut scene, &mut upload);
    }
    let first = named(&scene.world, "first_truck");
    let second = named(&scene.world, "second_truck");
    let source = query(&scene.world, first, "[name='door_left'] > renderable");
    let other = query(&scene.world, second, "[name='door_left'] > renderable");
    let marker_count = |scene: &Universe, scope| {
        scene
            .world
            .find_all_components(scope, "destroyable")
            .into_iter()
            .filter(|&id| {
                scene.world.parent_of(id).is_some_and(|parent| {
                    scene
                        .world
                        .get_component_by_id_as::<RenderableComponent>(parent)
                        .is_some()
                })
            })
            .count()
    };
    assert_eq!(marker_count(&scene, first), 22);
    assert_eq!(marker_count(&scene, second), 22);
    let cab = query(&scene.world, first, "#pickup_painted_body > renderable");
    assert!(!scene.world.children_of(cab).iter().any(|&id| {
        scene
            .world
            .get_component_by_id_as::<DestroyableComponent>(id)
            .is_some()
    }));
    assert!(!scene.world.all_components().any(|id| {
        scene
            .world
            .get_component_by_id_as::<EditorComponent>(id)
            .is_some()
            || scene
                .world
                .get_component_by_id_as::<EditorUIComponent>(id)
                .is_some()
    }));
    let hydraulics = query(&scene.world, first, "#pickup_hydraulics_panel");
    assert!(
        (scene
            .world
            .get_component_by_id_as::<TransformComponent>(hydraulics)
            .unwrap()
            .translation()[1]
            - 1.80)
            .abs()
            < 1e-6
    );
    let original = scene
        .world
        .children_of(source)
        .iter()
        .find_map(|&id| {
            scene
                .world
                .get_component_by_id_as::<CutGeometryComponent>(id)
        })
        .unwrap()
        .mesh
        .clone();
    let other_original = scene
        .world
        .children_of(other)
        .iter()
        .find_map(|&id| {
            scene
                .world
                .get_component_by_id_as::<CutGeometryComponent>(id)
        })
        .unwrap()
        .mesh
        .clone();
    assert!(
        std::sync::Arc::ptr_eq(&original, &other_original),
        "immutable geometry queries should be shared"
    );
    let batch = query(&scene.world, first, "#pickup_painted_body");
    let second_batch = query(&scene.world, second, "#pickup_painted_body");
    let untouched = visual_mesh(&scene, second_batch).unwrap();
    let (point, local, normal) = target_point(&scene, source);
    damage(&mut scene, point, 1.0);
    for _ in 0..3 {
        prepare(&mut scene, &mut upload);
    }
    assert_eq!(scene.systems.destroyable.cuts, 0);
    assert!(
        visual_mesh(&scene, batch).is_some(),
        "subthreshold damage retains batching"
    );
    damage(&mut scene, point, 2.0);
    finish_cut(&mut scene, &mut upload, 0);
    assert!(
        visual_mesh(&scene, batch).is_none(),
        "commit unpacks the affected truck's batch"
    );
    assert_eq!(visual_mesh(&scene, second_batch), Some(untouched));
    assert!(
        visual_mesh(&scene, cab).is_some(),
        "excluded cabin remains visible"
    );
    let part = query(&scene.world, first, "[name='door_left']");
    let derived_geometry: Vec<_> = scene
        .world
        .find_all_components(part, "cut_geometry")
        .into_iter()
        .filter_map(|id| {
            scene
                .world
                .get_component_by_id_as::<CutGeometryComponent>(id)
        })
        .filter(|g| !std::sync::Arc::ptr_eq(&g.mesh, &original))
        .collect();
    assert!(
        !derived_geometry.is_empty(),
        "cut must publish precise replacement geometry"
    );
    let origin = std::array::from_fn(|i| local[i] + normal[i] * 0.04);
    let direction = normal.map(|v| -v);
    assert!(original.ray(origin, direction).is_some());
    assert!(
        derived_geometry
            .iter()
            .all(|g| g.mesh.ray(origin, direction).is_none_or(|t| t > 0.08)),
        "rays must pass through the actual panel hole"
    );
    let visible = scene
        .world
        .find_all_components(part, "renderable")
        .into_iter()
        .find(|&id| id != source && visual_mesh(&scene, id).is_some())
        .unwrap();
    let before = TransformSystem::world_position(&scene.world, visible).unwrap();
    let root = scene
        .world
        .get_component_by_id_as_mut::<TransformComponent>(first)
        .unwrap();
    root.transform.translation[0] += 2.0;
    root.transform.recompute_model();
    scene
        .systems
        .transform_changed(&mut scene.world, &mut scene.visuals, first);
    for _ in 0..3 {
        prepare(&mut scene, &mut upload);
    }
    let moved = TransformSystem::world_position(&scene.world, visible).unwrap();
    assert!(
        (moved[0] - before[0] - 2.0).abs() < 1e-4,
        "replacement must follow the moving truck"
    );
    let wheel = query(
        &scene.world,
        first,
        "[name='front-left-WHEEL'] > renderable",
    );
    let (point, _, _) = target_point(&scene, wheel);
    let before = scene.systems.destroyable.cuts;
    damage(&mut scene, point, 3.0);
    finish_cut(&mut scene, &mut upload, before);
    assert_eq!(visual_mesh(&scene, second_batch), Some(untouched));
}
