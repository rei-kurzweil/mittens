//! Controlled offscreen stereo timing of the actual ship scene. Test-only hooks.
//! SHIP_PROFILE_CASE=full SHIP_PROFILE_ROOM=bridge cargo test --release --lib profile_ship_stereo -- --ignored --nocapture
use super::*;
use crate::engine::ecs::system::external_xr_frame::ExternalXrFrame;

#[test]
#[ignore = "requires Vulkan; ship stereo CPU/GPU timing without live headset pacing"]
fn profile_ship_stereo() -> Result<(), Box<dyn Error>> {
    let case = std::env::var("SHIP_PROFILE_CASE").unwrap_or("full".into());
    let room = std::env::var("SHIP_PROFILE_ROOM").unwrap_or("bridge".into());
    let motion = std::env::var("SHIP_PROFILE_MOTION").unwrap_or("stationary".into());
    assert!(matches!(
        motion.as_str(),
        "stationary" | "head" | "walking" | "driving"
    ));
    let frames: usize = std::env::var("SHIP_PROFILE_FRAMES")
        .unwrap_or("120".into())
        .parse()?;
    let warmup: usize = std::env::var("SHIP_PROFILE_WARMUP")
        .unwrap_or("120".into())
        .parse()?;
    assert!(frames > 0 && warmup >= 4);
    assert!(matches!(
        case.as_str(),
        "full"
            | "no_bloom"
            | "avatar_hidden"
            | "environment_hidden"
            | "no_callbacks"
            | "paired_bloom"
    ));
    let (position, mut yaw) = match room.as_str() {
        "bridge" => ([0., 1., -3.], 0f32),
        "cafeteria" => ([0., 1., 10.], std::f32::consts::PI),
        "orchard" => ([18., 6.2, 7.5], -2.1112158),
        _ => return Err("choose bridge, cafeteria or orchard".into()),
    };
    if motion == "driving" {
        yaw = 0.;
    }
    // Per-eye target; override to match the runtime's reported swapchain extent.
    let extent = [
        std::env::var("SHIP_PROFILE_WIDTH")
            .unwrap_or("1712".into())
            .parse::<u32>()?,
        std::env::var("SHIP_PROFILE_HEIGHT")
            .unwrap_or("1712".into())
            .parse::<u32>()?,
    ];
    assert!(extent.iter().all(|&value| value > 0));
    let context = RendererContext::new(None, None, None)?;
    let mut renderer = VulkanoState::new(
        context.clone(),
        RendererOutput::Xr(Format::B8G8R8A8_UNORM),
        MsaaMode::Msaa4x,
    )?;
    let mut scene = Universe::new(Default::default());
    scene.systems.set_vr_perf_enabled(true);
    scene.visuals.enable_draw_cache_profile();
    let clock = Arc::new(ProfileClock(std::sync::atomic::AtomicU64::new(
        0f64.to_bits(),
    )));
    scene.systems.clock.set_driver(clock.clone());
    let view = openxr::View {
        pose: openxr::Posef {
            orientation: openxr::Quaternionf {
                x: 0.,
                y: (yaw / 2.).sin(),
                z: 0.,
                w: (yaw / 2.).cos(),
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
    let source_path = std::env::var("SHIP_PROFILE_SOURCE")
        .unwrap_or_else(|_| "examples/ship_interior.mms".into());
    let mut source = std::fs::read_to_string(&source_path)?
        .replace(
            "T.position(0.0, 1.0, -3.0)",
            &format!(
                "T.position({},{},{})",
                position[0], position[1], position[2]
            ),
        )
        .replace(
            "let microphone = AudioInput {",
            "let microphone = AudioOscillator.square() { enabled(false) amplitude(0.0)",
        );
    if case == "no_bloom" {
        source = source.replace("let bloom_enabled = true", "let bloom_enabled = false");
    }
    scene.load_mms_source_at_path(&source, &source_path)?;
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
        scene.update(1. / 75., &Default::default());
        prepare(&mut scene, &mut uploader);
    }
    for id in scene.world.all_components() {
        if let Some(gltf) = scene.world.get_component_by_id_as::<GLTFComponent>(id) {
            assert!(gltf.spawned, "unloaded {}", gltf.uri);
        }
    }
    for (_, instance) in scene.visuals.materials.instances() {
        if let Some(custom) = scene
            .visuals
            .materials
            .definition(instance.definition)
            .and_then(|d| d.custom.as_ref())
        {
            let started = Instant::now();
            while custom.compiled.get().is_none() && started.elapsed() < Duration::from_secs(30) {
                std::thread::sleep(Duration::from_millis(5));
            }
            custom
                .compiled
                .get()
                .ok_or("shader compile timeout")?
                .as_ref()
                .map_err(Clone::clone)?;
        }
    }
    scene.systems.audio = crate::engine::ecs::system::AudioSystem::new();
    scene.systems.clock.set_driver(clock.clone());
    let physical_motion = if matches!(motion.as_str(), "walking" | "driving") {
        assert_eq!(room, "cafeteria");
        assert!(matches!(case.as_str(), "full" | "no_bloom"));
        Some(ship_motion::ShipMotion::setup(&mut scene, &motion)?)
    } else {
        None
    };
    let camera_replay = ship_motion::CameraReplay::from_env()?;
    let mut profile = ProfileState::new(&context)?;
    if matches!(case.as_str(), "avatar_hidden" | "environment_hidden") {
        profile.suppressed_meshes = scene
            .visuals
            .instances()
            .iter()
            .filter(|i| {
                let mut current = Some(i.component);
                let mut avatar = false;
                while let Some(id) = current {
                    avatar |= scene.world.component_label(id) == Some("ship_xr_mitten");
                    current = scene.world.parent_of(id);
                }
                if case == "avatar_hidden" {
                    avatar
                } else {
                    !avatar
                }
            })
            .map(|i| i.renderable.mesh)
            .collect();
    }
    let hidden_subtree = std::env::var("SHIP_PROFILE_HIDE_SUBTREE").ok();
    if let Some(label) = hidden_subtree.as_deref() {
        // AVC can move imported nodes outside their original GLTF component.
        let mut roots: HashSet<_> = scene
            .world
            .all_components()
            .filter(|&id| scene.world.component_label(id) == Some(label))
            .collect();
        for id in roots.clone() {
            if let Some(gltf) = scene.world.get_component_by_id_as::<GLTFComponent>(id) {
                roots.extend(gltf.spawned_node_transforms.iter().copied());
            }
        }
        let selected: HashSet<_> = scene
            .visuals
            .instances()
            .iter()
            .filter(|instance| {
                let mut current = Some(instance.component);
                while let Some(id) = current {
                    if roots.contains(&id) {
                        return true;
                    }
                    current = scene.world.parent_of(id);
                }
                false
            })
            .map(|i| i.renderable.mesh)
            .collect();
        assert!(
            !selected.is_empty(),
            "diagnostic subtree selected no meshes: {label}"
        );
        eprintln!(
            "SHIP_PROFILE_HIDE_SUBTREE {label}: {} meshes",
            selected.len()
        );
        profile.suppressed_meshes.extend(selected);
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
    let paired = case == "paired_bloom";
    assert!(!paired || (warmup % 2 == 0 && frames % 2 == 0));
    let authored_graph = scene.visuals.post_processing().clone();
    assert!(!paired || (authored_graph.enabled && authored_graph.bloom.is_some()));
    // ABBA blocks keep geometry/material IDs fixed and balance ordering. Each
    // state receives the requested warmup/sample count, split across two blocks.
    let block_span = (warmup + frames) / 2;
    let total_frames = if paired {
        2 * (warmup + frames)
    } else {
        warmup + frames
    };
    let mut rows = Vec::new();
    let mut counters_start = None;
    for frame in 0..total_frames {
        let block = if paired { frame / block_span } else { 0 };
        let bloom_on = if paired {
            matches!(block, 0 | 3)
        } else {
            case != "no_bloom"
        };
        let sampling = if paired {
            frame % block_span >= warmup / 2
        } else {
            frame >= warmup
        };
        if paired && frame % block_span == 0 {
            eprintln!("SHIP_BLOOM_BLOCK room={room} block={block} bloom={bloom_on}");
        }
        if frame == if paired { warmup / 2 } else { warmup } {
            if case == "no_callbacks" {
                scene.close_mms_session();
            }
            counters_start = Some(uploader.renderer.perf_counters());
            eprintln!("SHIP_PROFILE_BEGIN case={case} room={room} frames={frames}");
        }
        clock.0.store(
            (frame as f64 / 75.).to_bits(),
            std::sync::atomic::Ordering::Relaxed,
        );
        uploader.renderer.profile.as_mut().unwrap().reset_frame();
        scene.visuals.reset_draw_cache_profile();
        let material_before = material_cache_totals(uploader.renderer);
        let uploads_before = uploader.renderer.retained_uploads.borrow().stats;
        let commands_before = uploader.renderer.command_reuse.stats;
        if let Some(pilot) = physical_motion.as_ref() {
            pilot.input(&mut scene, view, frame);
        }
        if motion == "head" {
            let mut moving = view;
            moving.pose.position.x = (frame as f32 / 75.).sin() * 0.35;
            scene.systems.xr.set_external_frame(Some(ExternalXrFrame {
                views: Some([moving, moving]),
                ..Default::default()
            }));
        }
        let start = Instant::now();
        scene.update(1. / 75., &Default::default());
        let update_ms = start.elapsed().as_secs_f64() * 1000.;
        let prep = Instant::now();
        prepare(&mut scene, &mut uploader);
        if paired {
            scene.visuals.set_post_processing(if bloom_on {
                authored_graph.clone()
            } else {
                Default::default()
            });
        }
        if let Some(replay) = &camera_replay {
            replay.apply(&mut scene, frame, warmup);
        }
        let prepare_ms = prep.elapsed().as_secs_f64() * 1000.;
        let record = Instant::now();
        // Mirror remains frozen as in the actual bridge/cafeteria view.
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
            .ok_or("missing stereo completion")?()?;
        if let Some(future) = uploader.renderer.submission_future.as_mut() {
            future.cleanup_finished();
        }
        let wait_ms = wait.elapsed().as_secs_f64() * 1000.;
        let motion_state = physical_motion.as_ref().map(|pilot| pilot.sample(&scene));
        let p = uploader.renderer.profile.as_ref().unwrap();
        let gpu = p.gpu_ms()?;
        if sampling {
            let material_after = material_cache_totals(uploader.renderer);
            let material_delta: std::collections::BTreeMap<_, _> = [
                "immutable_hits",
                "immutable_misses",
                "mutable_allocations",
                "mutable_uploads",
                "mutable_snapshot_reuses",
                "mutable_binding_misses",
                "mutable_binding_evictions",
            ]
            .into_iter()
            .enumerate()
            .map(|(i, name)| (name, material_after[i].saturating_sub(material_before[i])))
            .collect();
            rows.push(serde_json::json!({"frame":frame,"bloom_enabled":bloom_on,"block":block,"update_ms":update_ms,"prepare_ms":prepare_ms,
                "record_submit_ms":record_ms,"wait_ms":wait_ms,"total_ms":start.elapsed().as_secs_f64()*1000.,
                "gpu_deform_ms":gpu.map(|g|g[0]),"gpu_graphics_ms":gpu.map(|g|g[1]),"draws":p.draws,
                "cpu_record_stages_ms":p.cpu_stages.milliseconds(),
                "visual_cache_counts":scene.visuals.draw_cache_profile_snapshot(),
                "view_cache_decisions":p.view_cache_decisions,
                "eye0_view":scene.visuals.visual_camera(crate::engine::graphics::CameraTarget::Xr).unwrap().eyes[0].view,
                "instance_buffer_uploads":p.instance_buffer_uploads.get(),
                "motion_state":motion_state,"gpu_pass_ms":p.gpu_pass_ms()?,
                "retained_uploads":uploader.renderer.retained_uploads.borrow().stats.delta(uploads_before),
                "command_reuse":uploader.renderer.command_reuse.stats.delta(commands_before),
                "material_cache_counts":material_delta,
                "dirty_jobs":p.dirty_components.len(),"deformation_cpu_ms":p.deformation_cpu_ns as f64/1e6}));
        }
    }
    if let Some(pilot) = physical_motion.as_ref() {
        pilot.validate(&rows);
    }
    if motion == "head" {
        assert_ne!(
            rows.first().unwrap()["eye0_view"],
            rows.last().unwrap()["eye0_view"],
            "moving-head fixture must change the rendered camera, not only its input pose"
        );
    }
    let mut timing = serde_json::Map::new();
    for key in [
        "update_ms",
        "prepare_ms",
        "record_submit_ms",
        "wait_ms",
        "total_ms",
        "gpu_deform_ms",
        "gpu_graphics_ms",
        "deformation_cpu_ms",
    ] {
        let values: Vec<_> = rows.iter().filter_map(|r| r[key].as_f64()).collect();
        if !values.is_empty() {
            timing.insert(key.into(), summary(&values));
        }
    }
    let mut gpu_passes = serde_json::Map::new();
    for name in rows[0]["gpu_pass_ms"].as_object().unwrap().keys() {
        let values: Vec<_> = rows
            .iter()
            .map(|r| r["gpu_pass_ms"][name].as_f64().unwrap())
            .collect();
        gpu_passes.insert(name.clone(), summary(&values));
    }
    let mut cpu_record_stages = serde_json::Map::new();
    for name in rows[0]["cpu_record_stages_ms"].as_object().unwrap().keys() {
        let values: Vec<_> = rows
            .iter()
            .filter_map(|row| row["cpu_record_stages_ms"][name].as_f64())
            .collect();
        cpu_record_stages.insert(name.clone(), summary(&values));
    }
    let summarize_counts = |values: &[f64]| {
        let timing = summary(values);
        serde_json::json!({"mean":timing["mean_ms"], "median":timing["median_ms"], "p95":timing["p95_ms"]})
    };
    let mut cache_counts = serde_json::Map::new();
    for field in ["visual_cache_counts", "material_cache_counts"] {
        let names: std::collections::BTreeSet<_> = rows
            .iter()
            .flat_map(|row| row[field].as_object().unwrap().keys().cloned())
            .collect();
        let values: serde_json::Map<_, _> = names
            .into_iter()
            .map(|name| {
                let counts: Vec<_> = rows
                    .iter()
                    .map(|row| row[field][&name].as_f64().unwrap_or(0.))
                    .collect();
                (name, summarize_counts(&counts))
            })
            .collect();
        cache_counts.insert(field.into(), values.into());
    }
    let allocations: Vec<_> = rows
        .iter()
        .map(|r| r["instance_buffer_uploads"][0].as_f64().unwrap())
        .collect();
    let writes: Vec<_> = rows
        .iter()
        .map(|r| r["instance_buffer_uploads"][1].as_f64().unwrap())
        .collect();
    cache_counts.insert(
        "instance_buffer_allocations".into(),
        summarize_counts(&allocations),
    );
    cache_counts.insert(
        "instance_buffer_instances_written".into(),
        summarize_counts(&writes),
    );
    for field in ["allocations", "reuses", "busy_replacements"] {
        let values: Vec<_> = rows
            .iter()
            .map(|r| r["retained_uploads"][field].as_f64().unwrap())
            .collect();
        cache_counts.insert(
            format!("retained_upload_{field}"),
            summarize_counts(&values),
        );
    }
    for field in ["hits", "rebuilds", "invalidations"] {
        let values: Vec<_> = rows
            .iter()
            .map(|r| r["command_reuse"][field].as_f64().unwrap())
            .collect();
        cache_counts.insert(format!("command_reuse_{field}"), summarize_counts(&values));
    }
    let paired_gpu = if paired {
        let summarize = |enabled: bool| {
            let values: Vec<_> = rows
                .iter()
                .filter(|r| r["bloom_enabled"].as_bool() == Some(enabled))
                .filter_map(|r| r["gpu_graphics_ms"].as_f64())
                .collect();
            assert_eq!(values.len(), frames);
            let blocks: Vec<_> = (0..4)
                .filter(|&block| matches!(block, 0 | 3) == enabled)
                .map(|block| {
                    let values: Vec<_> = rows
                        .iter()
                        .filter(|r| r["block"].as_u64() == Some(block))
                        .filter_map(|r| r["gpu_graphics_ms"].as_f64())
                        .collect();
                    serde_json::json!({"block":block,"timing":summary(&values)})
                })
                .collect();
            serde_json::json!({"gpu_graphics_ms":summary(&values),"blocks":blocks})
        };
        serde_json::json!({"on":summarize(true),"off":summarize(false),"samples_per_state":frames})
    } else {
        serde_json::Value::Null
    };
    let result = serde_json::json!({"hidden_subtree":hidden_subtree,"camera_reference":camera_replay.as_ref().map(|r|&r.path),"gpu_pass_ms":gpu_passes,"source":source_path,"paired_gpu":paired_gpu,"case":case,"room":room,"motion":motion,"cache_counts":cache_counts,"material_binding_limit":material_binding_limit(),"frames":frames,"warmup":warmup,"extent":extent,"msaa":4,
        "gpu":uploader.renderer.gpu_device_name(),"components":scene.world.all_components().count(),
        "instances":scene.visuals.instances().len(),"timing":timing,"cpu_record_stages_ms":cpu_record_stages,"sample_draws":rows.last().unwrap()["draws"],
        "counters":uploader.renderer.perf_counters().saturating_delta(counters_start.unwrap())});
    println!("SHIP_PROFILE_RESULT {result}");
    if let Some(path) = std::env::var_os("SHIP_PROFILE_OUTPUT") {
        let path = PathBuf::from(path);
        std::fs::write(
            path.with_extension("frames.json"),
            serde_json::to_vec(&rows)?,
        )?;
        std::fs::write(path, serde_json::to_vec_pretty(&result)?)?;
    }
    Ok(())
}

// Test-only aggregation happens outside the timed recording path.
fn material_cache_totals(renderer: &VulkanoState) -> [u64; 7] {
    let (misses, evictions) = renderer
        .material_instances
        .values()
        .fold((0, 0), |(m, e), entry| {
            (m + entry.bindings.misses, e + entry.bindings.evictions)
        });
    [
        renderer.cached_material_sets.hits,
        renderer.cached_material_sets.misses,
        renderer.material_upload_stats.allocations,
        renderer.material_upload_stats.uploads,
        renderer.material_upload_stats.snapshot_reuses,
        misses,
        evictions,
    ]
}
