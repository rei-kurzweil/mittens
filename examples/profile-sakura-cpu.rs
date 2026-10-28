//! CPU-only rooftop comparison; does not open a window or initialize OpenXR.
//!
//! Run from the repository root, one case at a time, in release mode:
//! CAT_PROFILE_SPATIAL=1 CAT_PROFILE_SYSTEMS=1 cargo run --release --example profile-sakura-cpu -- full 480
//!
//! Cases: baseline, baseline_rays, full, paused, no_contacts, two_rays,
//! rays_miss, no_callbacks, rays_no_callbacks, simple_city.
//! A CPU uploader publishes real mesh/texture bindings without GPU work. The XR
//! player is replaced by one moving, gravity-driven slide capsule. Optional rays
//! cast continuously to exercise spatial queries and hit-event delivery. Warmup
//! is 240 frames, simulated presentation is 75 Hz, physics remains 120 Hz. These
//! measurements exclude headset tracking, primary-avatar IK, GPU work and vsync.

use mittens_engine::engine::{
    Universe,
    ecs::{
        World,
        component::{CollidableComponent, CollidableMode, GLTFComponent, TransformComponent},
    },
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
};
use std::{
    error::Error,
    time::{Duration, Instant},
};

#[derive(Default)]
struct CpuUploader {
    meshes: u32,
    textures: u32,
}
impl MeshUploader for CpuUploader {
    fn upload_mesh(&mut self, _: &CpuMesh) -> Result<MeshHandle, Box<dyn Error>> {
        self.meshes += 1;
        Ok(MeshHandle(1000 + self.meshes))
    }
}
impl TextureUploader for CpuUploader {
    fn upload_texture_rgba8(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
    ) -> Result<TextureHandle, Box<dyn Error>> {
        self.textures += 1;
        Ok(TextureHandle(1000 + self.textures))
    }
    fn upload_texture_bc7(
        &mut self,
        _: &[u8],
        _: u32,
        _: u32,
        _: bool,
    ) -> Result<TextureHandle, Box<dyn Error>> {
        self.textures += 1;
        Ok(TextureHandle(1000 + self.textures))
    }
}
fn prepare(scene: &mut Universe, uploader: &mut CpuUploader) {
    scene.systems.prepare_render(
        &mut scene.world,
        &mut scene.visuals,
        &mut scene.render_assets,
        uploader,
        &mut scene.command_queue,
    );
}
fn ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}
fn main() -> Result<(), Box<dyn Error>> {
    let case = std::env::args().nth(1).unwrap_or("full".into());
    let frames: usize = std::env::args().nth(2).unwrap_or("480".into()).parse()?;
    let avatars: usize = std::env::args().nth(3).unwrap_or("3".into()).parse()?;
    if avatars > 12 {
        return Err("avatar count must be at most 12".into());
    }
    if !matches!(
        case.as_str(),
        "baseline"
            | "baseline_rays"
            | "full"
            | "paused"
            | "no_contacts"
            | "two_rays"
            | "rays_no_callbacks"
            | "rays_miss"
            | "no_callbacks"
            | "simple_city"
    ) {
        return Err(format!("unknown case: {case}").into());
    }
    if frames < 120 {
        return Err("sample at least 120 frames".into());
    }
    let mut source = std::fs::read_to_string("examples/sakura-drone-conversion.mms")?
        .replace(
            "let desktop_view_enabled = false",
            "let desktop_view_enabled = true",
        )
        .replace("let xr_view_enabled = true", "let xr_view_enabled = false");
    if !matches!(case.as_str(), "baseline" | "baseline_rays" | "simple_city") {
        source = source.replace(
            "let initial_lumae_count = 0",
            &format!("let initial_lumae_count = {avatars}"),
        );
    }
    if case == "simple_city" {
        source = source.replace(
            "let initial_full_buildings = 8",
            "let initial_full_buildings = 0",
        );
    }
    source.push_str(
        r#"
        Gravity {
            Velocity {
                T.position(-18.0, 26.5, -1.4) {
                    name = "profile_body"
                    Zone.capsule_y(0.25, 0.6) {
                        Collidable.slide()
                    }
                }
            }
        }
    "#,
    );
    if matches!(
        case.as_str(),
        "two_rays" | "baseline_rays" | "rays_no_callbacks" | "rays_miss"
    ) {
        source.push_str(r#"
            T.position(-18.25, 26.8, -1.4) { Raycast.continuous().parent_forward(true).max_distance(40.0) }
            T.position(-17.75, 26.8, -1.4) { Raycast.continuous().parent_forward(true).max_distance(40.0) }
        "#);
    }
    if case == "rays_miss" {
        source = source
            .replace(
                "T.position(-18.25, 26.8, -1.4)",
                "T.position(-18.25, 26.8, -1.4).rotation(1.57079633, 0.0, 0.0)",
            )
            .replace(
                "T.position(-17.75, 26.8, -1.4)",
                "T.position(-17.75, 26.8, -1.4).rotation(1.57079633, 0.0, 0.0)",
            );
    }
    let mut scene = Universe::new(World::default());
    scene.systems.set_vr_perf_enabled(true);
    scene.load_mms_source_at_path(&source, "examples/sakura-drone-conversion.mms")?;
    if case == "no_contacts" {
        for id in scene.world.all_components().collect::<Vec<_>>() {
            if let Some(c) = scene
                .world
                .get_component_by_id_as_mut::<CollidableComponent>(id)
            {
                c.enabled = false;
            }
        }
    }
    let body = scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some("profile_body"))
        .unwrap();
    let mut uploader = CpuUploader::default();
    let input = Default::default();
    for _ in 0..4 {
        scene.update(1.0 / 75.0, &input);
        prepare(&mut scene, &mut uploader);
    }
    for frame in 0..240 {
        {
            let t = scene
                .world
                .get_component_by_id_as_mut::<TransformComponent>(body)
                .unwrap();
            t.transform.translation[0] = -18.0 + (frame as f32 * 0.01).sin() * 0.4;
            t.transform.recompute_model();
        }
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, body);
        scene.update(1.0 / 75.0, &input);
        prepare(&mut scene, &mut uploader);
    }
    if case == "paused" {
        scene
            .systems
            .graphics_settings
            .set_secondary_motion_paused(true, &mut scene.command_queue);
    }
    if case != "no_contacts" {
        let y = scene
            .world
            .get_component_by_id_as::<TransformComponent>(body)
            .unwrap()
            .transform
            .translation[1];
        assert!(
            (26.44..26.47).contains(&y),
            "capsule must settle on the roof: {y}"
        );
    }
    if matches!(case.as_str(), "no_callbacks" | "rays_no_callbacks") {
        // Retain the ECS scene and native spatial systems, stop MMS delivery.
        scene.close_mms_session();
    }
    let models = scene
        .world
        .all_components()
        .filter(|&id| {
            scene
                .world
                .get_component_by_id_as::<GLTFComponent>(id)
                .is_some_and(|g| g.uri == "assets/components/characters/lumae/lumae.glb")
        })
        .count();
    let (statics, movers) = scene
        .world
        .all_components()
        .filter_map(|id| {
            scene
                .world
                .get_component_by_id_as::<CollidableComponent>(id)
        })
        .filter(|c| c.enabled)
        .fold((0, 0), |(s, m), c| {
            if c.mode == CollidableMode::Static {
                (s + 1, m)
            } else {
                (s, m + 1)
            }
        });
    eprintln!(
        "PROFILE_BEGIN case={case} frames={frames} components={} instances={} lumae={models} statics={statics} movers={movers}",
        scene.world.all_components().count(),
        scene.visuals.instances().len()
    );
    let mut updates = Vec::new();
    let mut preparations = Vec::new();
    let mut spring = [Duration::ZERO; 4];
    for frame in 0..frames {
        let frame_start = Instant::now();
        {
            let t = scene
                .world
                .get_component_by_id_as_mut::<TransformComponent>(body)
                .unwrap();
            t.transform.translation[0] = -18.0 + ((frame + 240) as f32 * 0.01).sin() * 0.4;
            t.transform.recompute_model();
        }
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, body);
        scene.update(1.0 / 75.0, &input);
        updates.push(ms(frame_start.elapsed()));
        let profile = scene.systems.vr_perf_pre_xr();
        for (sum, time) in spring.iter_mut().zip([
            profile.secondary_motion,
            profile.spring_transform_propagation,
            profile.post_secondary_skinning,
            profile.spring_visualization,
        ]) {
            *sum += time;
        }
        let prepare_start = Instant::now();
        prepare(&mut scene, &mut uploader);
        preparations.push(ms(prepare_start.elapsed()));
    }
    let update_avg = updates.iter().sum::<f64>() / frames as f64;
    let prepare_avg = preparations.iter().sum::<f64>() / frames as f64;
    updates.sort_by(f64::total_cmp);
    preparations.sort_by(f64::total_cmp);
    println!(
        "RESULT case={case} frames={frames} update_avg_ms={update_avg:.4} update_p50_ms={:.4} update_p95_ms={:.4} prepare_avg_ms={prepare_avg:.4} prepare_p95_ms={:.4} spring_ms={:.4} spring_transforms_ms={:.4} post_skin_ms={:.4} spring_viz_ms={:.4}",
        updates[frames / 2],
        updates[frames * 95 / 100],
        preparations[frames * 95 / 100],
        ms(spring[0]) / frames as f64,
        ms(spring[1]) / frames as f64,
        ms(spring[2]) / frames as f64,
        ms(spring[3]) / frames as f64
    );
    Ok(())
}
