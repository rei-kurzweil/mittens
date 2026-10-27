//! Crate-owned host for standalone MMS evaluation.

use crate::{
    ComponentHandle, Host, HostContext, HostError, HostErrorKind, HostRequest, HostResponse,
    MaterializedCE,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CollectedComponent {
    pub handle: ComponentHandle,
    pub tree: MaterializedCE,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalAttachment {
    pub parent: Option<ComponentHandle>,
    pub child: ComponentHandle,
}

/// Standalone host that collects component output without emulating engine
/// operations.
#[derive(Debug, Default)]
pub struct StandardHost {
    roots: Vec<CollectedComponent>,
    registered: Vec<CollectedComponent>,
    attachments: Vec<LocalAttachment>,
}

impl StandardHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// Trees submitted through `Emit`, before registered-handle references
    /// are resolved.
    pub fn roots(&self) -> &[CollectedComponent] {
        &self.roots
    }

    /// All trees assigned a local handle through `Register`.
    pub fn registered(&self) -> &[CollectedComponent] {
        &self.registered
    }

    /// Authoritative ordered attachment edges, retained for inspection.
    pub fn attachments(&self) -> &[LocalAttachment] {
        &self.attachments
    }

    pub fn component(&self, handle: ComponentHandle) -> Option<&MaterializedCE> {
        self.collected(handle).map(|component| &component.tree)
    }

    /// Build standalone component trees with registered handles resolved into
    /// ordinary nested children. Detached registered components are omitted.
    pub fn resolved_roots(&self) -> Result<Vec<CollectedComponent>, HostError> {
        let mut root_handles = self
            .roots
            .iter()
            .map(|root| root.handle)
            .collect::<Vec<_>>();
        for attachment in &self.attachments {
            if attachment.parent.is_none() && !root_handles.contains(&attachment.child) {
                root_handles.push(attachment.child);
            }
        }

        root_handles
            .into_iter()
            .map(|handle| self.resolve_component(handle, &mut Vec::new()))
            .collect()
    }

    fn collected(&self, handle: ComponentHandle) -> Option<&CollectedComponent> {
        self.roots
            .iter()
            .chain(&self.registered)
            .find(|component| component.handle == handle)
    }

    fn resolve_component(
        &self,
        handle: ComponentHandle,
        path: &mut Vec<ComponentHandle>,
    ) -> Result<CollectedComponent, HostError> {
        if path.contains(&handle) {
            return Err(invalid_graph(format!(
                "component attachment cycle contains {handle:?}"
            )));
        }
        let component = self.collected(handle).ok_or_else(|| {
            invalid_graph(format!("component handle {handle:?} has no collected tree"))
        })?;

        path.push(handle);
        let mut tree = component.tree.clone();
        self.resolve_tree(&mut tree, path)?;
        for attachment in self
            .attachments
            .iter()
            .filter(|attachment| attachment.parent == Some(handle))
        {
            let child = self.resolve_component(attachment.child, path)?;
            tree.children.push(crate::CeChild::Spawn(child.tree));
        }
        path.pop();

        Ok(CollectedComponent { handle, tree })
    }

    fn resolve_tree(
        &self,
        tree: &mut MaterializedCE,
        path: &mut Vec<ComponentHandle>,
    ) -> Result<(), HostError> {
        for child in &mut tree.children {
            match child {
                crate::CeChild::Spawn(tree) => self.resolve_tree(tree, path)?,
                crate::CeChild::Attach(handle) => {
                    let component = self.resolve_component(*handle, path)?;
                    *child = crate::CeChild::Spawn(component.tree);
                }
            }
        }
        Ok(())
    }

    fn is_attached(&self, handle: ComponentHandle) -> bool {
        self.roots.iter().any(|root| root.handle == handle)
            || self
                .attachments
                .iter()
                .any(|attachment| attachment.child == handle)
    }

    fn would_create_cycle(&self, parent: ComponentHandle, child: ComponentHandle) -> bool {
        let mut cursor = Some(parent);
        while let Some(handle) = cursor {
            if handle == child {
                return true;
            }
            cursor = self
                .attachments
                .iter()
                .find(|attachment| attachment.child == handle)
                .and_then(|attachment| attachment.parent);
        }
        false
    }
}

impl Host for StandardHost {
    fn dispatch_with_context(
        &mut self,
        context: &mut HostContext,
        request: HostRequest,
    ) -> Result<HostResponse, HostError> {
        match request {
            HostRequest::ValidateComponents { components } => {
                let mut live = Vec::new();
                for handle in components {
                    require_local_handle(context, handle, "validate_components")?;
                    if let Some(tree) = self.component(handle) {
                        live.push((handle, tree.component_type.clone()));
                    }
                }
                Ok(HostResponse::Components(live))
            }
            HostRequest::Emit { tree } | HostRequest::Spawn { tree } => {
                let handle = context.allocate_component();
                let component_type = tree.component_type.clone();
                self.roots.push(CollectedComponent { handle, tree });
                Ok(HostResponse::Component {
                    handle,
                    component_type,
                })
            }
            HostRequest::RegisterComponent { tree } | HostRequest::Register { tree } => {
                let handle = context.allocate_component();
                let component_type = tree.component_type.clone();
                self.registered.push(CollectedComponent { handle, tree });
                Ok(HostResponse::Component {
                    handle,
                    component_type,
                })
            }
            HostRequest::Attach { parent, child } => {
                require_local_handle(context, child, "attach")?;
                require_collected(self, child, "attach")?;
                if let Some(parent) = parent {
                    require_local_handle(context, parent, "attach")?;
                    require_collected(self, parent, "attach")?;
                    if self.would_create_cycle(parent, child) {
                        return Err(invalid_graph(format!(
                            "attaching {child:?} below {parent:?} would create a cycle"
                        )));
                    }
                }
                if self.is_attached(child) {
                    return Err(invalid_graph(format!(
                        "component handle {child:?} is already attached"
                    )));
                }
                self.attachments.push(LocalAttachment { parent, child });
                Ok(HostResponse::Unit)
            }
            HostRequest::LoadSource {
                importer,
                specifier,
            } => {
                let path = resolve_source_path(importer.as_ref(), &specifier)?;
                let source = std::fs::read_to_string(&path).map_err(|error| HostError {
                    kind: HostErrorKind::SourceFailure,
                    operation: "load_source".into(),
                    message: format!("cannot read '{}': {error}", path.display()),
                })?;
                Ok(HostResponse::Source(crate::LoadedSource {
                    identity: crate::SourceId::new(path.to_string_lossy()),
                    source,
                }))
            }
            other => Err(HostError::unsupported(other.operation_name())),
        }
    }
}

fn resolve_source_path(
    importer: Option<&crate::SourceId>,
    specifier: &str,
) -> Result<std::path::PathBuf, HostError> {
    let requested = std::path::Path::new(specifier);
    let path = if requested.is_absolute() {
        requested.to_path_buf()
    } else {
        let importer = importer.ok_or_else(|| HostError {
            kind: HostErrorKind::SourceFailure,
            operation: "load_source".into(),
            message: format!("relative import '{specifier}' has no importer identity"),
        })?;
        std::path::Path::new(importer.as_str())
            .parent()
            .ok_or_else(|| HostError {
                kind: HostErrorKind::SourceFailure,
                operation: "load_source".into(),
                message: format!("importer '{}' has no parent", importer.as_str()),
            })?
            .join(requested)
    };
    path.canonicalize().map_err(|error| HostError {
        kind: HostErrorKind::SourceFailure,
        operation: "load_source".into(),
        message: format!("cannot resolve '{}': {error}", path.display()),
    })
}

fn require_collected(
    host: &StandardHost,
    handle: ComponentHandle,
    operation: &str,
) -> Result<(), HostError> {
    if host.collected(handle).is_some() {
        Ok(())
    } else {
        Err(HostError {
            kind: HostErrorKind::InvalidRequest,
            operation: operation.into(),
            message: format!("component handle {handle:?} has no collected tree"),
        })
    }
}

fn invalid_graph(message: impl Into<String>) -> HostError {
    HostError {
        kind: HostErrorKind::InvalidRequest,
        operation: "attach".into(),
        message: message.into(),
    }
}

fn require_local_handle(
    context: &HostContext,
    handle: ComponentHandle,
    operation: &str,
) -> Result<(), HostError> {
    if context.owns_component(handle) {
        Ok(())
    } else {
        Err(HostError {
            kind: HostErrorKind::ForeignHandle,
            operation: operation.into(),
            message: format!("component handle {handle:?} is stale or foreign"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CeChild, ComponentNamePolicy, EvalError, EventStreamHost, HostEvent, Runtime, RuntimeSpec,
        Value,
    };

    #[test]
    fn standard_runtime_collects_open_component_forest_in_authored_order() {
        let runtime = Runtime::standard();
        let mut session = runtime.session(StandardHost::new());

        session
            .eval(
                r#"
            Scene {
                title = "smoke"
                Header { "first" }
                Body { "second" }
            }
            "#,
            )
            .unwrap();

        let roots = session.host().roots();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].tree.component_type, "Scene");
        assert_eq!(roots[0].tree.properties[0].name, "title");
        assert_eq!(
            roots[0].tree.properties[0].value,
            Value::String("smoke".into())
        );
        assert_eq!(roots[0].tree.children.len(), 2);
        assert!(
            matches!(&roots[0].tree.children[0], CeChild::Spawn(child) if child.component_type == "Header")
        );
        assert!(
            matches!(&roots[0].tree.children[1], CeChild::Spawn(child) if child.component_type == "Body")
        );
    }

    #[test]
    fn strict_runtime_rejects_unregistered_component_names() {
        let mut builder = RuntimeSpec::builder::<()>();
        builder.component_name_policy(ComponentNamePolicy::StrictRegistered);
        let runtime = builder.build().unwrap();
        let mut session = runtime.runtime().session(StandardHost::new());

        let EvalError::Runtime(message) = session.eval("UnknownComponent {}").unwrap_err() else {
            panic!("expected strict catalog validation error")
        };
        assert!(
            message.contains("unknown component 'UnknownComponent'"),
            "{message}"
        );
    }

    #[test]
    fn standard_host_returns_typed_errors_for_engine_only_operations() {
        let runtime = Runtime::standard();
        let mut session = runtime.session(StandardHost::new());

        let EvalError::Host(error) = session.eval("query(\"#engine\")").unwrap_err() else {
            panic!("expected typed host error")
        };
        assert_eq!(error.kind, HostErrorKind::UnsupportedHostOperation);
        assert_eq!(error.operation, "query");
    }

    #[test]
    fn standard_runtime_accepts_a_custom_host_without_a_builder() {
        let runtime = Runtime::standard();
        let host = EventStreamHost::new();
        let mut session = runtime.session(host);

        session.eval("CustomRoot { CustomChild {} }").unwrap();

        assert!(matches!(
            &session.host().events[..],
            [HostEvent::Emit { tree, .. }] if tree.component_type == "CustomRoot"
        ));
    }

    #[test]
    fn registered_attachments_resolve_into_a_component_forest() {
        let runtime = Runtime::standard();
        let parent_tree = runtime.materialize_component("Parent {}").unwrap();
        let child_tree = runtime.materialize_component("Child { Leaf {} }").unwrap();
        let mut context = HostContext::new(100);
        let mut host = StandardHost::new();

        let HostResponse::Component { handle: parent, .. } = host
            .dispatch_with_context(
                &mut context,
                HostRequest::RegisterComponent { tree: parent_tree },
            )
            .unwrap()
        else {
            panic!("registration did not return a parent handle")
        };
        let HostResponse::Component { handle: child, .. } = host
            .dispatch_with_context(
                &mut context,
                HostRequest::RegisterComponent { tree: child_tree },
            )
            .unwrap()
        else {
            panic!("registration did not return a child handle")
        };

        host.dispatch_with_context(
            &mut context,
            HostRequest::Attach {
                parent: Some(parent),
                child,
            },
        )
        .unwrap();
        host.dispatch_with_context(
            &mut context,
            HostRequest::Attach {
                parent: None,
                child: parent,
            },
        )
        .unwrap();

        let roots = host.resolved_roots().unwrap();
        assert_eq!(roots.len(), 1);
        assert_eq!(roots[0].tree.component_type, "Parent");
        assert!(matches!(
            &roots[0].tree.children[..],
            [CeChild::Spawn(child)] if child.component_type == "Child"
                && matches!(&child.children[..], [CeChild::Spawn(leaf)] if leaf.component_type == "Leaf")
        ));
    }

    #[test]
    fn attachment_rejects_reparenting() {
        let runtime = Runtime::standard();
        let mut context = HostContext::new(101);
        let mut host = StandardHost::new();
        let mut register = |source: &str, host: &mut StandardHost| {
            let tree = runtime.materialize_component(source).unwrap();
            let HostResponse::Component { handle, .. } = host
                .dispatch_with_context(&mut context, HostRequest::RegisterComponent { tree })
                .unwrap()
            else {
                panic!("registration did not return a handle")
            };
            handle
        };
        let first_parent = register("FirstParent {}", &mut host);
        let second_parent = register("SecondParent {}", &mut host);
        let child = register("Child {}", &mut host);

        host.dispatch_with_context(
            &mut context,
            HostRequest::Attach {
                parent: Some(first_parent),
                child,
            },
        )
        .unwrap();
        let error = host
            .dispatch_with_context(
                &mut context,
                HostRequest::Attach {
                    parent: Some(second_parent),
                    child,
                },
            )
            .unwrap_err();

        assert_eq!(error.kind, HostErrorKind::InvalidRequest);
        assert!(error.message.contains("already attached"), "{error}");
    }
}
