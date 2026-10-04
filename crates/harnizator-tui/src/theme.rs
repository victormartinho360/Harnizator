//! Theme system for Harnizator TUI.
//!
//! Supports predefined themes (gruvbox, dark, light) and full customization
//! via hex color values with a color picker interface.

use ratatui::style::Color;

/// A color palette for the TUI theme.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    /// Terminal background color
    pub background: Color,
    /// Primary text color
    pub foreground: Color,
    /// Selection highlight background
    pub selection: Color,
    /// Accent color for highlights
    pub accent: Color,
    /// Card/modal background
    pub card: Color,
    /// Border/divider color
    pub border: Color,
    /// Graph node colors by status
    pub graph_node: GraphNodeColors,
    /// Provider colors
    pub provider: ProviderColors,
}

/// Colors for graph nodes based on their status.
#[derive(Debug, Clone, PartialEq)]
pub struct GraphNodeColors {
    /// Running node color
    pub running: Color,
    /// Queued node color
    pub queued: Color,
    /// Idle node color
    pub idle: Color,
    /// Done node color
    pub done: Color,
    /// Failed node color
    pub failed: Color,
    /// Interrupted node color
    pub interrupted: Color,
}

/// Colors for providers.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderColors {
    /// Background color for provider items
    pub background: Color,
    /// Foreground color for provider text
    pub foreground: Color,
    /// Selected provider color
    pub selected: Color,
    /// Unselected provider color
    pub unselected: Color,
}

/// A complete theme with name and palette.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Unique theme name
    pub name: String,
    /// Color palette
    pub palette: Palette,
}

impl Theme {
    /// Create a new theme with the given name and palette.
    pub fn new(name: &str, palette: Palette) -> Self {
        Theme {
            name: name.to_string(),
            palette,
        }
    }

    /// Convert a ratatui Color to an RGB tuple.
    pub fn rgb(color: Color) -> (u8, u8, u8) {
        match color {
            Color::Rgb(r, g, b) => (r, g, b),
            Color::Indexed(idx) => {
                // Map indexed color to RGB (rough approximation for 256-color mode)
                let c = 16 + idx * 10;
                ((c % 36) * 25 / 33, ((c / 36) % 6) * 25 / 5, (c / 216) * 25 / 5)
            }
            Color::DarkGray => (64, 64, 64),
            Color::Gray => (128, 128, 128),
            Color::LightGray => (196, 196, 196),
            Color::White => (255, 255, 255),
            Color::Black => (0, 0, 0),
            Color::Red => (255, 0, 0),
            Color::Green => (0, 255, 0),
            Color::Yellow => (255, 255, 0),
            Color::Blue => (0, 0, 255),
            Color::Magenta => (255, 0, 255),
            Color::Cyan => (0, 255, 255),
            Color::DarkRed => (128, 0, 0),
            Color::DarkGreen => (0, 128, 0),
            Color::DarkYellow => (128, 128, 0),
            Color::DarkBlue => (0, 0, 128),
            Color::DarkMagenta => (128, 0, 128),
            Color::DarkCyan => (0, 128, 128),
        }
    }

    /// Convert an RGB tuple to a ratatui Color (Truecolor).
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Color {
        Color::Rgb(r, g, b)
    }
}

/// Predefined theme color strings (inline definitions, no external files needed).
/// Gruvbox theme colors as JSON string.
pub const GRUVOX_THEME: &str = "{}";
/// Dark theme colors as JSON string.
pub const DARK_THEME: &str = "{}";
/// Light theme colors as JSON string.
pub const LIGHT_THEME: &str = "{}";

// ============================================================================
// Predefined Themes
// ============================================================================

