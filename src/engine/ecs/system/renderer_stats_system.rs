use crate::engine::ecs::CommandQueue;
use crate::engine::ecs::ComponentId;
use crate::engine::ecs::SignalEmitter;
use crate::engine::ecs::World;
use crate::engine::ecs::component::{
    ColorComponent, EmissiveComponent, RendererStatsComponent, TextComponent,
};
use crate::engine::graphics::{CameraTarget, VisualWorld};

#[derive(Debug, Default)]
pub struct RendererStatsSystem;

impl RendererStatsSystem {
    pub fn tick_with_queue(
        &mut self,
        world: &mut World,
        visuals: &mut VisualWorld,
        queue: &mut CommandQueue,
        dt_sec: f32,
    ) {
        let stats_ids: Vec<ComponentId> = world
            .all_components()
            .filter(|&cid| {
                world
                    .get_component_by_id_as::<RendererStatsComponent>(cid)
                    .is_some()
            })
            .collect();

        for stats_id in stats_ids {
            let (should_update, config, subtree_ids) = {
                let Some(stats) =
                    world.get_component_by_id_as_mut::<RendererStatsComponent>(stats_id)
                else {
                    continue;
                };
                if !stats.enabled {
                    continue;
                }
                stats.accumulate_time(dt_sec);
                (
                    stats.should_update(),
                    StatsConfig::from_component(stats),
                    StatsSubtreeIds::from_component(stats),
                )
            };
            if !should_update {
                continue;
            }

            let (fps, dt_ms, label) = match config.target {
                CameraTarget::Window => {
                    let dt = visuals.window_frame_dt_sec();
                    (visuals.window_frame_fps(), dt * 1000.0, "Window")
                }
                CameraTarget::Xr => {
                    let dt = visuals.xr_frame_dt_sec().unwrap_or(0.0);
                    let fps = visuals.xr_frame_fps().unwrap_or(0.0);
                    (fps, dt * 1000.0, "XR")
                }
            };

            let smoothed = {
                let Some(stats) =
                    world.get_component_by_id_as_mut::<RendererStatsComponent>(stats_id)
                else {
                    continue;
                };
                stats.smooth_fps(fps)
            };

            let new_text = if fps > 0.0 && dt_ms > 0.0 {
                format!("{label}: {smoothed:.1} fps  ({dt_ms:.1} ms)")
            } else {
                format!("{label}: (no timing)")
            };

            let (text_id, updated_ids) =
                ensure_stats_text_subtree(world, queue, stats_id, config, subtree_ids);

            {
                if let Some(stats) =
                    world.get_component_by_id_as_mut::<RendererStatsComponent>(stats_id)
                {
                    updated_ids.write_back(stats);
                    stats.reset_update_timer();
                }
            }

            let Some(text_id) = text_id else {
                continue;
            };

            // Avoid spamming SetText if it wouldn't change.
            let needs_update = world
                .get_component_by_id_as::<TextComponent>(text_id)
                .map(|t| t.text != new_text)
                .unwrap_or(true);

            if needs_update {
                queue.push_intent_now(
                    stats_id,
                    crate::engine::ecs::IntentValue::SetText {
                        component_ids: vec![text_id],
                        text: new_text,
                    },
                );
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StatsConfig {
    target: CameraTarget,
    color: [f32; 4],
    emissive: bool,
}

impl StatsConfig {
    fn from_component(c: &RendererStatsComponent) -> Self {
        Self {
            target: c.target,
            color: c.color,
            emissive: c.emissive,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct StatsSubtreeIds {
    text: Option<ComponentId>,
    text_color: Option<ComponentId>,
    text_emissive: Option<ComponentId>,
}

impl StatsSubtreeIds {
    fn from_component(c: &mut RendererStatsComponent) -> Self {
        let (t, c_id, e_id) = c.runtime_subtree_ids_mut();
        Self {
            text: *t,
            text_color: *c_id,
            text_emissive: *e_id,
        }
    }

    fn write_back(self, c: &mut RendererStatsComponent) {
        let (t, c_id, e_id) = c.runtime_subtree_ids_mut();
        *t = self.text;
        *c_id = self.text_color;
        *e_id = self.text_emissive;
    }
}

// Camera target selection is explicit via RendererStatsComponent.target.

fn ensure_stats_text_subtree(
    world: &mut World,
    queue: &mut CommandQueue,
    stats_id: ComponentId,
    config: StatsConfig,
    mut ids: StatsSubtreeIds,
) -> (Option<ComponentId>, StatsSubtreeIds) {
    // Text.
    let text_id = match ids.text {
        Some(cid) if world.get_component_by_id_as::<TextComponent>(cid).is_some() => cid,
        _ => {
            let cid = world.add_component(TextComponent::new(""));
            let _ = world.add_child(stats_id, cid);
            ids.text = Some(cid);
            cid
        }
    };

    // Styling: immediate children of TextComponent.
    let has_color = match ids.text_color {
        Some(cid) => world
            .get_component_by_id_as::<ColorComponent>(cid)
            .is_some(),
        None => false,
    };
    if !has_color {
        let cid = world.add_component(ColorComponent { rgba: config.color });
        let _ = world.add_child(text_id, cid);
        ids.text_color = Some(cid);
    }
    if let Some(cid) = ids.text_color {
        if let Some(c) = world.get_component_by_id_as_mut::<ColorComponent>(cid) {
            c.rgba = config.color;
        }
    }

    let has_emissive = match ids.text_emissive {
        Some(cid) => world
            .get_component_by_id_as::<EmissiveComponent>(cid)
            .is_some(),
        None => false,
    };
    if !has_emissive {
        let cid = world.add_component(EmissiveComponent::new(if config.emissive {
            1.0
        } else {
            0.0
        }));
        let _ = world.add_child(text_id, cid);
        ids.text_emissive = Some(cid);
    }
    if let Some(cid) = ids.text_emissive {
        if let Some(e) = world.get_component_by_id_as_mut::<EmissiveComponent>(cid) {
            e.intensity = if config.emissive { 1.0 } else { 0.0 };
        }
    }

    // Initialize the whole subtree so the renderer/text system sees it.
    world.init_component_tree(text_id, queue);

    (Some(text_id), ids)
}
