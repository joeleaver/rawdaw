//! Slot-based modulation matrix.

/// Number of oscillators per voice this matrix understands. Aligned
/// with `rawdaw-synth-wavetable::NUM_OSCS` (= 3); the matrix carries
/// per-osc destination arrays of this width.
///
/// This couples the matrix to a fixed-osc-count assumption — the
/// alternative (generic over osc count) propagates const generics
/// into every downstream type. v2 takes the coupling; v3 can lift
/// it if multi-osc-count synths arrive.
pub const NUM_OSCS_PER_VOICE: usize = 3;

/// Where a modulation value originates.
///
/// Sources output normalized values: envelopes in `[0.0, 1.0]`,
/// LFOs and oscillators in `[-1.0, 1.0]`. The matrix multiplies a
/// source's value by the slot's `amount`; the destination applies
/// its native scale (see [`ModDestination`] documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModSource {
    /// Inactive slot — the matrix skips it.
    #[default]
    None,
    /// Amp envelope (ENV1).
    Env1,
    /// Free modulation envelope 2 (ENV2).
    Env2,
    /// Free modulation envelope 3 (ENV3).
    Env3,
    /// Shared global LFO. v2 supports one LFO; Vital has four.
    Lfo1,
    /// Audio-rate output of oscillator 0.
    Osc0,
    /// Audio-rate output of oscillator 1.
    Osc1,
    /// Audio-rate output of oscillator 2.
    Osc2,
}

impl ModSource {
    /// `true` if this source's value is only available *during* the
    /// per-sample audio render (oscillator outputs). Audio-rate
    /// sources participate in the topological sort that determines
    /// oscillator render order.
    pub fn is_audio_rate(&self) -> bool {
        matches!(self, ModSource::Osc0 | ModSource::Osc1 | ModSource::Osc2)
    }

    /// For oscillator sources, the osc index `0..=2`. `None` for
    /// control-rate sources (envelopes, LFOs).
    pub fn osc_index(&self) -> Option<u8> {
        match self {
            ModSource::Osc0 => Some(0),
            ModSource::Osc1 => Some(1),
            ModSource::Osc2 => Some(2),
            _ => None,
        }
    }
}

/// A modulation target.
///
/// Each variant defines what `amount = 1.0` in a [`ModSlot`] means
/// in its native unit (see `docs/wavetable-synth-mod-matrix-plan.md`
/// for the canonical scale table):
///
/// | Variant | `amount = 1.0` ⇒ |
/// |---|---|
/// | `FilterCutoff` | + 4000 Hz cutoff swing |
/// | `FilterResonance` | + 1.0 Q (additive) |
/// | `OscLevel(i)` | + 1.0 mix level (additive) |
/// | `OscTune(i)` | + 24 semitones |
/// | `OscFineTune(i)` | + 100 cents |
/// | `PmAmountOf(i)` | + 1.0 phase swing |
/// | `AmAmountOf(i)` | + 1.0 amplitude coefficient |
/// | `RmAmountOf(i)` | + 1.0 ring-mix wet |
/// | `LfoRate` | + 4 Hz LFO rate |
///
/// `Copy + Default`. Default is `FilterCutoff` — an arbitrary safe
/// pick; slots with `source = ModSource::None` ignore their
/// destination anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModDestination {
    #[default]
    FilterCutoff,
    FilterResonance,
    OscLevel(u8),
    OscTune(u8),
    OscFineTune(u8),
    PmAmountOf(u8),
    AmAmountOf(u8),
    RmAmountOf(u8),
    LfoRate,
}

impl ModDestination {
    /// The osc index for any osc-targeted destination. `None` for
    /// filter or LFO destinations.
    pub fn osc_index(&self) -> Option<u8> {
        match self {
            ModDestination::OscLevel(i)
            | ModDestination::OscTune(i)
            | ModDestination::OscFineTune(i)
            | ModDestination::PmAmountOf(i)
            | ModDestination::AmAmountOf(i)
            | ModDestination::RmAmountOf(i) => Some(*i),
            _ => None,
        }
    }

    /// `true` if this destination targets a valid oscillator index
    /// (`0..NUM_OSCS_PER_VOICE`) — or if it doesn't target an
    /// oscillator at all (filter, LFO).
    pub fn has_valid_osc_index(&self) -> bool {
        match self.osc_index() {
            None => true,
            Some(i) => (i as usize) < NUM_OSCS_PER_VOICE,
        }
    }
}

