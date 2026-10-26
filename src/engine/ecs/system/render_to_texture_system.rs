use crate::engine::ecs::component::TextureComponent;
use crate::engine::ecs::{ComponentId, World};
use crate::engine::graphics::{TextureUploader, VisualWorld};
use std::collections::{HashMap, HashSet};

pub const INTERNAL_RENDERER_STENCIL_CLIP_DEBUG_SELECTOR: &str = "render_graph.stencil_clip.debug";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderTextureConsumerKind {
    TextureRenderImage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTextureConsumerRegistration {
    pub component: ComponentId,
    pub selector: String,
    pub kind: RenderTextureConsumerKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderTextureProducerKind {
    InternalRendererImage,
    SceneCapture,
    CubeCapture,
    Mirror,
    Portal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTextureProducerRequest {
    pub selector: String,
    pub kind: RenderTextureProducerKind,
}

#[derive(Debug, Default)]
pub struct RenderToTextureSystem {
    consumers_by_component: HashMap<ComponentId, RenderTextureConsumerRegistration>,
    producer_requests_by_selector: HashMap<String, RenderTextureProducerRequest>,
}

impl RenderToTextureSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_texture(&mut self, world: &mut World, component: ComponentId) {
        let Some(texture) = world.get_component_by_id_as::<TextureComponent>(component) else {
            self.unregister_texture(component);
            return;
        };

        if let Some(selector) = texture.render_image.clone() {
            let previous = self.consumers_by_component.insert(
                component,
                RenderTextureConsumerRegistration {
                    component,
                    selector: selector.clone(),
                    kind: RenderTextureConsumerKind::TextureRenderImage,
                },
            );
            if let Some(previous) = previous {
                self.remove_selector_if_unused(&previous.selector);
            }

            self.producer_requests_by_selector.insert(
                selector.clone(),
                RenderTextureProducerRequest {
                    selector,
                    kind: producer_kind_for_selector(texture.render_image.as_deref().unwrap()),
                },
            );
        } else {
            self.unregister_texture(component);
        }
    }

    pub(crate) fn reconcile_consumers(&mut self, world: &World) {
        let stale: Vec<_> = self
            .consumers_by_component
            .iter()
            .filter_map(|(&id, registration)| {
                (world
                    .get_component_by_id_as::<TextureComponent>(id)
                    .and_then(|texture| texture.render_image.as_deref())
                    != Some(registration.selector.as_str()))
                .then_some(id)
            })
            .collect();
        for id in stale {
            self.unregister_texture(id);
        }
    }

    fn unregister_texture(&mut self, component: ComponentId) {
        if let Some(previous) = self.consumers_by_component.remove(&component) {
            self.remove_selector_if_unused(&previous.selector);
        }
    }

    fn remove_selector_if_unused(&mut self, selector: &str) {
        let still_used = self
            .consumers_by_component
            .values()
            .any(|registration| registration.selector == selector);
        if !still_used {
            self.producer_requests_by_selector.remove(selector);
        }
    }

    pub fn consumer_registrations(
        &self,
    ) -> impl Iterator<Item = &RenderTextureConsumerRegistration> {
        self.consumers_by_component.values()
    }

    pub fn producer_requests(&self) -> impl Iterator<Item = &RenderTextureProducerRequest> {
        self.producer_requests_by_selector.values()
    }

    pub fn flush_pending(&mut self, visuals: &mut VisualWorld, uploader: &mut dyn TextureUploader) {
        visuals.set_stencil_clip_debug_requested(
            self.producer_requests_by_selector
                .contains_key(INTERNAL_RENDERER_STENCIL_CLIP_DEBUG_SELECTOR),
        );

        let selectors: HashSet<String> = self
            .producer_requests_by_selector
            .values()
            .map(|request| request.selector.clone())
            .collect();
        let mirror_capture_selectors: HashSet<String> = visuals
            .mirrors()
            .iter()
            .flat_map(|mirror| {
                mirror
                    .captures
                    .iter()
                    .map(|capture| capture.target_key.clone())
            })
            .collect();

        // Keep frozen captures through pause/tracking loss, but release images
        // when neither an authored consumer nor a live mirror producer owns them.
        let prefixes: Vec<_> = visuals
            .mirrors()
            .iter()
            .map(|mirror| mirror.capture_prefix.clone())
            .collect();
        visuals.retain_runtime_textures(|key| {
            selectors.contains(key) || prefixes.iter().any(|prefix| key.starts_with(prefix))
        });

        for selector in selectors {
            if visuals.runtime_texture_handle(&selector).is_some() {
                continue;
            }

            match uploader.upload_texture_rgba8(&[0, 0, 0, 255], 1, 1) {
                Ok(handle) => visuals.set_runtime_texture_handle(selector, handle),
                Err(err) => {
                    println!(
                        "[RenderToTextureSystem] failed to allocate runtime texture handle: {err}"
                    );
                }
            }
        }

        for selector in mirror_capture_selectors {
            if visuals.runtime_texture_handle(&selector).is_some() {
                continue;
            }

            match uploader.upload_texture_rgba8(&[0, 0, 0, 255], 1, 1) {
                Ok(handle) => visuals.set_runtime_texture_handle(selector, handle),
                Err(err) => {
                    println!(
                        "[RenderToTextureSystem] failed to allocate mirror runtime texture handle: {err}"
                    );
                }
            }
        }
    }
}

fn producer_kind_for_selector(selector: &str) -> RenderTextureProducerKind {
    if selector.starts_with("capture.mirror.") {
        RenderTextureProducerKind::Mirror
    } else {
        RenderTextureProducerKind::InternalRendererImage
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::ecs::component::TransformComponent;
    use crate::engine::graphics::primitives::InstanceHandle;
    use crate::engine::graphics::visual_world::{
        MirrorCaptureRequest, MirrorViewerFamily, VisualMirror,
    };
    use crate::engine::graphics::{CameraData, TextureHandle};

    #[derive(Default)]
    struct Uploader(u32);
    impl TextureUploader for Uploader {
        fn upload_texture_rgba8(
            &mut self,
            _: &[u8],
            _: u32,
            _: u32,
        ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
            self.0 += 1;
            Ok(TextureHandle(self.0))
        }
        fn upload_texture_bc7(
            &mut self,
            _: &[u8],
            _: u32,
            _: u32,
            _: bool,
        ) -> Result<TextureHandle, Box<dyn std::error::Error>> {
            unreachable!()
        }
    }

    #[test]
    fn runtime_capture_consumer_retirement_waits_for_the_last_live_reference() {
        let mut world = World::default();
        let a = world.add_component(TextureComponent::render_image("render_graph.test"));
        let b = world.add_component(TextureComponent::render_image("render_graph.test"));
        let mut system = RenderToTextureSystem::default();
        let mut visuals = VisualWorld::default();
        let mut uploader = Uploader::default();
        system.register_texture(&mut world, a);
        system.register_texture(&mut world, b);
        system.flush_pending(&mut visuals, &mut uploader);
        assert_eq!(uploader.0, 1);
        let _ = world.remove_component_subtree(a);
        system.reconcile_consumers(&world);
        system.flush_pending(&mut visuals, &mut uploader);
        assert_eq!(
            visuals.runtime_texture_handle("render_graph.test"),
            Some(TextureHandle(1))
        );
        assert!(visuals.take_retired_runtime_textures().is_empty());
        let _ = world.remove_component_subtree(b);
        system.reconcile_consumers(&world);
        system.flush_pending(&mut visuals, &mut uploader);
        assert!(
            visuals
                .runtime_texture_handle("render_graph.test")
                .is_none()
        );
        assert_eq!(
            visuals.take_retired_runtime_textures(),
            vec![TextureHandle(1)]
        );
        assert_eq!(system.producer_requests().count(), 0);
    }

    #[test]
    fn runtime_capture_images_survive_tracking_loss_and_retire_with_the_producer() {
        let mut world = World::default();
        let owner = world.add_component(TransformComponent::new());
        let mut visuals = VisualWorld::default();
        let mut system = RenderToTextureSystem::default();
        let mut uploader = Uploader::default();
        let mut mirror = VisualMirror {
            mirror_component: owner,
            capture_prefix: "capture.mirror.test.".into(),
            captures: vec![MirrorCaptureRequest {
                family: MirrorViewerFamily::Stereoscopic,
                view_index: 0,
                camera: CameraData {
                    view: crate::utils::math::mat4_identity(),
                    proj: crate::utils::math::mat4_identity(),
                    transform: Default::default(),
                },
                target_key: "capture.mirror.test.stereo.0.color".into(),
            }],
            plane_origin: [0.0; 3],
            plane_normal: [0.0, 0.0, 1.0],
            aspect_ratio: 1.0,
            source_instance: InstanceHandle(1),
            resolution_scale: 0.25,
        };
        visuals.register_mirror(mirror.clone());
        system.flush_pending(&mut visuals, &mut uploader);
        mirror.captures.clear();
        visuals.clear_mirrors();
        visuals.register_mirror(mirror);
        visuals.set_mirror_captures_enabled(false);
        system.flush_pending(&mut visuals, &mut uploader);
        assert_eq!(uploader.0, 1);
        assert_eq!(visuals.runtime_texture_handles().count(), 1);
        assert!(visuals.take_retired_runtime_textures().is_empty());
        visuals.clear_mirrors();
        system.flush_pending(&mut visuals, &mut uploader);
        assert_eq!(visuals.runtime_texture_handles().count(), 0);
        assert_eq!(
            visuals.take_retired_runtime_textures(),
            vec![TextureHandle(1)]
        );
    }
}
