## Wavetable synth — multi-envelope + free modulation matrix plan (v2)

Grow `rawdaw-synth-wavetable` from v1 (three oscillators with
fixed-order PM/AM/RM, one shared LFO, one amp ADSR) to v2 by adding
**two more per-voice envelopes (ENV2 + ENV3) and a free modulation
matrix** that routes every source (envelopes, LFO, osc audio rate)
to every destination (filter cutoff, per-osc levels/tunes, PM/AM/RM
amounts, LFO rate, …). The matrix replaces v1's hardcoded
LFO→cutoff routing and v1's per-osc `mod_source` / `mod_mode` /
`mod_amount` fields with matrix slots. The
`modulator-index > carrier-index` constraint is lifted in favour of
a topological sort that rejects cycles at patch-apply time.

**Vital reference.** Vital has 3 envelopes (ENV1 = amp, ENV2/ENV3
free), 4 LFOs, 3 filters, all routed through a slot-based mod
matrix. v2 closes the envelope gap (we'll have 3 envelopes too) and
lands the matrix scaffolding; LFO multiplicity (4 LFOs), filter
multiplicity (3 filters), and the sample oscillator stay v3+.

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

**Multi-session scope.** This milestone is meaningfully bigger than
F1–F6. Plan to span 2–3 sessions of work; M0 (this doc) locks the
design so subsequent sessions can pick up any phase without
re-litigating decisions.

---

## Status — v2 DONE (M0–M6 all ✅)

Three envelopes + slot-based mod matrix is live. Joe confirmed the
M5 default patch sounds right on the round-1 fixture after a single
tuning iteration (filter resonance bumped from 0.7 to 2.5 so ENV2's
filter-cutoff sweep is audible as a resonant "wah" rather than a
gentle slope). 14 wavetable tests + 66 rawdaw-dsp tests pass;
clippy clean across all three feature builds.

- M0 ✅ this document, design decisions locked.
- M6 ✅ Plan doc closed out; project memory updated.
- M5 ✅ v2 default patch shipped. Filter base lowered to 800 Hz so
  ENV2's `+0.6` cutoff slot has room to sweep. Five matrix slots:
  LFO→cutoff, ENV2→cutoff (pluck), Osc1→PmAmountOf(0),
  Osc2→PmAmountOf(1), ENV3→PmAmountOf(0). Ear-test required one
  tuning iteration: bumped filter resonance from 0.7 to 2.5
  because the pluck character wasn't audible — turns out the
  load-bearing thing in a "plucked-filter" timbre is the resonant
  peak sweeping with the cutoff, not the cutoff motion itself.
  Other patch numbers (envelope shapes, slot amounts) held up.
- M4 ✅ Audio-rate matrix lands. `WavetableOscParams` shrunk to
  `{ tune_semitones, fine_cents, level }`; `ModMode` enum + the
  per-osc `mod_source` / `mod_mode` / `mod_amount` / `valid_mod_source`
  fields and method removed from `rawdaw-dsp`. v1's PM chain
  re-expressed as default-matrix slots (`Osc1 → PmAmountOf(0) @
  0.3`, `Osc2 → PmAmountOf(1) @ 0.15`). Voice's `tick()` switched
  to a two-stage evaluator: stage 1 ticks envelopes + iterates
  control-rate slots into `Modulations`; stage 2 renders oscs in
  `matrix.audio_rate_osc_order()` while folding in audio-rate slot
  contributions inline. RM uses a single `rm_amount_offset` per
  osc with the per-slot formula `(source - 1) × amount × scale` so
  the consumer's `output = raw × (1 + rm_amount_offset)` reproduces
  v1's single-slot RM dry/wet formula exactly. Added
  `osc_hz_base[NUM_OSCS]` so audio-rate `OscTune`/`OscFineTune`
  modulation re-derives Hz without re-running `note_offset_hz`.
  F4's modulation tests rewritten in matrix-slot terms; F3's
  coherent / detuned-three-osc tests install an explicit empty
  matrix so the default PM-chain slots don't perturb the headroom
  comparisons. 1 new dsp test (rm_contribution_uses_source_minus_one_formula);
  workspace dsp tests went 69 → 66 (lost 4 valid_mod_source tests
  + 1 default_params_are_inactive shape test = -5, gained the RM
  contribution test = +1, net -4). Wavetable suite stayed at 14
  tests. Deviation: dropped the "audio byte-identical to v1"
  done-when literal — M4 changes the audio path (single-osc
  baseline differs from v1 by ~f32-noise levels because the matrix
  evaluator orders operations slightly differently), but the
  modulation tests still hold the property pins.
- M3 ✅ Control-rate matrix wired into `WavetableVoice`. ENV1/2/3
  + LFO values feed `Modulations::add_contribution`; filter cutoff
  is now `PATCH_FILTER_CUTOFF_HZ + mods.filter_cutoff_hz_offset`
  instead of the hardcoded `lfo_value * PATCH_LFO_DEPTH_HZ`. The
  default matrix has one slot: `Lfo1 → FilterCutoff @ 0.1`, which
  resolves to the same 400 Hz swing v1 produced (0.1 × 4000 Hz
  scale = 400). v1 audio path is byte-identical — all 12 existing
  tests still pass, including the two that require sample-byte
  equality. 2 new M3 tests (12 → 14 wavetable tests):
  `empty_matrix_disables_lfo_cutoff_sweep` proves the matrix is
  load-bearing (without slots, no LFO movement), and
  `env2_to_cutoff_slot_changes_audio` proves ENV2 actually
  threads through. Dead constant `PATCH_LFO_DEPTH_HZ` removed and
  replaced with a comment pointing at the matrix slot.
- M2 ✅ `rawdaw-dsp::modulation` module landed. `ModSource` /
  `ModDestination` / `ModSlot` / `ModMatrix<const N>` /
  `Modulations` + `NUM_OSCS_PER_VOICE` constant. 12 new unit tests
  (57 → 69 dsp tests): empty matrix shape, control-rate-only
  preserves default order, audio-rate edge enforces source-before-
  carrier, v1 chained-PM topology preserves `[2, 1, 0]`,
  2-cycle / self-loop / out-of-range-index all rejected in debug,
  `add_contribution` routes correctly, plus enum classification
  checks. Topo sort uses fixed-size scratch buffers (`N ≤ 64`
  enforced via `debug_assert`) so `set_slots` is heap-alloc-free.
  Bonus: added `add_contribution` method on `Modulations` plus a
  per-destination `scale` constants module — patches read
  destination-native units (Hz, semitones, …) without each
  consumer redefining the scales. Not explicit in the M0 plan but
  load-bearing for M3's slot evaluation.
- M1 ✅ ENV2 + ENV3 envelopes live in `WavetableVoice`. All three
  trigger on `note_on`, release on `note_off`. ENV1 = amp (gates
  voice lifecycle); ENV2 + ENV3 tick every sample but their
  outputs are discarded (M3 will wire them through the matrix).
  Loose `PATCH_ATTACK_S` / `PATCH_DECAY_S` / … constants refactored
  into struct-shaped `PATCH_ENV1_PARAMS` / `PATCH_ENV2_PARAMS` /
  `PATCH_ENV3_PARAMS` `AdsrParams` constants. No new tests — the
  existing 12 wavetable tests serve as the audio-unchanged
  regression suite (in particular
  `three_coherent_oscs_match_single_osc_output` and
  `pm_amount_zero_is_a_noop` both require sample-byte equality, so
  any audio-path drift from ENV2/3 ticking would have surfaced
  there). Workspace tests + all three clippy gates clean.

---

## Design decisions locked in M0

### Three envelopes per voice; ENV1 is the amp, ENV2/ENV3 are free

`WavetableVoice` gains two more `Adsr` instances. All three trigger
on `note_on` and release on `note_off` simultaneously — Vital's
classic behaviour. ENV1 stays wired directly to the voice's output
amplitude (it gates `is_active()` and provides the post-mix gain);
ENV2 and ENV3 produce values that flow through the mod matrix to
whatever destinations the patch routes them at. Their hardcoded
ADSR params live as constants alongside ENV1's existing constants
until the patch-editor UI lands.

The amp envelope (ENV1) stays special-cased — it's the only
envelope that determines voice lifecycle (via `is_idle`). ENV2/ENV3
are pure modulators; they don't gate the voice. This matches Vital.

### Slot-based mod matrix with fixed-size slot array

The matrix carries a **fixed-size** slot array. v2 picks
`MOD_MATRIX_SLOTS = 16` — generous for a small synth, RT-safe (no
heap), and small enough to keep per-sample cost negligible (16
slots × 48 kHz × 16 voices = ~12 M slot evals/sec, well under audio
budget). Unused slots have `source = ModSource::None` and contribute
nothing.

Each slot is plain data:

```text
ModSlot {
    source:      ModSource,       // enum: None, Env1..3, Lfo1, Osc0..2, …
    destination: ModDestination,  // enum: FilterCutoff, OscLevel(i), …
    amount:      f32,             // signed; per-destination scale
}
```

Slots, sources, and destinations are all `Copy + Default` so a
patch's `[ModSlot; MOD_MATRIX_SLOTS]` array constructs without
ceremony.

### Sources output normalized values; destinations carry the scale

- Envelopes (ENV1/2/3) output `[0.0, 1.0]`.
- LFO outputs `[-1.0, 1.0]`.
- Oscillators output their raw audio sample, also `[-1.0, 1.0]`-ish
  for normalized wavetables.

The matrix doesn't re-normalize. Each destination defines what
`amount = 1.0` means in its native units:

| Destination | `amount = 1.0` ⇒ |
|---|---|
| `FilterCutoff` | + 4000 Hz of cutoff swing |
| `FilterResonance` | + 1.0 of Q (additive) |
| `OscLevel(i)` | + 1.0 of mix level (additive) |
| `OscTune(i)` | + 24 semitones |
| `OscFineTune(i)` | + 100 cents |
| `PmAmountOf(i)` | + 1.0 phase swing |
| `AmAmountOf(i)` | + 1.0 amplitude coefficient |
| `RmAmountOf(i)` | + 1.0 ring-mix wet |
| `LfoRate` | + 4 Hz LFO rate |

These per-destination scales keep `amount` itself dimensionless and
patches portable. The exact numbers above are starting values; M5
may retune them ear-test-driven.

### PM/AM/RM migrate from per-osc fields to matrix destinations

v1's per-osc `mod_source` / `mod_mode` / `mod_amount` fields are
**removed**. The same audio-rate routing is now expressed via
matrix slots whose destination is `PmAmountOf(i)` / `AmAmountOf(i)`
/ `RmAmountOf(i)`. The source for these is typically `OscN` (audio-
rate), but ENV2 phase-modulating osc[0] (a classic FM-organ patch)
or LFO ring-modulating osc[1] (audio-rate tremolo) are now also
expressible.

The destination's mathematical semantics stay the same as v1:

- `PmAmountOf(i)` adds to osc[i]'s phase-modulation input
  pre-wavetable-read (`tick_with_pm`).
- `AmAmountOf(i)` multiplies osc[i]'s post-tick sample by
  `(1 + value)`.
- `RmAmountOf(i)` mixes osc[i]'s post-tick sample with the
  modulated product per the v1 RM formula.

`WavetableOscParams` shrinks to `{ tune_semitones, fine_cents,
level }`. The `ModMode` enum can be deleted — modulation operations
are now identified by destination, not by per-osc mode.

### Cycle handling: topo sort + reject (not one-sample feedback)

Audio-rate matrix routing can express cycles
(`osc[0] → PmAmountOf(1) → osc[1] → PmAmountOf(0)`). At
`set_patch` time the matrix builds a per-sample dependency graph
of audio-rate sources / destinations and runs a topological sort.
**If sorting fails (cycle present), the offending slots are
disabled** (debug builds panic via `debug_assert`; release builds
sanitize the cycle-creating slots to `source = None`).

Trade-off recorded: Vital allows feedback via one-sample-delayed
audio-rate routing, which lets a patch route `osc[0] → osc[0]`
(self-FM) for classic FM bite. Implementing that means every
audio-rate-targeting slot reads its source's *previous*-sample
value, which adds a 21 µs delay (at 48 kHz) on every route — small
but observable in PM phase relationships. v2 picks topo-sort-and-
reject for simpler reasoning; v3 may add feedback as an opt-in
slot flag.

### Control-rate vs audio-rate sources tick in two stages

Audio-rate sources (`OscN`) only have valid values *during* the
per-sample render. Control-rate sources (`EnvN`, `LfoN`) are
independent of the audio-rate flow and can be ticked first.

Each sample:

1. Tick all envelopes (ENV1/2/3) → `[0, 1]` values.
2. Tick the LFO → `[-1, 1]` value.
3. Evaluate matrix slots whose source is control-rate, summing
   contributions per destination into a `Modulations` struct.
4. Render oscillators in topologically-sorted order, threading the
   audio-rate matrix contributions in as each carrier ticks.
5. Final mix, filter, amp envelope, output.

This staging keeps step 3 a single pass over all control-rate
slots, and step 4 evaluates audio-rate slots in the correct order
without iterating the whole matrix per osc.

### Per-destination contribution caching

The voice's per-sample tick evaluates the matrix once into a
fixed-size `Modulations` struct:

```text
Modulations {
    filter_cutoff_hz_offset: f32,
    filter_resonance_offset: f32,
    osc_level_offset: [f32; 3],
    osc_tune_offset: [f32; 3],
    osc_fine_offset: [f32; 3],
    pm_amount_offset: [f32; 3],
    am_amount_offset: [f32; 3],
    rm_amount_offset: [f32; 3],
    lfo_rate_offset: f32,
}
```

Each destination's *base* value (from the patch's static per-osc
params + per-filter params) is added in by the consumer (the voice's
filter / per-osc render). The struct is plain `Copy` data; no
allocations per tick.

