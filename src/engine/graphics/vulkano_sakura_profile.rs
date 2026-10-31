// Opt-in GPU profiling fixture. Ordinary builds contain none of these hooks.
use super::*;
use crate::engine::graphics::{CameraData, CameraTarget, MeshUploader, TextureUploader};
use crate::engine::{
    Universe,
    ecs::{
        ComponentId,
        component::{GLTFComponent, TransformComponent},
    },
};
use crate::utils::math::{mat4_inverse, quat_mul, quat_rotation_y};
use std::{
    collections::{BTreeMap, HashSet},
    error::Error,
    path::PathBuf,
    time::{Duration, Instant},
};
use vulkano::{
    query::{QueryPool, QueryPoolCreateInfo, QueryResultFlags, QueryType},
    sync::PipelineStage,
};

#[derive(Default, serde::Serialize)]
pub(super) struct DrawCounts {
    calls: u64,
    instances: u64,
    batch_sizes: BTreeMap<usize, u64>,
}

pub(super) struct ProfileState {
    pub(super) suppressed_meshes: HashSet<MeshHandle>,
    pub(super) npc_meshes: HashSet<MeshHandle>,
    pub(super) dirty_components: Vec<ComponentId>,
    pub(super) morph_upload_cpu_ns: u64,
    pub(super) deformation_cpu_ns: u64,
    pub(super) completion: Option<Box<dyn FnOnce() -> Result<(), Box<dyn Error>>>>,
    pool: Option<Arc<QueryPool>>,
    timestamp_period: f64,
    timestamp_mask: u64,
    draws: BTreeMap<String, DrawCounts>,
    npc_draws: BTreeMap<String, DrawCounts>,
}

impl ProfileState {
    fn new(context: &RendererContext) -> Result<Self, Box<dyn Error>> {
        let physical = context.device().physical_device();
        let bits = physical.queue_family_properties()
            [context.graphics_queue().queue_family_index() as usize]
            .timestamp_valid_bits;
        let pool = if bits.is_some() {
            Some(QueryPool::new(
                context.device().clone(),
                QueryPoolCreateInfo {
                    query_count: 6,
                    ..QueryPoolCreateInfo::query_type(QueryType::Timestamp)
                },
            )?)
        } else {
            None
        };
        Ok(Self {
            suppressed_meshes: HashSet::new(),
            npc_meshes: HashSet::new(),
            dirty_components: Vec::new(),
            morph_upload_cpu_ns: 0,
            deformation_cpu_ns: 0,
            completion: None,
            pool,
            timestamp_period: physical.properties().timestamp_period as f64,
            timestamp_mask: if bits.unwrap_or(64) == 64 {
                u64::MAX
            } else {
                (1u64 << bits.unwrap()) - 1
            },
            draws: BTreeMap::new(),
            npc_draws: BTreeMap::new(),
        })
    }
    pub(super) fn timestamp(
        &self,
        commands: &mut AutoCommandBufferBuilder<vulkano::command_buffer::PrimaryAutoCommandBuffer>,
        eye: usize,
        phase: u32,
    ) -> Result<(), Box<dyn Error>> {
        if let Some(pool) = &self.pool {
            // The harness waits the existing second-eye fence before reusing these queries.
            unsafe {
                if phase == 0 {
                    commands.reset_query_pool(pool.clone(), eye as u32 * 3..eye as u32 * 3 + 3)?;
                }
                commands.write_timestamp(
                    pool.clone(),
                    eye as u32 * 3 + phase,
                    if phase == 0 {
                        PipelineStage::TopOfPipe
                    } else {
                        PipelineStage::BottomOfPipe
                    },
                )?;
            }
        }
        Ok(())
    }
    fn gpu_ms(&self) -> Result<Option<[f64; 2]>, Box<dyn Error>> {
        let Some(pool) = &self.pool else {
            return Ok(None);
        };
        let mut timestamps = [0u64; 6];
        if !pool.get_results(0..6, &mut timestamps, QueryResultFlags::empty())? {
            return Err("timestamps unavailable after existing submission fence".into());
        }
        let mut result = [0.; 2];
        for eye in 0..2 {
            for stage in 0..2 {
                result[stage] += (timestamps[eye * 3 + stage + 1]
                    .wrapping_sub(timestamps[eye * 3 + stage])
                    & self.timestamp_mask) as f64
                    * self.timestamp_period
                    / 1e6;
            }
        }
        Ok(Some(result))
    }
    pub(super) fn draw(&mut self, pass: &str, mesh: MeshHandle, count: usize) {
        let record = |map: &mut BTreeMap<String, DrawCounts>| {
            let entry = map.entry(pass.to_owned()).or_default();
            entry.calls += 1;
            entry.instances += count as u64;
            *entry.batch_sizes.entry(count).or_default() += 1;
        };
        record(&mut self.draws);
        if self.npc_meshes.contains(&mesh) {
            record(&mut self.npc_draws);
        }
    }
    fn reset_frame(&mut self) {
        self.dirty_components.clear();
        self.morph_upload_cpu_ns = 0;
        self.deformation_cpu_ns = 0;
        self.draws.clear();
        self.npc_draws.clear();
    }
}

