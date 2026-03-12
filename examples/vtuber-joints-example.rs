use cat_engine::engine::ecs::component::{
    AmbientLightComponent, BackgroundColorComponent, Camera3DComponent, ClockComponent,
    ColorComponent, DirectionalLightComponent, EditorComponent, EmissiveComponent, GLTFComponent,
    InputComponent, InputTransformModeComponent, MeshComponent, PointerComponent, RayCastComponent,
    RaycastableComponent, RenderableComponent, SkinnedMeshComponent, TransformComponent,
};
use cat_engine::{engine, utils};
use std::collections::{HashMap, HashSet};

#[path = "example_util/mod.rs"]
mod example_util;

fn main() {
    utils::logger::init();

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    // Slow the global beat clock so beat-based animations run half as fast.
    let clock = universe
        .world
        .add_component(ClockComponent::new().with_bpm(60.0));
    universe.add(clock);

    // Light pink background.
    let background = universe
        .world
        .add_component(BackgroundColorComponent::rgba(1.0, 0.82, 0.90, 1.0));
    universe.add(background);

    // Small ambient so shadowed areas aren't pure black.
    let ambient = universe
        .world
        .add_component(AmbientLightComponent::rgb(0.10, 0.10, 0.12));
    universe.add(ambient);

    // --- Camera rig (WASD + mouse) ---
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(1.5));
    let input_mode = universe.world.add_component(
        InputTransformModeComponent::forward_z()
            .with_fps_rotation()
            .with_roll_axis_y(),
    );
    let _ = universe.attach(input, input_mode);

    // Start slightly pulled back looking towards the origin.
    let rig_transform = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, 0.0, 6.0));
    let _ = universe.attach(input, rig_transform);

    let camera3d = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(rig_transform, camera3d);

    // Raycaster + pointer so gizmos can be interacted with.
    let raycaster = universe
        .world
        .add_component(RayCastComponent::event_driven().with_max_distance(100.0));
    let _ = universe.attach(rig_transform, raycaster);

    let pointer = universe.world.add_component(PointerComponent::new());
    let _ = universe.attach(raycaster, pointer);

    // Topology: I { T { C3D } } — add a small camera-attached controls hint.
    example_util::spawn_desktop_camera_controls_hint(&mut universe, rig_transform);

    universe.add(input);

    // --- lighting ---
    let sun = universe.world.add_component(
        DirectionalLightComponent::new()
            .with_intensity(1.2)
            .with_color(1.0, 0.98, 0.94),
    );
    let sun_dir = universe
        .world
        .add_component(TransformComponent::new().with_position(0.0, -0.35, 1.0));
    let _ = universe.attach(sun_dir, sun);
    universe.add(sun_dir);

    let light_transform = universe.world.add_component(
        TransformComponent::new()
            .with_position(1.0, 6.0, 3.0)
            .with_scale(0.1, 0.1, 0.1),
    );
    let light = universe.world.add_component(
        engine::ecs::component::PointLightComponent::new()
            .with_distance(120.0)
            .with_color(1.0, 1.0, 1.0),
    );
    let _ = universe.attach(light_transform, light);
    universe.add(light_transform);

    // --- VTuber model ---
    let model_uri = "assets/models/pc-rei.hoodie.glb";

    // Wrap the model subtree in an editor root so transform-only glTF nodes can be visualized
    // (and thus raycasted/selected) without affecting non-editor scenes.
    let editor_root = universe.world.add_component(EditorComponent::new());

    let model_root = universe.world.add_component(TransformComponent::new());
    let model = universe.world.add_component(GLTFComponent::new(model_uri));

    // emissive for pc-rei
    let emissive = universe
        .world
        .add_component(EmissiveComponent { enabled: true });
    let _ = universe.attach(model, emissive);

    let _ = universe.attach(model_root, model);

    let _ = universe.attach(editor_root, model_root);

    // Initialize the editor root so GLTFComponent gets registered.
    universe.add(editor_root);

    // --- Background clouds (occluded + lit) ---
    let bg_root = universe.world.add_component(
        engine::ecs::component::BackgroundComponent::new().with_occlusion_and_lighting(),
    );
    universe.add(bg_root);
    let mut cloud_params = example_util::CloudRingParams::default();
    cloud_params.cloud_count = 8; // +3 clusters
    cloud_params.angle_jitter = 0.35;
    cloud_params.high_y_probability = 0.5;
    cloud_params.high_y_multiplier = 1.5;
    cloud_params.seed = 0x57_55_B0_01u32;
    example_util::spawn_cloud_ring(&mut universe, bg_root, cloud_params);

    // --- Simple environment ---
    let spawn_cube = |universe: &mut engine::Universe,
                      position: (f32, f32, f32),
                      scale: (f32, f32, f32),
                      color: (f32, f32, f32, f32)| {
        let transform = universe.world.add_component(
            TransformComponent::new()
                .with_position(position.0, position.1, position.2)
                .with_scale(scale.0, scale.1, scale.2),
        );
        let renderable = universe.world.add_component(RenderableComponent::cube());
        let color = universe
            .world
            .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));

        let _ = universe.attach(transform, renderable);
        let _ = universe.attach(renderable, color);

        universe.add(transform);
    };

    // floor
    spawn_cube(
        &mut universe,
        (0.0, -0.05, 0.0),
        (10.0, 0.1, 10.0),
        (0.92, 0.92, 0.92, 1.0),
    );

    // back wall
    spawn_cube(
        &mut universe,
        (-3.0, 1.5, -5.0),
        (3.0, 3.0, 1.0),
        (0.95, 0.94, 0.96, 1.0),
    );

    // desk
    spawn_cube(
        &mut universe,
        (0.0, 0.35, 1.0),
        (1.0, 0.75, 0.5),
        (0.75, 0.70, 0.65, 1.0),
    );

    // --- Editor-side stacked cubes (inside the editor subtree for picking/gizmos) ---
    {
        let spawn_editor_cube = |universe: &mut engine::Universe,
                                 editor_root: engine::ecs::ComponentId,
                                 name: &str,
                                 position: (f32, f32, f32),
                                 scale: (f32, f32, f32),
                                 color: (f32, f32, f32, f32)| {
            let transform = universe.world.add_component_boxed_named(
                format!("{name}_t"),
                Box::new(
                    TransformComponent::new()
                        .with_position(position.0, position.1, position.2)
                        .with_scale(scale.0, scale.1, scale.2),
                ),
            );
            let renderable = universe.world.add_component_boxed_named(
                format!("{name}_r"),
                Box::new(RenderableComponent::cube()),
            );
            let color_comp = universe.world.add_component_boxed_named(
                format!("{name}_color"),
                Box::new(ColorComponent::rgba(color.0, color.1, color.2, color.3)),
            );
            let raycastable = universe.world.add_component_boxed_named(
                format!("{name}_raycastable"),
                Box::new(RaycastableComponent::enabled()),
            );

            let _ = universe.world.add_child(transform, renderable);
            let _ = universe.world.add_child(renderable, color_comp);
            let _ = universe.world.add_child(renderable, raycastable);

            // One attach into the initialized editor subtree triggers init for the new subtree.
            let _ = universe.attach(editor_root, transform);
        };

        // Place the stack beside the desk (a bit to the right).
        let stack_x = 1.35;
        let stack_z = 1.0;
        let s = 0.25;
        let half = 0.5 * s;
        let light_brown = (0.80, 0.72, 0.55, 1.0);
        let cyan = (0.20, 1.00, 1.00, 1.0);

        spawn_editor_cube(
            &mut universe,
            editor_root,
            "editor_stack_0",
            (stack_x, half, stack_z),
            (s, s, s),
            light_brown,
        );
        spawn_editor_cube(
            &mut universe,
            editor_root,
            "editor_stack_1",
            (stack_x, half + 1.0 * s, stack_z),
            (s, s, s),
            light_brown,
        );
        spawn_editor_cube(
            &mut universe,
            editor_root,
            "editor_stack_2",
            (stack_x, half + 2.0 * s, stack_z),
            (s, s, s),
            light_brown,
        );
        spawn_editor_cube(
            &mut universe,
            editor_root,
            "editor_stack_top",
            (stack_x, half + 3.0 * s, stack_z),
            (s, s, s),
            cyan,
        );
    }

    let xr_root = universe
        .world
        .add_component(engine::ecs::component::OpenXRComponent::on());
    universe.add(xr_root);

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Spawn the glTF subtree once up-front so joint ComponentIds exist.
    {
        let systems = &mut universe.systems;
        systems.gltf.tick_with_queue(
            &mut universe.world,
            &mut universe.visuals,
            &mut systems.skinned_mesh,
            &mut universe.command_queue,
            0.0,
        );
    }
    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Register imported meshes into RenderAssets early so we can inspect skin weights
    // (textures will still be uploaded later during normal rendering).
    universe
        .systems
        .gltf
        .flush_mesh_imports_only(&mut universe.render_assets);

    // --- Joint printout + animation binding (in the example) ---
    let all_joints = collect_joint_transforms(
        &universe.world,
        &universe.systems.skinned_mesh,
        &universe.visuals,
        model,
        model_root,
    );
    println!("[vtuber-joints-example] joints found: {}", all_joints.len());
    for (i, (node_index, joint_tx)) in all_joints.iter().enumerate() {
        println!("  joint[{i:03}] node_index={node_index} transform={joint_tx:?}");
    }

    let node_index_to_transform: HashMap<usize, engine::ecs::ComponentId> =
        all_joints.iter().copied().collect();

    // Example settings are hardcoded to keep this example simple.
    let joint_offset: usize = 0;
    let wiggle_count: usize = 16;

    let target_mesh_key = "pc-rei.hoodie:Body_(merged).baked:prim0".to_string();
    let target_joint_names: Vec<String> = vec![
        "J_Bip_L_UpperArm".to_string(),
        "J_Bip_R_UpperArm".to_string(),
    ];

    let print_transform_updates: bool = false;

    let selected_joint_transforms: Vec<(usize, engine::ecs::ComponentId)> =
        select_named_joints(&universe.world, &all_joints, &target_joint_names)
            .or_else(|| {
                select_body_prim0_influencers(
                    &universe,
                    model_root,
                    &target_mesh_key,
                    wiggle_count,
                    &node_index_to_transform,
                )
            })
            .unwrap_or_else(|| select_joint_range(&all_joints, joint_offset, wiggle_count));
    println!(
        "[vtuber-joints-example] wiggle selection: target_mesh_key='{}' offset={} count={} selected={}",
        target_mesh_key,
        joint_offset,
        wiggle_count,
        selected_joint_transforms.len()
    );

    debug_print_selected_joint_influence(
        &universe,
        &target_mesh_key,
        model_root,
        &selected_joint_transforms,
    );

    println!("[vtuber-joints-example] selected joints:");
    for (i, (node_index, joint_tx)) in selected_joint_transforms.iter().enumerate() {
        let name = universe
            .world
            .get_component_record(*joint_tx)
            .map(|n| n.name.as_str())
            .unwrap_or("<unknown>");
        println!("  sel[{i:02}] node_index={node_index} name={name} transform={joint_tx:?}");
    }

    if print_transform_updates {
        println!("[vtuber-joints-example] note: joint animation disabled");
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}

