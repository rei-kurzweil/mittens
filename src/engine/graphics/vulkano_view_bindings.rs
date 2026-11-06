// Per-view bindings retain scene draw topology and authored texture identities.
#[derive(Default)]
struct ViewMirrorBindings {
    textures: HashMap<ComponentId, TextureHandle>,
}

impl ViewMirrorBindings {
    fn prepare(&mut self, visuals: &VisualWorld, kind: &RenderViewKind) {
        use crate::engine::graphics::visual_world::MirrorViewerFamily;
        let (family, view_index) = match *kind {
            RenderViewKind::Window => (MirrorViewerFamily::Monoscopic, 0),
            RenderViewKind::XrEye { eye } => (MirrorViewerFamily::Stereoscopic, eye),
            RenderViewKind::Mirror {
                family, view_index, ..
            } => (family, view_index),
        };
        self.textures.clear();
        for mirror in visuals.mirrors() {
            let Some(source) = visuals.instance(mirror.source_instance) else {
                continue;
            };
            let Some(capture) = mirror
                .captures
                .iter()
                .find(|capture| capture.family == family && capture.view_index == view_index)
            else {
                continue;
            };
            if let Some(handle) = visuals.runtime_texture_handle(&capture.target_key) {
                self.textures.insert(source.component, handle);
            }
        }
    }

    #[inline]
    fn texture(
        &self,
        source: Option<ComponentId>,
        authored: Option<TextureHandle>,
        fallback: TextureHandle,
    ) -> TextureHandle {
        source
            .and_then(|source| self.textures.get(&source).copied())
            .or(authored)
            .unwrap_or(fallback)
    }
}

impl VulkanoState {
    fn build_stencil_clip_debug_batches(
        visual_world: &VisualWorld,
    ) -> Vec<crate::engine::graphics::visual_world::DrawBatch> {
        visual_world
            .stencil_clip_order()
            .iter()
            .enumerate()
            .filter_map(|(slot, &instance_index)| {
                let instance = visual_world.instances().get(instance_index as usize)?;
                Some(crate::engine::graphics::visual_world::DrawBatch {
                    material: crate::engine::graphics::MaterialHandle::UNLIT_MESH,
                    mesh: instance.renderable.mesh,
                    texture: None,
                    view_texture_source: None,
                    texture_filtering: TextureFiltering::Nearest,
                    quant_steps: 1.0,
                    material_instance: None,
                    stencil_ref: 0,
                    start: slot,
                    count: 1,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod view_binding_tests {
    use super::*;
    use crate::engine::graphics::{
        CameraData, GpuRenderable, MaterialHandle, MeshHandle,
        visual_world::{MirrorCaptureRequest, MirrorViewerFamily, VisualMirror},
    };
    fn cid(id: u64) -> ComponentId {
        slotmap::KeyData::from_ffi(id).into()
    }
    #[test]
    fn per_view_reflections_do_not_mutate_authored_texture_or_draw_cache() {
        let mut visuals = VisualWorld::new();
        let source = visuals.register(
            cid(1),
            GpuRenderable::new(MeshHandle::SQUARE, MaterialHandle::MIRROR),
            crate::engine::ecs::Transform::default(),
            [1.; 4],
            1.,
            false,
            false,
            false,
            false,
            false,
            0.,
            Some(TextureHandle(50)),
            3.,
        );
        let camera = CameraData {
            view: crate::engine::ecs::Transform::default().model,
            proj: crate::engine::ecs::Transform::default().model,
            transform: Default::default(),
        };
        let captures = [
            (MirrorViewerFamily::Monoscopic, 0, "mono", 60),
            (MirrorViewerFamily::Stereoscopic, 0, "left", 61),
            (MirrorViewerFamily::Stereoscopic, 1, "right", 62),
        ];
        for (_, _, key, texture) in captures {
            visuals.set_runtime_texture_handle(key, TextureHandle(texture));
        }
        visuals.register_mirror(VisualMirror {
            mirror_component: cid(2),
            capture_prefix: "test".into(),
            captures: captures
                .iter()
                .map(|&(family, view_index, key, _)| MirrorCaptureRequest {
                    family,
                    view_index,
                    target_key: key.into(),
                    camera,
                    enabled: false,
                })
                .collect(),
            plane_origin: [0.; 3],
            plane_normal: [0., 0., 1.],
            aspect_ratio: 1.,
            source_instance: source,
            resolution_scale: 1.,
        });
        visuals.prepare_draw_cache();
        let mut bindings = ViewMirrorBindings::default();
        for (kind, expected) in [
            (RenderViewKind::XrEye { eye: 0 }, 61),
            (RenderViewKind::XrEye { eye: 1 }, 62),
            (RenderViewKind::Window, 60),
        ] {
            bindings.prepare(&visuals, &kind);
            assert_eq!(
                bindings.texture(Some(cid(1)), Some(TextureHandle(50)), TextureHandle(0)),
                TextureHandle(expected)
            );
            // Sharing an authored image does not give an unrelated surface a mirror override.
            assert_eq!(
                bindings.texture(Some(cid(3)), Some(TextureHandle(50)), TextureHandle(0)),
                TextureHandle(50)
            );
            assert_eq!(
                visuals.instance(source).unwrap().texture,
                Some(TextureHandle(50))
            );
            assert!(!visuals.prepare_draw_cache());
        }
        visuals.set_runtime_texture_handle("left", TextureHandle(77));
        bindings.prepare(&visuals, &RenderViewKind::XrEye { eye: 0 });
        assert_eq!(
            bindings.texture(Some(cid(1)), None, TextureHandle(0)),
            TextureHandle(77)
        );
        visuals.clear_mirrors();
        bindings.prepare(&visuals, &RenderViewKind::XrEye { eye: 0 });
        assert_eq!(
            bindings.texture(Some(cid(1)), Some(TextureHandle(50)), TextureHandle(0)),
            TextureHandle(50)
        );
        assert_eq!(
            bindings.texture(Some(cid(1)), None, TextureHandle(0)),
            TextureHandle(0)
        );
    }
}
