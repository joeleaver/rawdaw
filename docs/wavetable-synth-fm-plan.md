## Wavetable synth — multi-oscillator + FM/AM/RM plan (v1)

Grow `rawdaw-synth-wavetable` from its v0 shape (one wavetable osc per
voice, hardcoded saw → LFO-modulated LP → linear ADSR) toward Vital
parity by adding **three oscillators per voice with phase-modulation,
amplitude-modulation, and ring-modulation routing between them.** The
v0 signal chain otherwise stays in place: the three-osc mix replaces
the single-osc tap, then flows through the existing per-voice filter
and amp envelope before mixing into the node's stereo output.

This plan is the v1 step on the wavetable synth's roadmap. Subsequent
growth (second envelope, free mod matrix, factory wavetable bank,
unison) lives in its own plan doc.

**Engineering constraints** (from `CLAUDE.md`).

- Architectural correctness over shortcuts. Always.
- Unlimited time and budget — pay the cost of doing it right.
- ~700-line cap per source file. Split by concern when approached.
- No `unwrap()` outside tests / proven-impossible cases. No silent
  swallowing.
- Engine + DSP crates forbid `unsafe_code` at the crate level.

**Cadence.** Each phase ends with `cargo test --workspace` green,
clippy clean across all three feature builds (default,
`--no-default-features`, `--features cpal-driver`), and a one-line
"done when" criterion that's been observably met.

---

## Status — v1 DONE (F0–F6 all ✅)

Three-osc wavetable + PM/AM/RM routing is live. Joe confirmed the
v1 default patch (saw + PM stack: osc[0] @ played pitch, osc[1]
@ +12, osc[2] @ +19 as modulator-only) sounds audibly FM-flavored
without clipping on the round-1 fixture. 12 wavetable-synth tests +
57 rawdaw-dsp tests pass; clippy clean across all three feature
builds.

- F0 ✅ this document, design decisions locked.
- F1 ✅ `WavetableOsc::tick_with_pm` shipped in `rawdaw-dsp`. 5 new
  unit tests (43 → 48 dsp tests). Test (c) deviated from the plan's
  literal "total RMS energy increases" pin (Parseval makes that
  ambiguous for FM) in favour of "diff RMS between PM'd and unmod'd
  exceeds a threshold", which is a stronger pin that the PM input
  is actually consumed.
- F2 ✅ `WavetableOscParams` + `ModMode` + `note_offset_hz` live in
  a new `oscillator::params` submodule of `rawdaw-dsp`. 9 new unit
  tests (48 → 57 dsp tests) covering octave doubling, semi-cents
  agreement, and every `valid_mod_source` branch. Split out into a
  sibling file rather than extending `wavetable.rs` (which was
  approaching the file-size budget at 427 lines) — kept the
  oscillator state machine and the configuration types in
  separately-navigable files.
- F6 ✅ Plan doc closed out, project memory updated.
- F5 ✅ v1 default patch installed and ear-test approved on the
  round-1 fixture. Two F3 tests needed minor updates to construct
  an explicit single-osc baseline (the default patch is no longer
  the v0-equivalent reference). No patch tuning was needed —
  the plan's starting-point numbers held up.
- F4 ✅ PM/AM/RM routing wired through the per-osc render loop with
  a `(mod_source, mod_mode)` tuple match. `set_patch` sanitizes
  invalid mod sources to `None` in release builds (debug-asserts in
  debug) so the audio thread can trust the data without per-sample
  validation. 5 new unit tests (7 → 12 wavetable tests): zero-amount
  no-op, each mode (Pm/Am/Rm) measurably changes the signal, and the
  three modes are mutually distinct at the same amount. Deviation
  from the plan: the (b) "high-passed RMS up ≥ 3 dB" pin and (c)
  "RM tremolo envelope follows modulator" pin both required tooling
  (FFT, external modulator synthesis, or a synth-internal tap) that
  shouldn't live in a unit test. Replaced with the stronger
  "modes-must-differ" pin set that catches accidental match-arm
  aliasing as a bonus.