fn select_named_joints(
    world: &engine::ecs::World,
    all_joints: &[(usize, engine::ecs::ComponentId)],
    names: &[String],
) -> Option<Vec<(usize, engine::ecs::ComponentId)>> {
    if names.is_empty() {
        return None;
    }

    let mut out: Vec<(usize, engine::ecs::ComponentId)> = Vec::new();
    for wanted in names {
        let mut found: Option<(usize, engine::ecs::ComponentId)> = None;
        for &(node_index, joint_tx) in all_joints.iter() {
            let name = world
                .get_component_record(joint_tx)
                .map(|r| r.name.as_str())
                .unwrap_or("");
            if name == wanted {
                found = Some((node_index, joint_tx));
                break;
            }
        }

        if let Some(v) = found {
            out.push(v);
        } else {
            println!(
                "[vtuber-joints-example] warning: target joint '{}' not found (check names via REPL tree)",
                wanted
            );
        }
    }

    if out.is_empty() { None } else { Some(out) }
}

fn debug_print_selected_joint_influence(
    universe: &engine::Universe,
    mesh_key: &str,
    model_root: engine::ecs::ComponentId,
    selected: &[(usize, engine::ecs::ComponentId)],
) {
    let Some(renderable) = find_renderable_by_mesh_key(&universe.world, model_root, mesh_key)
    else {
        println!(
            "[vtuber-joints-example] influence: mesh_key='{}' renderable not found",
            mesh_key
        );
        return;
    };

    let renderable_handle = universe
        .world
        .get_component_by_id_as::<RenderableComponent>(renderable)
        .and_then(|r| r.get_handle());
    println!(
        "[vtuber-joints-example] influence: mesh_key='{}' renderable={:?} instance_handle={:?}",
        mesh_key, renderable, renderable_handle
    );
    let Some(skin_id) = find_skin_id_for_renderable(&universe.world, renderable) else {
        println!(
            "[vtuber-joints-example] influence: mesh_key='{}' has no skin_id",
            mesh_key
        );
        return;
    };
    let Some(skin) = universe.visuals.skin(skin_id) else {
        println!(
            "[vtuber-joints-example] influence: skin_id={:?} missing in visuals",
            skin_id
        );
        return;
    };
    let Some(cpu_h) = universe.render_assets.imported_mesh(mesh_key) else {
        println!(
            "[vtuber-joints-example] influence: mesh_key='{}' missing in RenderAssets (did meshes register?)",
            mesh_key
        );
        return;
    };
    let Some(cpu) = universe.render_assets.cpu_mesh(cpu_h) else {
        println!(
            "[vtuber-joints-example] influence: mesh_key='{}' cpu mesh handle invalid",
            mesh_key
        );
        return;
    };
    let (Some(joints0), Some(weights0)) = (cpu.joints0.as_ref(), cpu.weights0.as_ref()) else {
        println!(
            "[vtuber-joints-example] influence: mesh_key='{}' has no skin attributes",
            mesh_key
        );
        return;
    };

    let joint_count = skin.joint_count();
    let totals = compute_total_weight_per_joint(joints0, weights0, joint_count);

    println!(
        "[vtuber-joints-example] influence: mesh_key='{}' verts={} joints={}",
        mesh_key,
        cpu.vertices.len(),
        joint_count
    );

    for &(node_index, joint_tx) in selected.iter() {
        // Find the skin joint index that maps to this node.
        let skin_joint_index = skin
            .joint_node_indices
            .iter()
            .position(|&ni| ni == node_index);

        let name = universe
            .world
            .get_component_record(joint_tx)
            .map(|r| r.name.as_str())
            .unwrap_or("<unknown>");

        match skin_joint_index {
            Some(j) => {
                let total = totals.get(j).copied().unwrap_or(0.0);
                println!(
                    "  influence: name='{}' node_index={} skin_joint_index={} total_weight={:.3}",
                    name, node_index, j, total
                );
            }
            None => {
                println!(
                    "  influence: name='{}' node_index={} skin_joint_index=<none>",
                    name, node_index
                );
            }
        }
    }
}

