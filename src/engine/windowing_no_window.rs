//! Scene-selected XR-only or simulation-only host, without a desktop surface.
use super::{EngineError, EngineResult, InputHook};
use crate::engine::{Universe, ecs::component::XrComponent};
use std::time::{Duration, Instant};

pub(super) fn run(
    mut universe: Universe,
    mut frame_hook: Option<Box<dyn FnMut(&mut Universe)>>,
    mut input_hook: Option<InputHook>,
) -> EngineResult<()> {
    let xr_enabled = universe
        .world
        .all_components()
        .filter_map(|id| universe.world.get_component_by_id_as::<XrComponent>(id))
        .last()
        .is_some_and(|xr| xr.enabled);
    if xr_enabled {
        universe.init_renderer_for_xr().map_err(|error| {
            EngineError::Windowing(format!("XR-only renderer init failed: {error}"))
        })?;
    }
    let mut previous = Instant::now();
    loop {
        let now = Instant::now();
        let dt = now.saturating_duration_since(previous).as_secs_f32();
        previous = now;
        let mut input = universe.input_providers.poll();
        if let Some(hook) = input_hook.as_mut() {
            hook(&mut universe, &mut input);
        }
        universe.update(dt, &input);
        if let Some(hook) = frame_hook.as_mut() {
            hook(&mut universe);
        }
        if xr_enabled {
            universe.render();
            if universe.systems.xr.render_failed() {
                return Err(EngineError::Windowing("OpenXR rendering failed".into()));
            }
        }
        if universe.exit_requested() || universe.systems.xr.runtime_exit_requested() {
            return Ok(());
        }
        if !xr_enabled || !universe.systems.xr.session_running() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
