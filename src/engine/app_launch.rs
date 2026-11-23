//! Scene launch requests from MMS to an application that owns scene switching.
//!
//! The Android launcher calls `App.launch(path)`; the host takes the request at
//! a frame boundary and replaces the running scene. Hosts that never call
//! [`set_enabled`] (the desktop runtime) report the API as unavailable instead
//! of accepting requests nothing will act on.
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchRequest {
    /// Load this MMS file, relative to the host's project root or absolute.
    Scene(String),
    /// Return to the host's launcher scene.
    Home,
}

struct State {
    enabled: bool,
    pending: Option<LaunchRequest>,
    last_error: String,
}

static STATE: Mutex<State> = Mutex::new(State {
    enabled: false,
    pending: None,
    last_error: String::new(),
});

fn state() -> std::sync::MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Called by a host that will act on requests.
pub fn set_enabled(enabled: bool) {
    state().enabled = enabled;
}

/// Records a request; a later request in the same frame replaces it.
pub fn request(request: LaunchRequest) -> Result<(), String> {
    let mut state = state();
    if !state.enabled {
        return Err("this host does not support scene launching".into());
    }
    if let LaunchRequest::Scene(path) = &request {
        if !path.ends_with(".mms") {
            return Err(format!("'{path}' is not an MMS file"));
        }
    }
    state.pending = Some(request);
    Ok(())
}

pub fn take() -> Option<LaunchRequest> {
    state().pending.take()
}

/// Why the previous scene was left, for the launcher to display. Empty when
/// the last scene exited normally.
pub fn set_last_error(message: impl Into<String>) {
    state().last_error = message.into();
}

pub fn last_error() -> String {
    state().last_error.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    // One test owns the process-wide state so parallel tests cannot interleave.
    #[test]
    fn requests_require_an_enabled_host_and_mms_paths() {
        set_enabled(false);
        assert!(request(LaunchRequest::Home).is_err());
        set_enabled(true);
        assert!(request(LaunchRequest::Scene("examples/readme.md".into())).is_err());
        request(LaunchRequest::Scene("examples/a.mms".into())).unwrap();
        request(LaunchRequest::Scene("examples/b.mms".into())).unwrap();
        assert_eq!(take(), Some(LaunchRequest::Scene("examples/b.mms".into())));
        assert_eq!(take(), None);
        set_last_error("scene failed");
        assert_eq!(last_error(), "scene failed");
        set_last_error("");
        set_enabled(false);
    }
}