fn compute_total_weight_per_joint(
    joints0: &[[u16; 4]],
    weights0: &[[f32; 4]],
    joint_count: usize,
) -> Vec<f32> {
    let mut totals = vec![0.0f32; joint_count];
    if joint_count == 0 {
        return totals;
    }
    for (jv, wv) in joints0.iter().zip(weights0.iter()) {
        for lane in 0..4 {
            let j = jv[lane] as usize;
            if j >= joint_count {
                continue;
            }
            let w = wv[lane];
            if w > 0.0 {
                totals[j] += w;
            }
        }
    }
    totals
}

fn select_body_prim0_influencers(
    universe: &engine::Universe,
    model_root: engine::ecs::ComponentId,
    mesh_key: &str,
    count: usize,
    node_index_to_transform: &HashMap<usize, engine::ecs::ComponentId>,
) -> Option<Vec<(usize, engine::ecs::ComponentId)>> {
    if count == 0 {
        return Some(Vec::new());
    }

    let renderable = find_renderable_by_mesh_key(&universe.world, model_root, mesh_key)?;
    let skin_id = find_skin_id_for_renderable(&universe.world, renderable)?;
    let skin = universe.visuals.skin(skin_id)?;

    let cpu_h = universe.render_assets.imported_mesh(mesh_key)?;
    let cpu = universe.render_assets.cpu_mesh(cpu_h)?;
    let joints0 = cpu.joints0.as_ref()?;
    let weights0 = cpu.weights0.as_ref()?;

    let joint_count = skin.joint_count();
    if joint_count == 0 {
        return None;
    }

    let mut total_weight_per_joint: Vec<f32> = vec![0.0; joint_count];
    for (jv, wv) in joints0.iter().zip(weights0.iter()) {
        for lane in 0..4 {
            let j = jv[lane] as usize;
            if j >= joint_count {
                continue;
            }
            let w = wv[lane];
            if w > 0.0 {
                total_weight_per_joint[j] += w;
            }
        }
    }

    let mut ranked: Vec<(usize, f32)> =
        total_weight_per_joint.iter().copied().enumerate().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let min_total: f32 = 10.0;

    println!(
        "[vtuber-joints-example] auto joints for mesh_key='{}': verts={} joint_count={} min_total={}",
        mesh_key,
        cpu.vertices.len(),
        joint_count,
        min_total
    );

    let mut out: Vec<(usize, engine::ecs::ComponentId)> = Vec::with_capacity(count);
    let mut seen: HashSet<engine::ecs::ComponentId> = HashSet::new();

    for (joint_index, total) in ranked.into_iter() {
        if out.len() >= count {
            break;
        }
        if total < min_total {
            continue;
        }

        let node_index = *skin.joint_node_indices.get(joint_index)?;
        let Some(&joint_tx) = node_index_to_transform.get(&node_index) else {
            continue;
        };
        if !seen.insert(joint_tx) {
            continue;
        }

        let name = universe
            .world
            .get_component_record(joint_tx)
            .map(|n| n.name.clone())
            .unwrap_or_else(|| "<unknown>".to_string());

        println!(
            "  auto[{:<2}] joint_index={:<3} total_weight={:<10.3} node_index={:<3} name={} tx={:?}",
            out.len(),
            joint_index,
            total,
            node_index,
            name,
            joint_tx
        );

        out.push((node_index, joint_tx));
    }

    if out.is_empty() { None } else { Some(out) }
}