### `OscTune` modulation at audio rate is intentionally messy

Patches can route audio-rate sources to `OscTune(i)`, producing
true vibrato or chaotic detune. The per-sample tune offset
re-derives the osc's `phase_inc` each sample, which costs one `powf`
per affected osc per sample. v2 accepts this cost — patches that
don't route to `OscTune` skip the work entirely (the `Modulations`
struct's `osc_tune_offset[i] == 0.0` shortcut). v3 may add a
note-on-only "static-tune" destination that re-tunes once per note,
not per sample.

### Patch shape after v2

```text
WavetablePatch {
    osc_params: [WavetableOscParams; 3],   // tune/fine/level only
    env_params: [AdsrParams; 3],            // amp + 2 free envelopes
    lfo_rate_hz: f32,                       // base LFO rate
    filter_cutoff_hz: f32,                  // base cutoff
    filter_resonance: f32,                  // base Q
    matrix: [ModSlot; MOD_MATRIX_SLOTS],    // all routing lives here
}
```

The `WavetableSynthNode` owns one canonical patch and propagates it
to every voice at `prepare()` (and via `set_patch_for_test` in
unit tests). Voices hold a sanitized copy.

---

## Phase M0 — Plan + design decisions ◀ this doc

**Goal.** Lock the architecture before any DSP code lands.

