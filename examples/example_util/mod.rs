#![allow(dead_code)]

use cat_engine::engine::{self, ecs::component::{ColorComponent, TextBackgroundComponent, TextShadowComponent, TransparentCutoutComponent}};

/// Standard MMS demo scene rig: dark-blue background and navigable camera.
///
/// Lights should be declared in MMS, not spawned here.
///
/// Returns the camera `TransformComponent` id so callers can attach things to it.
///
/// `cam_pos` — world-space starting position of the camera.
pub fn spawn_mms_demo_rig(
    universe: &mut engine::Universe,
    cam_pos: [f32; 3],
) -> engine::ecs::ComponentId {
    use engine::ecs::component::{
        BackgroundColorComponent, Camera3DComponent, InputComponent,
        InputTransformModeComponent, TransformComponent,
    };

    // Dark blue clear colour.
    let bg_color = universe
        .world
        .add_component(BackgroundColorComponent::rgba(0.02, 0.03, 0.10, 1.0));
    universe.add(bg_color);

    // Camera rig: Input → Transform → Camera3D.
    let input = universe
        .world
        .add_component(InputComponent::new().with_speed(3.0));
    let input_mode = universe
        .world
        .add_component(InputTransformModeComponent::forward_z().with_roll_axis_y());
    let _ = universe.attach(input, input_mode);

    let cam_transform = universe.world.add_component(
        TransformComponent::new().with_position(cam_pos[0], cam_pos[1], cam_pos[2]),
    );
    let _ = universe.attach(input, cam_transform);

    let camera = universe.world.add_component(Camera3DComponent::new());
    let _ = universe.attach(cam_transform, camera);

    universe.add(input);

    spawn_desktop_camera_controls_hint(universe, cam_transform);

    cam_transform
}

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn rand01(seed: u32) -> f32 {
    (hash_u32(seed) as f32) / (u32::MAX as f32)
}

#[derive(Debug, Clone, Copy)]
pub struct CloudRingParams {
    pub cloud_count: u32,
    pub radius: f32,
    pub center_y: f32,
    pub puffs_per_cloud: u32,
    /// 0.0 = perfectly evenly spaced, 1.0 = up to one full step of jitter.
    pub angle_jitter: f32,
    /// Probability a given cluster is placed higher than `center_y`.
    pub high_y_probability: f32,
    /// Multiplier applied to `center_y` when a cluster is chosen to be high.
    pub high_y_multiplier: f32,
    /// Seed used for deterministic layout variation.
    pub seed: u32,
}

impl Default for CloudRingParams {
    fn default() -> Self {
        Self {
            cloud_count: 5,
            radius: 26.0,
            center_y: 2.0,
            puffs_per_cloud: 28,
            angle_jitter: 0.0,
            high_y_probability: 0.0,
            high_y_multiplier: 1.0,
            seed: 0xC10u32,
        }
    }
}

/// Spawns a ring of "cloud" puff clusters under an existing background root.
///
/// Assumes `bg_root` is a `BackgroundComponent` (usually `with_occlusion_and_lighting()`).
pub fn spawn_cloud_ring(
    universe: &mut engine::Universe,
    bg_root: engine::ecs::ComponentId,
    p: CloudRingParams,
) {
    if p.cloud_count == 0 || p.radius == 0.0 {
        return;
    }

    let cube_mesh = universe
        .render_assets
        .get_mesh(engine::graphics::BuiltinMeshType::Cube);

    let step = std::f32::consts::TAU / (p.cloud_count as f32);
    let angle_jitter = p.angle_jitter.clamp(0.0, 1.0);
    let high_y_probability = p.high_y_probability.clamp(0.0, 1.0);

    for i in 0..p.cloud_count {
        let seed_i = p.seed ^ i.wrapping_mul(0x9E37_79B9);
        let jitter = (rand01(seed_i ^ 0xa53a_9d2d) - 0.5) * step * angle_jitter;
        let a = (i as f32) * step + jitter;
        let cx = p.radius * a.cos();
        let cz = p.radius * a.sin();

        let cy = if rand01(seed_i ^ 0x7f4a_7c15) < high_y_probability {
            p.center_y * p.high_y_multiplier
        } else {
            p.center_y
        };

        let center_tx = universe.world.add_component(
            engine::ecs::component::TransformComponent::new().with_position(cx, cy, cz),
        );
        let _ = universe.attach(bg_root, center_tx);

        let base_seed = seed_i ^ 0x243f_6a88;
        for puff_i in 0..p.puffs_per_cloud {
            let seed = base_seed ^ puff_i.wrapping_mul(1_103_515_245);

            let ox = (rand01(seed ^ 0x68bc_21eb) - 0.5) * 9.0;
            let oy = (rand01(seed ^ 0x02e5_be93) - 0.5) * 4.0;
            let oz = (rand01(seed ^ 0xa1d3_4f2b) - 0.5) * 9.0;

            let base = 0.7 + rand01(seed ^ 0x9e37_79b9) * 2.8;
            let sx = base * (0.7 + rand01(seed ^ 0x243f_6a88) * 0.9);
            let sy = base * (0.6 + rand01(seed ^ 0x85a3_08d3) * 1.0);
            let sz = base * (0.7 + rand01(seed ^ 0x1319_8a2e) * 0.9);

            let tx = universe.world.add_component(
                engine::ecs::component::TransformComponent::new()
                    .with_position(ox, oy, oz)
                    .with_scale(sx, sy, sz),
            );
            let renderable =
                universe
                    .world
                    .add_component(engine::ecs::component::RenderableComponent::new(
                        engine::graphics::primitives::Renderable::new(
                            cube_mesh,
                            engine::graphics::primitives::MaterialHandle::TOON_MESH,
                        ),
                    ));

            let t = rand01(seed ^ 0x7f4a_7c15);
            let r = 0.70 + 0.10 * t;
            let g = 0.72 + 0.10 * t;
            let b = 0.80 + 0.12 * t;
            let color = universe
                .world
                .add_component(engine::ecs::component::ColorComponent::rgba(r, g, b, 1.0));

            let _ = universe.attach(center_tx, tx);
            let _ = universe.attach(tx, renderable);
            let _ = universe.attach(renderable, color);
        }
    }
}