/// One row in the modulation matrix.
///
/// `Copy + Default`. Default is an inactive slot (`source = None`,
/// `destination = FilterCutoff`, `amount = 0.0`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ModSlot {
    pub source: ModSource,
    pub destination: ModDestination,
    pub amount: f32,
}

impl ModSlot {
    /// `true` if this slot contributes to per-sample modulation.
    pub fn is_active(&self) -> bool {
        self.source != ModSource::None
    }

    /// `true` if this slot is an osc→osc audio-rate route — its
    /// modulator must render before its carrier.
    pub fn is_audio_rate_osc_route(&self) -> bool {
        self.source.is_audio_rate() && self.destination.osc_index().is_some()
    }
}

/// Per-sample modulation contributions, summed across all active
/// slots for a single sample.
///
/// Each consumer (filter, per-osc render) adds the appropriate
/// `*_offset` to its base value before applying it. Defaults to all
/// zero — an inactive matrix produces a `Modulations` that doesn't
/// alter the signal path.
#[derive(Debug, Clone, Copy, Default)]
pub struct Modulations {
    pub filter_cutoff_hz_offset: f32,
    pub filter_resonance_offset: f32,
    pub osc_level_offset: [f32; NUM_OSCS_PER_VOICE],
    pub osc_tune_offset: [f32; NUM_OSCS_PER_VOICE],
    pub osc_fine_offset: [f32; NUM_OSCS_PER_VOICE],
    pub pm_amount_offset: [f32; NUM_OSCS_PER_VOICE],
    pub am_amount_offset: [f32; NUM_OSCS_PER_VOICE],
    pub rm_amount_offset: [f32; NUM_OSCS_PER_VOICE],
    pub lfo_rate_offset: f32,
}

/// Per-destination scale: multiplier from "slot amount × source
/// value" (both in `[-1, 1]`-ish) into the destination's native
/// units (Hz, semitones, cents, dimensionless coefficients).
///
/// Centralizing these means a patch's `amount = 1.0` carries
/// portable meaning across all destinations.
pub mod scale {
    pub const FILTER_CUTOFF_HZ: f32 = 4000.0;
    pub const FILTER_RESONANCE: f32 = 1.0;
    pub const OSC_LEVEL: f32 = 1.0;
    pub const OSC_TUNE_SEMITONES: f32 = 24.0;
    pub const OSC_FINE_CENTS: f32 = 100.0;
    pub const PM_AMOUNT: f32 = 1.0;
    pub const AM_AMOUNT: f32 = 1.0;
    pub const RM_AMOUNT: f32 = 1.0;
    pub const LFO_RATE_HZ: f32 = 4.0;
}

impl Modulations {
    /// Add a single source's contribution at this sample into the
    /// destination's slot. `source_value` is the modulator's
    /// normalized output; `amount` is the slot's signed depth.
    /// The destination's per-unit scale is applied here.
    pub fn add_contribution(
        &mut self,
        destination: ModDestination,
        source_value: f32,
        amount: f32,
    ) {
        let contribution = source_value * amount;
        match destination {
            ModDestination::FilterCutoff => {
                self.filter_cutoff_hz_offset += contribution * scale::FILTER_CUTOFF_HZ;
            }
            ModDestination::FilterResonance => {
                self.filter_resonance_offset += contribution * scale::FILTER_RESONANCE;
            }
            ModDestination::OscLevel(i) => {
                if let Some(slot) = self.osc_level_offset.get_mut(i as usize) {
                    *slot += contribution * scale::OSC_LEVEL;
                }
            }
            ModDestination::OscTune(i) => {
                if let Some(slot) = self.osc_tune_offset.get_mut(i as usize) {
                    *slot += contribution * scale::OSC_TUNE_SEMITONES;
                }
            }
            ModDestination::OscFineTune(i) => {
                if let Some(slot) = self.osc_fine_offset.get_mut(i as usize) {
                    *slot += contribution * scale::OSC_FINE_CENTS;
                }
            }
            ModDestination::PmAmountOf(i) => {
                if let Some(slot) = self.pm_amount_offset.get_mut(i as usize) {
                    *slot += contribution * scale::PM_AMOUNT;
                }
            }
            ModDestination::AmAmountOf(i) => {
                if let Some(slot) = self.am_amount_offset.get_mut(i as usize) {
                    *slot += contribution * scale::AM_AMOUNT;
                }
            }
            ModDestination::RmAmountOf(i) => {
                // RM uses `output = raw × (1 + rm_amount_offset)`
                // for consumer symmetry with AM, but the per-slot
                // formula is `(source - 1) × amount × scale` instead
                // of `source × amount × scale`. Why: v1's single-slot
                // RM is `output = raw × (m × a + (1 - a))`, which
                // simplifies to `raw × (1 + (m - 1) × a)`. Treating
                // each slot's contribution as `(m - 1) × a` and
                // summing them generalizes the v1 formula to multi-
                // slot RM with well-defined "more-wet" semantics.
                if let Some(slot) = self.rm_amount_offset.get_mut(i as usize) {
                    *slot += (source_value - 1.0) * amount * scale::RM_AMOUNT;
                }
            }
            ModDestination::LfoRate => {
                self.lfo_rate_offset += contribution * scale::LFO_RATE_HZ;
            }
        }
    }
}

