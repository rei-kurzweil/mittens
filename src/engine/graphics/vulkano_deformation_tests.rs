use super::*;
use crate::engine::ecs::ComponentId;
use crate::engine::graphics::deformation::oct_decode_normal;
use crate::engine::graphics::mesh::CpuMorphTarget;
use crate::engine::graphics::primitives::InstanceHandle;
use crate::engine::graphics::{GpuRenderable, MaterialHandle, MeshFactory, Transform};
use vulkano::command_buffer::PrimaryAutoCommandBuffer;

fn skin_mesh(targets: &[([f32; 3], [f32; 3])]) -> CpuMesh {
    let mesh = MeshFactory::quad_2d();
    let count = mesh.vertices.len();
    mesh.with_skinning(vec![[0; 4]; count], vec![[1.0, 0.0, 0.0, 0.0]; count])
        .with_morph_targets(
            targets
                .iter()
                .map(|&(position, normal)| CpuMorphTarget {
                    position_deltas: vec![position; count],
                    normal_deltas: vec![normal; count],
                })
                .collect(),
        )
}

fn pose(translation: [f32; 3]) -> [[f32; 4]; 4] {
    let mut matrix = Transform::default().model;
    matrix[3][..3].copy_from_slice(&translation);
    matrix
}

fn instance(
    visuals: &mut VisualWorld,
    component: ComponentId,
    mesh: MeshHandle,
    translation: [f32; 3],
    weights: Vec<(u32, f32)>,
) -> InstanceHandle {
    let handle = visuals.register(
        component,
        GpuRenderable::new(mesh, MaterialHandle::SKINNED_TOON_MESH),
        Transform::default(),
        [1.0; 4],
        1.0,
        false,
        false,
        false,
        false,
        false,
        0.0,
        None,
        3.0,
    );
    assert!(visuals.set_skin_matrices(handle, &[pose(translation)]));
    assert!(visuals.set_active_morphs(component, weights));
    handle
}

// Read back the actual production compute result, including a copy in each
// recording so later deformations cannot hide corruption of an older result.
fn record(
    renderer: &mut VulkanoState,
    visuals: &mut VisualWorld,
) -> Result<
    (
        Arc<PrimaryAutoCommandBuffer>,
        Subbuffer<[GpuDeformedVertex]>,
    ),
    Box<dyn std::error::Error>,
> {
    visuals.sync_deformation_ranges(
        &renderer
            .meshes
            .iter()
            .map(|(&mesh, data)| (mesh, data.vertex_count))
            .collect(),
    );
    let dirty = visuals.take_dirty_bones_ranges();
    let mut commands = AutoCommandBufferBuilder::primary(
        renderer.command_buffer_allocator.clone(),
        renderer.context.graphics_queue().queue_family_index(),
        CommandBufferUsage::OneTimeSubmit,
    )?;
    let output = renderer.record_dirty_deformations(&mut commands, visuals, &dirty)?;
    let readback = Buffer::new_slice::<GpuDeformedVertex>(
        renderer.context.memory_allocator().clone(),
        BufferCreateInfo {
            usage: BufferUsage::TRANSFER_DST,
            ..Default::default()
        },
        AllocationCreateInfo {
            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                | MemoryTypeFilter::HOST_RANDOM_ACCESS,
            ..Default::default()
        },
        output.len(),
    )?;
    commands.copy_buffer(CopyBufferInfo::buffers(output, readback.clone()))?;
    visuals.mark_deformations_clean();
    Ok((commands.build()?, readback))
}

fn assert_vertices(
    readback: &Subbuffer<[GpuDeformedVertex]>,
    visuals: &VisualWorld,
    handle: InstanceHandle,
    translation: [f32; 3],
    normal: [f32; 3],
) {
    let data = readback.read().unwrap();
    let start = visuals.instance(handle).unwrap().deformed_base as usize;
    let length = normal.iter().map(|v| v * v).sum::<f32>().sqrt();
    for (index, base) in MeshFactory::quad_2d().vertices.iter().enumerate() {
        let actual = data[start + index];
        for axis in 0..3 {
            assert!(
                (actual.position[axis] - base.pos[axis] - translation[axis]).abs() < 0.00001,
                "vertex {index}, axis {axis}: {actual:?}, offset {translation:?}"
            );
        }
        let decoded = oct_decode_normal(actual.packed_normal);
        for axis in 0..3 {
            assert!(
                (decoded[axis] - normal[axis] / length).abs() < 0.0001,
                "vertex {index}: normal {decoded:?}, expected {normal:?}"
            );
        }
    }
}

