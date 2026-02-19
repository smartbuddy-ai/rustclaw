use ratatui::style::Color;

/// Color palette for left panel menu items — professional, distinct, accessible.
pub struct MenuColors;

impl MenuColors {
    pub const CHANNELS: Color = Color::Rgb(86, 156, 214);   // Soft blue
    pub const AGENTS: Color = Color::Rgb(181, 137, 214);     // Lavender purple
    pub const CRON: Color = Color::Rgb(78, 201, 176);        // Teal green
    pub const NODES: Color = Color::Rgb(220, 165, 80);       // Warm amber
    pub const WORKSPACE: Color = Color::Rgb(215, 130, 126);  // Soft coral
    pub const SETTINGS: Color = Color::Rgb(140, 170, 200);   // Steel blue

    pub const BG_PANEL: Color = Color::Rgb(30, 30, 36);      // Dark charcoal
    pub const BG_SELECTED: Color = Color::Rgb(45, 45, 55);   // Slightly lighter
    pub const BG_HOVER: Color = Color::Rgb(38, 38, 48);
    pub const FG_DIM: Color = Color::Rgb(120, 120, 135);     // Muted text
    pub const FG_TEXT: Color = Color::Rgb(200, 200, 210);     // Main text
    pub const FG_BRIGHT: Color = Color::Rgb(235, 235, 245);  // Bright text
    pub const BORDER: Color = Color::Rgb(55, 55, 65);        // Subtle border
    pub const ACCENT: Color = Color::Rgb(86, 156, 214);      // Blue accent
    pub const SUCCESS: Color = Color::Rgb(78, 201, 176);     // Green
    pub const WARNING: Color = Color::Rgb(220, 165, 80);     // Amber
    pub const ERROR: Color = Color::Rgb(215, 100, 100);      // Red
}
