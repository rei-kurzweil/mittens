use std::sync::Arc;
use std::time::Instant;

use crate::engine::startup_trace::{StartupCheckpoint, log_startup_progress};
use crate::engine::user_input::{InputState, UserInput, WinitHidProvider};
type InputHook = Box<dyn FnMut(&mut crate::engine::Universe, &mut InputState)>;
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
        Self::run_app_with_input_and_frame_hook(universe, Some(user_input), None, None)
    }

    /// Run using only the HID providers configured on the Universe, including
    /// an empty collection. Window lifecycle events remain handled by the host.
    pub fn run_app_with_providers(universe: crate::engine::Universe) -> EngineResult<()> {
        Self::run_app_with_input_and_frame_hook(universe, None, None, None)
    }

    /// Run a Universe and invoke `frame_hook` after each update and before
    /// rendering. This lets retained application sessions service work that
    /// was queued by systems during the update.
    pub fn run_app_with_frame_hook(
        universe: crate::engine::Universe,
        frame_hook: impl FnMut(&mut crate::engine::Universe) + 'static,
    ) -> EngineResult<()> {
        Self::run_app_with_input_and_frame_hook(
            universe,
            Some(UserInput::new()),
            Some(Box::new(frame_hook)),
            None,
        )
    }

    /// Inspect or route normalized input before ECS focus/gameplay consumers run.
    pub fn run_app_with_input_hook(
        universe: crate::engine::Universe,
        hook: impl FnMut(&mut crate::engine::Universe, &mut InputState) + 'static,
    ) -> EngineResult<()> {
        Self::run_app_with_input_and_frame_hook(
            universe,
            Some(UserInput::new()),
            None,
            Some(Box::new(hook)),
        )
    }

    fn run_app_with_input_and_frame_hook(
        mut universe: crate::engine::Universe,
        user_input: Option<UserInput>,
        frame_hook: Option<Box<dyn FnMut(&mut crate::engine::Universe)>>,
        input_hook: Option<InputHook>,
    ) -> EngineResult<()> {
        let event_loop = EventLoop::new()
            .map_err(|error| EngineError::Windowing(format!("event loop init failed: {error}")))?;
        event_loop.set_control_flow(ControlFlow::Poll);

        let user_input = user_input.map(WinitHidProvider::new);
        if let Some(provider) = &user_input {
            universe.input_providers.add(provider.clone());
        }

        let mut app = App {
            window: None,
            universe: Some(universe),
            last_frame: None,
            user_input,
            frame_hook,
            input_hook,
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
    user_input: Option<WinitHidProvider>,
    frame_hook: Option<Box<dyn FnMut(&mut crate::engine::Universe)>>,
    input_hook: Option<InputHook>,
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
            .with_title(concat!("mittens ", env!("CARGO_PKG_VERSION")))
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
        if let Some(provider) = &self.user_input {
            provider.handle_window_event(&event);
        }

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
                if let Some(universe) = self.universe.as_mut() {
                    universe.resize_renderer(size);
                }
                if let Some(w) = &self.window {
                    // w.pre_present_notify();
                    w.request_redraw();
                }
            }

            WindowEvent::RedrawRequested => {
                log_startup_progress(StartupCheckpoint::FirstRedrawRequested);
                let now = Instant::now();
                let dt = self
                    .last_frame
                    .replace(now)
                    .map(|prev| (now - prev).as_secs_f32())
                    .unwrap_or(0.0);

                let universe = self.universe.as_mut().expect("universe missing");

                let mut input = universe.input_providers.poll();
                if let Some(hook) = self.input_hook.as_mut() {
                    hook(universe, &mut input);
                }
                universe.update(dt, &input);

                if let Some(frame_hook) = self.frame_hook.as_mut() {
                    frame_hook(universe);
                }

                universe.render();

                if universe.exit_requested() {
                    event_loop.exit();
                    return;
                }

                if let Some(w) = &self.window {
                    // w.pre_present_notify();
                    w.request_redraw();
                }
            }

            _ => {}
        }
    }
}
