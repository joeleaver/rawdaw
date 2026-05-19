//! Modulation matrix engine — the slot-installation entry point
//! ([`ModMatrix::set_slots`]) plus the validation and topological-
//! sort passes that keep the audio-thread invariants safe.
//!
//! This is the "how the matrix is built" half of the modulation
//! module. The "what flows through it" lives in [`super::types`].

use super::types::{ModSlot, ModSource, NUM_OSCS_PER_VOICE};

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
        // out of range) and invalid sources (MIDI CC > 127).
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
                continue;
            }
            if let ModSource::MidiCC(cc) = slot.source
                && cc > 127
            {
                debug_assert!(
                    false,
                    "ModSlot source MidiCC({cc}) > 127 — out of range",
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
    use crate::modulation::{ModDestination, ModSlot, ModSource};

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
    #[should_panic(expected = "out of range")]
    fn out_of_range_midi_cc_rejected_in_debug() {
        let mut m: ModMatrix<4> = ModMatrix::default();
        let slots = [
            ModSlot {
                source: ModSource::MidiCC(200),
                destination: ModDestination::FilterCutoff,
                amount: 0.5,
            },
            ModSlot::default(),
            ModSlot::default(),
            ModSlot::default(),
        ];
        m.set_slots(slots);
    }
}
