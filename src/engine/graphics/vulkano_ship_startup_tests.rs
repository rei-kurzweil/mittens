use super::*;
use crate::engine::graphics::{MeshUploader, TextureHandle, TextureUploader};

struct Upload<'a> {
    renderer: &'a mut VulkanoState,
    mesh: &'a mut u32,
    texture: &'a mut u32,
}
impl MeshUploader for Upload<'_> {
    fn upload_mesh(&mut self, mesh: &CpuMesh) -> Result<MeshHandle, Box<dyn std::error::Error>> {
        let handle = MeshHandle(*self.mesh);
        *self.mesh += 1;
        self.renderer.upload_mesh(handle, mesh)?;
        Ok(handle)
    }
}
impl TextureUploader for Upload<'_> {
    fn upload_texture_rgba8(
        &mut self,
        rgba: &[u8],
        w: u32,
        h: u32,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        let handle = TextureHandle(*self.texture);
        *self.texture += 1;
        self.renderer.upload_texture_rgba8(handle, rgba, w, h)?;
        Ok(handle)
    }
    fn upload_texture_bc7(
        &mut self,
        rgba: &[u8],
        w: u32,
        h: u32,
        srgb: bool,
    ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
        let handle = TextureHandle(*self.texture);
        *self.texture += 1;
        self.renderer.upload_texture_bc7(handle, rgba, w, h, srgb)?;
        Ok(handle)
    }
}

