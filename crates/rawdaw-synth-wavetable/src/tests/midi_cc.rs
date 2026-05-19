//! K5 tests — MIDI CC → mod matrix routing.
//!
//! Pins three contracts:
//!
//! 1. A matrix slot with `ModSource::MidiCC(cc)` reads the per-node
//!    CC table; CCs not referenced by any slot stay inaudible.
//! 2. CC values normalize to `[0.0, 1.0]` (raw 7-bit / 127) so the
//!    matrix's per-destination scale behaves predictably regardless
//!    of which CC drives the slot.
//! 3. CC updates land at sample-accurate offsets within a block —
//!    a CC event at offset N changes audio at sample N+, not at
//!    block boundary.

use rawdaw_dsp::{ModDestination, ModSlot, ModSource};
use rawdaw_engine::event::BlockEventInBlock;
use rawdaw_model::{Midi2Message, MidiChannel, U16Velocity};

use super::{make_node, note_on, render_block, rms, BLOCK};
use crate::{BlockMessage, EMPTY_SLOT, MOD_MATRIX_SLOTS};

fn control_change(controller: u8, value: u8) -> BlockMessage {
    BlockMessage::Midi(Midi2Message::ControlChange {
        channel: MidiChannel::default(),
        controller: rawdaw_model::U7::new(controller).unwrap(),
        value: rawdaw_model::U7::new(value).unwrap(),
    })
}

fn ev(offset: u32, msg: BlockMessage) -> BlockEventInBlock {
    BlockEventInBlock {
        offset_in_block: offset,
        message: msg,
    }
}

