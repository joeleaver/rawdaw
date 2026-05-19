//! Modulation matrix types — slot-based routing of modulation
//! sources (envelopes, LFOs, oscillator audio, MIDI CCs) to
//! destinations (filter parameters, per-osc tune/level/PM/AM/RM
//! amounts, …).
//!
//! The matrix is the v2 replacement for v1's hardcoded LFO→cutoff
//! routing and per-osc `mod_source` / `mod_mode` / `mod_amount`
//! fields. Patches own a fixed-size `[ModSlot; N]` array; the
//! voice's per-sample tick evaluates active slots into a
//! [`Modulations`] bag that filter / per-osc render consult.
//!
//! See `docs/wavetable-synth-mod-matrix-plan.md` for the design
//! rationale (PM-not-FM phase semantics, topo-sort-and-reject cycle
//! handling, per-destination scale units, etc.).
//!
//! Split into two submodules to keep each under the workspace
//! ~700-line cap:
//!
//! - [`types`]: the data types — [`ModSource`], [`ModDestination`],
//!   [`ModSlot`], [`Modulations`], the per-destination [`scale`]
//!   constants.
//! - [`engine`]: the [`ModMatrix`] container plus the validation /
//!   topological-sort passes that install slot configurations
//!   safely.

mod engine;
mod types;

pub use engine::ModMatrix;
pub use types::{ModDestination, ModSlot, ModSource, Modulations, NUM_OSCS_PER_VOICE};
