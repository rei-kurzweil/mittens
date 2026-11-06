//! CPU-only ship workload probe; no graphics device or live OpenXR required.
//! cargo run --release --example profile-ship-cpu -- full 480
//! Cases: full, no_callbacks, no_contacts, no_springs, no_avatar,
//! no_vehicles, no_destroyables, no_input_controller, parked_vehicles, no_vehicle_sensors. Changes apply only to this probe's scene.
//! Synthetic tracked head: 75 Hz, 240 warmup frames, stationary in the bridge.
//! Excludes GPU drawing, live device input, audio capture and headset pacing.
use mittens_engine::engine::{
    Universe,
    ecs::component::{
        CollidableComponent, DestroyableComponent, GLTFComponent, GravityComponent,
        VelocityComponent, ZoneComponent,
    },
    ecs::system::external_xr_frame::ExternalXrFrame,
    graphics::{CpuMesh, MeshHandle, MeshUploader, TextureHandle, TextureUploader},
};
use std::{error::Error, time::Instant};

#[derive(Default)]
struct CpuUploader {
    meshes: u32,
    textures: u32,
    trace_uploads: bool,
    sample_uploads: Vec<(MeshHandle, usize, usize)>,
}
impl MeshUploader for CpuUploader {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn Error>> {
        self.meshes += 1;
        let handle = MeshHandle(1000 + self.meshes);
        if self.trace_uploads && self.sample_uploads.len() < 4 {
            eprintln!(
                "UPLOAD_TRACE vertices={} indices={} {}",
                mesh.vertices.len(),
                mesh.indices_u32.len(),
                std::backtrace::Backtrace::force_capture()
            );
            self.sample_uploads
                .push((handle, mesh.vertices.len(), mesh.indices_u32.len()));
        }
        Ok(handle)
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
fn stats(values: &mut [f64]) -> serde_json::Value {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values.sort_by(f64::total_cmp);
    serde_json::json!({"mean_ms": mean, "p50_ms": values[values.len()/2],
        "p95_ms": values[values.len()*95/100], "max_ms": values[values.len()-1]})
}
fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let case = args.next().unwrap_or_else(|| "full".into());
    let frames: usize = args.next().unwrap_or_else(|| "480".into()).parse()?;
    if args.next().is_some()
        || frames < 120
        || !matches!(
            case.as_str(),
            "full"
                | "no_callbacks"
                | "no_contacts"
                | "no_springs"
                | "no_avatar"
                | "no_vehicles"
                | "no_destroyables"
                | "no_input_controller"
                | "parked_vehicles"
                | "no_vehicle_sensors"
        )
    {
        return Err("usage: profile-ship-cpu <case> <frames >= 120>".into());
    }
    let mut source = std::fs::read_to_string("examples/ship_interior.mms")?;
    if case == "no_avatar" {
        source = source.replace(
            "let xr_avatar_enabled = true",
            "let xr_avatar_enabled = false",
        );
    }
    if case == "no_input_controller" {
        source = source.replace(
            "    ship_input_controller()",
            "    // profiling: input controller omitted",
        );
    }
    // Avoid acquiring a microphone in a CPU workload comparison.
    source = source.replace(
        "let microphone = AudioInput {",
        "let microphone = AudioOscillator.square() { enabled(false) amplitude(0.0)",
    );
    let mut scene = Universe::new(Default::default());
    scene.systems.set_vr_perf_enabled(true);
    let view = openxr::View {
        pose: openxr::Posef {
            orientation: openxr::Quaternionf {
                x: 0.,
                y: 0.,
                z: 0.,
                w: 1.,
            },
            position: openxr::Vector3f {
                x: 0.,
                y: 1.65,
                z: 0.,
            },
        },
        fov: openxr::Fovf {
            angle_left: -0.7,
            angle_right: 0.7,
            angle_up: 0.7,
            angle_down: -0.7,
        },
    };
    scene.systems.xr.set_external_frame(Some(ExternalXrFrame {
        views: Some([view, view]),
        ..Default::default()
    }));
    let loading = Instant::now();
    scene.load_mms_source_at_path(&source, "examples/ship_interior.mms")?;
    let mut uploader = CpuUploader::default();
    for _ in 0..4 {
        scene.update(1. / 75., &Default::default());
        prepare(&mut scene, &mut uploader);
    }
    // Apply removals after model materialization, before warmup.
    if matches!(case.as_str(), "no_vehicles" | "no_destroyables") {
        let ids: Vec<_> = scene
            .world
            .all_components()
            .filter(|&id| {
                if case == "no_vehicles" {
                    scene.world.component_label(id) == Some("pickup_truck")
                } else {
                    scene
                        .world
                        .get_component_by_id_as::<DestroyableComponent>(id)
                        .is_some()
                }
            })
            .collect();
        eprintln!("REMOVING case={case} roots={}", ids.len());
        for id in ids {
            if let Some(parent) = scene.world.parent_of(id) {
                if let Some(index) = scene
                    .world
                    .children_of(parent)
                    .iter()
                    .position(|&child| child == id)
                {
                    scene.remove_child(parent, index)?;
                }
            }
        }
    }
    if case == "parked_vehicles" {
        for id in scene.world.all_components().collect::<Vec<_>>() {
            match scene.world.component_label(id) {
                Some("pickup_gravity") => {
                    scene
                        .world
                        .get_component_by_id_as_mut::<GravityComponent>(id)
                        .unwrap()
                        .enabled = false
                }
                Some("pickup_velocity") => scene
                    .world
                    .get_component_by_id_as_mut::<VelocityComponent>(id)
                    .unwrap()
                    .zero_linear(),
                _ => {}
            }
        }
    }
    if case == "no_springs" {
        scene
            .systems
            .graphics_settings
            .set_secondary_motion_paused(true, &mut scene.command_queue);
    }
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
    for _ in 0..240 {
        scene.update(1. / 75., &Default::default());
        prepare(&mut scene, &mut uploader);
    }
    let loading_ms = loading.elapsed().as_secs_f64() * 1000.;
    if case == "no_vehicle_sensors" {
        for id in scene.world.all_components().collect::<Vec<_>>() {
            if matches!(
                scene.world.component_label(id),
                Some("pickup_activity_inner" | "pickup_activity_outer")
            ) {
                scene
                    .world
                    .get_component_by_id_as_mut::<ZoneComponent>(id)
                    .unwrap()
                    .events_enabled = false;
            }
        }
    }
    if case == "no_callbacks" {
        scene.close_mms_session();
    }
    let components = scene.world.all_components().count();
    let instances = scene.visuals.instances().len();
    let models: Vec<_> = scene
        .world
        .all_components()
        .filter_map(|id| {
            scene
                .world
                .get_component_by_id_as::<GLTFComponent>(id)
                .map(|g| g.uri.clone())
        })
        .collect();
    eprintln!(
        "PROFILE_BEGIN case={case} frames={frames} components={components} instances={instances} models={models:?}"
    );
    let (mut updates, mut preparations, mut totals) = (Vec::new(), Vec::new(), Vec::new());
    let mut spring = [0.; 4];
    let mesh_start = uploader.meshes;
    uploader.trace_uploads = std::env::var_os("SHIP_PROFILE_TRACE_UPLOADS").is_some();
    for frame in 0..frames {
        let started = Instant::now();
        scene.update(1. / 75., &Default::default());
        updates.push(started.elapsed().as_secs_f64() * 1000.);
        let profile = scene.systems.vr_perf_pre_xr();
        for (sum, time) in spring.iter_mut().zip([
            profile.secondary_motion,
            profile.spring_transform_propagation,
            profile.post_secondary_skinning,
            profile.spring_visualization,
        ]) {
            *sum += time.as_secs_f64() * 1000.;
        }
        let preparing = Instant::now();
        prepare(&mut scene, &mut uploader);
        preparations.push(preparing.elapsed().as_secs_f64() * 1000.);
        if frame == 1 && uploader.trace_uploads {
            for (handle, vertices, indices) in &uploader.sample_uploads {
                let owners: Vec<_> = scene
                    .visuals
                    .instances()
                    .iter()
                    .filter(|i| i.renderable.mesh == *handle)
                    .map(|i| {
                        let mut labels = Vec::new();
                        let mut id = Some(i.component);
                        while let Some(current) = id {
                            if let Some(label) = scene.world.component_label(current) {
                                labels.push(label.to_owned());
                            }
                            id = scene.world.parent_of(current);
                        }
                        labels
                    })
                    .collect();
                eprintln!(
                    "UPLOAD_OWNER {handle:?} vertices={vertices} indices={indices} owners={owners:?}"
                );
            }
        }
        totals.push(started.elapsed().as_secs_f64() * 1000.);
    }
    println!(
        "{}",
        serde_json::json!({"case":case,"frames":frames,"components":components,
        "instances":instances,"models":models,"load_and_warmup_ms":loading_ms,
        "update":stats(&mut updates),"prepare":stats(&mut preparations),"total":stats(&mut totals),
        "spring_stage_mean_ms":spring.map(|ms|ms/frames as f64),
        "sample_mesh_uploads":uploader.meshes-mesh_start})
    );
    Ok(())
}
