/// Minimal ANSI color helpers for the REPL.
///
/// This module intentionally keeps formatting simple and self-contained.

pub const RESET: &str = "\x1b[0m";

pub fn fg_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{};{};{}m", r, g, b)
}

pub fn bg_rgb(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[48;2;{};{};{}m", r, g, b)
}

pub fn paint(text: impl AsRef<str>, fg: Option<(u8, u8, u8)>, bg: Option<(u8, u8, u8)>) -> String {
    let mut out = String::new();
    if let Some((r, g, b)) = fg {
        out.push_str(&fg_rgb(r, g, b));
    }
    if let Some((r, g, b)) = bg {
        out.push_str(&bg_rgb(r, g, b));
    }
    out.push_str(text.as_ref());
    out.push_str(RESET);
    out
}

pub fn paint_fg(text: impl AsRef<str>, fg: (u8, u8, u8)) -> String {
    paint(text, Some(fg), None)
}

pub fn paint_bg(text: impl AsRef<str>, bg: (u8, u8, u8)) -> String {
    paint(text, None, Some(bg))
}

pub fn scale_rgb((r, g, b): (u8, u8, u8), factor: f32) -> (u8, u8, u8) {
    fn scale_u8(v: u8, factor: f32) -> u8 {
        let scaled = (f32::from(v) * factor).round();
        scaled.clamp(0.0, 255.0) as u8
    }

    (scale_u8(r, factor), scale_u8(g, factor), scale_u8(b, factor))
}
