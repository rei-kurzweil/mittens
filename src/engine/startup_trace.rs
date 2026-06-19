use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StartupCheckpoint {
    EventLoopResumed = 0,
    WindowCreated = 1,
    RendererInitialized = 2,
    FirstRedrawRequested = 3,
    FirstUpdateCompleted = 4,
    RenderPrepared = 5,
    XrRenderCompleted = 6,
    WindowRenderCompleted = 7,
}

impl StartupCheckpoint {
    const ALL: [StartupCheckpoint; 8] = [
        StartupCheckpoint::EventLoopResumed,
        StartupCheckpoint::WindowCreated,
        StartupCheckpoint::RendererInitialized,
        StartupCheckpoint::FirstRedrawRequested,
        StartupCheckpoint::FirstUpdateCompleted,
        StartupCheckpoint::RenderPrepared,
        StartupCheckpoint::XrRenderCompleted,
        StartupCheckpoint::WindowRenderCompleted,
    ];

    fn label(self) -> &'static str {
        match self {
            StartupCheckpoint::EventLoopResumed => "event loop resumed",
            StartupCheckpoint::WindowCreated => "window created",
            StartupCheckpoint::RendererInitialized => "renderer initialized",
            StartupCheckpoint::FirstRedrawRequested => "first redraw requested",
            StartupCheckpoint::FirstUpdateCompleted => "first update completed",
            StartupCheckpoint::RenderPrepared => "render prepared",
            StartupCheckpoint::XrRenderCompleted => "xr render completed",
            StartupCheckpoint::WindowRenderCompleted => "window render completed",
        }
    }
}

fn progress_state() -> &'static Mutex<Option<StartupCheckpoint>> {
    static STATE: OnceLock<Mutex<Option<StartupCheckpoint>>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(None))
}

pub(crate) fn log_startup_progress(checkpoint: StartupCheckpoint) {
    let mut state = progress_state()
        .lock()
        .expect("startup trace mutex poisoned");
    if state.is_some_and(|current| checkpoint <= current) {
        return;
    }

    *state = Some(checkpoint);

    let total = StartupCheckpoint::ALL.len();
    let completed = checkpoint as usize + 1;
    let filled = "█".repeat(completed);
    let empty = "░".repeat(total.saturating_sub(completed));
    println!(
        "[startup] [{}{}] {}/{} {}",
        filled,
        empty,
        completed,
        total,
        checkpoint.label()
    );
}
