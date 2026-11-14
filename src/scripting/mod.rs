pub mod ast {
    pub use meow_meow_script::ast::*;
}
pub mod block_effect_analyzer {
    pub use meow_meow_script::block_effect_analyzer::*;
}
#[path = "component_bindings/camera_xr.rs"]
mod camera_xr_binding;
pub mod component_method_registry;
pub mod component_registry;
mod configured_registry;
mod file_api;
mod file_browser;
#[path = "component_bindings/material_definition.rs"]
mod material_definition_binding;
#[path = "component_bindings/mirror.rs"]
mod mirror_binding;
mod rigging_binding;
mod controller_pose_api;
pub use file_browser::interaction::scroll as scroll_file_browser;
pub mod host;
mod json_api;
pub mod world_evaluator;
pub mod lowering {
    pub use meow_meow_script::lowering::*;
}
pub mod object;
pub mod parser {
    pub use meow_meow_script::parser::*;
}
pub mod repl;
pub mod runner;
pub mod runtime_config;
pub mod token {
    pub use meow_meow_script::token::*;
}
pub mod tokenizer {
    pub use meow_meow_script::tokenizer::*;
}
pub mod transform {
    pub use meow_meow_script::transform::*;
}
pub mod unparser {
    pub use meow_meow_script::unparser::*;
}

pub use ast::{
    AssignmentStatement, BinOpKind, BlockStatement, CallExpression, ComponentExpression,
    ConstructorCall, Expression, Ident, IfStatement, ReturnStatement, Span, Statement, UnaryOpKind,
};
pub use block_effect_analyzer::*;
pub use host::*;
pub use lowering::*;
pub use object::*;
pub use parser::*;
pub use runner::*;
pub use runtime_config::*;
pub use token::*;
pub use tokenizer::*;
pub use world_evaluator::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod sakura_scene_tests;

#[cfg(test)]
mod paper_craft_scene_tests;
#[cfg(test)]
mod paper_craft_screen_tests;

#[cfg(test)]
mod display_car_tests;

#[cfg(test)]
mod lumae_pacing_tests;

#[cfg(test)]
mod sakura_desktop_tests;

#[cfg(test)]
mod multiplayer_tests;
#[cfg(test)]
mod untracked_avatar_tests;

#[cfg(test)]
mod climbing_scene_tests;

#[cfg(test)]
mod ui_lighting_tests;

#[cfg(test)]
mod pool_viewer_tests;

#[cfg(test)]
mod destroyable_scene_tests;

#[cfg(test)]
mod editor_ui_snapshot_tests;

#[cfg(test)]
mod editor_panel_title_tests;

#[cfg(test)]
mod sakura_editor_regression_tests;

#[cfg(test)]
mod xr_flying_tests;

#[cfg(test)]
mod gravity_mode_tests;

#[cfg(test)]
mod accordion_tests;

#[cfg(test)]
mod xr_settings_walk_tests;

#[cfg(test)]
mod rocket_motion_tests;

#[cfg(test)]
mod editor_settings_trigger_tests;

mod udp_channel_api;

#[cfg(test)]
mod udp_channel_tests;

mod morph_api;
