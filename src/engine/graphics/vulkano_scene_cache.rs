// Whole graphics-primary reuse for views whose draw order is structural.
// Transforms, camera/light values, skin matrices and morph weights remain data.
#[derive(Clone, PartialEq)]
struct SceneIdentity {
    resources: Vec<u64>,
    post: Option<PostProcessingConfig>,
}
struct RetainedScene {
    key: SceneIdentity,
    commands: PrimaryCommands,
    #[cfg(test)]
    draws: Option<sakura_profile::DrawReplay>,
}
impl VulkanoState {
    fn scene_attachments(
        color: &Arc<ImageView>,
        resolve: Option<&Arc<ImageView>>,
        depth: &Arc<ImageView>,
        post: Option<&PostProcessInvocation>,
        active_bloom: bool,
    ) -> [usize; 24] {
        let mut images = [0; 24];
        images[0] = Arc::as_ptr(color) as usize;
        images[1] = resolve.map_or(0, |r| Arc::as_ptr(r) as usize);
        images[2] = Arc::as_ptr(depth) as usize;
        if let Some(post) = post {
            images[3] = Arc::as_ptr(&post.final_output_view) as usize;
            images[4..].copy_from_slice(&Self::bloom_attachments(post));
            // Zero denotes no extraction has initialized these targets yet.
            if !active_bloom {
                images[23] = 0;
            }
        }
        images
    }
    #[allow(clippy::too_many_arguments)]
    fn scene_identity(
        &mut self,
        visuals: &VisualWorld,
        extent: [u32; 2],
        attachments: &[usize; 24],
        post: Option<&PostProcessInvocation>,
        fg: &Arc<DescriptorSet>,
        bg: Option<&Arc<DescriptorSet>>,
        rig: &Arc<DescriptorSet>,
        buffers: [Option<&Subbuffer<[InstanceData]>>; 12],
    ) -> SceneIdentity {
        use slotmap::Key;
        let mut resources = std::mem::take(&mut self.command_reuse.scene_scratch);
        resources.clear();
        resources.extend(attachments.iter().map(|&id| id as u64));
        resources.extend([
            extent[0] as u64,
            extent[1] as u64,
            self.msaa_samples as u64,
            Arc::as_ptr(fg) as u64,
            bg.map_or(0, |bg| Arc::as_ptr(bg) as u64),
            Arc::as_ptr(rig) as u64,
        ]);
        resources.extend(visuals.clear_color().map(|v| v.to_bits() as u64));
        resources.push(post.map_or(0, |p| p.final_color_format as u64));
        for buffer in buffers {
            let BufferIdentity(ptr, offset, size) = buffer
                .map(buffer_identity)
                .unwrap_or(BufferIdentity(0, 0, 0));
            resources.extend([ptr as u64, offset, size]);
        }
        for (source, texture) in &self.view_mirror_bindings.textures {
            resources.extend([source.data().as_ffi(), texture.0 as u64]);
        }
        #[cfg(test)]
        resources.push(self.profile.as_ref().map_or(0, |p| p.timestamp_identity()) as u64);
        SceneIdentity {
            resources,
            post: post.map(|p| p.config.clone()),
        }
    }
}
