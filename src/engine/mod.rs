pub mod cli;
pub mod ecs;
pub mod graphics;
pub(crate) mod memory_trace;
pub mod repl;
pub(crate) mod startup_trace;
pub mod universe;
pub mod user_input;
pub mod windowing;

pub use cli::CLI;
pub use universe::Universe;
pub use windowing::Windowing;

pub fn debug_memory_log_line(line: impl AsRef<str>) {
    memory_trace::log_line(line);
}

pub fn debug_memory_sample(label: &str) {
    memory_trace::sample(label, None);
}

/// Engine-level error type placeholder.
#[derive(Debug)]
pub enum EngineError {
    Windowing(String),
}

pub type EngineResult<T> = Result<T, EngineError>;