fn find_renderable_by_mesh_key(
    world: &engine::ecs::World,
    root: engine::ecs::ComponentId,
    mesh_key: &str,
) -> Option<engine::ecs::ComponentId> {
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(m) = world.get_component_by_id_as::<MeshComponent>(node) {
            if m.key == mesh_key {
                return find_parent_renderable(world, node);
            }
        }
        for &ch in world.children_of(node) {
            stack.push(ch);
        }
    }
    None
}

fn find_parent_renderable(
    world: &engine::ecs::World,
    mut cid: engine::ecs::ComponentId,
) -> Option<engine::ecs::ComponentId> {
    while let Some(parent) = world.parent_of(cid) {
        if world
            .get_component_by_id_as::<RenderableComponent>(parent)
            .is_some()
        {
            return Some(parent);
        }
        cid = parent;
    }
    None
}

fn find_skin_id_for_renderable(
    world: &engine::ecs::World,
    renderable: engine::ecs::ComponentId,
) -> Option<engine::graphics::SkinId> {
    let mut stack = vec![renderable];
    while let Some(node) = stack.pop() {
        if let Some(sm) = world.get_component_by_id_as::<SkinnedMeshComponent>(node) {
            if let Some(id) = sm.skin_id {
                return Some(id);
            }
        }
        for &ch in world.children_of(node) {
            stack.push(ch);
        }
    }
    None
}