/// Fixed-size slot-based modulation matrix.
///
/// Patches install up to `N` slots; inactive slots have
/// `ModSource::None`. The matrix caches a topologically-sorted
/// oscillator render order so the audio thread reads it without
/// re-sorting per sample.
///
/// **Cycle handling.** [`Self::set_slots`] runs a topo sort over
/// the audio-rate osc→osc subgraph (slots whose source is an `Osc*`
/// and whose destination targets an osc). If a cycle is detected,
/// cycle-creating slots are sanitized to `ModSource::None` (debug
/// builds also `debug_assert!`-panic to surface the bug). Cycle
/// support via one-sample feedback is a v3+ growth item.
#[derive(Debug, Clone, Copy)]
pub struct ModMatrix<const N: usize> {
    slots: [ModSlot; N],
    /// Topologically-sorted osc render order. `osc_render_order[0]`
    /// renders first.
    osc_render_order: [u8; NUM_OSCS_PER_VOICE],
}

impl<const N: usize> Default for ModMatrix<N> {
    fn default() -> Self {
        Self {
            slots: [ModSlot::default(); N],
            // Matches v1's render direction (osc[2] → osc[1] →
            // osc[0]) so an empty matrix preserves v1 timing.
            osc_render_order: default_osc_order(),
        }
    }
}

impl<const N: usize> ModMatrix<N> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Install a fresh slot configuration. Sanitizes invalid
    /// destinations and cycle-creating slots, then re-derives the
    /// topo-sorted osc render order. Allocation-free.
    pub fn set_slots(&mut self, slots: [ModSlot; N]) {
        let mut sanitized = slots;

        // Pass 1: drop slots with invalid destinations (osc index
        // out of range).
        for slot in sanitized.iter_mut() {
            if !slot.is_active() {
                continue;
            }
            if !slot.destination.has_valid_osc_index() {
                debug_assert!(
                    false,
                    "ModSlot destination {:?} has out-of-range osc index",
                    slot.destination,
                );
                slot.source = ModSource::None;
            }
        }

        // Pass 2: topo-sort the audio-rate osc→osc subgraph,
        // disabling cycle-creating slots until the sort succeeds.
        self.osc_render_order = sanitize_cycles_and_sort(&mut sanitized);
        self.slots = sanitized;
    }

    /// Topologically-sorted osc render order (modulator before
    /// carrier). Stable across `set_slots` calls; the audio thread
    /// reads it once per sample.
    pub fn audio_rate_osc_order(&self) -> [u8; NUM_OSCS_PER_VOICE] {
        self.osc_render_order
    }

    /// Iterator over slots whose source is non-`None`.
    pub fn active_slots(&self) -> impl Iterator<Item = &ModSlot> {
        self.slots.iter().filter(|s| s.is_active())
    }

    /// Borrow the raw slot array — useful for tests + future patch
    /// editor UIs.
    pub fn slots(&self) -> &[ModSlot; N] {
        &self.slots
    }
}

/// Default render order when no audio-rate edges constrain it.
/// `[2, 1, 0]` matches v1's render direction so an empty matrix
/// preserves v1 timing byte-for-byte.
fn default_osc_order() -> [u8; NUM_OSCS_PER_VOICE] {
    [2, 1, 0]
}