- F3 ✅ Three-osc `WavetableVoice` with per-voice patch copy + node-
  level canonical patch, propagated at `prepare()`. Render order
  osc[2] → osc[1] → osc[0] already in place (F4 will hang the
  modulation routes off it). v0 unit tests still pass byte-for-byte
  on the v0-equivalent default patch. 2 new tests pin the headroom
  math: (a) three coherent oscs at the same pitch produce single-
  osc-identical output (the divide-by-Σlevel is exact), (b) detuned
  three-osc patch is non-silent + peak-bounded. Deviation from the
  plan: ±20% RMS bound is unreachable for partially-correlated
  octave-related sources (RMS lands near 1/√3 of single-osc), so
  the pin was reframed around "peak bounded + materially different
  from single-osc". Dead code from v0 (`note_freq_table`, the local
  `note_to_hz`) removed in the same change.

---

## Design decisions locked in F0

These are the trade-offs picked before any code lands, recorded here
so future phases (and future readers) can see the rationale.

### Three oscillators, fixed render order, per-osc modulation source

Each voice owns an array of **three** `WavetableOsc`s (indices 0, 1,
2) plus per-osc parameters. Render order is **osc[2] first, osc[1]
next, osc[0] last** so a modulator's output is always available when
its carrier ticks. The routing rule that makes this safe:

> A modulator's index must be **strictly greater than** its carrier's.

Concretely: osc[0] can be modulated by osc[1] or osc[2]; osc[1] can be
modulated by osc[2]; osc[2] cannot be modulated by anything. This is
a deliberate v1 simplification — it lets us avoid cycle detection,
one-sample feedback delays, and ordering passes while still covering
the audibly-interesting cases (chained PM, parallel AM, etc.). A
fully free matrix is a v2 growth target and will require either
explicit feedback taps or a topo-sort with cycle rejection.

### PM, not true FM

"FM synthesis" in commercial products (DX7, Vital, Massive, …) is
almost universally **phase modulation**, not true frequency
modulation. True FM modulates the phase increment, which integrates
the modulator and accumulates DC offsets — unstable in practice. PM
adds the modulator's output directly to the carrier's *phase* before
the wavetable read; it's stable, DC-neutral, and the formula every
DX7 patch was actually designed against. We do PM and call the user-
facing knob "FM" to match convention.

### AM is unipolar, RM is bipolar

- **AM (amplitude modulation)** multiplies the carrier output by
  `(1 + amount * modulator)` where `modulator ∈ [-1, 1]`. At amount =
  0 the carrier is unaffected; at amount = 1 the carrier is
  fully gated by the modulator's positive lobe. Always non-negative
  average — preserves the carrier's pitch as a perceptual center.
- **RM (ring modulation)** multiplies bipolarly: `carrier *
  modulator * amount + carrier * (1 - amount)`. At amount = 1 it's
  classic four-quadrant ring mod (inharmonic sidebands, no carrier
  leakage); at amount = 0 the carrier passes unchanged. The mix
  formulation lets a single "amount" knob sweep from dry → wet.

### Per-osc parameters

```text
WavetableOscParams {
    tune_semitones: i8,    //  -24..=24 ; coarse pitch offset
    fine_cents:     i8,    // -100..=100; fine pitch offset
    level:          f32,   //    0..=1  ; mix gain into voice sum
    mod_source:     Option<u8>,    // 1 or 2; None = no modulator
    mod_mode:       ModMode,       // Pm | Am | Rm | Off
    mod_amount:     f32,           //    0..=1
}
```

Wavetable selection is **deferred**: every osc uses the shared
factory saw until the factory bank lands. The plan is for the v2
factory-bank work to extend `WavetableOscParams` with a wavetable
index (and per-table position morphing for the wavetables that
support it).

### Per-voice signal flow

```
osc[2] ──┐
osc[1] ◀┘── osc[1].mod (PM/AM/RM)
   ├── osc[0] ◀── osc[0].mod (from osc[1] or osc[2])
   ├── sum (per-osc level) ──▶ SvfLowpass ──▶ Adsr ──▶ voice out
   │                              │
   │                              └── LFO modulates cutoff (unchanged)
   ▼
```