/// Gruvbox theme - warm brown/orange palette based on the popular gruvbox colorscheme.
pub const GRUVOX: Theme = Theme {
    name: "Gruvbox",
    palette: Palette {
        background: Theme::from_rgb(20, 19, 17),
        foreground: Theme::from_rgb(223, 213, 195),
        selection: Theme::from_rgb(152, 128, 80),
        accent: Theme::from_rgb(255, 185, 50),
        card: Theme::from_rgb(25, 24, 22),
        border: Theme::from_rgb(181, 157, 125),
        graph_node: GraphNodeColors {
            running: Theme::from_rgb(124, 255, 0),
            queued: Theme::from_rgb(181, 157, 125),
            idle: Theme::from_rgb(173, 157, 135),
            done: Theme::from_rgb(109, 183, 127),
            failed: Theme::from_rgb(255, 0, 0),
            interrupted: Theme::from_rgb(255, 255, 0),
        },
        provider: ProviderColors {
            background: Theme::from_rgb(25, 24, 22),
            foreground: Theme::from_rgb(223, 213, 195),
            selected: Theme::from_rgb(255, 185, 50),
            unselected: Theme::from_rgb(142, 127, 104),
        },
    },
};

/// Dark theme - dark background with blue accents.
pub const DARK: Theme = Theme {
    name: "Dark",
    palette: Palette {
        background: Theme::from_rgb(15, 15, 20),
        foreground: Theme::from_rgb(220, 220, 240),
        selection: Theme::from_rgb(50, 100, 150),
        accent: Theme::from_rgb(100, 200, 255),
        card: Theme::from_rgb(20, 20, 30),
        border: Theme::from_rgb(100, 100, 150),
        graph_node: GraphNodeColors {
            running: Theme::from_rgb(0, 255, 150),
            queued: Theme::from_rgb(150, 150, 200),
            idle: Theme::from_rgb(180, 180, 220),
            done: Theme::from_rgb(50, 200, 150),
            failed: Theme::from_rgb(255, 50, 50),
            interrupted: Theme::from_rgb(255, 150, 50),
        },
        provider: ProviderColors {
            background: Theme::from_rgb(20, 20, 30),
            foreground: Theme::from_rgb(220, 220, 240),
            selected: Theme::from_rgb(100, 200, 255),
            unselected: Theme::from_rgb(130, 130, 180),
        },
    },
};

/// Light theme - light background with high contrast.
pub const LIGHT: Theme = Theme {
    name: "Light",
    palette: Palette {
        background: Theme::from_rgb(245, 245, 250),
        foreground: Theme::from_rgb(30, 30, 40),
        selection: Theme::from_rgb(180, 180, 230),
        accent: Theme::from_rgb(50, 120, 200),
        card: Theme::from_rgb(255, 255, 255),
        border: Theme::from_rgb(200, 200, 220),
        graph_node: GraphNodeColors {
            running: Theme::from_rgb(0, 180, 0),
            queued: Theme::from_rgb(150, 150, 200),
            idle: Theme::from_rgb(180, 180, 220),
            done: Theme::from_rgb(0, 100, 200),
            failed: Theme::from_rgb(200, 0, 0),
            interrupted: Theme::from_rgb(200, 150, 0),
        },
        provider: ProviderColors {
            background: Theme::from_rgb(255, 255, 255),
            foreground: Theme::from_rgb(30, 30, 40),
            selected: Theme::from_rgb(50, 120, 200),
            unselected: Theme::from_rgb(150, 150, 180),
        },
    },
};

// ============================================================================
// Standard Colors (for color picker)
// ============================================================================

/// Standard colors available in the color picker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandardColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Default,
}

impl StandardColor {
    /// Get the ratatui Color for this standard color.
    pub fn color(self) -> Color {
        match self {
            StandardColor::Black => Color::Black,
            StandardColor::Red => Color::Red,
            StandardColor::Green => Color::Green,
            StandardColor::Yellow => Color::Yellow,
            StandardColor::Blue => Color::Blue,
            StandardColor::Magenta => Color::Magenta,
            StandardColor::Cyan => Color::Cyan,
            StandardColor::White => Color::White,
            StandardColor::Default => Color::Default,
        }
    }

    /// Get the name of this standard color.
    pub fn name(self) -> &'static str {
        match self {
            StandardColor::Black => "Black",
            StandardColor::Red => "Red",
            StandardColor::Green => "Green",
            StandardColor::Yellow => "Yellow",
            StandardColor::Blue => "Blue",
            StandardColor::Magenta => "Magenta",
            StandardColor::Cyan => "Cyan",
            StandardColor::White => "White",
            StandardColor::Default => "Default",
        }
    }
}

