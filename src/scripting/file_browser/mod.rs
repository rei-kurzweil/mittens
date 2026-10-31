//! Native file-browser widget, composed from MMS through opaque APIs.
pub(super) mod interaction;
mod model;
#[cfg(test)]
mod tests;
mod view;
use super::{
    host::MittensHost,
    runtime_config::{MittensApi as Api, MittensBinding},
};
use crate::engine::ecs::{
    ComponentId, EventSignal, SignalEmitter, SignalKind, World, component::Component,
};
use meow_meow_script as mms;
use model::{Model, PAGE_SIZE};

struct Browser {
    model: Model,
    view: view::View,
}
struct State(Option<Browser>);
#[derive(Clone, Copy)]
enum Control {
    Up,
    Previous,
    Next,
    Entry(usize),
}
struct ControlComponent(Control);
macro_rules! component {
    ($ty:ty, $name:literal) => {
        impl Component for $ty {
            fn name(&self) -> &'static str {
                $name
            }
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
                self
            }
        }
    };
}
component!(State, "file_browser_state");
component!(ControlComponent, "file_browser_control");
pub(super) fn register(builder: &mut mms::RuntimeSpecBuilder<MittensBinding>) {
    use mms::ValueType as T;
    builder.namespace("FileBrowser", |ns| {
        for (name, api, args, result) in [
            (
                "local",
                Api::BrowserLocal,
                vec![T::String, T::String],
                T::Component,
            ),
            ("remote", Api::BrowserRemote, vec![], T::Component),
            ("demo", Api::BrowserDemo, vec![], T::Component),
            ("snapshot", Api::BrowserSnapshot, vec![T::Component], T::Any),
            (
                "selection",
                Api::BrowserSelection,
                vec![T::Component],
                T::Any,
            ),
            ("action", Api::BrowserAction, vec![T::Component], T::Any),
            (
                "set_listing",
                Api::BrowserSetListing,
                vec![T::Component, T::Any],
                T::Bool,
            ),
            (
                "activate",
                Api::BrowserActivate,
                vec![T::Component, T::String, T::String, T::String],
                T::Bool,
            ),
            ("refresh", Api::BrowserRefresh, vec![T::Component], T::Bool),
            ("clear", Api::BrowserClear, vec![T::Component], T::Null),
            (
                "scroll",
                Api::BrowserScroll,
                vec![T::Component, T::F64],
                T::Null,
            ),
        ] {
            ns.api(
                name,
                mms::ValueSignature::new(args, result),
                MittensBinding::Api(api),
            );
        }
    });
}
pub(super) fn supports(api: Api) -> bool {
    matches!(
        api,
        Api::BrowserLocal
            | Api::BrowserRemote
            | Api::BrowserDemo
            | Api::BrowserSnapshot
            | Api::BrowserSelection
            | Api::BrowserAction
            | Api::BrowserSetListing
            | Api::BrowserActivate
            | Api::BrowserRefresh
            | Api::BrowserClear
            | Api::BrowserScroll
    )
}
fn with_browser<T>(
    world: &mut World,
    root: ComponentId,
    call: impl FnOnce(&mut Browser, &mut World) -> T,
) -> Result<T, String> {
    let id = world
        .children_of(root)
        .iter()
        .copied()
        .find(|&id| world.get_component_by_id_as::<State>(id).is_some())
        .ok_or("not a file browser")?;
    let mut browser = world
        .get_component_by_id_as_mut::<State>(id)
        .unwrap()
        .0
        .take()
        .ok_or("browser is busy")?;
    let result = call(&mut browser, world);
    world.get_component_by_id_as_mut::<State>(id).unwrap().0 = Some(browser);
    Ok(result)
}
fn changed(emit: &mut dyn SignalEmitter, root: ComponentId) {
    emit.push_event(
        root,
        EventSignal::DataEvent {
            name: "FileBrowserChanged".into(),
            payload: None,
        },
    );
}
fn control(world: &World, root: ComponentId, target: ComponentId) -> Option<Control> {
    let mut current = Some(target);
    while let Some(id) = current {
        for &child in world.children_of(id) {
            if let Some(control) = world.get_component_by_id_as::<ControlComponent>(child) {
                return Some(control.0);
            }
        }
        if id == root {
            break;
        }
        current = world.parent_of(id);
    }
    None
}
fn click(world: &mut World, emit: &mut dyn SignalEmitter, root: ComponentId, control: Control) {
    let _ = with_browser(world, root, |browser, world| {
        let model = &mut browser.model;
        let result = match control {
            Control::Previous => {
                model.page = model.page.saturating_sub(1);
                Ok(())
            }
            Control::Next => {
                if model.page < model.entries.len().div_ceil(5).saturating_sub(4) {
                    model.page += 1;
                }
                Ok(())
            }
            Control::Up => {
                if model.can_up {
                    model.activate("up", "")
                } else {
                    Ok(())
                }
            }
            // The engine Selection system owns pointer selection. Activation is
            // handled from SelectionChanged, after its state is committed.
            Control::Entry(_) => return,
        };
        if let Err(error) = result {
            model.error = error;
        }
        view::refresh(world, emit, root, model, &browser.view);
        changed(emit, root);
    });
}
pub(super) fn invoke(
    host: &mut MittensHost<'_, '_>,
    api: Api,
    args: Vec<mms::TransportValue>,
) -> Result<mms::HostResponse, mms::HostError> {
    use mms::{HostResponse as R, TransportValue as V};
    let fail = |error: String| mms::HostError::failure("FileBrowser", error);
    if matches!(
        api,
        Api::BrowserLocal | Api::BrowserRemote | Api::BrowserDemo
    ) {
        let model = if api == Api::BrowserLocal {
            let [V::String(root), V::String(relative)] = args.as_slice() else {
                return Err(fail("local expects root and relative path".into()));
            };
            Model::local(root, relative).map_err(fail)?
        } else if api == Api::BrowserDemo {
            Model::demo()
        } else {
            Model::default()
        };
        let assets = host
            .render_assets
            .as_deref_mut()
            .ok_or_else(|| fail("render assets unavailable".into()))?;
        let rx = host
            .rx
            .as_deref_mut()
            .ok_or_else(|| fail("signal service unavailable".into()))?;
        let (root, view) = view::build(host.world, assets).map_err(fail)?;
        view::refresh(host.world, host.emit, root, &model, &view);
        let state = host
            .world
            .add_component(State(Some(Browser { model, view })));
        host.world.add_child(root, state).unwrap();
        rx.add_handler_closure(SignalKind::Click, root, move |world, emit, signal| {
            if let Some(EventSignal::Click { renderable, .. }) = signal.event.as_ref() {
                if let Some(control) = control(world, root, *renderable) {
                    click(world, emit, root, control);
                }
            }
        });
        interaction::install(rx, root);
        return Ok(R::Component {
            handle: MittensHost::component_handle(root),
            component_type: "Transform".into(),
        });
    }
    let Some(V::Component(handle) | V::TypedComponent { handle, .. }) = args.first() else {
        return Err(fail("expected browser component".into()));
    };
    let root = MittensHost::component_id(*handle);
    let response = with_browser(host.world, root, |browser, world| {
        let model = &mut browser.model;
        match api {
            Api::BrowserSnapshot => R::Transport(model.snapshot()),
            Api::BrowserSelection => R::Transport(V::Table(Vec::from([
                ("name".into(), V::String(model.selected_name.clone())),
                ("source".into(), V::String(model.preview.clone())),
            ]))),
            Api::BrowserAction => R::Transport(model.action.clone().unwrap_or(V::Null)),
            Api::BrowserScroll => {
                if let Some(V::Number(rows)) = args.get(1) {
                    interaction::scroll_model(model, *rows);
                    view::refresh(world, host.emit, root, model, &browser.view);
                }
                R::Unit
            }
            Api::BrowserClear => {
                model.entries.clear();
                model.revision.clear();
                model.path = "Desktop offline".into();
                model.action = None;
                model.can_up = false;
                model.page = 0;
                view::refresh(world, host.emit, root, model, &browser.view);
                R::Unit
            }
            Api::BrowserSetListing => {
                let result = args
                    .get(1)
                    .ok_or("expected listing".to_string())
                    .and_then(|listing| model.set_listing(listing));
                let ok = result.is_ok();
                match result {
                    Ok(true) => view::refresh(world, host.emit, root, model, &browser.view),
                    Err(error) => model.error = error,
                    _ => {}
                }
                R::Transport(V::Bool(ok))
            }
            Api::BrowserRefresh | Api::BrowserActivate => {
                let result = if api == Api::BrowserRefresh {
                    let relative = model.relative.clone();
                    model.navigate(&relative)
                } else if let [_, V::String(revision), V::String(kind), V::String(id)] =
                    args.as_slice()
                {
                    if revision != &model.revision {
                        Err("obsolete listing".into())
                    } else {
                        model.activate(kind, id)
                    }
                } else {
                    Err("expected browser, revision, kind, entry ID".into())
                };
                let ok = result.is_ok();
                if let Err(error) = result {
                    model.error = error;
                }
                view::refresh(world, host.emit, root, model, &browser.view);
                changed(host.emit, root);
                R::Transport(V::Bool(ok))
            }
            _ => unreachable!(),
        }
    })
    .map_err(fail)?;
    Ok(response)
}
