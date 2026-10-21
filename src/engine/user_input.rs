//! Shared input snapshots and host HID providers. `UserInput` retains the
//! desktop event adapter; providers can be absent or combined by the host.

mod providers;
pub use providers::{HidProvider, HidProviders, InputProviderId, WinitHidProvider};

use std::collections::HashSet;

use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

/// One ordered gameplay-keyboard transition captured at the platform boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardInputRecord {
    /// Identifies the source so two keyboards can hold the same physical key.
    pub provider_id: InputProviderId,
    pub physical_key: PhysicalKey,
    pub code: Option<String>,
    pub key: String,
    pub transition: KeyboardTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardTransition {
    Down,
    Press,
    Up,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputFrameEvent {
    InsertText(String),
    Backspace,
    DeleteForward,
    MoveCaretLeft,
    MoveCaretRight,
}

/// Snapshot of user input.
///
/// This is intentionally minimal for now, but it already supports:
/// - current key/button state (`down`)
/// - per-frame transitions (`pressed`/`released`)
/// - cursor position and wheel delta
/// - mouse movement delta
#[derive(Default, Debug, Clone)]
pub struct InputState {
    pub keys_down: HashSet<Key>,
    pub keys_pressed: HashSet<Key>,
    pub keys_released: HashSet<Key>,

    pub mouse_down: HashSet<MouseButton>,
    pub mouse_pressed: HashSet<MouseButton>,
    pub mouse_released: HashSet<MouseButton>,

    /// Cursor position in physical pixels (as reported by winit).
    pub cursor_pos: Option<(f32, f32)>,

    /// Previous cursor position (updated at `begin_frame`).
    prev_cursor_pos: Option<(f32, f32)>,

    /// Mouse movement delta since last frame (current - previous).
    mouse_movement: (f32, f32),

    /// Derived mouse drag state (active when a button is held while the cursor moves).
    mouse_dragging: bool,
    mouse_drag_delta: (f32, f32),

    /// Accumulated wheel delta since last `begin_frame`.
    pub wheel_delta: (f32, f32),

    text_input_events: Vec<TextInputFrameEvent>,
    keyboard_events: Vec<KeyboardInputRecord>,
    physical_keys_down: Vec<(PhysicalKey, Option<String>, String, Key)>,
    // None preserves manually supplied snapshots. A provider-produced snapshot
    // explicitly lists live sources, including an empty list for idle hosts.
    active_providers: Option<HashSet<InputProviderId>>,
}

impl InputState {
    fn release_all_keys(&mut self) {
        for (physical_key, code, key, gameplay_key) in self.physical_keys_down.drain(..) {
            self.keyboard_events.push(KeyboardInputRecord {
                provider_id: InputProviderId::MANUAL,
                physical_key,
                code,
                key,
                transition: KeyboardTransition::Up,
            });
            self.keys_released.insert(gameplay_key);
        }
        self.keys_down.clear();
    }

    /// Called at the start of a render/update frame.
    ///
    /// Important: this does **not** clear edge-triggered sets (`pressed`/`released`).
    /// Those are cleared at `end_frame` so events delivered before `RedrawRequested`
    /// are still visible to systems during `Universe::update`.
    pub fn start_frame(&mut self) {
        // Update mouse movement delta.
        self.mouse_movement = match (self.cursor_pos, self.prev_cursor_pos) {
            (Some((cx, cy)), Some((px, py))) => (cx - px, cy - py),
            _ => (0.0, 0.0),
        };
        self.prev_cursor_pos = self.cursor_pos;

        // Derive drag state from buttons + movement.
        let any_button_down = !self.mouse_down.is_empty();
        let moved = self.mouse_movement.0 != 0.0 || self.mouse_movement.1 != 0.0;
        self.mouse_dragging = any_button_down && moved;
        self.mouse_drag_delta = if self.mouse_dragging {
            self.mouse_movement
        } else {
            (0.0, 0.0)
        };
    }

    /// Clears edge-triggered sets at the end of a frame.
    pub fn end_frame(&mut self) {
        self.keys_pressed.clear();
        self.keys_released.clear();
        self.mouse_pressed.clear();
        self.mouse_released.clear();
        self.wheel_delta = (0.0, 0.0);
        self.text_input_events.clear();
        self.keyboard_events.clear();
    }

    #[inline]
    pub fn key_down(&self, key: &Key) -> bool {
        self.keys_down.contains(key)
    }

    #[inline]
    pub fn key_pressed(&self, key: &Key) -> bool {
        self.keys_pressed.contains(key)
    }

    #[inline]
    pub fn key_released(&self, key: &Key) -> bool {
        self.keys_released.contains(key)
    }

    /// Returns the mouse movement delta (dx, dy) since the last frame.
    /// Returns (0, 0) if cursor position is not available.
    #[inline]
    pub fn mouse_movement(&self) -> (f32, f32) {
        self.mouse_movement
    }

    /// Whether the user is currently dragging the mouse (button held + cursor moved this frame).
    #[inline]
    pub fn mouse_dragging(&self) -> bool {
        self.mouse_dragging
    }

    /// Mouse drag delta (dx, dy) in pixels for this frame.
    #[inline]
    pub fn mouse_drag_delta(&self) -> (f32, f32) {
        self.mouse_drag_delta
    }

    /// Whether the given mouse button is currently dragging (that button is held + cursor moved
    /// this frame).
    #[inline]
    pub fn mouse_dragging_button(&self, button: MouseButton) -> bool {
        self.mouse_down.contains(&button)
            && (self.mouse_movement.0 != 0.0 || self.mouse_movement.1 != 0.0)
    }

    /// Mouse drag delta (dx, dy) in pixels for this frame, gated to the given button.
    #[inline]
    pub fn mouse_drag_delta_button(&self, button: MouseButton) -> (f32, f32) {
        if self.mouse_dragging_button(button) {
            self.mouse_movement
        } else {
            (0.0, 0.0)
        }
    }

    #[inline]
    pub fn text_input_events(&self) -> &[TextInputFrameEvent] {
        &self.text_input_events
    }

    #[inline]
    pub fn keyboard_events(&self) -> &[KeyboardInputRecord] {
        &self.keyboard_events
    }

    pub fn push_keyboard_event(&mut self, event: KeyboardInputRecord) {
        self.keyboard_events.push(event);
    }

    pub fn push_text_input_event(&mut self, event: TextInputFrameEvent) {
        self.text_input_events.push(event);
    }

    pub(crate) fn provider_is_active(&self, provider: InputProviderId) -> bool {
        self.active_providers
            .as_ref()
            .is_none_or(|active| active.contains(&provider))
    }
}

/// Stateful input event processor.
#[derive(Default, Debug, Clone)]
pub struct UserInput {
    state: InputState,
}

impl UserInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn state(&self) -> &InputState {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut InputState {
        &mut self.state
    }

    pub fn start_frame(&mut self) {
        self.state.start_frame();
    }

    pub fn end_frame(&mut self) {
        self.state.end_frame();
    }

    /// Feed a winit event into this input handler.
    ///
    /// Returns `true` if the event was recognized/consumed as input.
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                fn normalize_key(key: &Key) -> Key {
                    match key {
                        // Treat ASCII letters case-insensitively by storing the lowercase form.
                        // This makes WASD/QE work regardless of Shift state.
                        Key::Character(s) => {
                            if s.len() == 1 {
                                let c = s.chars().next().unwrap_or('\0');
                                if c.is_ascii_alphabetic() {
                                    return Key::Character(
                                        c.to_ascii_lowercase().to_string().into(),
                                    );
                                }
                            }
                            Key::Character(s.clone())
                        }
                        _ => key.clone(),
                    }
                }

                let key = normalize_key(&event.logical_key);
                let public_event = (
                    physical_key_code(event.physical_key),
                    logical_key(&event.logical_key),
                );
                match event.state {
                    ElementState::Pressed => {
                        if *is_synthetic {
                            return true;
                        }
                        let was_physically_down = self
                            .state
                            .physical_keys_down
                            .iter()
                            .any(|(physical_key, ..)| *physical_key == event.physical_key);
                        if !was_physically_down && !event.repeat {
                            self.state.physical_keys_down.push((
                                event.physical_key,
                                public_event.0.clone(),
                                public_event.1.clone(),
                                key.clone(),
                            ));
                            self.state.keyboard_events.push(KeyboardInputRecord {
                                provider_id: InputProviderId::MANUAL,
                                physical_key: event.physical_key,
                                code: public_event.0.clone(),
                                key: public_event.1.clone(),
                                transition: KeyboardTransition::Down,
                            });
                        }
                        if event.repeat && !was_physically_down {
                            return true;
                        }
                        self.state.keyboard_events.push(KeyboardInputRecord {
                            provider_id: InputProviderId::MANUAL,
                            physical_key: event.physical_key,
                            code: public_event.0,
                            key: public_event.1,
                            transition: KeyboardTransition::Press,
                        });
                        let was_down = self.state.keys_down.contains(&key);
                        self.state.keys_down.insert(key.clone());
                        if !was_down {
                            self.state.keys_pressed.insert(key);
                        }
                        match &event.logical_key {
                            Key::Named(NamedKey::Backspace) => {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::Backspace);
                            }
                            Key::Named(NamedKey::Delete) => {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::DeleteForward);
                            }
                            Key::Named(NamedKey::ArrowLeft) => {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::MoveCaretLeft);
                            }
                            Key::Named(NamedKey::ArrowRight) => {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::MoveCaretRight);
                            }
                            _ => {}
                        }
                        if let Some(text) = event.text.as_ref() {
                            let filtered: String =
                                text.chars().filter(|c| !c.is_control()).collect();
                            if !filtered.is_empty() {
                                self.state
                                    .text_input_events
                                    .push(TextInputFrameEvent::InsertText(filtered));
                            }
                        }
                    }
                    ElementState::Released => {
                        if let Some(index) = self
                            .state
                            .physical_keys_down
                            .iter()
                            .position(|(physical_key, ..)| *physical_key == event.physical_key)
                        {
                            let (_, code, logical_key, gameplay_key) =
                                self.state.physical_keys_down.remove(index);
                            self.state.keyboard_events.push(KeyboardInputRecord {
                                provider_id: InputProviderId::MANUAL,
                                physical_key: event.physical_key,
                                code,
                                key: logical_key,
                                transition: KeyboardTransition::Up,
                            });
                            self.state.keys_down.remove(&gameplay_key);
                            self.state.keys_released.insert(gameplay_key);
                        } else {
                            self.state.keys_down.remove(&key);
                            self.state.keys_released.insert(key);
                        }
                    }
                }
                true
            }

            WindowEvent::Focused(false) => {
                self.state.release_all_keys();
                self.state
                    .mouse_released
                    .extend(self.state.mouse_down.drain());
                self.state.cursor_pos = None;
                true
            }

            WindowEvent::MouseInput { state, button, .. } => {
                match state {
                    ElementState::Pressed => {
                        let was_down = self.state.mouse_down.contains(button);
                        self.state.mouse_down.insert(*button);
                        if !was_down {
                            self.state.mouse_pressed.insert(*button);
                        }
                    }
                    ElementState::Released => {
                        self.state.mouse_down.remove(button);
                        self.state.mouse_released.insert(*button);
                    }
                }
                true
            }

            WindowEvent::CursorMoved { position, .. } => {
                self.state.cursor_pos = Some((position.x as f32, position.y as f32));
                true
            }

            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (*x, *y),
                    MouseScrollDelta::PixelDelta(pos) => (pos.x as f32, pos.y as f32),
                };
                self.state.wheel_delta.0 += dx;
                self.state.wheel_delta.1 += dy;
                true
            }

            _ => false,
        }
    }
}