**Done when.** This doc is committed; the design-decisions section
above describes every decision the M1–M5 phases will rely on.

---

## Phase M1 — ENV2 + ENV3 envelopes in `WavetableVoice`

**Goal.** Add two more per-voice envelopes alongside the amp ADSR.
No matrix yet — the new envelopes tick every sample and produce
values, but nothing in the voice consumes them. Pure groundwork.

**Steps.**

- `WavetableVoice` grows `env2: Adsr`, `env3: Adsr`. Initialize
  alongside `amp` in `WavetableVoice::new`.
- `prepare()` calls `env2.prepare(sr)` / `env3.prepare(sr)` and
  installs hardcoded `AdsrParams` constants
  (`PATCH_ENV2_*` / `PATCH_ENV3_*`) — starting values picked for
  M5's default patch, can be retuned ear-test-driven.
- `note_on` / `note_off` trigger / release ENV2 + ENV3 alongside
  ENV1.
- `tick` calls `env2.tick()` and `env3.tick()` every sample (their
  outputs are stored in stack-locals and currently ignored — M3
  will start consuming them).

**Done when (✅ met).** Audio output unchanged from F5 — adding
ticking envelopes whose outputs nobody consumes doesn't shift the
signal. Verified via the v1 wavetable suite: all 12 tests pass,
including the two that require *sample-byte* equality
(`three_coherent_oscs_match_single_osc_output`,
`pm_amount_zero_is_a_noop`). Deviation from the plan: dropped the
"all three envelopes trigger on note_on" assertion test — it
required either a peek-into-voice affordance that doesn't fit the
existing test shape, or a structural test that would be tautological
against the obvious source. The sample-equality tests catch any
audio-path drift, which is the load-bearing pin.

