//! Engine dispatch for world/component event bindings and native bypass gates.
use super::*;

impl MittensHost<'_, '_> {
    /// Dispatch one engine event through the configured Rx routes. Script
    /// handlers enqueue opaque callback invocations; they never re-enter MMS
    /// from inside Rx dispatch.
    pub fn dispatch_event_handlers(
        &mut self,
        signal: &crate::engine::ecs::Signal,
    ) -> Result<(), mms::HostError> {
        let Some(rx) = self.rx.as_deref_mut() else {
            return Err(mms::HostError::unavailable(
                "register_signal_handler",
                "the current Mittens host lease has no signal routing service",
            ));
        };
        rx.dispatch_event_handlers(self.world, signal);
        Ok(())
    }

    pub(super) fn record_signal_route(
        &mut self,
        signal: &str,
        scope: Option<mms::ComponentHandle>,
        name: Option<String>,
        callback: mms::CallbackHandle,
    ) -> Result<mms::HostResponse, mms::HostError> {
        let kind = signal_kind(signal).ok_or_else(|| {
            mms::HostError::failure(
                "register_signal_handler",
                format!("unknown signal '{signal}'"),
            )
        })?;
        let scope = scope
            .map(|scope| self.existing_id(scope, "register_signal_handler"))
            .transpose()?;
        #[cfg(test)]
        crate::engine::update_cpu_profile::register_callback(callback.into_raw(), || {
            format!("{} scope={:?} type={:?} name={:?}",signal,scope,scope.and_then(|id|self.world.component_name(id)),scope.and_then(|id|self.world.component_label(id)))
        });
        if self.signal_routes.is_none()
            && (self.rx.is_none() || self.callback_invocations.is_none())
        {
            return Err(mms::HostError::unavailable(
                "register_signal_handler",
                "the current Mittens host lease has no callback queue or Rx service",
            ));
        }
        if let Some(routes) = self.signal_routes.as_deref_mut() {
            routes.push(SignalCallbackRoute {
                kind,
                scope,
                name: name.clone(),
                callback,
            });
        }
        if let (Some(rx), Some(invocations)) =
            (self.rx.as_deref_mut(), self.callback_invocations.as_ref())
        {
            let invocations = Arc::clone(invocations);
            let enqueue = move |_world: &mut World,
                                _emit: &mut dyn SignalEmitter,
                                signal: &crate::engine::ecs::Signal| {
                match event_arg_transport(signal) {
                    Ok(argument) => invocations.lock().unwrap().push(mms::CallbackInvocation {
                        callback,
                        args: vec![argument],
                    }),
                    Err(error) => eprintln!("[mms] signal payload conversion error: {error}"),
                }
            };
            rx.add_event_binding(
                kind,
                scope,
                callback.into_raw(),
                name,
                self.callback_delivery_enabled.as_ref().map(Arc::clone),
                enqueue,
            );
        }
        Ok(mms::HostResponse::Unit)
    }
    pub(super) fn change_event_binding(
        &mut self,
        callback: mms::CallbackHandle,
        bypassed: Option<bool>,
    ) -> Result<mms::HostResponse, mms::HostError> {
        let Some(rx) = self.rx.as_deref_mut() else {
            return Err(mms::HostError::unavailable(
                "event_binding",
                "the current host lease has no live event routing service",
            ));
        };
        if let Some(bypassed) = bypassed {
            if !rx.bypass_event_binding(callback.into_raw(), bypassed) {
                return Err(mms::HostError {
                    kind: mms::HostErrorKind::StaleHandle,
                    operation: "bypass".into(),
                    message: "event binding or its source has been removed".into(),
                });
            }
        } else {
            rx.remove_event_binding(callback.into_raw());
            if let Some(routes) = self.signal_routes.as_deref_mut() {
                routes.retain(|route| route.callback != callback);
            }
        }
        if bypassed != Some(false) {
            if let Some(pending) = &self.callback_invocations {
                pending
                    .lock()
                    .unwrap()
                    .retain(|invocation| invocation.callback != callback);
            }
        }
        Ok(mms::HostResponse::Unit)
    }
}
