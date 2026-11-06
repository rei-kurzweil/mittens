// Diagnostic mutation counts compile out of non-test builds.
macro_rules! visual_topology_dirty {
    ($world:expr, $reason:ident) => {
        $world.dirty_draw_cache = true;
        #[cfg(test)]
        $world.draw_cache_profile.topology(VisualChange::$reason);
    };
}
macro_rules! visual_data_dirty {
    ($world:expr, $reason:ident) => {
        $world.dirty_instance_data = true;
        #[cfg(test)]
        $world.draw_cache_profile.data(VisualChange::$reason);
    };
}
