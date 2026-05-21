//! Tunable constants for the kill-streak timer overlay.

use std::time::Duration;

/// Idle polling interval — used when not attached or not in combat.
pub const POLL_IDLE: Duration = Duration::from_millis(1000);

/// Combat polling interval — used when local player is alive and round is live.
pub const POLL_COMBAT: Duration = Duration::from_millis(20);

/// Timer polling interval — used while a streak timer is actively displayed.
pub const POLL_TIMER: Duration = Duration::from_millis(50);

/// Overlay UI tick (ms).
pub const OVERLAY_TICK_MS: u64 = 33;

/// Overlay base font size (pre supersample).
pub const OVERLAY_FONT_SIZE: u32 = 35;

/// Color shown briefly when the overlay is hidden / reset.
pub const OVERLAY_HIDDEN_RGB: (u8, u8, u8) = (220, 40, 40);

/// Timer color, low band.
pub const OVERLAY_TIMER_RGB_LOW: (u8, u8, u8) = (230, 30, 30);

/// Timer color, mid band.
pub const OVERLAY_TIMER_RGB_MID: (u8, u8, u8) = (255, 140, 0);

/// Timer color, high band.
pub const OVERLAY_TIMER_RGB_HIGH: (u8, u8, u8) = (50, 220, 90);

/// Fade-in duration (ms) when the timer value changes.
pub const OVERLAY_FADE_MS: u64 = 220;

/// Amount to lighten the band color for the gradient's top edge
/// (0.0 = unchanged, 1.0 = pure white).
pub const OVERLAY_GRADIENT_TOP_LIGHTEN: f32 = 0.65;

/// Amount to darken the band color for the gradient's bottom edge
/// (0.0 = unchanged, 1.0 = pure black).
pub const OVERLAY_GRADIENT_BOTTOM_DARKEN: f32 = 0.20;

/// Outline radius (px) around the rasterized text.
pub const OVERLAY_OUTLINE_RADIUS_PX: u32 = 2;

/// Outline color (RGB) — slight blue-black for contrast over most backgrounds.
pub const OVERLAY_OUTLINE_RGB: (u8, u8, u8) = (12, 12, 18);
