use super::*;

#[test]
#[ignore = "requires Vulkan; validates bounded custom-size buffers and retained revisions"]
fn custom_f32_snapshots_preserve_revisions_and_reuse_after_ten_thousand_updates()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::engine::graphics::{
        custom_material::{CustomMaterialDefinition, compile},
        materials::MaterialRegistry,
    };
    let context = RendererContext::new(None, None, None)?;
    let mut renderer = VulkanoState::new(
        context,
        RendererOutput::Xr(Format::B8G8R8A8_UNORM),
        MsaaMode::Off,
    )?;
    let mut world = crate::engine::ecs::World::default();
    let source = world
        .add_component(crate::engine::ecs::component::material::MaterialBaseComponent::default());
    let fields = [
        "time_seconds",
        "hue_speed",
        "wave_speed",
        "wave_count",
        "intensity",
    ]
    .map(crate::engine::graphics::materials::MaterialInputField::from)
    .to_vec();
    let shader = compile("assets/shaders/youtuber-led-wave.frag", &fields)?;
    let definition = Arc::new(CustomMaterialDefinition {
        base: crate::engine::graphics::materials::MaterialBaseKind::Surface,
        fields,
        fragment_uri: "assets/shaders/youtuber-led-wave.frag".into(),
        compiled: std::sync::OnceLock::from(Ok(shader)),
    });
    let mut visuals = VisualWorld::new();
    visuals.materials = MaterialRegistry::default();
    let definition = visuals
        .materials
        .register_custom_definition(source, definition);
    let material = visuals
        .materials
        .definition(definition)
        .unwrap()
        .static_program;
    let mut held = Vec::new();
    for revision in 0..8 {
        let id = visuals.materials.upsert_source(
            source,
            definition,
            vec![revision as f32, 0.03, 0.5, 6.0, 1.0],
        );
        renderer.sync_material_instances(&visuals);
        held.push(
            renderer
                .custom_material_set(id, material, TextureHandle(0), TextureFiltering::Linear)?
                .unwrap(),
        );
    }
    let id = visuals
        .materials
        .upsert_source(source, definition, vec![8.0, 0.03, 0.5, 6.0, 1.0]);
    renderer.sync_material_instances(&visuals);
    assert!(
        renderer
            .custom_material_set(id, material, TextureHandle(0), TextureFiltering::Linear)
            .is_err()
    );
    for (index, snapshot) in renderer.material_instances[id]
        .custom_snapshots
        .get(&(TextureHandle(0), TextureFiltering::Linear))
        .unwrap()
        .iter()
        .enumerate()
    {
        let bytes = snapshot.buffer.read()?;
        assert_eq!(
            f32::from_ne_bytes(bytes[0..4].try_into().unwrap()),
            index as f32
        );
    }
    held.clear();
    for edit in 0..10_000 {
        visuals
            .materials
            .upsert_source(source, definition, vec![edit as f32, 0.03, 0.5, 6.0, 1.0]);
        renderer.sync_material_instances(&visuals);
        renderer.custom_material_set(id, material, TextureHandle(0), TextureFiltering::Linear)?;
    }
    assert_eq!(renderer.material_upload_stats.allocations, 8);
    assert_eq!(
        renderer.material_instances[id]
            .custom_snapshots
            .get(&(TextureHandle(0), TextureFiltering::Linear))
            .unwrap()
            .len(),
        8
    );
    // A second typed schema exercises all vector widths and std140 padding
    // through actual GPU buffers/descriptors, alongside scalar snapshots.
    use crate::engine::graphics::materials::{MaterialFieldType as T, MaterialInputField};
    let mixed_fields = [
        ("gain", T::F32),
        ("tint", T::Vec3),
        ("uv", T::Vec2),
        ("rgba", T::Vec4),
    ]
    .map(|(name, ty)| MaterialInputField {
        name: name.into(),
        ty,
    })
    .to_vec();
    let single = crate::engine::graphics::custom_material::compile_source(
        "#version 450\nlayout(location=0) out vec4 color; layout(set=1,binding=0,std140) uniform Params {float gain;vec3 tint;vec2 uv;vec4 rgba;} p; void main(){color=p.rgba*vec4(p.tint*p.gain,1)+vec4(p.uv,0,0);}",
        "mixed-numeric",
        &mixed_fields,
    )?;
    let other = world
        .add_component(crate::engine::ecs::component::material::MaterialBaseComponent::default());
    let other_definition = visuals.materials.register_custom_definition(
        other,
        Arc::new(CustomMaterialDefinition {
            base: crate::engine::graphics::materials::MaterialBaseKind::Surface,
            fields: mixed_fields,
            fragment_uri: "mixed-numeric".into(),
            compiled: std::sync::OnceLock::from(Ok(single)),
        }),
    );
    let other_material = visuals
        .materials
        .definition(other_definition)
        .unwrap()
        .static_program;
    let other_id = visuals.materials.upsert_source(
        other,
        other_definition,
        vec![0.75, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9],
    );
    renderer.sync_material_instances(&visuals);
    renderer.custom_material_set(
        other_id,
        other_material,
        TextureHandle(0),
        TextureFiltering::Linear,
    )?;
    assert_eq!(renderer.custom_programs.len(), 4);
    assert_eq!(
        renderer.material_instances[other_id]
            .custom_snapshots
            .get(&(TextureHandle(0), TextureFiltering::Linear))
            .unwrap()[0]
            .buffer
            .len(),
        64
    );
    assert_eq!(
        renderer.material_instances[id]
            .custom_snapshots
            .get(&(TextureHandle(0), TextureFiltering::Linear))
            .unwrap()[0]
            .buffer
            .len(),
        32
    );
    assert_eq!(renderer.material_upload_stats.allocations, 9);
    // A shared material's numeric revision does not identify its per-draw
    // texture or sampler. Inspect the actual descriptor resources as well.
    let original = renderer
        .custom_material_set(
            other_id,
            other_material,
            TextureHandle(0),
            TextureFiltering::Linear,
        )?
        .unwrap();
    let image_handle = TextureHandle(90_001);
    renderer.upload_texture_rgba8(image_handle, &[210, 80, 30, 255], 1, 1)?;
    let textured = renderer
        .custom_material_set(
            other_id,
            other_material,
            image_handle,
            TextureFiltering::Nearest,
        )?
        .unwrap();
    assert!(!Arc::ptr_eq(&original, &textured));
    assert_eq!(
        renderer.material_instances[other_id].custom_snapshots.len(),
        2
    );
    let resources = textured.resources();
    let Some(vulkano::descriptor_set::DescriptorBindingResources::ImageViewSampler(images)) =
        resources.binding(1)
    else {
        panic!("material texture descriptor")
    };
    let (image, sampler) = images[0].as_ref().unwrap();
    assert!(Arc::ptr_eq(
        &image.image_view,
        &renderer.textures[&image_handle].view
    ));
    assert!(Arc::ptr_eq(sampler, &renderer.sampler_nearest));
    // Runtime captures publish a new image view under the same texture handle.
    // An unchanged numeric revision must still bind the replacement view.
    let replacement = TextureHandle(90_002);
    renderer.upload_texture_rgba8(replacement, &[30, 180, 90, 255], 1, 1)?;
    let replacement_view = renderer.textures[&replacement].view.clone();
    renderer.textures.get_mut(&image_handle).unwrap().view = replacement_view.clone();
    let replaced = renderer
        .custom_material_set(
            other_id,
            other_material,
            image_handle,
            TextureFiltering::Nearest,
        )?
        .unwrap();
    assert!(!Arc::ptr_eq(&textured, &replaced));
    let replacement_resources = replaced.resources();
    let Some(vulkano::descriptor_set::DescriptorBindingResources::ImageViewSampler(images)) =
        replacement_resources.binding(1)
    else {
        panic!("replacement image descriptor");
    };
    assert!(Arc::ptr_eq(
        &images[0].as_ref().unwrap().0.image_view,
        &replacement_view
    ));
    // More textures than the revision limit must remain usable on one shared
    // character material. Eviction drops cache ownership, not retained sets.
    let mut texture_sets = Vec::new();
    for index in 0..material_binding_limit() + 8 {
        let texture = TextureHandle(91_000 + index as u32);
        renderer.upload_texture_rgba8(texture, &[80, 120, 160, 255], 1, 1)?;
        texture_sets.push(
            renderer
                .custom_material_set(other_id, other_material, texture, TextureFiltering::Linear)?
                .unwrap(),
        );
    }
    assert_eq!(
        renderer.material_instances[other_id].custom_snapshots.len(),
        material_binding_limit()
    );
    assert!(
        renderer.material_instances[other_id]
            .custom_snapshots
            .values()
            .all(|slots| slots.len() <= MATERIAL_SNAPSHOT_LIMIT)
    );
    assert!(Arc::ptr_eq(
        &images[0].as_ref().unwrap().0.image_view,
        &replacement_view
    ));
    Ok(())
}