fn collect_joint_transforms(
    world: &engine::ecs::World,
    skinned_mesh: &engine::ecs::system::SkinnedMeshSystem,
    visuals: &engine::graphics::VisualWorld,
    gltf_component: engine::ecs::ComponentId,
    root: engine::ecs::ComponentId,
) -> Vec<(usize, engine::ecs::ComponentId)> {
    fn find_first_skin_id_in_subtree(
        world: &engine::ecs::World,
        root: engine::ecs::ComponentId,
    ) -> Option<engine::graphics::SkinId> {
        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            if let Some(sm) = world.get_component_by_id_as::<SkinnedMeshComponent>(node) {
                if let Some(id) = sm.skin_id {
                    return Some(id);
                }
            }
            for &ch in world.children_of(node) {
                stack.push(ch);
            }
        }
        None
    }

    let Some(skin_id) = find_first_skin_id_in_subtree(world, root) else {
        return Vec::new();
    };
    let Some(skin) = visuals.skin(skin_id) else {
        return Vec::new();
    };
    let Some(joints) = skinned_mesh.instance_joints_for_skin(gltf_component, skin_id) else {
        return Vec::new();
    };

    let joint_count = skin.joint_count().min(joints.len());
    let mut out: Vec<(usize, engine::ecs::ComponentId)> = Vec::with_capacity(joint_count);
    for i in 0..joint_count {
        let Some(joint_tx) = joints[i] else {
            continue;
        };
        let node_index = skin.joint_node_indices[i];
        out.push((node_index, joint_tx));
    }

    out
}