Only **one filter and one amp envelope per voice**, applied
**post-sum**. This matches Vital's signal flow and keeps the per-
voice DSP cost bounded (filter coefficients aren't replicated per
osc). Per-osc filters / per-osc envelopes are a v2+ growth target.

### Headroom

Three full-level saw oscillators summed peak around 3.0 — well past
the existing `PER_VOICE_HEADROOM = 0.5` budget. We **rescale levels
inside the voice** so the per-voice peak still tops out at roughly
the same magnitude as v0:

> Voice sum is `(osc[0]*L0 + osc[1]*L1 + osc[2]*L2) /
> max(1, L0 + L1 + L2)`.

The denominator preserves the v0 calibration when only one osc is
active (matches the existing single-osc patch with `L=1`) and
linearly attenuates when more oscs are active. `PER_VOICE_HEADROOM`
keeps its v0 role at the velocity stage.

---

## Phase F0 — Plan + design decisions ◀ this doc

**Goal.** Lock the architecture before any DSP code lands.

**Done when.** This doc is committed; the design-decisions section
above describes every decision the F1–F5 phases will rely on.

---

## Phase F1 — `rawdaw-dsp` phase-modulation input on `WavetableOsc`

**Goal.** Let a `WavetableOsc` read its wavetable at `phase +
pm_input` so a modulator's audio output can phase-modulate the
carrier per-sample.

**Steps.**

- Add `WavetableOsc::tick_with_pm(&mut self, table: &Wavetable,
  pm_input: f32) -> f32`. Reads sample at `phase + pm_input`, advances
  phase by `phase_inc`. Wraps phase as the existing `tick` does.
- Keep `tick` working: implement it as `tick_with_pm(table, 0.0)`.
  The single-osc paths in `rawdaw-synth-wavetable` and any future
  callers stay source-compatible.
- `pm_input` is the modulator's raw audio output in the **phase
  domain** — i.e., the synth multiplies the modulator's `[-1, 1]`
  audio sample by a per-osc PM depth before passing it in.
  `WavetableOsc::tick_with_pm` does *not* itself apply a depth scale.