struct Uploader<'a> {
    renderer: &'a mut VulkanoState,
    mesh: u32,
    texture: u32,
}
impl MeshUploader for Uploader<'_> {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn Error>> {
        let handle = MeshHandle(self.mesh);
        self.mesh += 1;
        self.renderer.upload_mesh(handle, mesh)?;
        Ok(handle)
    }
}
impl TextureUploader for Uploader<'_> {
    fn upload_texture_rgba8(
        &mut self,
        rgba: &[u8],
        width: u32,
        height: u32,
    ) -> Result<TextureHandle, Box<dyn Error>> {
        let handle = TextureHandle(self.texture);
        self.texture += 1;
        self.renderer
            .upload_texture_rgba8(handle, rgba, width, height)?;
        Ok(handle)
    }
    fn upload_texture_bc7(
        &mut self,
        blocks: &[u8],
        width: u32,
        height: u32,
        srgb: bool,
    ) -> Result<TextureHandle, Box<dyn Error>> {
        let handle = TextureHandle(self.texture);
        self.texture += 1;
        self.renderer
            .upload_texture_bc7(handle, blocks, width, height, srgb)?;
        Ok(handle)
    }
}
struct ProfileClock(std::sync::atomic::AtomicU64);
impl crate::engine::ecs::system::ClockDriver for ProfileClock {
    fn name(&self) -> &'static str {
        "sakura-profile"
    }
    fn time_now_sec(&self) -> f64 {
        f64::from_bits(self.0.load(std::sync::atomic::Ordering::Relaxed))
    }
}
fn named(scene: &Universe, label: &str) -> ComponentId {
    scene
        .world
        .all_components()
        .find(|&id| scene.world.component_label(id) == Some(label))
        .unwrap_or_else(|| panic!("missing {label}"))
}
fn npc(scene: &Universe, mut id: ComponentId) -> bool {
    loop {
        if scene
            .world
            .component_label(id)
            .is_some_and(|name| name.starts_with("sakura_lumae_") && name.ends_with("_placement"))
        {
            return true;
        }
        match scene.world.parent_of(id) {
            Some(parent) => id = parent,
            None => return false,
        }
    }
}
fn wait_gate(path: &std::path::Path) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    while !path.exists() {
        if started.elapsed() > Duration::from_secs(120) {
            return Err(format!("timed out waiting for {}", path.display()).into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    Ok(())
}
fn summary(values: &[f64]) -> serde_json::Value {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    serde_json::json!({"mean_ms":values.iter().sum::<f64>()/values.len() as f64,"median_ms":sorted[sorted.len()/2],"p95_ms":sorted[((sorted.len()-1) as f64*0.95).ceil() as usize]})
}

#[test]
#[ignore = "requires Vulkan; controlled Sakura stereo CPU/GPU profiling matrix"]
fn profile_sakura_lumae_stereo() -> Result<(), Box<dyn Error>> {
    let count: usize = std::env::var("SAKURA_PROFILE_LUMAE")
        .unwrap_or("0".into())
        .parse()?;
    let motion = std::env::var("SAKURA_PROFILE_MOTION").unwrap_or("static".into());
    let hidden = std::env::var("SAKURA_PROFILE_HIDE").is_ok_and(|s| s == "1");
    let frames: usize = std::env::var("SAKURA_PROFILE_FRAMES")
        .unwrap_or("600".into())
        .parse()?;
    let warmup: usize = std::env::var("SAKURA_PROFILE_WARMUP")
        .unwrap_or("240".into())
        .parse()?;
    assert!(count <= 2 && matches!(motion.as_str(), "static" | "moving") && frames > 0);
    let gate = std::env::var_os("SAKURA_PROFILE_GATE").map(PathBuf::from);
    let start = Instant::now();
    let context = RendererContext::new(None, None, None)?;
    let extent = [1440, 1600];
    let mut renderer = VulkanoState::new(
        context.clone(),
        RendererOutput::Xr(Format::B8G8R8A8_UNORM),
        MsaaMode::Msaa4x,
    )?;
    let context_ms = start.elapsed().as_secs_f64() * 1000.;
    let clock = Arc::new(ProfileClock(std::sync::atomic::AtomicU64::new(
        0f64.to_bits(),
    )));
    let mut scene = Universe::new(Default::default());
    scene.systems.clock.set_driver(clock.clone());
    scene.systems.set_vr_perf_enabled(true);
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sakura-drone-conversion.mms"
    ))
    .replace("let xr_view_enabled = true", "let xr_view_enabled = false")
    .replace(
        "let desktop_view_enabled = false",
        "let desktop_view_enabled = true",
    )
    .replace(
        "let initial_lumae_count = 0",
        &format!("let initial_lumae_count = {count}"),
    );
    let source = source
        + r#"
        T.position(-18.0, 25.6, 6.5) {
            name = "profile_driver_placement"
            GLTF.new("assets/models/mitten.glb") {name = "profile_driver" pistol_material}
        }
        GraphicsSettings.secondary_motion.toggle(false)
    "#;
    scene.load_mms_source_at_path(&source, "examples/sakura-drone-conversion.mms")?;
    let mut uploader = Uploader {
        renderer: &mut renderer,
        mesh: 1000,
        texture: 1000,
    };
    let prepare = |scene: &mut Universe, uploader: &mut Uploader<'_>| {
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            uploader,
            &mut scene.command_queue,
        );
    };
    for _ in 0..4 {
        scene.update(0., &Default::default());
        prepare(&mut scene, &mut uploader);
    }
    for id in scene.world.all_components() {
        if let Some(gltf) = scene.world.get_component_by_id_as::<GLTFComponent>(id) {
            assert!(gltf.spawned, "unloaded {}", gltf.uri);
        }
    }
    for index in 1..=count {
        let placement = named(&scene, &format!("sakura_lumae_{index}_placement"));
        scene
            .world
            .get_component_by_id_as_mut::<TransformComponent>(placement)
            .unwrap()
            .transform = TransformComponent::new()
            .with_position(-18.7 + (index - 1) as f32 * 1.4, 25.6, 2.5)
            .transform;
        scene
            .systems
            .transform_changed(&mut scene.world, &mut scene.visuals, placement);
    }
    scene.update(0., &Default::default());
    prepare(&mut scene, &mut uploader);
    for (_, instance) in scene.visuals.materials.instances() {
        if let Some(custom) = scene
            .visuals
            .materials
            .definition(instance.definition)
            .and_then(|d| d.custom.as_ref())
        {
            let start = Instant::now();
            while custom.compiled.get().is_none() && start.elapsed() < Duration::from_secs(30) {
                std::thread::sleep(Duration::from_millis(5));
            }
            custom
                .compiled
                .get()
                .ok_or("material compilation timed out")?
                .as_ref()
                .map_err(Clone::clone)?;
        }
    }
    // Idle weapon audio must not replace the deterministic test clock.
    scene.systems.audio = crate::engine::ecs::system::AudioSystem::new();
    scene.systems.clock.set_driver(clock.clone());
    let driver = named(&scene, "profile_driver");
    let joint = scene
        .world
        .scripting_query_roots(driver)
        .into_iter()
        .find_map(|root| scene.world.find_component(root, "#J_Bip_C_Head"))
        .ok_or("driver head joint unavailable")?;
    let rest = scene
        .world
        .get_component_by_id_as::<TransformComponent>(joint)
        .unwrap()
        .transform
        .rotation;
    let npc_components: HashSet<_> = scene
        .visuals
        .instances()
        .iter()
        .filter(|i| npc(&scene, i.component))
        .map(|i| i.component)
        .collect();
    let npc_meshes: HashSet<_> = scene
        .visuals
        .instances()
        .iter()
        .filter(|i| npc_components.contains(&i.component))
        .map(|i| i.renderable.mesh)
        .collect();
    let identity=scene.visuals.instances().iter().filter(|i|npc_components.contains(&i.component)).map(|i|serde_json::json!({"mesh":format!("{:?}",i.renderable.mesh),"material":format!("{:?}",i.material_instance),"texture":format!("{:?}",i.texture),"multiple_layers":i.multiple_layers,"cutout":i.transparent_cutout,"bones":i.bones_count})).collect::<Vec<_>>();
    let mut profile = ProfileState::new(&context)?;
    profile.npc_meshes = npc_meshes.clone();
    if hidden {
        profile.suppressed_meshes = npc_meshes.clone();
    }
    uploader.renderer.profile = Some(profile);
    let output = Image::new(
        context.memory_allocator().clone(),
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format: Format::B8G8R8A8_UNORM,
            extent: [extent[0], extent[1], 1],
            array_layers: 2,
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::TRANSFER_SRC,
            ..Default::default()
        },
        Default::default(),
    )?;
    let eyes = (0..2)
        .map(|eye| {
            ImageView::new(
                output.clone(),
                ImageViewCreateInfo {
                    view_type: vulkano::image::view::ImageViewType::Dim2d,
                    subresource_range: ImageSubresourceRange {
                        array_layers: eye..eye + 1,
                        ..output.subresource_range()
                    },
                    ..ImageViewCreateInfo::from_image(&output)
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rotation =
        TransformComponent::look_at_world_rotation([-18., 27.25, 5.5], [-18., 26.7, 2.5]).unwrap();
    let camera = TransformComponent::new()
        .with_position(-18., 27.25, 5.5)
        .with_rotation_quat(quat_mul(rotation, quat_rotation_y(std::f32::consts::PI)));
    let mut projection = scene.visuals.camera_proj_for(CameraTarget::Window);
    projection[0][0] = projection[1][1].abs() * extent[1] as f32 / extent[0] as f32;
    let cameras: Vec<_> = (0..2)
        .map(|eye| {
            let mut model = camera.transform.model;
            for axis in 0..3 {
                model[3][axis] += model[0][axis] * if eye == 0 { -0.032 } else { 0.032 };
            }
            CameraData {
                view: mat4_inverse(model).unwrap(),
                proj: projection,
                transform: Default::default(),
            }
        })
        .collect();
    let load_ms = start.elapsed().as_secs_f64() * 1000. - context_ms;
    let mut rows = Vec::with_capacity(frames);
    let mut counters_start = None;
    let mut material_start = (0, 0, 0, 0, 0, 0);
    let binding_counts = |r: &VulkanoState| -> [u64; 3] {
        r.material_instances
            .values()
            .fold([0; 3], |mut total, entry| {
                total[0] += entry.bindings.hits;
                total[1] += entry.bindings.misses;
                total[2] += entry.bindings.evictions;
                total
            })
    };
    let mut binding_start = [0; 3];
    for frame in 0..warmup + frames {
        if frame == warmup {
            if let Some(gate) = &gate {
                std::fs::write(gate.with_extension("ready"), b"ready")?;
                wait_gate(&gate.with_extension("go"))?;
            }
            counters_start = Some(uploader.renderer.perf_counters());
            binding_start = binding_counts(uploader.renderer);
            let renderer = &*uploader.renderer;
            material_start = (
                renderer.cached_material_sets.hits,
                renderer.cached_material_sets.misses,
                renderer.cached_material_sets.evictions,
                renderer.material_upload_stats.allocations,
                renderer.material_upload_stats.uploads,
                renderer.material_upload_stats.snapshot_reuses,
            );
        }
        let before = Instant::now();
        clock.0.store(
            (frame as f64 / 75.).to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        uploader.renderer.profile.as_mut().unwrap().reset_frame();
        let driver_started = Instant::now();
        if motion == "moving" {
            scene
                .world
                .get_component_by_id_as_mut::<TransformComponent>(joint)
                .unwrap()
                .transform
                .rotation = quat_mul(
                rest,
                quat_rotation_y(0.15 * (frame as f32 / 75. * std::f32::consts::PI).sin()),
            );
            scene
                .world
                .get_component_by_id_as_mut::<TransformComponent>(joint)
                .unwrap()
                .transform
                .recompute_model();
            scene
                .systems
                .transform_changed(&mut scene.world, &mut scene.visuals, joint);
        }
        let transform_ms = driver_started.elapsed().as_secs_f64() * 1000.;
        let skin_before = scene.systems.skinned_mesh.profile_tick_ns;
        let binding_before = scene.systems.skinned_mesh.profile_bindings_updated;
        let update = Instant::now();
        scene.update(1. / 75., &Default::default());
        let update_ms = update.elapsed().as_secs_f64() * 1000.;
        let skin_ms = (scene.systems.skinned_mesh.profile_tick_ns - skin_before) as f64 / 1e6;
        let bindings = scene.systems.skinned_mesh.profile_bindings_updated - binding_before;
        let prep = Instant::now();
        prepare(&mut scene, &mut uploader);
        scene.visuals.set_xr_camera(cameras.clone());
        let prepare_ms = prep.elapsed().as_secs_f64() * 1000.;
        let record = Instant::now();
        for eye in 0..2 {
            uploader.renderer.submit_xr_eye_to_view(
                &mut scene.visuals,
                eye,
                extent,
                2,
                eyes[eye].clone(),
            )?;
        }
        let record_ms = record.elapsed().as_secs_f64() * 1000.;
        let wait = Instant::now();
        uploader
            .renderer
            .profile
            .as_mut()
            .unwrap()
            .completion
            .take()
            .unwrap()()?;
        if let Some(future) = uploader.renderer.submission_future.as_mut() {
            future.cleanup_finished();
        }
        let wait_ms = wait.elapsed().as_secs_f64() * 1000.;
        let profile = uploader.renderer.profile.as_ref().unwrap();
        let gpu = profile.gpu_ms()?;
        let npc_dirty = profile
            .dirty_components
            .iter()
            .filter(|id| npc_components.contains(id))
            .count();
        if frame >= warmup {
            assert_eq!(scene.systems.clock.driver_name(), "sakura-profile");
            assert_eq!(
                npc_dirty, 0,
                "stationary NPCs must not be deformed after warmup"
            );
            if motion == "static" {
                assert!(
                    profile.dirty_components.is_empty(),
                    "fully static scene is still dirty"
                );
            }
            if hidden {
                assert!(
                    profile.npc_draws.is_empty(),
                    "hidden control still submits NPC graphics draws"
                );
            }
            rows.push(serde_json::json!({"frame":frame-warmup,"total_ms":before.elapsed().as_secs_f64()*1000.,"driver_transform_ms":transform_ms,"update_ms":update_ms,"skin_ms":skin_ms,"bindings_updated":bindings,"prepare_ms":prepare_ms,"record_submit_ms":record_ms,"wait_ms":wait_ms,"gpu_deform_ms":gpu.map(|g|g[0]),"gpu_graphics_ms":gpu.map(|g|g[1]),"deformation_cpu_ms":profile.deformation_cpu_ns as f64/1e6,"morph_upload_cpu_ms":profile.morph_upload_cpu_ns as f64/1e6,"npc_dirty_jobs":npc_dirty,"all_dirty_jobs":profile.dirty_components.len(),"draws":profile.draws,"npc_draws":profile.npc_draws}));
        }
    }
    let renderer = &*uploader.renderer;
    let counters = renderer
        .perf_counters()
        .saturating_delta(counters_start.unwrap());
    assert_eq!(
        counters.deformation_morph_delta_upload_bytes, 0,
        "immutable morph deltas must remain uploaded after warmup, including moving frames"
    );
    if motion == "static" {
        assert_eq!(counters.deformation_jobs, 0);
        assert_eq!(counters.deformation_bone_upload_bytes, 0);
    }
    if motion == "moving" {
        assert!(
            counters.deformation_jobs > 0,
            "moving driver produced no deformation work"
        );
    }
    assert_eq!(counters.deformation_output_resizes, 0);
    assert_eq!(counters.deformation_bone_resizes, 0);
    let mut timing = serde_json::Map::new();
    for key in [
        "total_ms",
        "driver_transform_ms",
        "update_ms",
        "skin_ms",
        "prepare_ms",
        "record_submit_ms",
        "wait_ms",
        "gpu_deform_ms",
        "gpu_graphics_ms",
        "deformation_cpu_ms",
        "morph_upload_cpu_ms",
    ] {
        let values: Vec<_> = rows.iter().filter_map(|row| row[key].as_f64()).collect();
        if !values.is_empty() {
            timing.insert(key.into(), summary(&values));
        }
    }
    let binding_end = binding_counts(renderer);
    let binding_deltas = [
        binding_end[0] - binding_start[0],
        binding_end[1] - binding_start[1],
        binding_end[2] - binding_start[2],
    ];
    let binding_details: Vec<_> = renderer.material_instances.iter().map(|(id,entry)|serde_json::json!({"id":format!("{id:?}"),"hits":entry.bindings.hits,"misses":entry.bindings.misses,"evictions":entry.bindings.evictions,"retained":entry.bindings.len()})).collect();
    let result = serde_json::json!({"lumae":count,"motion":motion,"hidden":hidden,"frames":frames,"warmup":warmup,"extent":extent,"msaa":4,"gpu":renderer.gpu_device_name(),"context_ms":context_ms,"load_upload_ms":load_ms,"timing":timing,"counters":counters,"cached_material_hits":renderer.cached_material_sets.hits-material_start.0,"cached_material_misses":renderer.cached_material_sets.misses-material_start.1,"cached_material_evictions":renderer.cached_material_sets.evictions-material_start.2,"material_binding_cache_deltas":binding_deltas,"material_binding_cache_totals":binding_details,"material_allocations":renderer.material_upload_stats.allocations-material_start.3,"material_uploads":renderer.material_upload_stats.uploads-material_start.4,"material_reuses":renderer.material_upload_stats.snapshot_reuses-material_start.5,"bones_palette_bytes":scene.visuals.bones_palette().len()*64,"unique_npc_meshes":npc_meshes.len(),"npc_instances":identity,"morph_arena_bytes":renderer.deformation_morph_cpu.len()*std::mem::size_of::<GpuMorphDelta>(),"deformation_live_bytes":renderer.deformation_stats.live_cache_bytes,"deformation_allocated_bytes":renderer.deformation_stats.allocated_cache_bytes,"gpu_meshes":renderer.meshes.len(),"gpu_textures":renderer.textures.len(),"sample_draws":rows.last().unwrap()["draws"],"sample_npc_draws":rows.last().unwrap()["npc_draws"]});
    println!("SAKURA_PROFILE_RESULT {}", result);
    if let Some(path) = std::env::var_os("SAKURA_PROFILE_OUTPUT") {
        let path = PathBuf::from(path);
        std::fs::write(
            path.with_extension("frames.json"),
            serde_json::to_vec(&rows)?,
        )?;
        std::fs::write(path, serde_json::to_vec_pretty(&result)?)?;
    }
    if let Some(path) = std::env::var_os("SAKURA_PROFILE_PREVIEW") {
        use vulkano::command_buffer::CopyImageToBufferInfo;
        let readback = Buffer::new_slice::<u8>(
            context.memory_allocator().clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_DST,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                ..Default::default()
            },
            (extent[0] * extent[1] * 4) as u64,
        )?;
        let mut commands = AutoCommandBufferBuilder::primary(
            uploader.renderer.command_buffer_allocator.clone(),
            context.graphics_queue().queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;
        let mut copy = CopyImageToBufferInfo::image_buffer(output, readback.clone());
        copy.regions[0].image_subresource.array_layers = 0..1;
        commands.copy_image_to_buffer(copy)?;
        let complete = uploader
            .renderer
            .submission_future
            .take()
            .unwrap()
            .then_execute(context.graphics_queue().clone(), commands.build()?)?
            .then_signal_fence_and_flush()?;
        complete.wait(None)?;
        uploader.renderer.submission_future = Some(complete.boxed());
        let pixels = readback.read()?;
        let rgba: Vec<u8> = pixels
            .chunks_exact(4)
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect();
        image::save_buffer(
            PathBuf::from(path),
            &rgba,
            extent[0],
            extent[1],
            image::ColorType::Rgba8,
        )?;
    }
    if let Some(gate) = &gate {
        std::fs::write(gate.with_extension("done"), b"done")?;
        wait_gate(&gate.with_extension("exit"))?;
    }
    Ok(())
}
