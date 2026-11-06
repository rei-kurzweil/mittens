//! Opt-in fixed-size counters. No maps or allocation in mutation hot paths.
use super::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
#[repr(usize)]
pub(super) enum VisualChange {
    BindMaterialInstance,
    Clear,
    Register,
    RegisterStencilClip,
    Remove,
    RemoveMaterialSource,
    RetainRuntimeTextures,
    SetSkinMatrices,
    SyncDeformationRanges,
    UnregisterStencilClip,
    Update,
    UpdateColor,
    UpdateEmissive,
    UpdateMaterial,
    UpdateModel,
    UpdateOpacityState,
    UpdateQuantSteps,
    UpdateSkinRange,
    UpdateStencilRef,
    UpdateTexture,
    UpdateTextureFiltering,
    UpdateToonOutline,
    UpdateTransform,
    UpdateTransmission,
    UpdateTransmissionRoughness,
    UpdateTransparentCutout,
}
const REASONS: &[&str] = &[
    "bind_material_instance",
    "clear",
    "register",
    "register_stencil_clip",
    "remove",
    "remove_material_source",
    "retain_runtime_textures",
    "set_skin_matrices",
    "sync_deformation_ranges",
    "unregister_stencil_clip",
    "update",
    "update_color",
    "update_emissive",
    "update_material",
    "update_model",
    "update_opacity_state",
    "update_quant_steps",
    "update_skin_range",
    "update_stencil_ref",
    "update_texture",
    "update_texture_filtering",
    "update_toon_outline",
    "update_transform",
    "update_transmission",
    "update_transmission_roughness",
    "update_transparent_cutout",
];
const COUNT: usize = REASONS.len();

#[derive(Default)]
pub(super) struct DrawCacheProfile {
    enabled: bool,
    topology: [u64; COUNT],
    data: [u64; COUNT],
    rebuild_causes: [u64; COUNT],
    pending: u64,
    calls: u64,
    rebuilds: u64,
    transparency_calls: u64,
}
impl DrawCacheProfile {
    pub(super) fn topology(&mut self, reason: VisualChange) {
        if self.enabled {
            self.topology[reason as usize] += 1;
            self.pending |= 1 << reason as usize;
        }
    }
    pub(super) fn data(&mut self, reason: VisualChange) {
        if self.enabled {
            self.data[reason as usize] += 1;
        }
    }
    pub(super) fn prepare(&mut self, rebuilt: bool) {
        if !self.enabled {
            return;
        }
        self.calls += 1;
        if rebuilt {
            self.rebuilds += 1;
            for (index, count) in self.rebuild_causes.iter_mut().enumerate() {
                if self.pending & (1 << index) != 0 {
                    *count += 1;
                }
            }
            self.pending = 0;
        }
    }
    pub(super) fn transparency(&mut self) {
        if self.enabled {
            self.transparency_calls += 1;
        }
    }
}
impl VisualWorld {
    pub(crate) fn enable_draw_cache_profile(&mut self) {
        self.draw_cache_profile.enabled = true;
    }
    pub(crate) fn reset_draw_cache_profile(&mut self) {
        let old = &self.draw_cache_profile;
        self.draw_cache_profile = DrawCacheProfile {
            enabled: old.enabled,
            pending: old.pending,
            ..Default::default()
        };
    }
    pub(crate) fn draw_cache_profile_snapshot(&self) -> BTreeMap<String, u64> {
        let p = &self.draw_cache_profile;
        let mut out = BTreeMap::from([
            ("prepare_calls".into(), p.calls),
            ("cache_hits".into(), p.calls - p.rebuilds),
            ("rebuilds".into(), p.rebuilds),
            ("transparency_prepare_calls".into(), p.transparency_calls),
        ]);
        for (prefix, values) in [
            ("topology_invalidations", &p.topology),
            ("instance_data_writes", &p.data),
            ("rebuild_causes", &p.rebuild_causes),
        ] {
            for (name, &count) in REASONS.iter().zip(values) {
                if count != 0 {
                    out.insert(format!("{prefix}.{name}"), count);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::graphics::{MaterialHandle, primitives::MeshHandle};

    fn instance(world: &mut VisualWorld) -> InstanceHandle {
        world.register(
            ComponentId::from(slotmap::KeyData::from_ffi(1)),
            GpuRenderable::new(MeshHandle::CUBE, MaterialHandle::TOON_MESH),
            Transform::default(),
            [1.; 4],
            1.,
            false,
            false,
            false,
            false,
            false,
            0.,
            None,
            3.,
        )
    }

    #[test]
    fn transform_updates_hit_cache_while_structure_changes_rebuild_once() {
        let mut world = VisualWorld::default();
        world.enable_draw_cache_profile();
        let handle = instance(&mut world);
        assert!(world.prepare_draw_cache());
        world.reset_draw_cache_profile();
        assert!(world.update_model(handle, Transform::default().model));
        assert!(!world.prepare_draw_cache());
        let counts = world.draw_cache_profile_snapshot();
        assert_eq!(counts["cache_hits"], 1);
        assert_eq!(counts["instance_data_writes.update_model"], 1);
        assert_eq!(counts["rebuilds"], 0);

        world.update_color(handle, [0.5; 4]);
        world.update_color(handle, [0.6; 4]);
        assert!(world.prepare_draw_cache());
        assert!(!world.prepare_draw_cache());
        let counts = world.draw_cache_profile_snapshot();
        assert_eq!(counts["topology_invalidations.update_color"], 2);
        assert_eq!(counts["rebuild_causes.update_color"], 1);
        assert_eq!(counts["rebuilds"], 1);
        assert_eq!(counts["cache_hits"], 2);
    }

    #[test]
    fn measurement_reset_preserves_pending_rebuild_cause() {
        let mut world = VisualWorld::default();
        world.enable_draw_cache_profile();
        let handle = instance(&mut world);
        world.prepare_draw_cache();
        world.update_texture(handle, Some(crate::engine::graphics::TextureHandle(73)));
        world.reset_draw_cache_profile();
        assert!(world.prepare_draw_cache());
        let counts = world.draw_cache_profile_snapshot();
        assert_eq!(counts["rebuild_causes.update_texture"], 1);
        assert!(!counts.contains_key("topology_invalidations.update_texture"));
    }
}
