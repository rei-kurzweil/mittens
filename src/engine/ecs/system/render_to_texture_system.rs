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
    }
}

fn producer_kind_for_selector(selector: &str) -> RenderTextureProducerKind {
    if selector.starts_with("capture.mirror.") {
        RenderTextureProducerKind::Mirror
    } else {
        RenderTextureProducerKind::InternalRendererImage
    }
}
