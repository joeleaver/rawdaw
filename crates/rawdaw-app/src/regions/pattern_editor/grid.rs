//! Grid-snap primitives shared between the pitched and drum
//! pattern editors.
//!
//! `GridSpec` is the editor-local note-snap resolution
//! (per P2 design decision 6) — transient, not serialized into
//! the project. Both editors carry their own `Signal<GridSpec>`
//! sourced from the editor header; the model never observes it.
//!
//! Snapping is *floor-to-grid*: a click between two grid lines
//! drops onto the earlier one. That matches typical DAW behavior
//! and keeps drag interactions predictable (the cell the cursor
//! is over is the cell that receives the event).

use rawdaw_model::time::{MusicalTime, PPQ};

/// Editor-local snap resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GridSpec {
    /// `4` = quarter, `8` = eighth, `16` = sixteenth, `32` =
    /// thirty-second. Must be >= 1; values that don't divide PPQ
    /// cleanly are still well-defined (we use integer division).
    pub subdivision: u32,
    /// When `true`, each subdivision is treated as a triplet — the
    /// resulting step is two-thirds of the straight subdivision.
    pub triplet: bool,
}

impl GridSpec {
    pub const STRAIGHT_SIXTEENTH: Self = Self {
        subdivision: 16,
        triplet: false,
    };

    /// Tick length of one snap step. Computed as
    /// `(PPQ * 4) / subdivision`, optionally multiplied by 2/3 for
    /// triplet mode. Returns at least 1 tick so callers can safely
    /// use the result as a divisor.
    pub fn step_ticks(self) -> i64 {
        let denom = self.subdivision.max(1) as i64;
        let straight = (PPQ * 4) / denom;
        let scaled = if self.triplet {
            (straight * 2) / 3
        } else {
            straight
        };
        scaled.max(1)
    }
}

impl Default for GridSpec {
    fn default() -> Self {
        Self::STRAIGHT_SIXTEENTH
    }
}

/// Snap a `MusicalTime` down to the nearest grid step. Floors at 0 so
/// a tiny negative offset doesn't wrap.
pub fn snap_time_to_grid(time: MusicalTime, grid: GridSpec) -> MusicalTime {
    let step = grid.step_ticks();
    let ticks = time.as_ticks();
    if ticks <= 0 {
        return MusicalTime::ZERO;
    }
    MusicalTime::ticks((ticks / step) * step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_ticks_straight_sixteenth_is_pq4_over_16() {
        // PPQ = 960; 1/16 straight = 960*4/16 = 240 ticks.
        assert_eq!(GridSpec::STRAIGHT_SIXTEENTH.step_ticks(), 240);
    }

    #[test]
    fn step_ticks_quarter_is_full_ppq() {
        let g = GridSpec {
            subdivision: 4,
            triplet: false,
        };
        assert_eq!(g.step_ticks(), PPQ);
    }

    #[test]
    fn step_ticks_triplet_is_two_thirds_of_straight() {
        // Quarter-note triplet: 960 * 2 / 3 = 640 ticks (three of
        // these fit into half a bar of 4/4).
        let g = GridSpec {
            subdivision: 4,
            triplet: true,
        };
        assert_eq!(g.step_ticks(), 640);
        // Eighth-note triplet: 480 * 2 / 3 = 320 ticks.
        let g = GridSpec {
            subdivision: 8,
            triplet: true,
        };
        assert_eq!(g.step_ticks(), 320);
    }

    #[test]
    fn step_ticks_min_clamps_to_one() {
        // Defensive: even a wildly fine subdivision shouldn't divide
        // by zero or hand back a non-positive step.
        let g = GridSpec {
            subdivision: u32::MAX,
            triplet: false,
        };
        assert!(g.step_ticks() >= 1);
    }

    #[test]
    fn snap_floors_to_grid() {
        let g = GridSpec::STRAIGHT_SIXTEENTH;
        assert_eq!(snap_time_to_grid(MusicalTime::ticks(0), g), MusicalTime::ticks(0));
        assert_eq!(snap_time_to_grid(MusicalTime::ticks(239), g), MusicalTime::ticks(0));
        assert_eq!(snap_time_to_grid(MusicalTime::ticks(240), g), MusicalTime::ticks(240));
        assert_eq!(snap_time_to_grid(MusicalTime::ticks(241), g), MusicalTime::ticks(240));
        assert_eq!(snap_time_to_grid(MusicalTime::ticks(480), g), MusicalTime::ticks(480));
    }

    #[test]
    fn snap_floors_negative_to_zero() {
        let g = GridSpec::STRAIGHT_SIXTEENTH;
        assert_eq!(snap_time_to_grid(MusicalTime::ticks(-100), g), MusicalTime::ZERO);
    }
}