/// Run topo sort on the audio-rate osc→osc edges in `slots`. On
/// cycle detection, disable one offending slot and retry until the
/// sort succeeds. Returns the final osc render order.
fn sanitize_cycles_and_sort<const N: usize>(
    slots: &mut [ModSlot; N],
) -> [u8; NUM_OSCS_PER_VOICE] {
    loop {
        match try_topo_sort(slots) {
            Ok(order) => return order,
            Err(cycle_slot) => {
                debug_assert!(
                    false,
                    "ModMatrix audio-rate cycle: disabling slot {cycle_slot}",
                );
                slots[cycle_slot].source = ModSource::None;
                // Retry — each iteration disables at least one slot,
                // so the loop terminates within N iterations.
            }
        }
    }
}

/// Try to topologically sort the oscs based on audio-rate edges in
/// `slots`. Returns `Err(slot_idx)` for one slot index that's part
/// of a cycle on failure. Tie-breaks remaining nodes by **highest
/// index first** so an empty matrix produces `[2, 1, 0]` (matches
/// v1's render direction).
fn try_topo_sort<const N: usize>(slots: &[ModSlot; N]) -> Result<[u8; NUM_OSCS_PER_VOICE], usize> {
    // Build in-degree counts per osc (only counts edges within the
    // 3-osc graph).
    let mut in_degree = [0u8; NUM_OSCS_PER_VOICE];
    // Store edges as (src, dst, slot_idx). Bounded by N.
    let mut edge_src = [0u8; 64];
    let mut edge_dst = [0u8; 64];
    let mut edge_slot = [0usize; 64];
    let mut edge_count = 0usize;
    debug_assert!(N <= 64, "ModMatrix N must be ≤ 64 for the topo-sort scratch buffers");

    for (i, slot) in slots.iter().enumerate() {
        if !slot.is_audio_rate_osc_route() {
            continue;
        }
        let src = slot.source.osc_index().expect("audio-rate source has osc index");
        let dst = slot.destination.osc_index().expect("osc-targeted destination has osc index");
        if src == dst {
            // Self-edge (e.g. Osc0 → PmAmountOf(0)). Treated as a
            // 1-cycle and rejected.
            return Err(i);
        }
        if (src as usize) >= NUM_OSCS_PER_VOICE || (dst as usize) >= NUM_OSCS_PER_VOICE {
            // Out-of-range osc index — already validated by
            // set_slots, but defend against caller misuse.
            return Err(i);
        }
        edge_src[edge_count] = src;
        edge_dst[edge_count] = dst;
        edge_slot[edge_count] = i;
        edge_count += 1;
        in_degree[dst as usize] += 1;
    }

    // Kahn's algorithm. Tie-break by highest index first.
    let mut order = [0u8; NUM_OSCS_PER_VOICE];
    let mut placed = 0usize;
    let mut remaining = [true; NUM_OSCS_PER_VOICE];

    while placed < NUM_OSCS_PER_VOICE {
        // Find a remaining node with in_degree == 0; prefer highest
        // index for v1-render-direction compatibility.
        let mut chosen: Option<u8> = None;
        for n in (0..NUM_OSCS_PER_VOICE as u8).rev() {
            if remaining[n as usize] && in_degree[n as usize] == 0 {
                chosen = Some(n);
                break;
            }
        }
        match chosen {
            Some(n) => {
                order[placed] = n;
                placed += 1;
                remaining[n as usize] = false;
                // Decrement in-degree of targets of this node's
                // outgoing edges.
                for e in 0..edge_count {
                    if edge_src[e] == n && remaining[edge_dst[e] as usize] {
                        in_degree[edge_dst[e] as usize] -= 1;
                    }
                }
            }
            None => {
                // Cycle: find one slot whose destination is still in
                // `remaining` (i.e. participates in the cycle) and
                // return its index.
                for e in 0..edge_count {
                    if remaining[edge_dst[e] as usize] {
                        return Err(edge_slot[e]);
                    }
                }
                // Defensive: unreachable since no-progress implies
                // an edge into a remaining node exists.
                unreachable!("no-progress with no remaining edges");
            }
        }
    }

    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_matrix_is_empty() {
        let m: ModMatrix<8> = ModMatrix::default();
        assert_eq!(m.active_slots().count(), 0);
        assert_eq!(m.audio_rate_osc_order(), [2, 1, 0]);
    }

    #[test]
    fn active_slots_filter_out_none_sources() {
        let mut m: ModMatrix<4> = ModMatrix::default();
        let mut slots = [ModSlot::default(); 4];
        slots[0] = ModSlot {
            source: ModSource::Env1,
            destination: ModDestination::FilterCutoff,
            amount: 0.5,
        };
        slots[2] = ModSlot {
            source: ModSource::Lfo1,
            destination: ModDestination::FilterCutoff,
            amount: 0.1,
        };
        m.set_slots(slots);
        assert_eq!(m.active_slots().count(), 2);
    }

    #[test]
    fn control_rate_only_matrix_keeps_default_render_order() {
        let mut m: ModMatrix<4> = ModMatrix::default();
        let slots = [
            ModSlot {
                source: ModSource::Lfo1,
                destination: ModDestination::FilterCutoff,
                amount: 0.1,
            },
            ModSlot {
                source: ModSource::Env2,
                destination: ModDestination::FilterCutoff,
                amount: 0.6,
            },
            ModSlot::default(),
            ModSlot::default(),
        ];
        m.set_slots(slots);
        // Control-rate sources add no audio-rate edges → default
        // render order persists.
        assert_eq!(m.audio_rate_osc_order(), [2, 1, 0]);
    }

    #[test]
    fn audio_rate_edge_orders_modulator_before_carrier() {
        // Osc0 → PmAmountOf(1) means osc0 must render before osc1.
        // With no other constraints, osc2 is unconstrained → tie-
        // break by highest index ⇒ osc2 first, then osc0 (forced
        // before osc1), then osc1.
        let mut m: ModMatrix<4> = ModMatrix::default();
        let slots = [
            ModSlot {
                source: ModSource::Osc0,
                destination: ModDestination::PmAmountOf(1),
                amount: 0.3,
            },
            ModSlot::default(),
            ModSlot::default(),
            ModSlot::default(),
        ];
        m.set_slots(slots);
        let order = m.audio_rate_osc_order();
        // osc0 must appear before osc1.
        let pos0 = order.iter().position(|&x| x == 0).unwrap();
        let pos1 = order.iter().position(|&x| x == 1).unwrap();
        assert!(pos0 < pos1, "osc0 must render before osc1; got {order:?}");
    }

    #[test]
    fn v1_chained_pm_topology_preserves_original_order() {
        // v1 patch's audio-rate routing: osc[1] → PmAmountOf(0),
        // osc[2] → PmAmountOf(1). Must render osc2, osc1, osc0 in
        // that order.
        let mut m: ModMatrix<4> = ModMatrix::default();
        let slots = [
            ModSlot {
                source: ModSource::Osc1,
                destination: ModDestination::PmAmountOf(0),
                amount: 0.3,
            },
            ModSlot {
                source: ModSource::Osc2,
                destination: ModDestination::PmAmountOf(1),
                amount: 0.15,
            },
            ModSlot::default(),
            ModSlot::default(),
        ];
        m.set_slots(slots);
        assert_eq!(m.audio_rate_osc_order(), [2, 1, 0]);
    }

    #[test]
    #[should_panic(expected = "audio-rate cycle")]
    fn two_cycle_rejected_in_debug() {
        // Osc0 → PmAmountOf(1) AND Osc1 → PmAmountOf(0) creates a
        // 2-cycle. Debug builds debug_assert; release builds
        // sanitize. Test runs in debug → expect panic.
        let mut m: ModMatrix<4> = ModMatrix::default();
        let slots = [
            ModSlot {
                source: ModSource::Osc0,
                destination: ModDestination::PmAmountOf(1),
                amount: 0.3,
            },
            ModSlot {
                source: ModSource::Osc1,
                destination: ModDestination::PmAmountOf(0),
                amount: 0.3,
            },
            ModSlot::default(),
            ModSlot::default(),
        ];
        m.set_slots(slots);
    }

    #[test]
    #[should_panic(expected = "audio-rate cycle")]
    fn self_loop_rejected_in_debug() {
        // Osc0 → PmAmountOf(0): self-modulation. Treated as a
        // 1-cycle and rejected. v3 may add opt-in feedback support.
        let mut m: ModMatrix<4> = ModMatrix::default();
        let slots = [
            ModSlot {
                source: ModSource::Osc0,
                destination: ModDestination::PmAmountOf(0),
                amount: 0.5,
            },
            ModSlot::default(),
            ModSlot::default(),
            ModSlot::default(),
        ];
        m.set_slots(slots);
    }

    #[test]
    #[should_panic(expected = "out-of-range osc index")]
    fn out_of_range_osc_index_rejected_in_debug() {
        let mut m: ModMatrix<4> = ModMatrix::default();
        let slots = [
            ModSlot {
                source: ModSource::Env1,
                destination: ModDestination::OscLevel(99),
                amount: 0.5,
            },
            ModSlot::default(),
            ModSlot::default(),
            ModSlot::default(),
        ];
        m.set_slots(slots);
    }

    #[test]
    fn modulations_add_contribution_routes_to_right_field() {
        let mut m = Modulations::default();
        m.add_contribution(ModDestination::FilterCutoff, 1.0, 0.1);
        // 1.0 × 0.1 × 4000 Hz scale = 400 Hz offset.
        assert!((m.filter_cutoff_hz_offset - 400.0).abs() < 1e-3);

        m.add_contribution(ModDestination::PmAmountOf(0), 1.0, 0.5);
        // 1.0 × 0.5 × 1.0 PM scale = 0.5.
        assert!((m.pm_amount_offset[0] - 0.5).abs() < 1e-6);

        m.add_contribution(ModDestination::OscTune(2), 0.5, 1.0);
        // 0.5 × 1.0 × 24 semi scale = 12 semis.
        assert!((m.osc_tune_offset[2] - 12.0).abs() < 1e-4);
    }

    #[test]
    fn rm_contribution_uses_source_minus_one_formula() {
        // RM's contribution is `(source - 1) × amount × scale` so
        // that the consumer's `output = raw × (1 + rm_offset)`
        // reproduces v1's single-slot `raw × (m × a + (1 - a))`.
        let mut m = Modulations::default();
        // amount = 1.0, source = 1.0 ⇒ contribution (1 - 1) × 1 = 0.
        // Consumer: raw × (1 + 0) = raw. (Pure wet RM at modulator
        // peak passes the carrier through unchanged.)
        m.add_contribution(ModDestination::RmAmountOf(0), 1.0, 1.0);
        assert!((m.rm_amount_offset[0] - 0.0).abs() < 1e-6);

        // amount = 1.0, source = -1.0 ⇒ contribution (-1 - 1) × 1 = -2.
        // Consumer: raw × (1 - 2) = -raw. (Pure wet RM flips sign
        // when modulator is at -1 — classic four-quadrant ring mod.)
        let mut m = Modulations::default();
        m.add_contribution(ModDestination::RmAmountOf(0), -1.0, 1.0);
        assert!((m.rm_amount_offset[0] - (-2.0)).abs() < 1e-6);

        // amount = 0.5, source = 0.0 ⇒ contribution (0 - 1) × 0.5 = -0.5.
        // Consumer: raw × 0.5. (50% dry/wet at zero modulator gives
        // half-attenuated carrier — matches v1.)
        let mut m = Modulations::default();
        m.add_contribution(ModDestination::RmAmountOf(0), 0.0, 0.5);
        assert!((m.rm_amount_offset[0] - (-0.5)).abs() < 1e-6);

        // amount = 0.0, any source ⇒ contribution 0.
        // Consumer: raw × 1 = raw (full dry).
        let mut m = Modulations::default();
        m.add_contribution(ModDestination::RmAmountOf(0), 0.5, 0.0);
        assert!((m.rm_amount_offset[0] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn modulations_default_is_all_zero() {
        let m = Modulations::default();
        assert_eq!(m.filter_cutoff_hz_offset, 0.0);
        assert_eq!(m.filter_resonance_offset, 0.0);
        assert_eq!(m.osc_level_offset, [0.0; NUM_OSCS_PER_VOICE]);
        assert_eq!(m.pm_amount_offset, [0.0; NUM_OSCS_PER_VOICE]);
        assert_eq!(m.lfo_rate_offset, 0.0);
    }

    #[test]
    fn mod_source_audio_rate_classification() {
        assert!(!ModSource::None.is_audio_rate());
        assert!(!ModSource::Env1.is_audio_rate());
        assert!(!ModSource::Lfo1.is_audio_rate());
        assert!(ModSource::Osc0.is_audio_rate());
        assert!(ModSource::Osc1.is_audio_rate());
        assert!(ModSource::Osc2.is_audio_rate());
    }

    #[test]
    fn mod_destination_osc_index_extraction() {
        assert_eq!(ModDestination::FilterCutoff.osc_index(), None);
        assert_eq!(ModDestination::LfoRate.osc_index(), None);
        assert_eq!(ModDestination::PmAmountOf(1).osc_index(), Some(1));
        assert_eq!(ModDestination::OscTune(2).osc_index(), Some(2));
    }
}