/// Convert a hex string to a ratatui Color.
/// Supports formats: "#RRGGBB", "#RGB", "RRGGBB"
pub fn hex_to_color(hex: &str) -> Color {
    let hex = hex.trim();
    let hex = if hex.starts_with('#') {
        &hex[1..]
    } else {
        hex
    };

    let len = hex.len();
    let r = if len >= 2 {
        u8::from_str_radix(&hex[0..2], 16).unwrap_or(0)
    } else {
        0
    };
    let g = if len >= 4 {
        u8::from_str_radix(&hex[2..4], 16).unwrap_or(0)
    } else {
        0
    };
    let b = if len >= 6 {
        u8::from_str_radix(&hex[4..6], 16).unwrap_or(0)
    } else if len == 3 {
        // Expand #RGB to #RRGGBB
        let r_nibble = u8::from_str_radix(&hex[0..1], 16).unwrap_or(0);
        let g_nibble = u8::from_str_radix(&hex[1..2], 16).unwrap_or(0);
        let b_nibble = u8::from_str_radix(&hex[2..3], 16).unwrap_or(0);
        u8::from_str_radix(&format!("{}{}{}{}{}{}", r_nibble, r_nibble, g_nibble, g_nibble, b_nibble, b_nibble), 16).unwrap_or(0)
    } else {
        0
    };

    Color::Rgb(r, g, b)
}

/// Convert a ratatui Color to a hex string (format "#RRGGBB").
pub fn color_to_hex(color: Color) -> String {
    let (r, g, b) = Theme::rgb(color);
    format!("#{:02X}{:02X}{:02X}", r, g, b)
}

/// Get the standard colors for the color picker.
pub fn standard_colors() -> Vec<StandardColor> {
    vec![
        StandardColor::Black,
        StandardColor::Red,
        StandardColor::Green,
        StandardColor::Yellow,
        StandardColor::Blue,
        StandardColor::Magenta,
        StandardColor::Cyan,
        StandardColor::White,
    ]
}

/// Get the default (predefined) themes.
pub fn default_themes() -> Vec<Theme> {
    vec![GRUVOX, DARK, LIGHT]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_to_color() {
        // Test #RRGGBB format
        let color = hex_to_color("#ff0000");
        assert_eq!(Theme::rgb(color), (255, 0, 0));

        // Test #RGB format
        let color = hex_to_color("#f00");
        assert_eq!(Theme::rgb(color), (255, 0, 0));

        // Test without #
        let color = hex_to_color("00ff00");
        assert_eq!(Theme::rgb(color), (0, 255, 0));
    }

    #[test]
    fn test_color_to_hex() {
        let color = Theme::from_rgb(255, 185, 50);
        let hex = color_to_hex(color);
        assert_eq!(hex, "#FFB932");
    }

    #[test]
    fn test_standard_colors() {
        let colors = standard_colors();
        assert_eq!(colors.len(), 8);
        assert!(colors.contains(&StandardColor::Red));
        assert!(colors.contains(&StandardColor::Blue));
    }

    #[test]
    fn test_default_themes() {
        let themes = default_themes();
        assert_eq!(themes.len(), 3);
        assert_eq!(themes[0].name, "Gruvbox");
        assert_eq!(themes[1].name, "Dark");
        assert_eq!(themes[2].name, "Light");
    }

    #[test]
    fn test_gruvbox_theme_palette() {
        let palette = GRUVOX.palette.background;
        let (r, g, b) = Theme::rgb(palette);
        // Gruvbox background should be dark
        assert!(r < 50 && g < 50 && b < 50);
    }

    #[test]
    fn test_dark_theme_palette() {
        let palette = DARK.palette.background;
        let (r, g, b) = Theme::rgb(palette);
        // Dark theme background should be dark
        assert!(r < 40 && g < 40 && b < 40);
    }

    #[test]
    fn test_light_theme_palette() {
        let palette = LIGHT.palette.background;
        let (r, g, b) = Theme::rgb(palette);
        // Light theme background should be light
        assert!(r > 200 && g > 200 && b > 200);
    }
}