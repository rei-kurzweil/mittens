//! Live engine service leases for MMS host calls.
use super::*;

/// Engine implementation of the host-neutral Meow Meow host contract.
pub(crate) struct MittensHost<'a, 'bindings> {
    pub world: &'a mut World,
    pub rx: Option<&'a mut RxWorld>,
    pub render_assets: Option<&'a mut RenderAssets>,
    pub(super) graphics_settings:
        Option<&'a mut crate::engine::ecs::system::GraphicsSettingsSystem>,
    pub(super) xr_settings: Option<&'a mut crate::engine::ecs::system::XrSettingsSystem>,
    pub(super) benchmarks: Option<&'a mut crate::engine::benchmark::Benchmarks>,
    pub emit: &'a mut dyn SignalEmitter,
    pub intents: &'a mut Vec<IntentValue>,
    pub(super) bindings: Option<
        &'bindings mms::ImplementationBindings<crate::scripting::runtime_config::MittensBinding>,
    >,
    pub(super) signal_routes: Option<&'a mut Vec<SignalCallbackRoute>>,
    pub(super) callback_invocations: Option<Arc<Mutex<Vec<mms::CallbackInvocation>>>>,
    pub(super) callback_delivery_enabled: Option<Arc<AtomicBool>>,
    pub(super) deferred_callback_phase: Option<DeferredCallbackHostPhase>,
    pub(super) legacy_component_fallbacks: usize,
    pub(super) legacy_method_fallbacks: usize,
}

impl<'a, 'bindings> MittensHost<'a, 'bindings> {
    /// Assemble the standard live-engine lease used by RuntimeSpec work.
    /// Keeping this bundle here prevents session and application layers from
    /// learning which Mittens services each host operation borrows.
    pub(crate) fn runtime_lease(
        world: &'a mut World,
        rx: &'a mut RxWorld,
        render_assets: &'a mut RenderAssets,
        emit: &'a mut dyn SignalEmitter,
        intents: &'a mut Vec<IntentValue>,
    ) -> MittensHost<'a, 'a> {
        MittensHost::new(world, emit, intents)
            .with_rx(rx)
            .with_render_assets(render_assets)
    }

    pub fn new(
        world: &'a mut World,
        emit: &'a mut dyn SignalEmitter,
        intents: &'a mut Vec<IntentValue>,
    ) -> Self {
        Self {
            world,
            rx: None,
            render_assets: None,
            graphics_settings: None,
            xr_settings: None,
            benchmarks: None,
            emit,
            intents,
            bindings: None,
            signal_routes: None,
            callback_invocations: None,
            callback_delivery_enabled: None,
            deferred_callback_phase: None,
            legacy_component_fallbacks: 0,
            legacy_method_fallbacks: 0,
        }
    }

    pub fn with_rx(mut self, rx: &'a mut RxWorld) -> Self {
        self.rx = Some(rx);
        self
    }
    pub fn with_render_assets(mut self, assets: &'a mut RenderAssets) -> Self {
        self.render_assets = Some(assets);
        self
    }

    pub fn with_graphics_settings(
        mut self,
        settings: &'a mut crate::engine::ecs::system::GraphicsSettingsSystem,
    ) -> Self {
        self.graphics_settings = Some(settings);
        self
    }

    pub fn with_benchmarks(
        mut self,
        benchmarks: &'a mut crate::engine::benchmark::Benchmarks,
    ) -> Self {
        self.benchmarks = Some(benchmarks);
        self
    }

    pub fn with_xr_settings(
        mut self,
        settings: &'a mut crate::engine::ecs::system::XrSettingsSystem,
    ) -> Self {
        self.xr_settings = Some(settings);
        self
    }

    pub fn with_bindings<'next>(
        self,
        bindings: &'next mms::ImplementationBindings<
            crate::scripting::runtime_config::MittensBinding,
        >,
    ) -> MittensHost<'a, 'next> {
        MittensHost {
            world: self.world,
            rx: self.rx,
            render_assets: self.render_assets,
            graphics_settings: self.graphics_settings,
            xr_settings: self.xr_settings,
            benchmarks: self.benchmarks,
            emit: self.emit,
            intents: self.intents,
            bindings: Some(bindings),
            signal_routes: self.signal_routes,
            callback_invocations: self.callback_invocations,
            callback_delivery_enabled: self.callback_delivery_enabled,
            deferred_callback_phase: self.deferred_callback_phase,
            legacy_component_fallbacks: self.legacy_component_fallbacks,
            legacy_method_fallbacks: self.legacy_method_fallbacks,
        }
    }

    pub fn with_signal_routes(mut self, routes: &'a mut Vec<SignalCallbackRoute>) -> Self {
        self.signal_routes = Some(routes);
        self
    }

    pub fn signal_routes(&self) -> Option<&[SignalCallbackRoute]> {
        self.signal_routes.as_deref().map(Vec::as_slice)
    }

    pub fn with_callback_invocations(
        mut self,
        invocations: Arc<Mutex<Vec<mms::CallbackInvocation>>>,
    ) -> Self {
        self.callback_invocations = Some(invocations);
        self
    }

    /// Gate callback delivery for the lifetime of the session that owns the
    /// queued invocations. Closing that session disables its Rx routes even if
    /// the engine has not yet removed their host-side registrations.
    pub fn with_callback_delivery_enabled(mut self, enabled: Arc<AtomicBool>) -> Self {
        self.callback_delivery_enabled = Some(enabled);
        self
    }

    pub fn with_deferred_callback_phase(mut self, phase: DeferredCallbackHostPhase) -> Self {
        self.deferred_callback_phase = Some(phase);
        self
    }
}
