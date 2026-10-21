#[cfg(feature = "desktop")]
pub mod cli;
pub mod ecs;
pub mod graphics;
pub mod repl;
pub(crate) mod startup_trace;
pub mod transform;
pub mod universe;
pub mod user_input;
pub mod vr_perf;
#[cfg(feature = "desktop")]
pub mod windowing;

#[cfg(feature = "desktop")]
pub use cli::CLI;
pub use transform::{TransformMatrix, TransformSpace, TransformTrs, TransformTrsError};
pub use universe::Universe;
#[cfg(feature = "desktop")]
pub use windowing::Windowing;

/// Engine-level error type placeholder.
#[derive(Debug)]
pub enum EngineError {
    Windowing(String),
}

pub type EngineResult<T> = Result<T, EngineError>;
