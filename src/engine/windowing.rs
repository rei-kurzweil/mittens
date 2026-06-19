use std::sync::Arc;
use std::time::Instant;

use crate::engine::startup_trace::{StartupCheckpoint, log_startup_progress};
use crate::engine::user_input::UserInput;
use crate::engine::{EngineError, EngineResult};

use winit::application::ApplicationHandler;
use winit::event::{ElementState, KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};

/// Minimal winit wrapper (2025 winit style: ApplicationHandler).
pub struct Windowing;

impl Windowing {
    /// Run a Universe in a winit window with default input handling.
    pub fn run_app(universe: crate::engine::Universe) -> EngineResult<()> {
        Self::run_app_with_input(universe, UserInput::new())
    }

    /// Run a Universe in a winit window with a caller-provided `UserInput`.
    pub fn run_app_with_input(
        universe: crate::engine::Universe,
        user_input: UserInput,
    ) -> EngineResult<()> {
        let event_loop = EventLoop::new()
            .map_err(|error| EngineError::Windowing(format!("event loop init failed: {error}")))?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let mut app = App {
            window: None,
            universe: Some(universe),
            last_frame: None,
            user_input,
            startup_error: None,
        };

        event_loop.run_app(&mut app).map_err(|error| {
            EngineError::Windowing(format!("window event loop failed: {error}"))
        })?;

        if let Some(error) = app.startup_error.take() {
            return Err(error);
        }

        Ok(())
    }
}

struct App {
    window: Option<Arc<Window>>,
    universe: Option<crate::engine::Universe>,
    last_frame: Option<Instant>,
    user_input: UserInput,
    startup_error: Option<EngineError>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        log_startup_progress(StartupCheckpoint::EventLoopResumed);

        let preferred_window_size = self
            .universe
            .as_mut()
            .and_then(|universe| universe.take_preferred_window_size())
            .unwrap_or([1024, 768]);

        let attrs: WindowAttributes = Window::default_attributes()
            .with_title("cat engine 0.5 \"mittens\" ")
            .with_inner_size(winit::dpi::LogicalSize::new(
                preferred_window_size[0] as f64,
                preferred_window_size[1] as f64,
            ))
            .with_resizable(true);

        let window = match event_loop.create_window(attrs) {
            Ok(window) => window,
            Err(error) => {
                self.startup_error = Some(EngineError::Windowing(format!(
                    "window creation failed: {error}"
                )));
                event_loop.exit();
                return;
            }
        };
        let window = Arc::new(window);
        log_startup_progress(StartupCheckpoint::WindowCreated);

        // Initialize renderer backend for this window via Universe
        if let Some(universe) = self.universe.as_mut() {
            if let Err(error) = universe.init_renderer_for_window(&window) {
                self.startup_error = Some(EngineError::Windowing(format!(
                    "renderer init failed: {error}"
                )));
                event_loop.exit();
                return;
            }
        }
        log_startup_progress(StartupCheckpoint::RendererInitialized);

        self.window = Some(window);
        self.last_frame = Some(Instant::now());

        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Feed input events into our input handler, but keep window lifecycle/render events here.
        // This intentionally ignores resize/draw.
        let _was_input_event = self.user_input.handle_window_event(&event);

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        logical_key: Key::Named(NamedKey::Escape),
                        state: ElementState::Pressed,
                        ..
                    },
                ..
            } => event_loop.exit(),

            WindowEvent::Resized(size) => {
                println!("[Windowing] Resized event received: {:?}", size);
                if let Some(w) = &self.window {
                    let actual_size = w.inner_size();
                    println!("[Windowing] Window's actual inner_size: {:?}", actual_size);
                    // Ensure window is still resizable (in case something changed it)
                    if !w.is_resizable() {
                        println!("[Windowing] WARNING: Window is not resizable!");
                    }
                }
                if let Some(universe) = self.universe.as_mut() {
                    universe.resize_renderer(size);
                }
                if let Some(w) = &self.window {
                    println!("[Windowing] resized; requesting redraw");
                    // w.pre_present_notify();
                    w.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                log_startup_progress(StartupCheckpoint::FirstRedrawRequested);
                // Start of our "frame" from an input perspective: compute deltas, but keep
                // edge-triggered sets so they remain visible during `universe.update`.
                self.user_input.start_frame();

                let now = Instant::now();
                let dt = self
                    .last_frame
                    .replace(now)
                    .map(|prev| (now - prev).as_secs_f32())
                    .unwrap_or(0.0);

                let universe = self.universe.as_mut().expect("universe missing");

                universe.update(dt, self.user_input.state());

                universe.render();

                // Clear edge-triggered sets after the frame has consumed them.
                self.user_input.end_frame();

                if let Some(w) = &self.window {
                    // w.pre_present_notify();
                    w.request_redraw();
                }
            }

            _ => {}
        }
    }
}
