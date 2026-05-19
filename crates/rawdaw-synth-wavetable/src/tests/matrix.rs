//! M3 mod-matrix routing pins + U3 `with_patch` tests.

use rawdaw_dsp::{ModDestination, ModSlot, ModSource};
use rawdaw_engine::event::BlockEventInBlock;
use rawdaw_engine::node::AudioNode;
use rawdaw_model::U16Velocity;

use super::oscillators::{coherent_three_osc_patch, render_steady_state};
use super::{make_node, note_on, render_block, rms, BLOCK, SR};
use crate::{WavetablePatch, WavetableSynthNode, EMPTY_SLOT, MOD_MATRIX_SLOTS};

/// With an empty matrix (no slots active), filter cutoff sits
/// static at the patch base — no LFO sweep on the filter. The
/// audio still moves (the carrier saw is still bright), but the
/// LFO-driven cutoff wobble that v1 had is gone. Diff RMS
/// against the default-matrix render confirms the matrix is
/// actually wired (removing the slot changes the audio).
#[test]
fn empty_matrix_disables_lfo_cutoff_sweep() {
    let default_matrix_buf =
        render_steady_state(coherent_three_osc_patch(), WavetablePatch::default().matrix);

    let mut node = make_node();
    node.set_patch_for_test(coherent_three_osc_patch());
    node.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
    let mut buf = Vec::new();
    render_block(
        &mut node,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut buf,
    );
    for _ in 0..3 {
        render_block(&mut node, &[], &mut buf);
    }
    render_block(&mut node, &[], &mut buf);
    let empty_matrix_buf = buf[..BLOCK].to_vec();

    assert!(
        rms(&empty_matrix_buf) > 0.01,
        "empty matrix should still produce audio; rms = {}",
        rms(&empty_matrix_buf),
    );

    let diff_rms = rms(
        &default_matrix_buf
            .iter()
            .zip(empty_matrix_buf.iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.005,
        "removing LFO→cutoff slot should audibly change output; diff_rms = {diff_rms}",
    );
}

/// Adding an ENV2→cutoff slot to a known patch produces an
/// audibly different render than the same patch without the
/// slot. Pins that ENV2 is correctly threaded through the
/// matrix and that the matrix's contribution actually reaches
/// the filter cutoff.
#[test]
fn env2_to_cutoff_slot_changes_audio() {
    let mut without = make_node();
    without.set_matrix_for_test([EMPTY_SLOT; MOD_MATRIX_SLOTS]);
    let mut without_buf = Vec::new();
    render_block(
        &mut without,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut without_buf,
    );

    let mut with = make_node();
    let mut slots = [EMPTY_SLOT; MOD_MATRIX_SLOTS];
    slots[0] = ModSlot {
        source: ModSource::Env2,
        destination: ModDestination::FilterCutoff,
        amount: 0.6,
    };
    with.set_matrix_for_test(slots);
    let mut with_buf = Vec::new();
    render_block(
        &mut with,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut with_buf,
    );

    let diff_rms = rms(
        &without_buf[..BLOCK]
            .iter()
            .zip(with_buf[..BLOCK].iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.005,
        "ENV2→cutoff slot should audibly change output; diff_rms = {diff_rms}",
    );
}

// ── U3 with_patch tests ─────────────────────────────────────────────

/// `with_patch` lands a custom patch that's audibly different from
/// the M5 default. Pins that `WavetableSynthNode::with_patch` is
/// wired — without it, the node would silently fall back to default.
#[test]
fn with_patch_uses_custom_filter_cutoff() {
    let default_buf = render_steady_state(
        coherent_three_osc_patch(),
        WavetablePatch::default().matrix,
    );

    // Brighter custom patch — same osc + matrix shape as the default
    // render-helper produces, but the filter base is opened up so the
    // saw harmonics survive.
    let bright = WavetablePatch {
        osc_params: coherent_three_osc_patch(),
        filter_cutoff_hz: 8000.0,
        ..WavetablePatch::default()
    };
    let mut node = WavetableSynthNode::with_patch(bright);
    node.prepare(SR, BLOCK);
    let mut buf = Vec::new();
    render_block(
        &mut node,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut buf,
    );
    for _ in 0..3 {
        render_block(&mut node, &[], &mut buf);
    }
    render_block(&mut node, &[], &mut buf);
    let bright_buf = buf[..BLOCK].to_vec();

    let diff_rms = rms(
        &default_buf
            .iter()
            .zip(bright_buf.iter())
            .map(|(a, b)| a - b)
            .collect::<Vec<_>>(),
    );
    assert!(
        diff_rms > 0.005,
        "with_patch(custom) should differ from default; diff_rms = {diff_rms}",
    );
}

/// `WavetableSynthNode::new()` and `WavetableSynthNode::with_patch(WavetablePatch::default())`
/// produce byte-identical audio. Pins that `new` is the right
/// delegate for `with_patch(default)`.
#[test]
fn new_equals_with_patch_default() {
    let mut a = WavetableSynthNode::new();
    a.prepare(SR, BLOCK);
    let mut b = WavetableSynthNode::with_patch(WavetablePatch::default());
    b.prepare(SR, BLOCK);

    let mut a_buf = Vec::new();
    render_block(
        &mut a,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut a_buf,
    );
    let mut b_buf = Vec::new();
    render_block(
        &mut b,
        &[BlockEventInBlock {
            offset_in_block: 0,
            message: note_on(60, U16Velocity::HALF),
        }],
        &mut b_buf,
    );
    for i in 0..(2 * BLOCK) {
        assert_eq!(a_buf[i], b_buf[i], "sample {i}: new and with_patch(default) must match");
    }
}
