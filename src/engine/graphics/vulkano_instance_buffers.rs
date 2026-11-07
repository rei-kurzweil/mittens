// Instance-data upload helpers, shared by scene and capture views.
impl VulkanoState {
    fn build_instance_buffer_for_order_or_dummy(
        &self,
        visual_world: &VisualWorld,
        role: InstanceUpload,
        order: &[u32],
    ) -> Result<Subbuffer<[InstanceData]>, Box<dyn std::error::Error>> {
        static DID_LOG_SKIN_INSTANCE_RANGES: AtomicBool = AtomicBool::new(false);

        if !order.is_empty() && env_flag("CAT_DEBUG_SKIN_INSTANCE_RANGES") {
            let instances_ref = visual_world.instances();
            let skinned_count = order
                .iter()
                .filter(|&&idx| instances_ref[idx as usize].bones_count > 0)
                .count();

            if skinned_count > 0 && !DID_LOG_SKIN_INSTANCE_RANGES.swap(true, Ordering::Relaxed) {
                let mut skinned = Vec::new();
                for &idx in order.iter() {
                    let inst = instances_ref[idx as usize];
                    if inst.bones_count > 0 {
                        skinned.push((idx, inst.renderable, inst.bones_base, inst.bones_count));
                        if skinned.len() >= 24 {
                            break;
                        }
                    }
                }

                let total = order.len();
                println!(
                    "[VulkanoRenderer] instances: total={} with_bones={} (showing up to {})",
                    total,
                    skinned_count,
                    skinned.len()
                );
                for (i, (idx, renderable, base, count)) in skinned.iter().enumerate() {
                    println!(
                        "  skinned[{i:02}] instance_index={idx} renderable={renderable:?} bones_base={base} bones_count={count}"
                    );
                }
            }
        }

        let (buf, allocated) = self.upload_instance_order(visual_world, role, order)?;
        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            let [allocations, instances] = profile.instance_buffer_uploads.get();
            profile.instance_buffer_uploads.set([
                allocations + u64::from(allocated),
                instances + order.len().max(1) as u64,
            ]);
        }
        #[cfg(not(test))]
        let _ = allocated;
        Ok(buf)
    }

    fn build_instance_buffer_for_order_opt(
        &self,
        visual_world: &VisualWorld,
        role: InstanceUpload,
        order: &[u32],
    ) -> Result<Option<Subbuffer<[InstanceData]>>, Box<dyn std::error::Error>> {
        if order.is_empty() {
            return Ok(None);
        }

        let (buf, allocated) = self.upload_instance_order(visual_world, role, order)?;
        #[cfg(test)]
        if let Some(profile) = self.profile.as_ref() {
            let [allocations, instances] = profile.instance_buffer_uploads.get();
            profile.instance_buffer_uploads.set([
                allocations + u64::from(allocated),
                instances + order.len() as u64,
            ]);
        }
        #[cfg(not(test))]
        let _ = allocated;
        Ok(Some(buf))
    }
    fn upload_instance_order(
        &self,
        world: &VisualWorld,
        role: InstanceUpload,
        order: &[u32],
    ) -> Result<(Subbuffer<[InstanceData]>, bool), Box<dyn std::error::Error>> {
        self.retained_uploads.borrow_mut().instances(
            self.context.memory_allocator().clone(),
            role,
            order.len(),
            |index| {
                let Some(&instance) = order.get(index) else {
                    return InstanceData::default();
                };
                let inst = world.instances()[instance as usize];
                let m = inst.transform.model;
                InstanceData {
                    i_model_c0: m[0],
                    i_model_c1: m[1],
                    i_model_c2: m[2],
                    i_model_c3: m[3],
                    i_color: inst.color,
                    i_emissive: inst.emissive,
                    i_opacity: inst.opacity,
                    i_deformed_base: inst.deformed_base,
                    i_deformed_count: inst.deformed_count,
                    i_transmission: inst.transmission,
                    i_transmission_roughness: inst.transmission_roughness,
                    i_outline_width: inst.toon_outline.map_or(0., |o| o.width),
                    i_outline_color: inst.toon_outline.map_or([0.; 4], |o| o.color),
                }
            },
        )
    }
}
