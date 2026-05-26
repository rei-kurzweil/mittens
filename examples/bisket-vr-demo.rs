use cat_engine::engine::ecs::component::{
    AvatarControlComponent, ColorComponent, EmissiveComponent, OverlayComponent,
    RaycastableComponent, RenderableComponent, TransformComponent,
};
use cat_engine::engine::ecs::{ComponentId, EventSignal, Signal, SignalEmitter, SignalKind, World};
use cat_engine::utils::math::mat_to_quat;
use cat_engine::{engine, meow_meow, utils};
use std::sync::OnceLock;

// Bones the dump-button reports.  Spine first, then arms — same ordering as
// the bisket-vr-debug harness so the two outputs are comparable.
const DUMP_BONES: &[&str] = &[
    "J_Bip_C_Hips",
    "J_Bip_C_Spine",
    "J_Bip_C_Chest",
    "J_Bip_C_UpperChest",
    "J_Bip_C_Neck",
    "J_Bip_C_Head",
    "J_Bip_L_UpperArm",
    "J_Bip_L_LowerArm",
    "J_Bip_L_Hand",
    "J_Bip_R_UpperArm",
    "J_Bip_R_LowerArm",
    "J_Bip_R_Hand",
];

struct DumpTargets {
    /// (label, bone_component_id), filled at startup once the armature exists.
    bones: Vec<(String, ComponentId)>,
    /// driven_t = parent of AVC — HMD pose source.
    driven_t: ComponentId,
    /// splice_head — runtime AimConstraint sink (parent of head_bone).
    splice_head: ComponentId,
}

static DUMP_TARGETS: OnceLock<DumpTargets> = OnceLock::new();

fn world_pos(world: &World, id: ComponentId) -> [f32; 3] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| {
            let m = t.transform.matrix_world;
            [m[3][0], m[3][1], m[3][2]]
        })
        .unwrap_or([0.0; 3])
}

fn world_rot_quat(world: &World, id: ComponentId) -> [f32; 4] {
    world
        .get_component_by_id_as::<TransformComponent>(id)
        .map(|t| mat_to_quat(t.transform.matrix_world))
        .unwrap_or([0.0, 0.0, 0.0, 1.0])
}

fn on_dump_click(
    world: &mut World,
    _emit: &mut dyn SignalEmitter,
    env: &Signal,
) {
    if !matches!(env.event.as_ref(), Some(EventSignal::Click { .. })) {
        return;
    }
    let Some(targets) = DUMP_TARGETS.get() else { return };

    let driven_p = world_pos(world, targets.driven_t);
    let driven_q = world_rot_quat(world, targets.driven_t);
    let splice_p = world_pos(world, targets.splice_head);

    println!("\n================ BONE DUMP (click) ================");
    println!(
        "driven_t  pos=[{:+.3},{:+.3},{:+.3}]  rot_xyzw=[{:+.3},{:+.3},{:+.3},{:+.3}]",
        driven_p[0], driven_p[1], driven_p[2],
        driven_q[0], driven_q[1], driven_q[2], driven_q[3],
    );
    println!(
        "splice_head  pos=[{:+.3},{:+.3},{:+.3}]  (= AimConstraint output, head pivot)",
        splice_p[0], splice_p[1], splice_p[2],
    );
    println!("\n  {:<22}  {:>7}  {:>7}  {:>7}     {:>+7}  {:>+7}  {:>+7}",
             "bone", "x", "y", "z", "Δx_to_drv", "Δy_to_drv", "Δz_to_drv");
    let mut prev_pos: Option<[f32; 3]> = None;
    for (i, (label, bone_id)) in targets.bones.iter().enumerate() {
        let p = world_pos(world, *bone_id);
        println!(
            "  {:<22}  {:>+7.3}  {:>+7.3}  {:>+7.3}     {:>+7.3}  {:>+7.3}  {:>+7.3}",
            label, p[0], p[1], p[2],
            p[0] - driven_p[0], p[1] - driven_p[1], p[2] - driven_p[2],
        );
        // Segment length to previous bone (only meaningful within a chain —
        // spine bones are contiguous; arm bones are split per side, so we
        // insert a blank gap when the chain breaks).
        if let Some(prev) = prev_pos {
            let is_spine = label.starts_with("J_Bip_C_");
            let prev_was_spine = targets.bones[i - 1].0.starts_with("J_Bip_C_");
            let is_left_arm_step = label.starts_with("J_Bip_L_") && targets.bones[i - 1].0.starts_with("J_Bip_L_");
            let is_right_arm_step = label.starts_with("J_Bip_R_") && targets.bones[i - 1].0.starts_with("J_Bip_R_");
            if (is_spine && prev_was_spine) || is_left_arm_step || is_right_arm_step {
                let d = [(p[0]-prev[0]), (p[1]-prev[1]), (p[2]-prev[2])];
                let len = (d[0]*d[0] + d[1]*d[1] + d[2]*d[2]).sqrt();
                println!("    └─ segment from prev: len={:.4}m", len);
            }
        }
        prev_pos = Some(p);
    }
    println!("====================================================\n");
}

