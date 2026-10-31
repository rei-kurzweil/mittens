//! Selection and row scrolling over a fixed native cell pool; no stencil/copies.
use super::{Control, changed, control, view, with_browser};
use crate::engine::ecs::{
    ComponentId, EventSignal, RxWorld, SignalEmitter, SignalKind, World,
    component::TransformComponent,
};

pub(super) fn scroll_model(model: &mut super::Model, rows: f64) {
    if !rows.is_finite() {
        return;
    }
    let limit = model.entries.len().div_ceil(5).saturating_sub(4);
    model.page = (model.page as f64 + rows).round().clamp(0.0, limit as f64) as usize;
}
pub fn scroll(world: &mut World, emit: &mut dyn SignalEmitter, root: ComponentId, rows: f64) {
    let _ = with_browser(world, root, |browser, world| {
        let old = browser.model.page;
        scroll_model(&mut browser.model, rows);
        if old != browser.model.page {
            view::refresh(world, emit, root, &browser.model, &browser.view);
        }
    });
}
pub(super) fn install(rx: &mut RxWorld, root: ComponentId) {
    rx.add_handler_closure(
        SignalKind::SelectionChanged,
        root,
        move |world, emit, signal| {
            let Some(EventSignal::SelectionChanged {
                selected_component: Some(selected),
                ..
            }) = signal.event.as_ref()
            else {
                return;
            };
            let Some(Control::Entry(slot)) = control(world, root, *selected) else {
                return;
            };
            let _ = with_browser(world, root, |browser, world| {
                if let Some(entry) = browser.model.entries.get(browser.model.page * 5 + slot) {
                    let id = entry.id.clone();
                    // Dummy selection only changes its source preview. A remote
                    // browser reports an opaque selection; neither executes code.
                    if let Err(error) = browser.model.activate("open", &id) {
                        browser.model.error = error;
                    }
                }
                view::refresh(world, emit, root, &browser.model, &browser.view);
                changed(emit, root);
            });
        },
    );
    let mut drag = 0.0_f32;
    rx.add_handler_closure(SignalKind::DragMove, root, move |world, emit, signal| {
        let Some(EventSignal::DragMove { delta_world, .. }) = signal.event.as_ref() else {
            return;
        };
        let scale = world
            .get_component_by_id_as::<TransformComponent>(root)
            .map(|t| {
                let y = t.transform.matrix_world[1];
                (y[0] * y[0] + y[1] * y[1] + y[2] * y[2]).sqrt()
            })
            .unwrap_or(1.0)
            .max(0.0001);
        drag += delta_world[1] / scale;
        let rows = (drag / 8.8).trunc();
        if rows != 0.0 {
            drag -= rows * 8.8;
            scroll(world, emit, root, rows as f64);
        }
    });
}
