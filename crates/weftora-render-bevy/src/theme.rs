//! Adapter defaults shared by authored widgets and the fallback player.
use bevy::prelude::*;

pub const PANEL: Color = Color::srgba(0.035, 0.055, 0.09, 0.96);
pub const BUTTON: Color = Color::srgb(0.10, 0.15, 0.22);
pub const ACCENT: Color = Color::srgb(0.95, 0.75, 0.43);
pub const BORDER: Color = Color::srgb(0.24, 0.31, 0.40);
pub const SUCCESS: Color = Color::srgb(0.51, 0.85, 0.68);
pub const MUTED: Color = Color::srgb(0.64, 0.71, 0.80);
pub const TEXT: Color = Color::srgb(0.94, 0.96, 0.98);

pub fn button_node() -> Node {
    Node {
        min_height: px(44),
        min_width: px(44),
        padding: UiRect::axes(px(18), px(10)),
        border: UiRect::all(px(1)),
        border_radius: BorderRadius::all(px(8)),
        align_items: AlignItems::Center,
        flex_shrink: 0.0,
        ..default()
    }
}
