//! F3 (multi-oscillator headroom) + F4 (PM/AM/RM modulation routing)
//! tests.

use rawdaw_dsp::{ModDestination, ModSlot, ModSource, WavetableOscParams};
use rawdaw_engine::event::BlockEventInBlock;
use rawdaw_model::U16Velocity;

use super::{make_node, note_on, render_block, rms, BLOCK};
use crate::{EMPTY_SLOT, MOD_MATRIX_SLOTS, NUM_OSCS};

/// Explicit single-osc patch — the v0-equivalent baseline used
/// by the F3 headroom tests. Constructed here rather than read
/// from `PATCH_OSC_PARAMS` because the default patch is the v1
/// three-osc PM stack; these tests need to compare against a
/// known single-osc reference. M4-shape: only tune/fine/level
/// fields; modulation routing lives in matrix slots installed
/// separately via `set_matrix_for_test`.
pub(super) fn single_osc_patch() -> [WavetableOscParams; NUM_OSCS] {
    [
        WavetableOscParams {
            tune_semitones: 0,
            fine_cents: 0,
            level: 1.0,
        },
        WavetableOscParams::default(),
        WavetableOscParams::default(),
    ]
}

pub(super) fn coherent_three_osc_patch() -> [WavetableOscParams; NUM_OSCS] {
    let one = WavetableOscParams {
        tune_semitones: 0,
        fine_cents: 0,
        level: 1.0,
    };
    [one, one, one]
}

/// Three oscs at the same pitch and full level must produce the
/// same audio as a single osc at full level. The headroom-
/// preserving sum divides by `Σ level_i = 3` while each osc
/// contributes the identical waveform (same Hz, phase reset at
/// note_on), so `3 * single / 3 == single`. Pinned with an
/// empty matrix so the v1 PM-chain slots in the default matrix
/// don't perturb the comparison.
#[test]
fn three_coherent_oscs_match_single_osc_output() {
    let mut single = make_node();
    single.set_patch_for_test(single_osc_patch());
    single.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
    let mut single_buf = Vec::new();
    render_block(
        &mut single,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut single_buf,
    );

    let mut triple = make_node();
    triple.set_patch_for_test(coherent_three_osc_patch());
    triple.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
    let mut triple_buf = Vec::new();
    render_block(
        &mut triple,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut triple_buf,
    );

    // Sample-identical within f32 round-off (the divide-by-level-sum
    // and sum-of-three introduces a tiny rounding error per sample).
    let mut max_diff = 0.0_f32;
    for i in 0..(2 * BLOCK) {
        let d = (single_buf[i] - triple_buf[i]).abs();
        max_diff = max_diff.max(d);
    }
    assert!(
        max_diff < 1e-5,
        "coherent three-osc patch must match single-osc output; max_diff = {max_diff}",
    );
}

/// Detuned three-osc patch (oct down, root, oct up) must be
/// audible and bounded — i.e., the headroom-preserving sum
/// prevents the summed peak from exceeding the single-osc peak.
/// Plan called for "RMS within ±20% of v0 RMS"; that target is
/// unreachable for partially-correlated octave-related sources
/// (RMS lands near `1/sqrt(3) ≈ 0.58×` of v0 due to phase
/// incoherence after the divide-by-3), so the pin is reframed
/// around the property the headroom math actually guarantees.
#[test]
fn detuned_three_osc_output_is_bounded_and_non_silent() {
    let mut single = make_node();
    single.set_patch_for_test(single_osc_patch());
    single.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
    let mut single_buf = Vec::new();
    render_block(
        &mut single,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut single_buf,
    );
    // Skip the attack ramp.
    render_block(&mut single, &[], &mut single_buf);
    let single_peak = single_buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);

    let mut triple = make_node();
    triple.set_patch_for_test([
        WavetableOscParams {
            tune_semitones: 0,
            fine_cents: 0,
            level: 1.0,
        },
        WavetableOscParams {
            tune_semitones: 12,
            fine_cents: 0,
            level: 1.0,
        },
        WavetableOscParams {
            tune_semitones: -12,
            fine_cents: 0,
            level: 1.0,
        },
    ]);
    triple.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
    let mut triple_buf = Vec::new();
    render_block(
        &mut triple,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut triple_buf,
    );
    render_block(&mut triple, &[], &mut triple_buf);
    let triple_peak = triple_buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    let triple_rms = rms(&triple_buf[..BLOCK]);

    assert!(
        triple_rms > 0.01,
        "detuned three-osc output should be audible; rms = {triple_rms}",
    );
    assert!(
        triple_peak <= single_peak * 1.05,
        "detuned three-osc peak {triple_peak} should stay under single-osc peak {single_peak} (×1.05 slack)",
    );
    let diff_rms = rms(
        &single_buf[..BLOCK]
            .iter()
            .zip(triple_buf[..BLOCK].iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.01,
        "detuned three-osc must differ from single-osc; diff rms = {diff_rms}",
    );
}