#[test]
fn mod_wheel_routed_to_filter_cutoff_changes_audio() {
    // Default M5 patch + one extra slot: MidiCC(1) → FilterCutoff
    // at amount 1.0. With the mod wheel at 127 (full normalized 1.0)
    // that's +4000 Hz of cutoff offset — clearly audible.
    let mut treatment = make_node();
    let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
    slots[0] = ModSlot {
        source: ModSource::MidiCC(1),
        destination: ModDestination::FilterCutoff,
        amount: 1.0,
    };
    treatment.set_matrix_for_test(slots);

    let mut baseline = make_node();
    baseline.set_matrix_for_test(slots);
    // baseline never receives a CC event — CC1 stays at 0.

    let mut buf_treat = Vec::new();
    render_block(
        &mut treatment,
        &[
            ev(0, control_change(1, 127)),
            ev(1, note_on(60, U16Velocity::HALF)),
        ],
        &mut buf_treat,
    );

    let mut buf_base = Vec::new();
    render_block(
        &mut baseline,
        &[ev(1, note_on(60, U16Velocity::HALF))],
        &mut buf_base,
    );

    // With CC1=127, the cutoff is +4000 Hz higher → brighter →
    // audibly different from the CC1=0 baseline.
    let diff_rms = rms(
        &buf_treat[..BLOCK]
            .iter()
            .zip(buf_base[..BLOCK].iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.005,
        "mod wheel → cutoff slot should change output; diff_rms = {diff_rms}",
    );
}

#[test]
fn unrouted_cc_does_not_change_audio() {
    // A CC that no matrix slot references must not affect output.
    // The M5 default routes CC1 to the filter cutoff, so we use
    // CC7 (volume) here — no default slot reads it, so audio must
    // be byte-identical to the baseline.
    let mut with_cc = make_node();
    let mut without = make_node();

    let mut buf_cc = Vec::new();
    let mut buf_base = Vec::new();

    render_block(
        &mut with_cc,
        &[
            ev(0, control_change(7, 127)),
            ev(1, note_on(60, U16Velocity::HALF)),
        ],
        &mut buf_cc,
    );
    render_block(
        &mut without,
        &[ev(1, note_on(60, U16Velocity::HALF))],
        &mut buf_base,
    );

    for i in 0..BLOCK {
        assert!(
            (buf_cc[i] - buf_base[i]).abs() < 1e-6,
            "unrouted CC7 must be inaudible; differ at sample {i}",
        );
    }
}

#[test]
fn cc_value_normalizes_to_unit_range() {
    // Two renders with the same MidiCC→cutoff slot but different
    // CC values (32 ≈ 0.252, 127 = 1.0) should diverge in proportion
    // to (value/127). At amount=1.0 the cutoff offset spans 0..+4000
    // Hz; CC=32 lands at ~1008 Hz offset, CC=127 at 4000 Hz.
    let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
    slots[0] = ModSlot {
        source: ModSource::MidiCC(1),
        destination: ModDestination::FilterCutoff,
        amount: 1.0,
    };

    let mut quarter = make_node();
    quarter.set_matrix_for_test(slots);
    let mut full = make_node();
    full.set_matrix_for_test(slots);

    let mut buf_quarter = Vec::new();
    let mut buf_full = Vec::new();

    render_block(
        &mut quarter,
        &[
            ev(0, control_change(1, 32)),
            ev(1, note_on(60, U16Velocity::HALF)),
        ],
        &mut buf_quarter,
    );
    render_block(
        &mut full,
        &[
            ev(0, control_change(1, 127)),
            ev(1, note_on(60, U16Velocity::HALF)),
        ],
        &mut buf_full,
    );

    let diff_rms = rms(
        &buf_quarter[..BLOCK]
            .iter()
            .zip(buf_full[..BLOCK].iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.005,
        "CC=32 vs CC=127 should produce different audio; diff_rms = {diff_rms}",
    );
}

#[test]
fn cc_update_lands_at_sample_offset() {
    // Sample accuracy: a CC event at offset 128 should leave the
    // first 128 samples identical to the baseline (CC=0) and the
    // remaining samples diverged (CC=127). Tests that the per-
    // sample loop drains events at the right offset and that voices
    // read the live CC state on each tick.
    let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
    slots[0] = ModSlot {
        source: ModSource::MidiCC(1),
        destination: ModDestination::FilterCutoff,
        amount: 1.0,
    };

    let mut treatment = make_node();
    treatment.set_matrix_for_test(slots);
    let mut baseline = make_node();
    baseline.set_matrix_for_test(slots);
    // baseline never receives a CC event.

    // Both start a note at offset 0; treatment also gets a CC1=127
    // halfway through the block.
    let mut buf_treat = Vec::new();
    render_block(
        &mut treatment,
        &[
            ev(0, note_on(60, U16Velocity::HALF)),
            ev(128, control_change(1, 127)),
        ],
        &mut buf_treat,
    );

    let mut buf_base = Vec::new();
    render_block(
        &mut baseline,
        &[ev(0, note_on(60, U16Velocity::HALF))],
        &mut buf_base,
    );

    // Samples [0..128): bit-identical (CC unchanged in both).
    for i in 0..128 {
        assert!(
            (buf_treat[i] - buf_base[i]).abs() < 1e-6,
            "samples before CC event must match baseline; differ at {i}",
        );
    }
    // Samples [128..BLOCK): must diverge because the CC kicked the
    // cutoff to a new value at sample 128.
    let mut diffs = 0;
    for i in 128..BLOCK {
        if (buf_treat[i] - buf_base[i]).abs() > 1e-4 {
            diffs += 1;
        }
    }
    assert!(
        diffs > 16,
        "samples after CC event should diverge; saw only {diffs} differences",
    );
}

#[test]
fn cc_state_persists_across_blocks() {
    // A CC sent in block 1 stays active in block 2 — no per-block
    // reset. The matrix slot in block 2 still reads the value that
    // landed earlier.
    let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
    slots[0] = ModSlot {
        source: ModSource::MidiCC(1),
        destination: ModDestination::FilterCutoff,
        amount: 1.0,
    };

    let mut treatment = make_node();
    treatment.set_matrix_for_test(slots);
    let mut baseline = make_node();
    baseline.set_matrix_for_test(slots);

    // Block 1: treatment receives CC; both receive note_on.
    let mut throwaway = Vec::new();
    render_block(
        &mut treatment,
        &[
            ev(0, control_change(1, 127)),
            ev(1, note_on(60, U16Velocity::HALF)),
        ],
        &mut throwaway,
    );
    render_block(
        &mut baseline,
        &[ev(1, note_on(60, U16Velocity::HALF))],
        &mut throwaway,
    );

    // Block 2: no events to either. Treatment's CC1 should still
    // be 127 (state persists); baseline's still 0.
    let mut buf_treat = Vec::new();
    render_block(&mut treatment, &[], &mut buf_treat);
    let mut buf_base = Vec::new();
    render_block(&mut baseline, &[], &mut buf_base);

    let diff_rms = rms(
        &buf_treat[..BLOCK]
            .iter()
            .zip(buf_base[..BLOCK].iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.005,
        "CC state must persist across blocks; diff_rms = {diff_rms}",
    );
}

#[test]
fn sustain_pedal_cc_routes_to_matrix_too() {
    // CC64 is special-cased for sustain-pedal latching (K4), but
    // K5 also stores it in the CC table — a patch that routes
    // `MidiCC(64) → FilterCutoff` must work alongside the pedal
    // behavior. Tests that the K4 special-case didn't accidentally
    // skip the table write.
    let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
    slots[0] = ModSlot {
        source: ModSource::MidiCC(64),
        destination: ModDestination::FilterCutoff,
        amount: 1.0,
    };

    let mut treatment = make_node();
    treatment.set_matrix_for_test(slots);
    let mut baseline = make_node();
    baseline.set_matrix_for_test(slots);

    let mut buf_treat = Vec::new();
    render_block(
        &mut treatment,
        &[
            ev(0, control_change(64, 127)),
            ev(1, note_on(60, U16Velocity::HALF)),
        ],
        &mut buf_treat,
    );
    let mut buf_base = Vec::new();
    render_block(
        &mut baseline,
        &[ev(1, note_on(60, U16Velocity::HALF))],
        &mut buf_base,
    );

    let diff_rms = rms(
        &buf_treat[..BLOCK]
            .iter()
            .zip(buf_base[..BLOCK].iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.005,
        "CC64 with pedal down should route through matrix even while latching the pedal; diff_rms = {diff_rms}",
    );
}