#[test]
#[ignore = "requires Vulkan; captures the ship's startup XR view"]
fn ship_xr_first_view_is_not_black() -> Result<(), Box<dyn std::error::Error>> {
    let context = RendererContext::new(None, None, None)?;
    let extent = [480, 320];
    let format = Format::B8G8R8A8_UNORM;
    let mut renderer = VulkanoState::new(
        context.clone(),
        RendererOutput::Xr(format),
        MsaaMode::Msaa4x,
    )?;
    let output = Image::new(
        context.memory_allocator().clone(),
        ImageCreateInfo {
            image_type: ImageType::Dim2d,
            format,
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
                    view_type: ImageViewType::Dim2d,
                    subresource_range: ImageSubresourceRange {
                        array_layers: eye..eye + 1,
                        ..output.subresource_range()
                    },
                    ..ImageViewCreateInfo::from_image(&output)
                },
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut scene = crate::engine::Universe::new(Default::default());
    let kiosk = std::env::var_os("SHIP_CAPTURE_HOTDOG").is_some();
    let room = std::env::var("SHIP_CAPTURE_ROOM").unwrap_or_default();
    let yaw: f32 = if matches!(room.as_str(), "landing" | "grocery" | "mirror") {
        std::f32::consts::FRAC_PI_2
    } else if room == "biodomes" {
        -std::f32::consts::FRAC_PI_2
    } else if room == "truck" {
        -2.1112158
    } else if kiosk {
        std::f32::consts::PI
    } else {
        0.0
    };
    let view = openxr::View {
        pose: openxr::Posef {
            orientation: openxr::Quaternionf {
                x: 0.,
                y: (yaw / 2.0).sin(),
                z: 0.,
                w: (yaw / 2.0).cos(),
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
    scene.systems.xr.set_external_frame(Some(
        crate::engine::ecs::system::external_xr_frame::ExternalXrFrame {
            views: Some([view, view]),
            ..Default::default()
        },
    ));
    eprintln!("loading ship GPU fixture");
    let source = include_str!("../../../examples/ship_interior.mms");
    let isolated = std::env::var("SHIP_CAPTURE_HOTDOG").as_deref() == Ok("isolated");
    let source = if isolated {
        r#"
        import { hot_dog_stand } from "../assets/components/hot_dog_stand.mms"
        import { ship_solid } from "../assets/components/ship_parts.mms"
        import { ship_light_pool } from "../assets/components/ship_lighting.mms"
        RendererSettings { window_visible(false) }
        AL.rgb(0.25,0.25,0.25)
        T.position(-4.3,0,16.1) { hot_dog_stand() }
        ship_solid([0,-0.1,15], [20,0.2,20], [0.19,0.23,0.25])
        T.position(-4.3,0,13.0) { InputXR.on() { T { CXR {} } } }
        "#
        .to_string()
    } else if !room.is_empty() {
        let camera = match room.as_str() {
            "mirror" => "T.position(-20.8,5.2,10.45)",
            "grocery" => "T.position(-2.6,0.0,3.0)",
            "sky" => "T.position(43.0,30.0,15.0)",
            "washroom" => "T.position(-20.0,5.2,11.2)",
            "landing" => "T.position(-8.0,5.2,15.0)",
            "biodomes" => "T.position(17.0,5.2,15.0)",
            "truck" => "T.position(18.0,5.2,7.5)",
            "truck_cab" => "T.position(124.52,5.2,20.8)",
            _ => panic!("unknown SHIP_CAPTURE_ROOM: {room}"),
        };
        source
            .replace(
                "let xr_avatar_enabled = true",
                "let xr_avatar_enabled = false",
            )
            .replace("T.position(0.0, 0.0, -3.0)", camera)
    } else if kiosk {
        source
            .replace(
                "let xr_avatar_enabled = true",
                "let xr_avatar_enabled = false",
            )
            .replace("T.position(0.0, 0.0, -3.0)", "T.position(-7.3, 0.0, 13.0)")
    } else {
        source.to_string()
    };
    let source = if matches!(room.as_str(), "biodomes" | "truck" | "truck_cab") {
        format!(
            "{source}\nfor label in [\"ship_convenience_shop\",\"ship_liquor_shop\",\"ship_motorhomes\",\"ship_driving_vehicles\"] {{ let control=query(\"#\"+label) if control {{ control.set_expanded(true) }} }}"
        )
    } else {
        source
    };
    // This preview teleports past the doorway activation zones.
    let source = if room == "mirror" {
        format!("{source}\nquery(\"#ship_womens_washroom_mirror\").set_enabled(true)")
    } else {
        source
    };
    // Capture XR explicitly even when the editable example defaults to desktop.
    let source = source
        .lines()
        .map(|line| {
            if line.starts_with("let desktop_view_enabled = ") {
                "let desktop_view_enabled = false"
            } else if line.starts_with("let xr_view_enabled = ") {
                "let xr_view_enabled = true"
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    scene.load_mms_source_at_path(&source, "examples/ship_interior.mms")?;
    let mut mesh = 1000;
    let mut texture = 1000;
    for frame in 0..24 {
        eprintln!("ship GPU frame {frame}");
        scene.update(1. / 90., &Default::default());
        scene.systems.prepare_render(
            &mut scene.world,
            &mut scene.visuals,
            &mut scene.render_assets,
            &mut Upload {
                renderer: &mut renderer,
                mesh: &mut mesh,
                texture: &mut texture,
            },
            &mut scene.command_queue,
        );
        if room == "mirror" {
            renderer.submit_xr_mirror_captures(&mut scene.visuals)?;
        }
        for eye in 0..2 {
            renderer.submit_xr_eye_to_view(
                &mut scene.visuals,
                eye,
                extent,
                2,
                eyes[eye].clone(),
            )?;
        }
    }
    if !isolated {
        let legacy_surfaces = scene
            .visuals
            .instances()
            .iter()
            .filter(|instance| {
                matches!(
                    instance.renderable.material,
                    MaterialHandle::TOON_MESH | MaterialHandle::SKINNED_TOON_MESH
                ) && !instance
                    .material_instance
                    .and_then(|id| scene.visuals.materials.instance(id))
                    .and_then(|material| scene.visuals.materials.definition(material.definition))
                    .is_some_and(|definition| definition.custom.is_some())
            })
            .count();
        assert_eq!(
            legacy_surfaces, 0,
            "ship default surfaces must use anime shading"
        );
    }
    if kiosk {
        for label in [
            "ship_hot_dog_vendor_root",
            "ship_hot_dog_vendor",
            "J_Bip_C_Head",
            "ship_vendor_held_hot_dog",
        ] {
            let id = scene
                .world
                .all_components()
                .find(|&id| scene.world.component_label(id) == Some(label))
                .unwrap();
            eprintln!(
                "{label}: {:?}",
                crate::engine::ecs::system::TransformSystem::world_position(&scene.world, id)
            );
        }
        let held = scene
            .world
            .all_components()
            .find(|&id| scene.world.component_label(id) == Some("ship_vendor_held_hot_dog"))
            .expect("vendor must attach her hot dog after GLTF initialization");
        assert_eq!(
            scene
                .world
                .component_label(scene.world.parent_of(held).unwrap()),
            Some("J_Bip_R_Hand")
        );
    }
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
        (extent[0] * extent[1] * 8) as u64,
    )?;
    let mut commands = AutoCommandBufferBuilder::primary(
        renderer.command_buffer_allocator.clone(),
        context.graphics_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;
    commands.copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
        output,
        readback.clone(),
    ))?;
    let completion = renderer
        .submission_future
        .take()
        .unwrap()
        .then_execute(context.graphics_queue().clone(), commands.build()?)?
        .then_signal_fence_and_flush()?;
    completion.wait(None)?;
    renderer.submission_future = Some(completion.boxed());
    let pixels = readback.read()?;
    let rgba: Vec<u8> = pixels[..(extent[0] * extent[1] * 4) as usize]
        .chunks_exact(4)
        .flat_map(|p| [p[2], p[1], p[0], p[3]])
        .collect();
    image::save_buffer(
        if room == "mirror" {
            "/tmp/ship-washroom-mirror.png"
        } else if room == "grocery" {
            "/tmp/ship-grocery-security-store.png"
        } else if room == "sky" {
            "/tmp/ship-gradient-cloud-sky.png"
        } else if room == "landing" {
            "/tmp/ship-washroom-landing.png"
        } else if room == "washroom" {
            "/tmp/ship-washroom.png"
        } else if room == "biodomes" {
            "/tmp/ship-biodomes.png"
        } else if room == "truck" {
            "/tmp/ship-pickup.png"
        } else if room == "truck_cab" {
            "/tmp/ship-pickup-cab.png"
        } else if kiosk {
            "/tmp/ship-hotdog-stand.png"
        } else {
            "/tmp/ship-xr-first-view.png"
        },
        &rgba,
        extent[0],
        extent[1],
        image::ColorType::Rgba8,
    )?;
    let visible = rgba
        .chunks_exact(4)
        .filter(|p| p[0].max(p[1]).max(p[2]) > 12)
        .count();
    eprintln!(
        "ship visible pixels {visible}, instances {}",
        scene.visuals.instance_count()
    );
    assert!(
        visible > (extent[0] * extent[1] / 10) as usize,
        "ship startup view is black"
    );
    Ok(())
}