fn physical_key_code(key: PhysicalKey) -> Option<String> {
    let PhysicalKey::Code(code) = key else {
        return None;
    };
    macro_rules! standard_code {
        ($($variant:ident),+ $(,)?) => {
            match code {
                $(KeyCode::$variant => stringify!($variant),)+
                KeyCode::SuperLeft => "MetaLeft",
                KeyCode::SuperRight => "MetaRight",
                _ => return None,
            }
        };
    }
    // Explicit allowlist: these strings are the public API, not enum Debug output.
    let name = standard_code!(
        Backquote,
        Backslash,
        BracketLeft,
        BracketRight,
        Comma,
        Digit0,
        Digit1,
        Digit2,
        Digit3,
        Digit4,
        Digit5,
        Digit6,
        Digit7,
        Digit8,
        Digit9,
        Equal,
        IntlBackslash,
        IntlRo,
        IntlYen,
        KeyA,
        KeyB,
        KeyC,
        KeyD,
        KeyE,
        KeyF,
        KeyG,
        KeyH,
        KeyI,
        KeyJ,
        KeyK,
        KeyL,
        KeyM,
        KeyN,
        KeyO,
        KeyP,
        KeyQ,
        KeyR,
        KeyS,
        KeyT,
        KeyU,
        KeyV,
        KeyW,
        KeyX,
        KeyY,
        KeyZ,
        Minus,
        Period,
        Quote,
        Semicolon,
        Slash,
        AltLeft,
        AltRight,
        Backspace,
        CapsLock,
        ContextMenu,
        ControlLeft,
        ControlRight,
        Enter,
        ShiftLeft,
        ShiftRight,
        Space,
        Tab,
        Convert,
        KanaMode,
        Lang1,
        Lang2,
        Lang3,
        Lang4,
        Lang5,
        NonConvert,
        Delete,
        End,
        Help,
        Home,
        Insert,
        PageDown,
        PageUp,
        ArrowDown,
        ArrowLeft,
        ArrowRight,
        ArrowUp,
        NumLock,
        Numpad0,
        Numpad1,
        Numpad2,
        Numpad3,
        Numpad4,
        Numpad5,
        Numpad6,
        Numpad7,
        Numpad8,
        Numpad9,
        NumpadAdd,
        NumpadBackspace,
        NumpadClear,
        NumpadClearEntry,
        NumpadComma,
        NumpadDecimal,
        NumpadDivide,
        NumpadEnter,
        NumpadEqual,
        NumpadHash,
        NumpadMemoryAdd,
        NumpadMemoryClear,
        NumpadMemoryRecall,
        NumpadMemoryStore,
        NumpadMemorySubtract,
        NumpadMultiply,
        NumpadParenLeft,
        NumpadParenRight,
        NumpadStar,
        NumpadSubtract,
        Escape,
        Fn,
        FnLock,
        PrintScreen,
        ScrollLock,
        Pause,
        BrowserBack,
        BrowserFavorites,
        BrowserForward,
        BrowserHome,
        BrowserRefresh,
        BrowserSearch,
        BrowserStop,
        Eject,
        LaunchApp1,
        LaunchApp2,
        LaunchMail,
        MediaPlayPause,
        MediaSelect,
        MediaStop,
        MediaTrackNext,
        MediaTrackPrevious,
        Power,
        Sleep,
        AudioVolumeDown,
        AudioVolumeMute,
        AudioVolumeUp,
        WakeUp,
        Meta,
        Hyper,
        Turbo,
        Abort,
        Resume,
        Suspend,
        Again,
        Copy,
        Cut,
        Find,
        Open,
        Paste,
        Props,
        Select,
        Undo,
        Hiragana,
        Katakana,
        F1,
        F2,
        F3,
        F4,
        F5,
        F6,
        F7,
        F8,
        F9,
        F10,
        F11,
        F12,
        F13,
        F14,
        F15,
        F16,
        F17,
        F18,
        F19,
        F20,
        F21,
        F22,
        F23,
        F24,
        F25,
        F26,
        F27,
        F28,
        F29,
        F30,
        F31,
        F32,
        F33,
        F34,
        F35,
    );
    Some(name.to_string())
}

