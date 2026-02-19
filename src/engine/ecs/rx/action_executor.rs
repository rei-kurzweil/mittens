use crate::engine::ecs::component::Action;
use crate::engine::ecs::system::ActionSystem;
use crate::engine::ecs::{CommandQueue, World};

use super::RxWorld;

#[derive(Debug, Default)]
pub struct ActionExecutor;

impl ActionExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn execute(
        &mut self,
        world: &mut World,
        queue: &mut CommandQueue,
        rx: &mut RxWorld,
        beat_now: f64,
        action: &Action,
    ) {
        let mut system = ActionSystem::default();
        system.execute_impl(world, queue, rx, beat_now, action);
    }
}