fn main() {
    utils::logger::init();

    let output = meow_meow::MeowMeowRunner::eval(include_str!("bisket-vr-demo.mms"));

    for error in &output.errors {
        eprintln!("[mms] {error}");
    }
    println!("[mms] {} intent(s) from bisket-vr-demo.mms", output.intents.len());

    let world = engine::ecs::World::default();
    let mut universe = engine::Universe::new(world);

    let scope = engine::ecs::ComponentId::default();
    for intent in output.intents {
        universe.command_queue.push_intent_now(scope, intent);
    }

    universe.systems.process_commands(
        &mut universe.world,
        &mut universe.visuals,
        &mut universe.command_queue,
    );

    // Force the glTF subtree to spawn so the armature exists before we query for
    // bone markers (otherwise the bone components aren't in the world yet).
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

    // Bone markers — small cubes attached as children of named bones, so they
    // visibly track the bone's world pose. Useful for debugging where each
    // bone actually sits relative to the XR camera and the overlay cube.
    let marker_joints: &[(&str, (f32, f32, f32, f32))] = &[
        ("[name='J_Bip_C_Head']",      (0.85, 0.20, 0.85, 0.9)),
        ("[name='J_Bip_C_Neck']",      (0.20, 0.85, 0.85, 0.9)),
        ("[name='J_Bip_C_UpperChest']",(0.20, 0.20, 0.85, 0.9)),
        ("[name='J_Sec_Hair1_08']",    (1.00, 0.00, 0.00, 1.0)),
        ("[name='J_Sec_Hair2_08']",    (1.00, 0.40, 0.40, 1.0)),
    ];

    // Find any global root to start the descendant search from. Scanning all
    // components is fine here — this only runs once at startup.
    let roots: Vec<engine::ecs::ComponentId> = universe
        .world
        .all_components()
        .filter(|&id| universe.world.parent_of(id).is_none())
        .collect();

    for &(selector, color) in marker_joints {
        let bone = roots
            .iter()
            .find_map(|&r| universe.find_component(r, selector));
        let Some(bone) = bone else {
            eprintln!("[bisket-vr-demo] bone not found: {selector}");
            continue;
        };
        // Topology:  bone -> OV -> marker_t -> marker_r (+C +EM +Raycastable)
        // OV is a phase marker: its subtree renders in the overlay pass (drawn
        // last, on top of everything). Without this the marker cube is occluded
        // by the avatar's head/body mesh in first-person VR.
        let marker_ov = universe.world.add_component(OverlayComponent::new());
        let marker_t = universe
            .world
            .add_component(TransformComponent::new().with_scale(0.025, 0.025, 0.025));
        let marker_r = universe.world.add_component(RenderableComponent::cube());
        let marker_c = universe
            .world
            .add_component(ColorComponent::rgba(color.0, color.1, color.2, color.3));
        let marker_rcast = universe.world.add_component(RaycastableComponent::enabled());
        let marker_em = universe.world.add_component(EmissiveComponent::on());
        let _ = universe.world.add_child(marker_r, marker_c);
        let _ = universe.world.add_child(marker_r, marker_em);
        let _ = universe.world.add_child(marker_r, marker_rcast);
        let _ = universe.world.add_child(marker_t, marker_r);
        let _ = universe.world.add_child(marker_ov, marker_t);
        let _ = universe.attach(bone, marker_ov);
    }

    // ----------------------------------------------------------------------
    // Dump button — click to print a bone snapshot for offline analysis.
    //
    // Topology: scene_root → OV → button_t → button_r (+ C, EM, Raycastable).
    // OV phase so the cube is visible through the avatar body in first-person VR.
    // Click handler walks the cached bone list, prints world pos + segment
    // lengths to stdout.  Compare with bisket-vr-debug harness output.
    // ----------------------------------------------------------------------
    {
        // Resolve AVC + driven_t for the dump.
        let avc_id_opt = universe.world.all_components().find(|&id| {
            universe.world.get_component_by_id_as::<AvatarControlComponent>(id).is_some()
        });
        if let Some(avc_id) = avc_id_opt {
            let driven_t = universe.world.parent_of(avc_id).unwrap_or(avc_id);

            // Find each bone under any root (model_root is unique per avatar here).
            let mut bones: Vec<(String, ComponentId)> = Vec::new();
            for name in DUMP_BONES {
                let sel = format!("[name='{}']", name);
                if let Some(bone) = roots.iter().find_map(|&r| universe.find_component(r, &sel)) {
                    bones.push((name.to_string(), bone));
                } else {
                    eprintln!("[dump] bone not found: {}", name);
                }
            }
            // splice_head = parent of head bone (runtime-inserted by AVC).
            let splice_head = bones
                .iter()
                .find(|(n, _)| n == "J_Bip_C_Head")
                .and_then(|(_, head)| universe.world.parent_of(*head))
                .unwrap_or(avc_id);

            let _ = DUMP_TARGETS.set(DumpTargets { bones, driven_t, splice_head });

            // Spawn the button.  Position chosen so it's reachable in-VR from
            // a standing pose: 0.35 m to the right, hip-height, slightly out.
            let btn_t = universe.world.add_component(
                TransformComponent::new()
                    .with_position(0.35, 1.05, -0.35)
                    .with_scale(0.06, 0.06, 0.06),
            );
            let btn_r = universe.world.add_component(RenderableComponent::cube());
            let btn_c = universe.world.add_component(ColorComponent::rgba(0.95, 0.20, 0.40, 1.0));
            let btn_em = universe.world.add_component(EmissiveComponent::on());
            let btn_rcast = universe.world.add_component(RaycastableComponent::enabled());
            let btn_ov = universe.world.add_component(OverlayComponent::new());
            let _ = universe.world.add_child(btn_r, btn_c);
            let _ = universe.world.add_child(btn_r, btn_em);
            let _ = universe.world.add_child(btn_r, btn_rcast);
            let _ = universe.world.add_child(btn_t, btn_r);
            let _ = universe.world.add_child(btn_ov, btn_t);

            // Attach to a stable scene root (the first global root we found).
            let scene_root = roots.iter().copied().find(|&r| {
                universe.world.get_component_by_id_as::<TransformComponent>(r).is_some()
                    || universe.world.children_of(r).len() > 0
            }).unwrap_or_else(|| roots[0]);
            let _ = universe.attach(scene_root, btn_ov);

            // Wire Click handler.  Scope = the renderable (where the raycast hits).
            universe.add_signal_handler(SignalKind::Click, btn_r, on_dump_click);

            println!("[dump] button spawned at [0.35, 1.05, -0.35] — click in VR or with desktop pointer to dump bones");
        } else {
            eprintln!("[dump] AvatarControlComponent not found — skipping dump button");
        }
    }

    universe.enable_repl();
    engine::Windowing::run_app(universe).expect("Windowing failed");
}
