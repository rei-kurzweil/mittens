// Reusable primary commands. Data contents are deliberately absent from keys:
// recorded handles/ranges/configuration must stay stable, while synchronized uploads vary.
type PrimaryCommands = Arc<vulkano::command_buffer::PrimaryAutoCommandBuffer>;
struct ViewCommands {
    preceding: Vec<PrimaryCommands>,
    final_commands: PrimaryCommands,
}
impl ViewCommands {
    fn execute(
        self,
        mut prior: Box<dyn GpuFuture>,
        queue: Arc<vulkano::device::Queue>,
    ) -> Result<Box<dyn GpuFuture>, Box<dyn std::error::Error>> {
        for commands in self.preceding {
            prior = chain_primary(prior, queue.clone(), commands)?;
        }
        chain_primary(prior, queue, self.final_commands)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct BufferIdentity(usize, u64, u64);
fn buffer_identity<T: BufferContents + ?Sized>(buffer: &Subbuffer<T>) -> BufferIdentity {
    BufferIdentity(
        Arc::as_ptr(buffer.buffer()) as usize,
        buffer.offset(),
        buffer.size(),
    )
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlobalIdentity(BufferIdentity, BufferIdentity, usize);
#[derive(Clone, PartialEq)]
struct BloomIdentity {
    resources: Vec<u64>,
    batches: Vec<crate::engine::graphics::visual_world::DrawBatch>,
}
struct RetainedBloom {
    key: BloomIdentity,
    commands: PrimaryCommands,
    output: Option<Arc<ImageView>>,
    #[cfg(test)]
    draws: Option<sakura_profile::DrawReplay>,
}
#[derive(Default, Clone, Copy, serde::Serialize)]
struct CommandReuseStats {
    hits: u64,
    rebuilds: u64,
    invalidations: u64,
}
impl CommandReuseStats {
    fn delta(self, old: Self) -> Self {
        Self {
            hits: self.hits - old.hits,
            rebuilds: self.rebuilds - old.rebuilds,
            invalidations: self.invalidations - old.invalidations,
        }
    }
}
struct CommandReuse {
    globals:
        crate::engine::graphics::material_cache::MaterialCache<GlobalIdentity, Arc<DescriptorSet>>,
    bloom: crate::engine::graphics::material_cache::MaterialCache<UploadView, RetainedBloom>,
    scenes:
        crate::engine::graphics::material_cache::MaterialCache<(UploadView, usize), RetainedScene>,
    scene_primed:
        crate::engine::graphics::material_cache::MaterialCache<(UploadView, usize), [usize; 24]>,
    scene_scratch: Vec<u64>,
    scene_enabled: bool,
    primed: crate::engine::graphics::material_cache::MaterialCache<UploadView, [usize; 20]>,
    dependencies: Vec<u64>,
    scratch: Vec<u64>,
    bloom_scratch: BloomIdentity,
    stats: CommandReuseStats,
    enabled: bool,
}
impl Default for CommandReuse {
    fn default() -> Self {
        #[cfg(test)]
        let enabled = std::env::var("SHIP_PROFILE_REUSE_COMMANDS").as_deref() != Ok("0");
        #[cfg(not(test))]
        let enabled = true;
        Self {
            globals: crate::engine::graphics::material_cache::MaterialCache::new(128),
            bloom: crate::engine::graphics::material_cache::MaterialCache::new(64),
            scenes: crate::engine::graphics::material_cache::MaterialCache::new(64),
            scene_primed: crate::engine::graphics::material_cache::MaterialCache::new(64),
            scene_scratch: Vec::new(),
            scene_enabled: {
                #[cfg(test)]
                {
                    std::env::var("SHIP_PROFILE_REUSE_SCENE").as_deref() != Ok("0")
                }
                #[cfg(not(test))]
                {
                    true
                }
            },
            primed: crate::engine::graphics::material_cache::MaterialCache::new(64),
            dependencies: Vec::new(),
            scratch: Vec::new(),
            bloom_scratch: BloomIdentity {
                resources: Vec::new(),
                batches: Vec::new(),
            },
            stats: Default::default(),
            enabled,
        }
    }
}
impl VulkanoState {
    // Drop cached recordings before material snapshot selection on a revision change.
    // Otherwise the cache could unnecessarily pin every old snapshot in its bounded pool.
    fn refresh_command_dependencies(&mut self, visuals: &VisualWorld) {
        use slotmap::Key;
        let state = &mut self.command_reuse;
        let scratch = &mut state.scratch;
        scratch.clear();
        scratch.push(visuals.materials.instances().count() as u64);
        for (id, entry) in visuals.materials.instances() {
            scratch.extend([
                id.data().as_ffi(),
                entry.definition.data().as_ffi(),
                entry.revision,
            ]);
        }
        scratch.push(self.textures.len() as u64);
        for (id, texture) in &self.textures {
            scratch.extend([id.0 as u64, Arc::as_ptr(&texture.view) as u64]);
        }
        scratch.push(self.custom_programs.len() as u64);
        for (id, program) in &self.custom_programs {
            scratch.push(id.0 as u64);
            if let Ok(program) = program {
                scratch.extend([
                    Arc::as_ptr(&program.pipeline) as u64,
                    program
                        .extraction_pipeline
                        .as_ref()
                        .map_or(0, |p| Arc::as_ptr(p) as u64),
                ]);
            } else {
                scratch.extend([0, 0]);
            }
        }
        scratch.push(self.meshes.len() as u64);
        for (id, mesh) in &self.meshes {
            scratch.push(id.0 as u64);
            for buffer in [
                buffer_identity(&mesh.vertices),
                buffer_identity(&mesh.indices),
            ] {
                scratch.extend([buffer.0 as u64, buffer.1, buffer.2]);
            }
            scratch.push(mesh.index_count as u64);
        }
        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            // Suppression changes affect which commands exist, not only their data.
            scratch.push(profile.suppressed_meshes.len() as u64);
            scratch.extend(profile.suppressed_meshes.iter().map(|m| m.0 as u64));
        }
        if *scratch != state.dependencies {
            state.bloom.retain(|_, _| false);
            state.scenes.retain(|_, _| false);
            state.scene_primed.retain(|_, _| false);
            state.primed.retain(|_, _| false);
            state.stats.invalidations += 1;
            std::mem::swap(scratch, &mut state.dependencies);
        }
    }
    fn bloom_attachments(post: &PostProcessInvocation) -> [usize; 20] {
        let mut images = [0; 20];
        images[19] = post
            .config
            .bloom
            .as_ref()
            .map_or(0, |b| b.layers.clamp(1, 6) as usize);
        let mut cursor = 0;
        for image in [
            Some(&post.targets.main_color),
            post.targets.main_msaa_color.as_ref(),
            Some(&post.targets.depth),
            post.targets.bloom_source_msaa.as_ref(),
            post.targets.bloom_source.as_ref(),
            post.targets.bloom_a.as_ref(),
            post.targets.bloom_b.as_ref(),
        ] {
            images[cursor] = image.map_or(0, |i| Arc::as_ptr(i) as usize);
            cursor += 1;
        }
        for level in &post.targets.bloom_pyramid {
            images[cursor..cursor + 2].copy_from_slice(&[
                Arc::as_ptr(&level.a) as usize,
                Arc::as_ptr(&level.b) as usize,
            ]);
            cursor += 2;
        }
        images
    }
    fn bloom_can_split(&mut self, post: &PostProcessInvocation) -> bool {
        let view = self.retained_uploads.borrow().active.unwrap();
        self.command_reuse
            .primed
            .get(&view)
            .is_some_and(|old| *old == Self::bloom_attachments(post))
    }
    fn retained_global_set(
        &mut self,
        camera: Subbuffer<CameraUBO>,
        lights: Subbuffer<LightsSSBO>,
        fallback: Arc<ImageView>,
    ) -> Result<Arc<DescriptorSet>, Box<dyn std::error::Error>> {
        let key = GlobalIdentity(
            buffer_identity(&camera),
            buffer_identity(&lights),
            Arc::as_ptr(&fallback) as usize,
        );
        if let Some(set) = self.command_reuse.globals.get(&key) {
            return Ok(set.clone());
        }
        let set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            self.set_layouts.global.clone(),
            [
                WriteDescriptorSet::buffer(0, camera),
                WriteDescriptorSet::buffer(1, lights),
                WriteDescriptorSet::image_view_sampler(
                    2,
                    fallback,
                    self.sampler_scene_color.clone(),
                ),
            ],
            [],
        )?;
        self.command_reuse.globals.insert(key, set.clone());
        Ok(set)
    }
    #[allow(clippy::too_many_arguments)]
    fn retained_bloom_commands(
        &mut self,
        visuals: &VisualWorld,
        post: &PostProcessInvocation,
        extent: [u32; 2],
        fg: &Arc<DescriptorSet>,
        bg: Option<&Arc<DescriptorSet>>,
        rig: &Arc<DescriptorSet>,
        emission: Option<&Subbuffer<[InstanceData]>>,
        emission_count: usize,
        cutout: Option<&Subbuffer<[InstanceData]>>,
        cutout_count: usize,
        background: Option<&Subbuffer<[InstanceData]>>,
        background_count: usize,
    ) -> Result<(PrimaryCommands, Option<Arc<ImageView>>), Box<dyn std::error::Error>> {
        let mut key = std::mem::replace(
            &mut self.command_reuse.bloom_scratch,
            BloomIdentity {
                resources: Vec::new(),
                batches: Vec::new(),
            },
        );
        key.resources.clear();
        key.batches.clear();
        let resources = &mut key.resources;
        resources.extend([
            extent[0] as u64,
            extent[1] as u64,
            post.final_color_format as u64,
            Arc::as_ptr(fg) as u64,
            bg.map_or(0, |s| Arc::as_ptr(s) as u64),
            Arc::as_ptr(rig) as u64,
            emission_count as u64,
            cutout_count as u64,
            background_count as u64,
            post.targets.bloom_extent[0] as u64,
            post.targets.bloom_extent[1] as u64,
            post.config
                .effective_blur_radius_pixels(post.targets.bloom_extent[0])
                .unwrap_or(0) as u64,
        ]);
        if let Some(bloom) = &post.config.bloom {
            resources.extend([1, bloom.intensity.to_bits() as u64, bloom.layers as u64]);
        } else {
            resources.extend([0, 0, 0]);
        }
        for image in [
            Some(&post.targets.depth),
            post.targets.bloom_source_msaa.as_ref(),
            post.targets.bloom_source.as_ref(),
            post.targets.bloom_a.as_ref(),
            post.targets.bloom_b.as_ref(),
        ] {
            resources.push(image.map_or(0, |i| Arc::as_ptr(i) as u64));
        }
        for level in &post.targets.bloom_pyramid {
            resources.extend([
                Arc::as_ptr(&level.a) as u64,
                Arc::as_ptr(&level.b) as u64,
                level.extent[0] as u64,
                level.extent[1] as u64,
            ]);
        }
        for buffer in [emission, cutout, background] {
            let BufferIdentity(ptr, offset, size) = buffer
                .map(buffer_identity)
                .unwrap_or(BufferIdentity(0, 0, 0));
            resources.extend([ptr as u64, offset, size]);
        }
        for batches in [
            visuals.emissive_draw_batches(),
            visuals.emissive_cutout_batches(),
            visuals.background_occluded_lit_emissive_batches(),
        ] {
            resources.push(batches.len() as u64);
            key.batches.extend_from_slice(batches);
            for batch in batches {
                if let Some(mesh) = self.meshes.get(&batch.mesh) {
                    for buffer in [
                        buffer_identity(&mesh.vertices),
                        buffer_identity(&mesh.indices),
                    ] {
                        resources.extend([buffer.0 as u64, buffer.1, buffer.2]);
                    }
                    resources.push(mesh.index_count as u64);
                } else {
                    resources.extend([0; 7]);
                }
                let texture = self.view_mirror_bindings.texture(
                    batch.view_texture_source,
                    batch.texture,
                    self.default_white_texture,
                );
                resources.push(texture.0 as u64);
            }
        }
        let view = self.retained_uploads.borrow().active.expect("active view");
        if let Some(cached) = self
            .command_reuse
            .bloom
            .get(&view)
            .filter(|cached| cached.key == key)
        {
            self.command_reuse.stats.hits += 1;
            #[cfg(test)]
            if let (Some(profile), Some(draws)) = (self.profile.as_mut(), cached.draws.as_ref()) {
                profile.replay_draws(draws);
            }
            let result = (cached.commands.clone(), cached.output.clone());
            self.command_reuse.bloom_scratch = key;
            return Ok(result);
        }
        #[cfg(test)]
        let before = self.profile.as_ref().map(|p| p.snapshot_draws());
        let mut commands = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            self.context.graphics_queue().queue_family_index(),
            CommandBufferUsage::SimultaneousUse,
        )?;
        let output = self.record_bloom_passes(
            &mut commands,
            visuals,
            post,
            extent,
            fg,
            bg,
            rig,
            emission,
            emission_count,
            cutout,
            cutout_count,
            background,
            background_count,
        )?;
        let commands = commands.build()?;
        #[cfg(test)]
        let draws = self
            .profile
            .as_ref()
            .zip(before)
            .map(|(profile, before)| profile.draw_difference(before));
        self.command_reuse.bloom.insert(
            view,
            RetainedBloom {
                key: key.clone(),
                commands: commands.clone(),
                output: output.clone(),
                #[cfg(test)]
                draws,
            },
        );
        self.command_reuse.bloom_scratch = key;
        self.command_reuse.stats.rebuilds += 1;
        Ok((commands, output))
    }
}

#[cfg(test)]
mod command_reuse_tests {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/engine/graphics/vulkano_command_reuse_tests.rs"
    ));
}

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/engine/graphics/vulkano_submission_chain.rs"
));

include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/src/engine/graphics/vulkano_scene_commands.rs"
));
