//! Resolved editor MMS paths, retained after the host configures its asset root.
use std::sync::OnceLock;
fn resolved(cell: &'static OnceLock<String>, relative: &str) -> &'static str {
    cell.get_or_init(|| {
        crate::engine::runtime_assets::path(relative)
            .to_string_lossy()
            .into_owned()
    })
}
pub(super) fn panels() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    resolved(&PATH, "assets/components/internal/panels.mms")
}
pub(super) fn icons() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    resolved(&PATH, "assets/components/icons.mms")
}
pub(super) fn items() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    resolved(&PATH, "assets/components/internal/panel_items.mms")
}
pub(super) fn details() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    resolved(&PATH, "assets/components/internal/inspector_details.mms")
}