/// Spawn a small camera-attached help text for desktop controls.
///
/// The returned component is the hint root `TransformComponent`.
///
/// This is intended for examples that use the common topology:
///
/// `I { T { C3D } }` (InputComponent → TransformComponent → Camera3DComponent)
pub fn spawn_desktop_camera_controls_hint(
    universe: &mut engine::Universe,
    camera_transform: engine::ecs::ComponentId,
) -> engine::ecs::ComponentId {
    use engine::ecs::component::{
        ColorComponent, EditorComponent, EmissiveComponent, RaycastableComponent, TextComponent,
        TextureFilteringComponent, TransformComponent,
    };

    // We intentionally do NOT parent the hint under the camera rig.
    // This keeps it out of the camera subtree (and avoids inheriting camera motion/rotation).
    //
    // Instead, we place it in world space based on the camera rig's current pose.
    // That makes it start out “in front-right of the camera” without being attached.
    let (cam_pos, cam_rot) = universe
        .world
        .get_component_by_id_as::<TransformComponent>(camera_transform)
        .map(|t| (t.transform.translation, t.transform.rotation))
        .unwrap_or(([0.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]));

    // Camera convention in examples: forward is -Z.
    // Local-space offset from the camera rig at spawn time.
    let local_offset = [0.65, 0.25, -1.7];

    // Rotate the offset by the camera rig's rotation so it appears in front-right of the view.
    let world_offset = cat_engine::utils::math::quat_rotate_vec3(cam_rot, local_offset);
    let world_pos = [
        cam_pos[0] + world_offset[0],
        cam_pos[1] + world_offset[1],
        cam_pos[2] + world_offset[2],
    ];

    let hint_root = universe.world.add_component(
        TransformComponent::new()
            .with_position(world_pos[0], world_pos[1], world_pos[2])
            .with_scale(0.055, 0.055, 1.0),
    );

    // Put the hint into an editor subtree so clicking it attaches gizmos.
    let editor_root = universe.world.add_component(EditorComponent::new());
    let _ = universe.attach(hint_root, editor_root);

    let text = universe.world.add_component(TextComponent::new(
        "use wasd/rf/qe\nand right-mouse\nclick and drag\nto move/look",
    ));

    // Make glyph renderables raycastable so the hint can be clicked without adding a
    // large invisible pick plane (clicking is per-glyph / per-text-quad).
    let raycastable = universe
        .world
        .add_component(RaycastableComponent::enabled());
    let _ = universe.attach(text, raycastable);

    // Text color should be an immediate child of the TextComponent root.
    let color = universe
        .world
        .add_component(ColorComponent::rgba(0.0, 0.0, 0.0, 1.0));
    let _ = universe.attach(text, color);

    let text_background = universe.world.add_component(
        TextBackgroundComponent::new()
            .with_padding_top(0.75)
            .with_padding_right(3.75),
    );
    let bg_color = universe
        .world
        .add_component(ColorComponent::rgba(0.9, 0.9, 0.9, 0.8));
    let _ = universe.attach(text, text_background);
    let _ = universe.attach(text_background, bg_color);

    // TextSystem looks for these as immediate children of the TextComponent root.
    let emissive = universe.world.add_component(EmissiveComponent::on());
    let filtering = universe
        .world
        .add_component(TextureFilteringComponent::nearest_magnification());

    let cutout = universe.world.add_component(TransparentCutoutComponent::new());
    let _ = universe.attach(text, cutout);

    let _ = universe.attach(editor_root, text);
    let _ = universe.attach(text, emissive);
    let _ = universe.attach(text, filtering);

    // Ensure the text is initialized even if the caller only adds the camera rig.
    universe.add(hint_root);

    hint_root
}
