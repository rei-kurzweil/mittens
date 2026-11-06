//! Retained callbacks and deferred event-binding cancellation.
use super::*;

impl<H: Host> Session<H> {
    pub fn invoke_callback(
        &mut self,
        handle: CallbackHandle,
        args: Vec<Value>,
    ) -> Result<Value, EvalError> {
        self.prune_component_fields()?;
        if !self.context.owns_callback(handle) {
            return Err(crate::HostError {
                kind: crate::HostErrorKind::ForeignHandle,
                operation: "invoke_callback".into(),
                message: format!("callback handle {handle:?} belongs to another session"),
            }
            .into());
        }
        let callback = self
            .callbacks
            .get(&handle)
            .cloned()
            .ok_or_else(|| crate::HostError {
                kind: crate::HostErrorKind::StaleHandle,
                operation: "invoke_callback".into(),
                message: format!(
                    "callback handle {handle:?} is no longer retained by this session"
                ),
            })?;
        let scopes = std::mem::take(&mut self.scopes);
        let callbacks = std::mem::take(&mut self.callbacks);
        let module_cache = std::mem::take(&mut self.module_cache);
        let mut evaluator = Evaluator::for_session(
            &mut self.host,
            scopes,
            self.heap.clone(),
            callbacks,
            self.runtime.catalog.clone(),
            &mut self.context,
            None,
            module_cache,
        );
        let result = evaluator.invoke_value(callback, args);
        let (scopes, callbacks, module_cache) = evaluator.into_session_state();
        self.scopes = scopes;
        self.callbacks = callbacks;
        self.module_cache = module_cache;
        result
    }
    pub fn invoke_callback_invocation(
        &mut self,
        invocation: crate::CallbackInvocation,
    ) -> Result<Value, EvalError> {
        if !self.context.owns_callback(invocation.callback) {
            return Err(crate::HostError {
                kind: crate::HostErrorKind::ForeignHandle,
                operation: "invoke_callback".into(),
                message: format!(
                    "callback handle {:?} belongs to another session",
                    invocation.callback
                ),
            }
            .into());
        }
        // Removal or bypass can happen earlier in the same deferred batch.
        // Foreign identities above remain errors; cancelled local work is skipped.
        if !self.callbacks.contains_key(&invocation.callback)
            || self
                .context
                .event_bindings
                .get(&invocation.callback)
                .is_some_and(|binding| binding.bypassed)
        {
            return Ok(Value::Null);
        }
        for argument in &invocation.args {
            adopt_transport_components(&mut self.context, argument);
        }
        let args = invocation
            .args
            .into_iter()
            .map(crate::evaluator::transport_to_value)
            .collect::<Result<Vec<_>, _>>()?;
        self.invoke_callback(invocation.callback, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_binding_removal_releases_callbacks_without_retaining_tombstones() {
        let mut builder = RuntimeSpec::builder::<()>();
        builder
            .with_standard_builtins()
            .signal("FrameTick", Vec::new(), ());
        let configured = builder.build().unwrap();
        let mut session = configured.runtime().session(crate::EventStreamHost::new());
        for _ in 0..512 {
            session
                .eval("let binding=on(world, \"FrameTick\", fn(event) {}); binding.remove()")
                .unwrap();
            assert!(session.callbacks.is_empty());
            assert!(session.context.event_bindings.is_empty());
        }
        assert!(session.callbacks.capacity() < 16);
        assert!(session.context.event_bindings.capacity() < 16);
    }
}