#[test]
#[ignore = "requires Vulkan; verifies shared immutable morph arenas, independent poses/weights, and retained recordings"]
fn morph_arena_reuse_preserves_weights_poses_and_recorded_generations()
-> Result<(), Box<dyn std::error::Error>> {
    let context = RendererContext::new(None, None, None)?;
    let queue = context.graphics_queue().clone();
    let mut renderer = VulkanoState::new(
        context,
        RendererOutput::Xr(Format::B8G8R8A8_UNORM),
        MsaaMode::Off,
    )?;
    let mut components = slotmap::SlotMap::<ComponentId, ()>::with_key();
    let ids: Vec<_> = (0..4).map(|_| components.insert(())).collect();
    let mut visuals = VisualWorld::new();
    let skin_only = MeshHandle(0);
    renderer.upload_mesh(skin_only, &skin_mesh(&[]))?;
    let plain = instance(&mut visuals, ids[0], skin_only, [0.0; 3], vec![]);

    let (commands, pixels) = record(&mut renderer, &mut visuals)?;
    commands
        .execute(queue.clone())?
        .then_signal_fence_and_flush()?
        .wait(None)?;
    assert_vertices(&pixels, &visuals, plain, [0.0; 3], [0.0, 0.0, 1.0]);
    let placeholder = Arc::downgrade(renderer.deformation_morph_buffer.as_ref().unwrap().buffer());
    assert_eq!(renderer.deformation_morph_buffer.as_ref().unwrap().len(), 1);

    // A second dirty skin-only frame reuses the placeholder; an unchanged frame
    // does not allocate or dispatch anything after capacities are initialized.
    let before = renderer.perf_counters();
    assert!(visuals.set_skin_matrices(plain, &[pose([1.0, 0.0, 0.0])]));
    let (commands, pixels) = record(&mut renderer, &mut visuals)?;
    commands
        .execute(queue.clone())?
        .then_signal_fence_and_flush()?
        .wait(None)?;
    assert_vertices(&pixels, &visuals, plain, [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
    assert_eq!(
        renderer
            .perf_counters()
            .saturating_delta(before)
            .deformation_morph_delta_upload_bytes,
        0
    );
    assert!(std::ptr::eq(
        placeholder.as_ptr(),
        Arc::as_ptr(renderer.deformation_morph_buffer.as_ref().unwrap().buffer())
    ));
    let before = renderer.perf_counters();
    let (commands, _) = record(&mut renderer, &mut visuals)?;
    commands
        .execute(queue.clone())?
        .then_signal_fence_and_flush()?
        .wait(None)?;
    let unchanged = renderer.perf_counters().saturating_delta(before);
    assert_eq!(unchanged.deformation_jobs, 0);
    assert_eq!(unchanged.deformation_buffer_allocations, 0);

    let shared = MeshHandle(1);
    renderer.upload_mesh(
        shared,
        &skin_mesh(&[
            ([1.0, 2.0, 0.0], [0.25, 0.0, 0.0]),
            ([-2.0, 0.0, 1.0], [0.0, 0.5, 0.0]),
        ]),
    )?;
    let a = instance(
        &mut visuals,
        ids[1],
        shared,
        [10.0, 0.0, 0.0],
        vec![(0, 0.5), (1, 0.25)],
    );
    let b = instance(
        &mut visuals,
        ids[2],
        shared,
        [0.0, 5.0, 0.0],
        vec![(0, -0.25)],
    );
    let arena = Arc::downgrade(renderer.deformation_morph_buffer.as_ref().unwrap().buffer());
    assert_eq!(renderer.deformation_morph_buffer.as_ref().unwrap().len(), 8);
    let before = renderer.perf_counters();
    let (old_commands, old_pixels) = record(&mut renderer, &mut visuals)?;
    assert_eq!(
        renderer
            .perf_counters()
            .saturating_delta(before)
            .deformation_morph_delta_upload_bytes,
        0
    );

    // Grow the immutable arena while the old descriptor is only recorded, not
    // submitted. New mesh data must not replace the buffer owned by that command.
    let later = MeshHandle(2);
    let before_growth = renderer.perf_counters();
    renderer.upload_mesh(later, &skin_mesh(&[([3.0, -1.0, 0.0], [0.0, 0.0, 0.25])]))?;
    assert_eq!(renderer.meshes[&later].morph_delta_base, Some(8));
    assert_eq!(
        renderer.deformation_morph_buffer.as_ref().unwrap().len(),
        12
    );
    assert_eq!(
        renderer
            .perf_counters()
            .saturating_delta(before_growth)
            .deformation_morph_delta_upload_bytes,
        12 * 32
    );
    assert!(!std::ptr::eq(
        arena.as_ptr(),
        Arc::as_ptr(renderer.deformation_morph_buffer.as_ref().unwrap().buffer())
    ));
    assert!(
        arena.upgrade().is_some(),
        "old recording must retain its arena"
    );
    let c = instance(&mut visuals, ids[3], later, [0.0, 0.0, 2.0], vec![(0, 0.5)]);
    assert!(visuals.set_skin_matrices(b, &[pose([0.0, 6.0, 0.0])]));
    assert!(visuals.set_active_morphs(ids[2], vec![(1, 0.75)]));
    visuals.mark_all_deformations_dirty();
    let before = renderer.perf_counters();
    let (new_commands, new_pixels) = record(&mut renderer, &mut visuals)?;
    assert_eq!(
        renderer
            .perf_counters()
            .saturating_delta(before)
            .deformation_morph_delta_upload_bytes,
        0
    );
    let completion = old_commands
        .execute(queue.clone())?
        .then_execute(queue.clone(), new_commands)?
        .then_signal_fence_and_flush()?;
    completion.wait(None)?;
    assert_vertices(
        &old_pixels,
        &visuals,
        a,
        [10.0, 1.0, 0.25],
        [0.125, 0.125, 1.0],
    );
    assert_vertices(
        &old_pixels,
        &visuals,
        b,
        [-0.25, 4.5, 0.0],
        [-0.0625, 0.0, 1.0],
    );
    assert_vertices(
        &new_pixels,
        &visuals,
        a,
        [10.0, 1.0, 0.25],
        [0.125, 0.125, 1.0],
    );
    assert_vertices(
        &new_pixels,
        &visuals,
        b,
        [-1.5, 6.0, 0.75],
        [0.0, 0.375, 1.0],
    );
    assert_vertices(
        &new_pixels,
        &visuals,
        c,
        [1.5, -0.5, 2.0],
        [0.0, 0.0, 1.125],
    );
    drop(completion);
    assert!(
        arena.upgrade().is_none(),
        "completed recording must release its old arena"
    );
    Ok(())
}
