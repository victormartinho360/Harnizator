# Theme Development Plan

## Goal
Implement a fully customizable theme system for the Harnizator TUI application with:
- Predefined themes: gruvbox, dark, light
- Full customization via hex color values
- Color picker with standard colors
- 3-column theme configuration screen
- Dynamic thumbnail preview of graph and provider screens

## Success Criteria
1. Theme module with structured color palettes
2. 3-column UI screen for theme configuration
3. Color picker functionality with hex input and standard colors
4. Dynamic thumbnails that render graph/provider screens with applied theme
5. TDD tests covering theme functionality
6. Theme switching integrated into AppState and UI rendering

## Implementation Plan

### 1. Theme Data Structure (`crates/harnizator-tui/src/theme.rs`)
- Define `Theme` struct with color palette:
  - `background`: Terminal background color
  - `foreground`: Primary text color
  - `selection`: Selection background highlight
  - `accent`: Accent color for highlights
  - `card`: Card/modal background
  - `border`: Border colors
  - `graph_node`: Node colors by status
  - `provider`: Provider colors
- Implement predefined themes:
  - `gruvbox`: Warm brown/orange palette
  - `dark`: Dark theme with blue accents
  - `light`: Light theme with high contrast
- Support hex color parsing and conversion to `ratatui::Color`
- Theme loading/saving configuration

### 2. Theme Screen - 3 Column Layout
Create new screen `Screen::Themes` with layout:

**Column 1 - Theme List** (width ~25%):
- Scrollable list of themes
- First option: "New Theme" (always at top)
- Default themes: Gruvbox, Dark, Light
- Custom themes stored in config
- Click/select to choose theme

**Column 2 - Color Editor** (width ~40%):
- Display current theme's color palette
- Hex input fields for each color
- Color preview squares
- Color picker button that opens standardized color selector
- Standard colors palette: red, green, blue, yellow, magenta, cyan, white, black
- Real-time preview of color changes

**Column 3 - Thumbnail Preview** (width ~35%):
- Miniature graph screen render applying current theme
- Miniature provider screen render applying current theme
- Dynamic based on actual TUI components
- Show sample graph nodes with status colors
- Show sample provider list with theme colors
- Update live as theme colors change

### 3. Color Picker Component
- Floating modal or inline picker
- Grid of standard color swatches
- Hex input field with validation
- Copy to clipboard functionality
- Recent/used colors tracking

### 4. Thumbnail Generation
- Render small graph area using `GraphLayout` from `graph.rs`
- Render small provider area using existing provider rendering
- Apply theme colors via Style modifications
- Capture as text representation for display
- Update efficiently without full re-render

### 5. TDD Tests
- Test theme creation and default themes
- Test hex color parsing and conversion
- Test theme switching preserves state
- Test color editor inputs validation
- Test thumbnail generation correctness
- Property-based tests for theme color combinations

### 6. Integration with AppState
- Add `theme: Theme` field to `AppState`
- Theme switch action in `handle_key`
- UI rendering uses current theme colors
- State persistence for custom themes

## Edge Cases
- Invalid hex colors (handle gracefully, revert to last valid)
- Color contrast accessibility (minimum contrast ratio)
- Terminal color limit support (256 colors + truecolor)
- Large terminal windows vs small (thumbnail scaling)
- Theme persistence across sessions
- Default themes cannot be deleted, only modified

## Assumptions
- Using ratatui for TUI rendering
- Terminal supports truecolor (24-bit RGB)
- Theme config stored as JSON or TOML
- Color picker uses standard 16+ colors

## Public API Changes
- `Theme` struct and constructors
- `ThemeScreen` enum/action handling
- `theme_switcher` function for key bindings
- `render_theme_thumbnail` utility function

## Data Flow
1. User selects "New Theme" or existing theme in Column 1
2. Column 2 populates with theme's current colors
3. User modifies hex values or clicks color picker
4. Column 3 updates thumbnail preview in real-time
5. "Apply" button saves theme and switches application theme
6. All screens update to use new theme colors