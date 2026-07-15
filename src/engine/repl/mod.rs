pub mod color;
pub mod pipe;
pub mod repl;
pub mod repl_backend;
pub mod util;

pub use repl::Repl;
pub use repl_backend::ReplBackend;

use std::sync::atomic::{AtomicBool, Ordering};
static STDIN_REPL_ACTIVE: AtomicBool = AtomicBool::new(false);

pub(crate) fn claim_stdin() -> Result<(), &'static str> {
    if STDIN_REPL_ACTIVE.swap(true, Ordering::AcqRel) {
        Err("an stdin REPL is already active")
    } else {
        Ok(())
    }
}

pub(crate) fn release_stdin() {
    STDIN_REPL_ACTIVE.store(false, Ordering::Release);
}