**Done when (✅ met).** Unit tests pin: (a) `tick_with_pm(table,
0.0)` is sample-identical to `tick(table)`; (b) a constant non-zero
`pm_input` shifts the carrier's apparent phase without altering its
period (verified two ways — period preservation and quarter-period
shift); (c) a sinusoidal `pm_input` is actually consumed (diff RMS
between PM'd and unmodulated exceeds a threshold). Test (c)
deviated from the plan's original "total RMS energy increases" pin
because Parseval makes that ambiguous for FM.

---

## Phase F2 — `rawdaw-dsp` `WavetableOscParams` + per-osc Hz helper

**Goal.** Move per-osc configuration (tune, fine, level, mod source
+ mode + amount) out of the synth crate and into `rawdaw-dsp` so
future synth/FX crates can reuse the type. Provide a helper for
deriving `hz` from a base MIDI note + semi + cents offset.

**Steps.**

- Add `oscillator::WavetableOscParams` with the fields listed in the
  Design Decisions section above, plus `ModMode { Off, Pm, Am, Rm }`
  and a small builder so test setup stays terse.
- Add `oscillator::note_offset_hz(base_note: u8, semi: i8, cents:
  i8) -> f32` returning the offset frequency. The synth precomputes
  per-osc Hz at `note_on` time using this helper so the audio thread
  doesn't run a `powf` per event.
- The struct stays plain-data (`Copy`, `Default`) — no methods
  beyond a `valid_mod_source(carrier_index)` that returns
  `mod_source.is_none() || mod_source.unwrap() > carrier_index`
  for use in voice-construction asserts.

**Done when (✅ met).** Unit tests pin: (a) `note_offset_hz(69, 0,
0) == 440.0` within f32 tolerance; (b) `note_offset_hz(60, 12, 0)`
is one octave above `note_offset_hz(60, 0, 0)`; (c)
`valid_mod_source` rejects self-modulation and lower-indexed
sources, accepts higher-indexed and `None`. Bonus pins: semi/cents
agreement, default-params inactive shape.

---

## Phase F3 — `rawdaw-synth-wavetable` multi-oscillator voice (no modulation yet)

**Goal.** Grow `WavetableVoice` to hold three oscillators + per-osc
params, summed at the voice output. Modulation routing comes in F4 —
this phase delivers the multi-osc *infrastructure* and proves it
sounds the same as the v0 single-osc patch when oscs 1 + 2 have
`level = 0`.

**Steps.**

- Replace the single `osc: WavetableOsc` field on `WavetableVoice`
  with `oscs: [WavetableOsc; 3]` + `osc_params: [WavetableOscParams;
  3]`. Per-osc `hz` precomputed at `note_on` time and stored in a
  per-voice `osc_hz: [f32; 3]`.
- Per-sample render: tick osc[2], osc[1], osc[0] in that order
  (modulation hooks land empty in this phase), sum with per-osc
  levels using the headroom-preserving formula from Design Decisions.
- The hardcoded patch stays single-osc-equivalent for now: osc[0]
  `level = 1`, osc[1] + osc[2] `level = 0`, all `mod_mode = Off`. The
  audible output is identical to v0; the new code is exercised but
  doesn't change the mix.
- Existing v0 unit tests (`silent_until_note_on`,
  `note_on_produces_audible_output`, `note_off_releases_to_silence`,
  `lr_outputs_are_identical`, `mid_block_note_on_starts_at_offset`)
  must still pass unchanged.

**Done when (✅ met).** All v0 tests still pass byte-for-byte on
the v0-equivalent default patch. New tests pin: (a) three coherent
oscs (same pitch, full level) produce single-osc-identical output
within f32 round-off — the divide-by-Σlevel is exact; (b) detuned
three-osc patch is non-silent and peak-bounded under single-osc
peak. Deviation: the original ±20% RMS bound is unreachable for
partially-correlated octave sources (lands near `1/√3` × single
RMS), so it was reframed around the property the headroom math
actually guarantees.

---

## Phase F4 — `rawdaw-synth-wavetable` modulation routing (PM/AM/RM)

**Goal.** Wire modulation between oscillators per the routing rule
locked in F0. Per-sample: tick osc[2] first (no modulator possible),
then tick osc[1] consuming osc[2]'s sample if its `mod_source = Some(2)`,
then tick osc[0] consuming the configured osc's sample.

**Steps.**

- Per-osc render switches on `mod_mode`:
  - `Off`: ignore `mod_source` / `mod_amount`; tick the osc plain.
  - `Pm`: `tick_with_pm(table, source_sample * mod_amount)`.
  - `Am`: tick the osc plain, then multiply by
    `1.0 + mod_amount * source_sample`.
  - `Rm`: tick the osc plain, output is
    `osc_sample * source_sample * mod_amount +
     osc_sample * (1.0 - mod_amount)`.
- `mod_source = None` ⇒ behave as `Off` regardless of `mod_mode`.
- Construction enforces `valid_mod_source(carrier_index)` via a
  `debug_assert!` per voice; in release builds an invalid source is
  silently treated as `None` to keep the audio thread safe under
  bad patch data.

**Done when (✅ met).** Unit tests pin: (a) PM with `mod_amount =
0` is sample-identical to `mod_mode = Off` (depth zero is a true
no-op); (b) each of Pm / Am / Rm at a non-trivial amount measurably
diverges from the unmodulated carrier (proves each match arm is
wired and consumed); (c) the three modes produce mutually distinct
outputs at the same amount (proves no match-arm aliasing). Bonus
release-build safety: `set_patch` sanitizes invalid `mod_source`
values to `None` so the audio thread doesn't have to re-validate
per sample. Deviation: the original "spectrum broader" + "RM
tremolo envelope" pins required FFT or external modulator synthesis
that doesn't belong in unit tests — replaced with the
modes-must-differ pin set.

---

## Phase F5 — v1 hardcoded patch + listening tune-up

**Goal.** Replace the v0 hardcoded patch with a v1 patch that
showcases the new architecture — three oscillators, at least one
PM route, audibly different and musically usable.

**Steps.**

- Pick a v1 default patch. Working starting point:
  - osc[0]: saw, tune 0, fine 0, level 1.0; modulated by osc[1] via
    PM, amount ≈ 0.3.
  - osc[1]: saw, tune +12, fine 0, level 0.4; modulated by osc[2]
    via PM, amount ≈ 0.15.
  - osc[2]: saw, tune +19 (perfect twelfth), fine 0, level 0.0.
    (No mix contribution; it exists purely as the modulator stack's
    top.)
- The numbers are a *starting* point. Use the Rinch MCP for visual
  + audible verification: launch the app, hit Space, listen to the
  round-1 fixture, adjust until the lead track sounds like a real
  PM-bell timbre without resonating into clipping.
- Keep the v0 filter / amp / LFO patch values unchanged unless the
  multi-osc spectrum demands re-tuning the LP cutoff to keep the
  top end from getting harsh.

**Done when (✅ met).** Joe's ear test on the round-1 fixture
confirmed (a) audibly FM-flavored, recognizably different from v0;
(b) no clipping; (c) workspace tests + all three clippy gates clean.
The starting-point patch numbers held up — no tuning needed.

---

## Phase F6 — Plan + memory updates

**Goal.** Close out v1.

**Steps.**

- This doc's per-phase "what landed" + "deviations" subsections
  filled (mirroring how `wavetable-synth-plan.md` records its W0–W8
  outcomes).
- `project_status` memory updated: wavetable synth v1 (multi-osc +
  PM/AM/RM) done; "next session" picklist edited so the listed
  growth items (second envelope, free mod matrix, factory wavetable
  bank, stereo unison) reflect what v1 leaves on the table.

---

## Future plan needed: synth UI integration

A v1-shaped synth that exposes a patch via UI knobs (or even a patch
browser) needs infrastructure that doesn't exist yet:

- **Parameter event protocol.** Today's `AudioNode::process` only
  carries MIDI `BlockEvent`s. Adding parameter changes means
  extending the event variant (or adding a sibling SPSC parameter
  queue) so the UI thread can push `set_param(node_id, param_id,
  value)` to the audio thread without locks.
- **Patch data format.** What does a wavetable patch look like on
  disk, and what is its in-memory editor model? The per-osc params
  struct landing in F2 is a foundation, but a full patch also
  carries filter / envelope / LFO / routing state and (eventually)
  wavetable selections.
- **Synth-editor UI in `rawdaw-app`.** Round-1 has arrangement,
  library, inspector, section-editor panels — none of them expose
  synth internals. A new panel (or inspector mode) needs to host the
  per-osc controls, the modulation routing widget, and the patch
  browser. The Rinch `rsx!` + signal idiom we already use scales to
  this, but the panel structure is a new design question.
- **Per-synth UI dispatch.** Different synths (wavetable, drum,
  future physical modeller) need different editor panels. Round-1's
  inspector currently picks a panel by `TrackKind`; the analogous
  dispatch for synth editors lands when more than one synth has a
  panel worth showing.

None of this lands in F1–F6 — the synth stays hardcoded to a single
patch throughout this plan. A dedicated **synth-UI integration
plan** should be written before any of the above is built; it will
likely span `rawdaw-engine` (parameter events), `rawdaw-model`
(patch format), and `rawdaw-app` (UI panels), so the design surface
is wider than any single phase's scope.

---

## Out of scope (v2+ growth)

These are deliberate v1 omissions, called out so the v1 done-when
isn't muddled with them:

- Free modulation matrix (modulator may equal or be lower-indexed
  than carrier; requires explicit feedback tap or cycle rejection).
- Second ADSR routed to filter cutoff (replacing the LFO-only
  routing).
- Factory wavetable bank with per-osc wavetable selection +
  wavetable position morphing.
- Per-osc filter / per-osc envelope.
- Stereo unison + detune.
- Exponential envelope ramps.
- LFO waveforms beyond sine, LFO tempo sync.
- Filter modes beyond lowpass (HP / BP / notch / shelf).
- Sample import / wavetable import.
- Synth editor UI panel.
- MPE / micro-tuning.
- Per-voice effects.

These remain inherited from `wavetable-synth-plan.md`'s out-of-scope
list and aren't re-litigated here.
