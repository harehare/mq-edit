use ratatui::style::Color;

// ── Tarn color palette ───────────────────────────────────────────────────────

pub const BG: Color = Color::Rgb(30, 41, 59);
pub const BG_PANEL: Color = Color::Rgb(42, 52, 68);
pub const BG_DARK: Color = Color::Rgb(35, 46, 61);
pub const BG_SEL: Color = Color::Rgb(58, 79, 100);

pub const BORDER: Color = Color::Rgb(74, 85, 104);

pub const FG: Color = Color::Rgb(226, 232, 240);
pub const FG_MUTED: Color = Color::Rgb(148, 163, 184);
pub const FG_DIM: Color = Color::Rgb(107, 122, 144);

pub const ACCENT: Color = Color::Rgb(103, 184, 227);
pub const ACCENT_HI: Color = Color::Rgb(133, 212, 255);

pub const FUNC: Color = Color::Rgb(86, 212, 212);
pub const STRING: Color = Color::Rgb(137, 221, 255);
pub const NUMBER: Color = Color::Rgb(222, 147, 95);
pub const VARIABLE: Color = Color::Rgb(156, 220, 254);
pub const ESCAPE: Color = Color::Rgb(103, 232, 249);
pub const OPERATOR: Color = Color::Rgb(163, 178, 198);
pub const COMMENT: Color = Color::Rgb(126, 143, 166);
pub const CONSTANT: Color = Color::Rgb(133, 212, 255);

pub const SUCCESS: Color = Color::Rgb(104, 211, 145);
pub const WARNING: Color = Color::Rgb(246, 173, 85);
pub const ERROR: Color = Color::Rgb(252, 129, 129);

// ── Heading background colors ────────────────────────────────────────────────

pub const H1_BG: Color = Color::Rgb(28, 55, 78);
pub const H2_BG: Color = Color::Rgb(25, 50, 74);
pub const H3_BG: Color = Color::Rgb(36, 44, 68);
pub const H4_BG: Color = Color::Rgb(40, 46, 62);
pub const H_OTHER_BG: Color = BG_PANEL;
