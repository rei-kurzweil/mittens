use crate::meow_meow::ast::Span;

// -----------------------------------------------------------------------------
// Lexical tokens
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    String(String),
    Number(f64),

    Let,
    If,
    Else,
    Return,
    True,
    False,
    Null,

    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,

    Comma,
    Dot,
    Eq,
    Semicolon,

    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    EqEq,
    BangEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    AmpAmp,
    PipePipe,
    Bang,
    Fn,
    For,
    In,
    Break,
    Continue,

    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenizeError {
    pub message: String,
    pub span: Span,
}

// -----------------------------------------------------------------------------
// Component type shortforms
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentShortformEntry {
    pub short: &'static str,
    pub full: &'static str,
}

/// Mapping of short component identifiers (used in `.mms`) to the canonical
/// component type name that the host registry resolves.
///
/// Notes:
/// - This list is intentionally curated (not auto-derived) so we can keep it
///   stable and ergonomic.
/// - Shortforms are case-sensitive.
pub const COMPONENT_SHORTFORMS: &[ComponentShortformEntry] = &[
    // User-provided core set
    ComponentShortformEntry { short: "I", full: "Input" },
    ComponentShortformEntry { short: "T", full: "Transform" },
    ComponentShortformEntry { short: "R", full: "Renderable" },
    ComponentShortformEntry { short: "C", full: "Color" },
    ComponentShortformEntry { short: "RC", full: "Raycast" },
    ComponentShortformEntry { short: "A", full: "Animation" },
    ComponentShortformEntry { short: "KF", full: "Keyframe" },
    ComponentShortformEntry { short: "AC", full: "Action" },
    ComponentShortformEntry { short: "BG", full: "Background" },
    ComponentShortformEntry { short: "OV", full: "Overlay" },
    ComponentShortformEntry { short: "OP", full: "Opacity" },
    ComponentShortformEntry { short: "BGC", full: "BackgroundColor" },
    ComponentShortformEntry { short: "TXT", full: "Text" },
    ComponentShortformEntry { short: "C3D", full: "Camera3D" },
    ComponentShortformEntry { short: "C2D", full: "Camera2D" },
    ComponentShortformEntry { short: "PL", full: "PointLight" },
    ComponentShortformEntry { short: "DL", full: "DirectionalLight" },
    ComponentShortformEntry { short: "AL", full: "AmbientLight" },
    ComponentShortformEntry { short: "ED", full: "Editor" },
    ComponentShortformEntry { short: "GZM", full: "Gizmo" },

    // Proposed additions based on existing engine components
    ComponentShortformEntry { short: "GLTF", full: "GLTF" },
    ComponentShortformEntry { short: "UV", full: "UV" },
    ComponentShortformEntry { short: "EM", full: "Emissive" },
    ComponentShortformEntry { short: "CK", full: "Clock" },
    ComponentShortformEntry { short: "PTR", full: "Pointer" },
    ComponentShortformEntry { short: "COL", full: "Collision" },
    ComponentShortformEntry { short: "COLS", full: "CollisionShape" },
    ComponentShortformEntry { short: "GVT", full: "Gravity" },
    ComponentShortformEntry { short: "KIN", full: "KineticResponse" },
    ComponentShortformEntry { short: "LQ", full: "LightQuantization" },
    ComponentShortformEntry { short: "TC", full: "TransparentCutout" },
    ComponentShortformEntry { short: "SM", full: "SkinnedMesh" },
    ComponentShortformEntry { short: "XR", full: "OpenXR" },
    ComponentShortformEntry { short: "CXR", full: "CameraXR" },
    ComponentShortformEntry { short: "CTLXR", full: "ControllerXR" },
    ComponentShortformEntry { short: "AVC", full: "AvatarControl" },
    ComponentShortformEntry { short: "MESH", full: "Mesh" },

    // Audio graph-ish components (optional; names kept explicit)
    ComponentShortformEntry { short: "AOUT", full: "AudioOutput" },
    ComponentShortformEntry { short: "AOSC", full: "AudioOscillator" },
    ComponentShortformEntry { short: "AG", full: "AudioGain" },
    ComponentShortformEntry { short: "AMIX", full: "AudioMix" },
    ComponentShortformEntry { short: "ALIM", full: "AudioLimiter" },
    ComponentShortformEntry { short: "ABUF", full: "AudioBufferSize" },
    ComponentShortformEntry { short: "ALPF", full: "AudioLowPassFilter" },
    ComponentShortformEntry { short: "AHPF", full: "AudioHighPassFilter" },
    ComponentShortformEntry { short: "ABPF", full: "AudioBandPassFilter" },

    // Routing
    ComponentShortformEntry { short: "SRU", full: "SignalRouteUpward" },

    // Music
    ComponentShortformEntry { short: "NOTE", full: "MusicNote" },

];

pub fn expand_component_shortform(ident: &str) -> Option<&'static str> {
    COMPONENT_SHORTFORMS
        .iter()
        .find(|e| e.short == ident)
        .map(|e| e.full)
}

pub fn shortform_for_component(full: &str) -> Option<&'static str> {
    COMPONENT_SHORTFORMS
        .iter()
        .find(|e| e.full == full)
        .map(|e| e.short)
}