fn select_joint_range(
    joints: &[(usize, engine::ecs::ComponentId)],
    offset: usize,
    count: usize,
) -> Vec<(usize, engine::ecs::ComponentId)> {
    if joints.is_empty() || count == 0 {
        return Vec::new();
    }
    let len = joints.len();
    let start = offset % len;
    let n = count.min(len);

    (0..n).map(|i| joints[(start + i) % len]).collect()
}

fn quat_from_axis_angle(axis: [f32; 3], angle_rad: f32) -> [f32; 4] {
    let [ax, ay, az] = axis;
    let len2 = ax * ax + ay * ay + az * az;
    if len2 <= 0.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv_len = len2.sqrt().recip();
    let (ax, ay, az) = (ax * inv_len, ay * inv_len, az * inv_len);

    let half = 0.5 * angle_rad;
    let (s, c) = half.sin_cos();
    [ax * s, ay * s, az * s, c]
}

fn quat_conjugate(q: [f32; 4]) -> [f32; 4] {
    [-q[0], -q[1], -q[2], q[3]]
}

fn quat_normalize(q: [f32; 4]) -> [f32; 4] {
    let len2 = q[0] * q[0] + q[1] * q[1] + q[2] * q[2] + q[3] * q[3];
    if len2 <= 0.0 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv = len2.sqrt().recip();
    [q[0] * inv, q[1] * inv, q[2] * inv, q[3] * inv]
}

fn quat_rotate_vec3(q: [f32; 4], v: [f32; 3]) -> [f32; 3] {
    // v' = q * (v,0) * conj(q)
    let qv = [v[0], v[1], v[2], 0.0];
    let t = quat_mul(q, qv);
    let out = quat_mul(t, quat_conjugate(q));
    [out[0], out[1], out[2]]
}

fn quat_mul(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let (ax, ay, az, aw) = (a[0], a[1], a[2], a[3]);
    let (bx, by, bz, bw) = (b[0], b[1], b[2], b[3]);
    [
        aw * bx + ax * bw + ay * bz - az * by,
        aw * by - ax * bz + ay * bw + az * bx,
        aw * bz + ax * by - ay * bx + az * bw,
        aw * bw - ax * bx - ay * by - az * bz,
    ]
}
