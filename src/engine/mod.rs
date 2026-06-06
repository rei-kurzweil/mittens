pub mod cli;
pub mod ecs;
pub mod graphics;
pub mod repl;
pub mod universe;
pub mod user_input;
pub mod windowing;

pub use cli::CLI;
pub use universe::Universe;
pub use windowing::Windowing;

/// Engine-level error type placeholder.
#[derive(Debug)]
pub enum EngineError {
    Windowing(String),
}

pub type EngineResult<T> = Result<T, EngineError>;