fn logical_key(key: &Key) -> String {
    match key {
        Key::Character(value) => value.to_string(),
        Key::Named(named) => named_key_name(*named).to_string(),
        Key::Dead(_) => "Dead".to_string(),
        Key::Unidentified(_) => "Unidentified".to_string(),
    }
}

fn named_key_name(key: NamedKey) -> &'static str {
    macro_rules! standard_key {
        ($($variant:ident),+ $(,)?) => {
            match key {
                $(NamedKey::$variant => stringify!($variant),)+
                NamedKey::Meta | NamedKey::Super => "Meta",
                NamedKey::Space => " ",
                _ => "Unidentified",
            }
        };
    }
    standard_key!(
        Alt,
        AltGraph,
        CapsLock,
        Control,
        Fn,
        FnLock,
        NumLock,
        ScrollLock,
        Shift,
        Symbol,
        SymbolLock,
        Hyper,
        Enter,
        Tab,
        ArrowDown,
        ArrowLeft,
        ArrowRight,
        ArrowUp,
        End,
        Home,
        PageDown,
        PageUp,
        Backspace,
        Clear,
        Copy,
        CrSel,
        Cut,
        Delete,
        EraseEof,
        ExSel,
        Insert,
        Paste,
        Redo,
        Undo,
        Accept,
        Again,
        Attn,
        Cancel,
        ContextMenu,
        Escape,
        Execute,
        Find,
        Help,
        Pause,
        Play,
        Props,
        Select,
        ZoomIn,
        ZoomOut,
        BrightnessDown,
        BrightnessUp,
        Eject,
        LogOff,
        Power,
        PowerOff,
        PrintScreen,
        Hibernate,
        Standby,
        WakeUp,
        AllCandidates,
        Alphanumeric,
        CodeInput,
        Compose,
        Convert,
        FinalMode,
        GroupFirst,
        GroupLast,
        GroupNext,
        GroupPrevious,
        ModeChange,
        NextCandidate,
        NonConvert,
        PreviousCandidate,
        Process,
        SingleCandidate,
        HangulMode,
        HanjaMode,
        JunjaMode,
        Eisu,
        Hankaku,
        Hiragana,
        HiraganaKatakana,
        KanaMode,
        KanjiMode,
        Katakana,
        Romaji,
        Zenkaku,
        ZenkakuHankaku,
        Soft1,
        Soft2,
        Soft3,
        Soft4,
        ChannelDown,
        ChannelUp,
        Close,
        MailForward,
        MailReply,
        MailSend,
        MediaClose,
        MediaFastForward,
        MediaPause,
        MediaPlay,
        MediaPlayPause,
        MediaRecord,
        MediaRewind,
        MediaStop,
        MediaTrackNext,
        MediaTrackPrevious,
        New,
        Open,
        Print,
        Save,
        SpellCheck,
        Key11,
        Key12,
        AudioBalanceLeft,
        AudioBalanceRight,
        AudioBassBoostDown,
        AudioBassBoostToggle,
        AudioBassBoostUp,
        AudioFaderFront,
        AudioFaderRear,
        AudioSurroundModeNext,
        AudioTrebleDown,
        AudioTrebleUp,
        AudioVolumeDown,
        AudioVolumeUp,
        AudioVolumeMute,
        MicrophoneToggle,
        MicrophoneVolumeDown,
        MicrophoneVolumeUp,
        MicrophoneVolumeMute,
        SpeechCorrectionList,
        SpeechInputToggle,
        LaunchApplication1,
        LaunchApplication2,
        LaunchCalendar,
        LaunchContacts,
        LaunchMail,
        LaunchMediaPlayer,
        LaunchMusicPlayer,
        LaunchPhone,
        LaunchScreenSaver,
        LaunchSpreadsheet,
        LaunchWebBrowser,
        LaunchWebCam,
        LaunchWordProcessor,
        BrowserBack,
        BrowserFavorites,
        BrowserForward,
        BrowserHome,
        BrowserRefresh,
        BrowserSearch,
        BrowserStop,
        AppSwitch,
        Call,
        Camera,
        CameraFocus,
        EndCall,
        GoBack,
        GoHome,
        HeadsetHook,
        LastNumberRedial,
        Notification,
        MannerMode,
        VoiceDial,
        TV,
        TV3DMode,
        TVAntennaCable,
        TVAudioDescription,
        TVAudioDescriptionMixDown,
        TVAudioDescriptionMixUp,
        TVContentsMenu,
        TVDataService,
        TVInput,
        TVInputComponent1,
        TVInputComponent2,
        TVInputComposite1,
        TVInputComposite2,
        TVInputHDMI1,
        TVInputHDMI2,
        TVInputHDMI3,
        TVInputHDMI4,
        TVInputVGA1,
        TVMediaContext,
        TVNetwork,
        TVNumberEntry,
        TVPower,
        TVRadioService,
        TVSatellite,
        TVSatelliteBS,
        TVSatelliteCS,
        TVSatelliteToggle,
        TVTerrestrialAnalog,
        TVTerrestrialDigital,
        TVTimer,
        AVRInput,
        AVRPower,
        ColorF0Red,
        ColorF1Green,
        ColorF2Yellow,
        ColorF3Blue,
        ColorF4Grey,
        ColorF5Brown,
        ClosedCaptionToggle,
        Dimmer,
        DisplaySwap,
        DVR,
        Exit,
        FavoriteClear0,
        FavoriteClear1,
        FavoriteClear2,
        FavoriteClear3,
        FavoriteRecall0,
        FavoriteRecall1,
        FavoriteRecall2,
        FavoriteRecall3,
        FavoriteStore0,
        FavoriteStore1,
        FavoriteStore2,
        FavoriteStore3,
        Guide,
        GuideNextDay,
        GuidePreviousDay,
        Info,
        InstantReplay,
        Link,
        ListProgram,
        LiveContent,
        Lock,
        MediaApps,
        MediaAudioTrack,
        MediaLast,
        MediaSkipBackward,
        MediaSkipForward,
        MediaStepBackward,
        MediaStepForward,
        MediaTopMenu,
        NavigateIn,
        NavigateNext,
        NavigateOut,
        NavigatePrevious,
        NextFavoriteChannel,
        NextUserProfile,
        OnDemand,
        Pairing,
        PinPDown,
        PinPMove,
        PinPToggle,
        PinPUp,
        PlaySpeedDown,
        PlaySpeedReset,
        PlaySpeedUp,
        RandomToggle,
        RcLowBattery,
        RecordSpeedNext,
        RfBypass,
        ScanChannelsToggle,
        ScreenModeNext,
        Settings,
        SplitScreenToggle,
        STBInput,
        STBPower,
        Subtitle,
        Teletext,
        VideoModeNext,
        Wink,
        ZoomToggle,
        F1,
        F2,
        F3,
        F4,
        F5,
        F6,
        F7,
        F8,
        F9,
        F10,
        F11,
        F12,
        F13,
        F14,
        F15,
        F16,
        F17,
        F18,
        F19,
        F20,
        F21,
        F22,
        F23,
        F24,
        F25,
        F26,
        F27,
        F28,
        F29,
        F30,
        F31,
        F32,
        F33,
        F34,
        F35,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_keyboard_names_preserve_physical_and_logical_meaning() {
        assert_eq!(
            physical_key_code(KeyCode::KeyW.into()).as_deref(),
            Some("KeyW")
        );
        assert_eq!(
            physical_key_code(KeyCode::ShiftLeft.into()).as_deref(),
            Some("ShiftLeft")
        );
        assert_eq!(logical_key(&Key::Character("W".into())), "W");
        assert_eq!(logical_key(&Key::Character("é".into())), "é");
        assert_eq!(logical_key(&Key::Named(NamedKey::ArrowUp)), "ArrowUp");
        assert_eq!(logical_key(&Key::Dead(None)), "Dead");
    }

    #[test]
    fn focus_loss_releases_held_keys_in_press_order() {
        let mut input = InputState::default();
        let w = Key::Character("w".into());
        input.keys_down.insert(w.clone());
        input.physical_keys_down.push((
            KeyCode::ShiftLeft.into(),
            Some("ShiftLeft".into()),
            "Shift".into(),
            Key::Named(NamedKey::Shift),
        ));
        input.physical_keys_down.push((
            KeyCode::KeyW.into(),
            Some("KeyW".into()),
            "W".into(),
            w.clone(),
        ));

        input.release_all_keys();

        assert!(input.keys_down.is_empty());
        assert!(input.keys_released.contains(&w));
        assert_eq!(input.keyboard_events[0].code.as_deref(), Some("ShiftLeft"));
        assert_eq!(input.keyboard_events[1].code.as_deref(), Some("KeyW"));
    }
}
