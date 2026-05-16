//! Design tokens for the rawdaw UI.
//!
//! Mirrors `docs/design/mockups/round-1/components/data.js`. Source of
//! truth is `docs/design/ui-principles.md` — the principles dictate the
//! palette logic (object identity color is durable; status never uses
//! identity colors; chrome recedes).
//!
//! The tokens here are plain Rust constants for round 1. When Rinch's
//! theme provider becomes load-bearing (e.g. for runtime light/dark
//! switching), we'll move these into a `Theme::builder()` configuration.

#![allow(dead_code)] // tokens accrete; not every one is consumed in round 1

// ─── Surfaces (low → high) ─────────────────────────────────────────────────

/// Window base. Near-black gray, not `#000` — elevated surfaces gain
/// contrast through subtle lighter tints.
pub const BG0: &str = "#0F1115";
/// Panes (top bar, library, inspector, ribbon background).
pub const BG1: &str = "#15181E";
/// Elevated cards, hover lift.
pub const BG2: &str = "#1B1F26";
/// Pressed / selected backdrop.
pub const BG3: &str = "#22262F";
/// 1 px chrome / pane separators.
pub const LINE: &str = "#262B34";
/// Bar guide lines inside section blocks; very faint.
pub const LINE_SOFT: &str = "#1E222A";

// ─── Text shades ───────────────────────────────────────────────────────────

pub const TEXT0: &str = "rgba(232,234,238,0.96)";
pub const TEXT1: &str = "rgba(232,234,238,0.62)";
pub const TEXT2: &str = "rgba(232,234,238,0.42)";
pub const TEXT3: &str = "rgba(232,234,238,0.28)";

// ─── Status palette (status ≠ identity color) ──────────────────────────────

/// Single warm neutral used for focus rings and the playhead. Quieter
/// than any identity color so the playhead reads as "live" not "loud".
pub const ACCENT: &str = "#D5C8A6";
/// Record dot, errors. Never reused for object identity.
pub const DANGER: &str = "#C76A6A";
/// Play arrow / "active" pill foreground. Status, not identity.
pub const OK: &str = "#7FA88A";

// ─── Typography ────────────────────────────────────────────────────────────

pub const FONT_SANS: &str =
    "\"Inter Tight\",\"Inter Tight Fallback\",ui-sans-serif,system-ui,sans-serif";
/// Same family; we just keep a separate token because v2 may swap in a
/// monospaced or feature-rich numeric font.
pub const FONT_NUM: &str = FONT_SANS;

// ─── Region heights (px) ───────────────────────────────────────────────────

pub const H_TOPBAR: u32 = 48;
pub const H_RULER: u32 = 26;
pub const H_RIBBON: u32 = 46;
pub const H_LANE: u32 = 132;
pub const H_DETAIL: u32 = 32;

// ─── Identity palette (earthy / muted) ─────────────────────────────────────
//
// ~10 hues, moderate saturation. Production: assigned deterministically
// by name hash. Fixture: hand-picked to match the round-1 mockup.
//
// Never used for status. Used as a left stripe on section blocks, a
// swatch in the library, and a chip in chord-ribbon cells derived from a
// named chord loop.

pub const PAL_BLUE: &str = "#7C9EC2";
pub const PAL_TERRA: &str = "#B58A6B";
pub const PAL_SAGE: &str = "#8AA876";
pub const PAL_ROSE: &str = "#B5848F";
pub const PAL_OLIVE: &str = "#A89A6B";
pub const PAL_PLUM: &str = "#9C84B5";
pub const PAL_TEAL: &str = "#6FA89E";
pub const PAL_SAND: &str = "#C9A88E";
pub const PAL_SLATE: &str = "#8090A0";
pub const PAL_CLAY: &str = "#B07A6F";