---

## Phase M2 — `rawdaw-dsp::modulation` matrix types

**Goal.** Land the matrix data model in `rawdaw-dsp`. Pure types +
small helpers; no integration with the synth yet.

**Steps.**

- New module `rawdaw-dsp::modulation` (sibling to `envelope`,
  `oscillator`, …).
- Public types:
  - `ModSource { None, Env1, Env2, Env3, Lfo1, Osc0, Osc1, Osc2 }`.
    `Default = None`.
  - `ModDestination { FilterCutoff, FilterResonance, OscLevel(u8),
    OscTune(u8), OscFineTune(u8), PmAmountOf(u8), AmAmountOf(u8),
    RmAmountOf(u8), LfoRate }`. The `u8` carries an osc index
    `0..=2`. `Default = FilterCutoff` (an arbitrary safe pick;
    slots with `source = None` ignore their destination anyway).
  - `ModSlot { source, destination, amount }`. `Copy + Default`.
  - `ModMatrix<const N: usize>` — wraps `[ModSlot; N]` plus a topo-
    sort cache. `Default` produces an empty matrix.
- `ModMatrix::set_slots(&mut self, slots: [ModSlot; N])`:
  validates routing (no cycles in the audio-rate subgraph),
  caches the sorted osc render order, and stores the slots. Cycles
  produce a `debug_assert!` panic; in release the cycle-creating
  slots are silently set to `source = None`.
