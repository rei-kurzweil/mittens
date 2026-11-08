use super::*;

impl RuntimeSpecSession {
    /// Service queued signal callbacks using one live host lease. Returned
    /// strings are diagnostics only; effects were written to the lease's
    /// intent sink.
    pub(crate) fn service_callbacks_with_host(
        &mut self,
        host: crate::scripting::host::MittensHost<'_, '_>,
    ) -> Vec<String> {
        #[cfg(test)]
        let mut cpu_profile = crate::engine::update_cpu_profile::Timer::new();
        if self.is_closed() {
            return vec!["Mittens RuntimeSpec session is closed".into()];
        }
        let invocations = {
            let mut queued = self.callback_invocations.lock().unwrap();
            std::mem::take(&mut *queued)
        };
        #[cfg(test)]
        crate::engine::update_cpu_profile::callbacks(invocations.len());
        if invocations.is_empty() {
            return Vec::new();
        }

        let idle = self.session.take().expect("checked above");
        let host = host
            .with_bindings(self.configured.bindings())
            .with_callback_invocations(Arc::clone(&self.callback_invocations))
            .with_callback_delivery_enabled(Arc::clone(&self.callback_delivery_enabled));
        let (idle, errors) = idle.with_host(host, |session| {
            #[cfg(test)]
            cpu_profile.mark(crate::engine::update_cpu_profile::Phase::CallbackLeaseEnter);
            let errors = invocations
                .into_iter()
                .filter_map(|invocation| {
                    #[cfg(test)]
                    let _callback_scope = crate::engine::update_cpu_profile::callback_scope(
                        invocation.callback.into_raw(),
                        &invocation.args,
                    );
                    session.invoke_callback_invocation(invocation).err()
                })
                .map(|error| error.to_string())
                .collect();
            #[cfg(test)]
            cpu_profile.mark(crate::engine::update_cpu_profile::Phase::CallbackInvoke);
            errors
        });
        #[cfg(test)]
        cpu_profile.mark(crate::engine::update_cpu_profile::Phase::CallbackLeaseExit);
        self.session = Some(idle);
        errors
    }
}