// ── F4 modulation-routing tests ─────────────────────────────────────
//
// The five tests below pin the PM/AM/RM wiring. They use a shared
// helper to render a long-enough audio window past the amp attack
// ramp so the asserts measure steady-state behavior, not the
// transient.

pub(super) fn render_steady_state(
    patch: [WavetableOscParams; NUM_OSCS],
    matrix: [ModSlot; MOD_MATRIX_SLOTS],
) -> Vec<f32> {
    let mut node = make_node();
    node.set_patch_for_test(patch);
    node.set_matrix_for_test(matrix);
    let mut buf = Vec::new();
    render_block(
        &mut node,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut buf,
    );
    // Burn three more blocks so the amp envelope is at sustain
    // and the LFO has stabilized.
    for _ in 0..3 {
        render_block(&mut node, &[], &mut buf);
    }
    // Grab one more block as the measurement window.
    render_block(&mut node, &[], &mut buf);
    buf[..BLOCK].to_vec()
}

/// Patch: carrier on osc[0], silent modulator on osc[1] (audible
/// only as the matrix source). The returned `osc_params` shape
/// is the same regardless of routing; the modulation lives in
/// the matrix slots, which the caller supplies.
fn carrier_with_silent_modulator_patch() -> [WavetableOscParams; NUM_OSCS] {
    [
        WavetableOscParams {
            tune_semitones: 0,
            fine_cents: 0,
            level: 1.0,
        },
        WavetableOscParams {
            // Perfect fifth above carrier; level=0 so it doesn't
            // contribute to the audio sum, only feeds the matrix.
            tune_semitones: 7,
            fine_cents: 0,
            level: 0.0,
        },
        WavetableOscParams::default(),
    ]
}

/// Build a one-slot matrix routing `Osc1 → destination @ amount`.
/// `destination == None` ⇒ empty matrix (no modulation).
fn matrix_one_slot(
    destination: Option<ModDestination>,
    amount: f32,
) -> [ModSlot; MOD_MATRIX_SLOTS] {
    let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
    if let Some(dst) = destination {
        slots[0] = ModSlot {
            source: ModSource::Osc1,
            destination: dst,
            amount,
        };
    }
    slots
}

#[test]
fn pm_amount_zero_is_a_noop() {
    let pm_off = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(None, 0.0),
    );
    let pm_zero = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(Some(ModDestination::PmAmountOf(0)), 0.0),
    );
    for (i, (a, b)) in pm_off.iter().zip(pm_zero.iter()).enumerate() {
        assert_eq!(*a, *b, "sample {i}: empty matrix and Pm@0 must match exactly");
    }
}

#[test]
fn pm_engaged_changes_the_signal() {
    let pm_off = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(None, 0.0),
    );
    let pm_on = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(Some(ModDestination::PmAmountOf(0)), 0.3),
    );
    let diff_rms = rms(
        &pm_off
            .iter()
            .zip(pm_on.iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.01,
        "Pm @ 0.3 should diverge from carrier alone; diff_rms = {diff_rms}",
    );
}

#[test]
fn am_engaged_changes_the_signal() {
    let am_off = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(None, 0.0),
    );
    let am_on = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(Some(ModDestination::AmAmountOf(0)), 0.5),
    );
    let diff_rms = rms(
        &am_off
            .iter()
            .zip(am_on.iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.01,
        "Am @ 0.5 should diverge from carrier alone; diff_rms = {diff_rms}",
    );
}

#[test]
fn rm_engaged_changes_the_signal() {
    let rm_off = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(None, 0.0),
    );
    let rm_on = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(Some(ModDestination::RmAmountOf(0)), 1.0),
    );
    let diff_rms = rms(
        &rm_off
            .iter()
            .zip(rm_on.iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.01,
        "Rm @ 1.0 should diverge from carrier alone; diff_rms = {diff_rms}",
    );
}

#[test]
fn modes_produce_distinct_outputs() {
    // Defensive pin: at the same non-trivial amount, the three
    // modulation destinations must produce mutually different
    // outputs. Catches accidental match-arm aliasing in
    // `Modulations::add_contribution`.
    let pm = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(Some(ModDestination::PmAmountOf(0)), 0.5),
    );
    let am = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(Some(ModDestination::AmAmountOf(0)), 0.5),
    );
    let rm = render_steady_state(
        carrier_with_silent_modulator_patch(),
        matrix_one_slot(Some(ModDestination::RmAmountOf(0)), 0.5),
    );
    let pa_diff = rms(&pm.iter().zip(am.iter()).map(|(a, b)| a - b).collect::<Vec<_>>());
    let pr_diff = rms(&pm.iter().zip(rm.iter()).map(|(a, b)| a - b).collect::<Vec<_>>());
    let ar_diff = rms(&am.iter().zip(rm.iter()).map(|(a, b)| a - b).collect::<Vec<_>>());
    assert!(pa_diff > 0.005, "Pm vs Am should differ; pa_diff = {pa_diff}");
    assert!(pr_diff > 0.005, "Pm vs Rm should differ; pr_diff = {pr_diff}");
    assert!(ar_diff > 0.005, "Am vs Rm should differ; ar_diff = {ar_diff}");
}