- `ModMatrix::audio_rate_osc_order(&self) -> [u8; 3]`: returns the
  topologically-sorted osc index order computed by `set_slots`. The
  voice's per-sample tick consults this to know what to render
  first.
- `ModMatrix::active_slots(&self) -> impl Iterator<Item = &ModSlot>`:
  yields slots with `source != ModSource::None`. The voice's
  per-sample tick iterates these once to assemble the
  `Modulations` struct.
- `Modulations` struct (the per-sample destination contributions
  bag described in M0 design decisions) lives alongside the
  matrix types.

**Done when (✅ met).** Unit tests pin: (a) empty matrix produces
an empty `active_slots` iterator and a `[2, 1, 0]` default order
(matches v1's render direction); (b) audio-rate edge enforces
source-before-carrier ordering; (c) 2-cycle, self-loop, and
out-of-range osc index all rejected in debug (debug_assert panic);
(d) topo sort uses fixed-size scratch buffers, no heap. Bonus pins:
v1's chained-PM topology preserves `[2, 1, 0]`,
`add_contribution` routes to the right `Modulations` field,
control-rate-only matrices keep the default order.

---

## Phase M3 — Control-rate matrix wired into `WavetableVoice`

**Goal.** Replace v1's hardcoded `LFO → filter cutoff` routing
with a matrix slot. The voice now evaluates the control-rate
subset of the matrix per sample and uses the resulting
`Modulations` to modulate the filter cutoff (and resonance, and
LFO rate, once those slots exist in the default patch).

**Steps.**

- `WavetableVoice` grows a `matrix: ModMatrix<MOD_MATRIX_SLOTS>`
  field (sanitized copy from the node's canonical patch, same
  pattern as `osc_params`).
- The voice's per-sample tick:
  1. Tick ENV1, ENV2, ENV3, LFO into local f32s.
  2. Iterate `matrix.active_slots()`; for each slot whose source
     is control-rate (ENV/LFO), add its contribution to the
     destination's slot in a stack-local `Modulations` struct.
  3. Compute the filter cutoff as
     `PATCH_FILTER_CUTOFF_HZ + modulations.filter_cutoff_hz_offset`
     (with `SvfLowpass::set_cutoff` clamping as before).
  4. Render oscillators (still in v1's `osc[2] → osc[1] → osc[0]`
     order — M4 generalizes this).
  5. Audio-rate slot destinations (`PmAmountOf` etc.) stay
     hardcoded to v1's per-osc fields for this phase; M4 migrates
     them.
- The hardcoded `LFO → cutoff` routing is removed from
  `WavetableVoice::tick`. The node's default patch grows a matrix
  with one slot: `{ source: Lfo1, destination: FilterCutoff,
  amount: 0.1 }` (0.1 × destination scale of 4000 Hz = 400 Hz
  swing, matching v1).

**Done when (✅ met).** Audio byte-identical to v1 with the
default matrix (the one LFO→cutoff slot at `amount=0.1` resolves
to v1's 400 Hz swing). Verified via the 12 existing wavetable
tests still passing, including two that require sample-byte
equality. 2 new tests pin: (b) ENV2→cutoff slot audibly diverges
from no-slot rendering; (a)+(c) merged into
`empty_matrix_disables_lfo_cutoff_sweep` — an empty matrix produces
non-silent audio but differs from the default matrix render
(proves the slot is load-bearing). Deviation: didn't write a
separate "matrix with zero amount = no slot" pin because
`set_slots` doesn't distinguish them; both produce zero
contribution.

---

## Phase M4 — Audio-rate matrix destinations (PM/AM/RM via slots)

**Goal.** Migrate v1's per-osc `mod_source` / `mod_mode` /
`mod_amount` to matrix slots. The `ModMode` enum is removed from
`rawdaw-dsp`; `WavetableOscParams` shrinks. The topological sort
in `ModMatrix::set_slots` now matters — it determines osc render
order based on audio-rate dependencies between oscs.

**Steps.**

- `WavetableOscParams` shrinks to
  `{ tune_semitones, fine_cents, level }`. Remove `mod_source`,
  `mod_mode`, `mod_amount`, `valid_mod_source`. Delete `ModMode`
  enum and its module from `rawdaw-dsp`.
- `WavetableVoice::tick` per-osc rendering switches from the
  `(mod_source, mod_mode)` tuple match to consulting the
  `Modulations` struct's per-osc `pm_amount_offset` /
  `am_amount_offset` / `rm_amount_offset`:
  - PM: `osc.tick_with_pm(table, pm_amount_offset[i])`. The
    matrix has already multiplied source × slot amount.
  - AM: `osc.tick(table) * (1 + am_amount_offset[i])`.
  - RM: dry/wet mix using `rm_amount_offset[i]` as wet coefficient.
- Audio-rate slot evaluation happens inline with the osc render
  loop: before ticking osc[i], read the slots that target
  `PmAmountOf(i)` / `AmAmountOf(i)` / `RmAmountOf(i)` whose
  source is `OscN`, multiply the source's already-rendered sample
  by the slot's amount, accumulate into the per-osc offset.
- Topo-sort enforces that any `OscN`-sourcing slot's source osc
  is rendered before its destination osc. The sorted order from
  `ModMatrix::audio_rate_osc_order` drives the render loop.
- v1's per-osc PM chain (osc[0] PM by osc[1] @ 0.3, etc.) is
  expressed in the default patch as three matrix slots.

**Done when (✅ met).** F4's modulation tests (PM zero is no-op,
Pm/Am/Rm engaged change signal, modes mutually distinct) all hold
when rewritten in matrix-slot terms. Cycle rejection covered in
M2's dsp tests. Deviation: the "byte-identical to v1" pin was
softened — the matrix evaluator reorders some floating-point
operations vs. v1's per-osc-field tuple match, producing
sub-perceptual `f32`-noise differences. Property pins (sample
equality at zero amount, audible divergence at non-zero amount,
modes distinct) hold; absolute byte-identity to v1 doesn't, and
shouldn't be the load-bearing pin anyway since the per-osc tuple
match was being torn out.

---

## Phase M5 — v2 default patch (multi-envelope + matrix-rich)

**Goal.** Replace the F5 v1 patch with a v2 patch that exercises
ENV2, ENV3, and the matrix. Ear-test driven, same shape as F5.

**Steps.**

- Pick a v2 default patch. Working starting point:
  - osc[0]: saw, tune 0, fine 0, level 1.0.
  - osc[1]: saw, tune +12, fine 0, level 0.4.
  - osc[2]: saw, tune +19, fine 0, level 0.0 (modulator-only).
  - ENV1 (amp): 5 ms / 80 ms / 0.7 / 200 ms (unchanged).
  - ENV2 (filter pluck): 5 ms / 250 ms / 0.0 / 50 ms — quick
    decay into nothing, classic plucked filter envelope.
  - ENV3 (modulation accent): 5 ms / 400 ms / 0.3 / 200 ms — slow
    decay that adds movement to a modulation amount.
  - LFO: 4 Hz sine (unchanged).
  - Filter: 800 Hz cutoff, Q = 0.7 (lower than v1's 1500 Hz so
    ENV2's filter sweep is audible — ear-test will tune).
  - Matrix slots:
    1. `LFO → FilterCutoff @ 0.1` — preserves v1's LFO-cutoff
       movement.
    2. `ENV2 → FilterCutoff @ 0.6` — filter envelope opens at note
       onset, decays into nothing.
    3. `Osc1 → PmAmountOf(0) @ 0.3` — v1's primary PM route.
    4. `Osc2 → PmAmountOf(1) @ 0.15` — v1's secondary PM route.
    5. `ENV3 → PmAmountOf(0) @ -0.15` — ENV3 pulls the PM depth
       down over time so the sound brightens then mellows.
- Ear-test on the round-1 fixture; tune until the sound is
  recognizably more *moving* than v1 (filter sweep + modulation
  accent) without clipping.

**Done when (✅ met).** Joe's ear test confirmed the plucked-
filter envelope on the round-1 fixture after one tuning iteration
(filter resonance bumped from 0.7 to 2.5). No clipping, workspace
tests + all three clippy gates clean. Deviation: the plan's prose
called ENV3's PM-amount slot "brightens then mellows" but the
sign (`-0.15`) actually produces "starts clean, fills in" — the
sign was kept as-is and the doc comment in `PATCH_MATRIX_SLOTS`
explains both phrasings so future patch tuning can pick.

---

## Phase M6 — Plan + memory updates

**Goal.** Close out v2.

**Steps.**

- This doc's per-phase "Done when" markers updated with ✅ status
  and any deviations recorded.
- `project_status` memory updated: wavetable synth v2
  (multi-envelope + free mod matrix) done; growth list reflects
  what v2 leaves on the table (additional LFOs, additional
  filters, sample oscillator, factory wavetable bank, exponential
  ramps, …).

---

## Future plan needed: synth UI integration

Carried over from the F-plan and amplified — v2 lands several new
patch fields (ENV2/3 params, 16 mod-matrix slots, per-slot source
/destination/amount) that have nowhere to live in a UI yet. The
synth-UI integration plan called out in
`docs/wavetable-synth-fm-plan.md` is the same one needed here; this
milestone just makes its absence more obvious because the patch
surface gets bigger.

Specifically the matrix editor is its own non-trivial UI design
problem — Vital's matrix view is a scrollable list of slots with
inline source/destination dropdowns and an amount knob, and
porting that to Rinch's `rsx!` + signal idiom needs its own
exploration. The synth-UI plan should size for this.

---

## Out of scope (v3+ growth)

These are deliberate v2 omissions, called out so the v2 done-when
isn't muddled with them:

- Additional LFOs (Vital has 4; we keep 1).
- Additional filters (Vital has 3; we keep 1 per voice).
- Sample oscillator (Vital's 4th audio source).
- One-sample-delayed feedback in the audio-rate matrix (Vital
  allows; v2 rejects cycles).
- Factory wavetable bank with per-osc wavetable selection +
  wavetable position morphing (inherited v1+ out-of-scope item).
- Exponential envelope ramps.
- LFO waveforms beyond sine; LFO tempo sync.
- Filter modes beyond lowpass.
- Stereo unison + detune.
- Sample import / wavetable import.
- Synth editor UI panel (lives in its own dedicated plan).
- MPE / micro-tuning.
- Per-voice effects.
- Random and macro mod sources (Vital has both; v2 doesn't).
- Curve / quantize per-slot transforms (Vital has both per slot).

Inherited from `wavetable-synth-fm-plan.md`'s out-of-scope list.
