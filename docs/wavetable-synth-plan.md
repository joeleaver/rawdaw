# Wavetable / FM synth — milestone plan

Replace the `SineNode` placeholder behind round-1's lead track with a
real wavetable/FM synth modelled on Vital. v0 is the smallest synth
that's still a real instrument: one wavetable oscillator, one ADSR,
one LFO, one state-variable lowpass filter. Subsequent passes grow
toward Vital parity (multi-osc + FM/AM, second envelope, full mod
matrix, factory wavetable bank, unison, FX).

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

## Status — v0 DONE (W0–W8 all ✅)

The wavetable synth's v0 is live. The Melodic-role track in the
round-1 fixture plays through `WavetableSynthNode` while bass /
drums / pad stay on the sine placeholder until their domain-specific
synths land.

Verified end-to-end: launch app, hit Space, lead track plays through
the band-limited saw → LFO-modulated lowpass → linear ADSR signal
chain. 51 tests across `rawdaw-dsp` (29) + `rawdaw-synth-wavetable`
(5) + previously-passing rawdaw workspace tests (the rest), clippy
clean across default / `--no-default-features` / `--features
cpal-driver` builds.

---

## Phase W0 — Plan + workspace scaffolding ✅ this doc

**Goal.** Lock the crate split + plan before any DSP code lands.

**Decision: two new crates.**

- `rawdaw-dsp` — leaf-level DSP primitives (oscillators, envelopes,
  filters, LFOs, voicing). No deps on `rawdaw-engine` or
  `rawdaw-model`. Every future synth / FX crate consumes from here.
- `rawdaw-synth-wavetable` — the Vital-class synth. Wraps
  `rawdaw-dsp` primitives in an `AudioNode` implementation.
  Depends on `rawdaw-dsp` + `rawdaw-engine` + `rawdaw-model`.

Alternative considered: one `rawdaw-synth` crate with submodules
for wavetable / physical / drum. Rejected because the future
`rawdaw-synth-physical` will have a completely different
dependency surface (numeric ODE solvers, waveguide-mesh types)
that doesn't belong alongside a wavetable synth's import
infrastructure. Split the crates now while there's only one synth
to be moved.

**Done when (✅ met).** Both crates compile (empty libs); workspace
`cargo build` passes; this doc is committed.

---

## Phase W1 — `rawdaw-dsp` wavetable oscillator

**Goal.** Phase-accumulator oscillator that plays a single
procedurally-generated factory wavetable.

**Steps.**

- Public `WavetableOsc` with `prepare(sample_rate)`, `set_note(MidiNote)`,
  `tick() -> f32`. State: phase (0..1), phase_inc (per-sample), table
  reference. Linear sample interpolation.
- Public `Wavetable` (owned `Vec<f32>` of length `TABLE_LEN`) with a
  factory constructor `Wavetable::saw_default()` that procedurally
  generates a band-limited saw via additive synthesis up to Nyquist
  for a low reference note. Mip-mapping (per-note bandlimits) is a
  v1 growth target; v0 accepts aliasing at high notes.
- 128-entry frequency lookup built at `prepare()` so per-note
  retunings don't run a powf in the audio thread.

**Done when (✅ met).** Unit tests pin: produces frequency-correct output at
A4 (440 Hz period in samples), phase continuity across blocks,
allocation-free `tick()`.

---

## Phase W2 — `rawdaw-dsp` ADSR envelope

**Goal.** Linear-ramp ADSR per voice. Exponential ramps are a v1
growth target.

**Steps.**

- `Adsr` with `attack_s`, `decay_s`, `sustain_level`, `release_s`
  parameters. States: `Idle` → `Attack` → `Decay` → `Sustain` →
  `Release` → `Idle`.
- `note_on()` resets to Attack from current level (or 0 if Idle);
  `note_off()` transitions to Release from current level.
- `tick() -> f32` advances state and returns the current level
  [0.0, 1.0]. `is_idle() -> bool` reports whether the voice can be
  freed.

**Done when (✅ met).** Unit tests pin: peak at end-of-attack, sustain
plateau after decay, release-from-mid-attack works (no value pop),
linear ramp slopes match expected rates.

---

## Phase W3 — `rawdaw-dsp` state-variable lowpass filter

**Goal.** 12 dB/oct LP filter (Cytomic / Andrew Simper formulation).
HP / BP / notch modes from the same coefficients are a v1 growth.

**Steps.**

- `SvfLowpass` with `prepare(sample_rate)`, `set_cutoff(hz)`,
  `set_resonance(q)`, `tick(input) -> f32`.
- Internal state: two integrators (z1, z2), pre-computed g and k
  coefficients refreshed when cutoff / resonance change. Use the
  trapezoidal-integrator variant for stability at high resonance.

**Done when (✅ met).** Unit tests pin: DC passes at cutoff > Nyquist/4,
impulse response decays, stable at Q = 10 (no NaN / explosion),
cutoff parameter at sample_rate / 2 doesn't crash.

---

## Phase W4 — `rawdaw-dsp` LFO (sine, free-running)

**Goal.** Free-running sine LFO with rate in Hz. Tempo sync is a
v1 growth.

**Steps.**

- `SineLfo` with `prepare(sample_rate)`, `set_rate_hz(hz)`,
  `tick() -> f32` returning [-1.0, 1.0]. Resettable phase via
  `reset()`.

**Done when (✅ met).** Unit tests pin: rate = 1 Hz produces 0 → 1 → 0 → -1
→ 0 over one second at 48 kHz, phase continuity across blocks.

---

## Phase W5 — `rawdaw-dsp` voice manager

**Goal.** Generic polyphonic voice pool with allocate / steal.

**Steps.**

- `VoicePool<V: Voice>` parametric over a `Voice` trait that exposes
  `note() -> u8`, `is_active() -> bool`, `note_on(MidiNote, U16Velocity)`,
  `note_off(MidiNote)`, `tick(...)`. 16-voice pool by default;
  pool size is a parameter.
- Allocation strategy: prefer inactive voice; otherwise steal the
  oldest active voice (age counter monotonically increments on
  every `note_on`). Mirrors the existing `SineNode` semantics so
  the v0 wavetable synth feels identical at the queue layer.
- The pool is generic so the future physical modeller / drum synth
  can reuse it without duplicating allocation / stealing logic.

**Done when (✅ met).** Unit tests pin: allocation prefers inactive,
saturation triggers oldest-steal, NoteOff matches by MIDI note,
voice age increments correctly.

---

## Phase W6 — `rawdaw-synth-wavetable` `WavetableSynthNode`

**Goal.** Assemble the v0 synth as an `AudioNode` implementation.

**Steps.**

- Per-voice state: a `Voice` struct holding `WavetableOsc`, `Adsr`,
  `SvfLowpass` (with cutoff modulated by a shared LFO + filter ADSR
  in v1; just static cutoff modulated by the LFO in v0).
- Shared per-node state: one `SineLfo`, voice pool, shared wavetable
  reference. Per-voice filter (each voice has its own integrators).
- Hardcoded v0 patch:
  - osc: factory saw wavetable
  - amp ADSR: 5 ms / 80 ms / 0.7 / 200 ms
  - filter cutoff: 1.5 kHz, resonance: 0.7
  - LFO: 4 Hz, ±400 Hz cutoff modulation
- `AudioNode` impl with single stereo output port (L=R for v0).

**Done when (✅ met).** Unit tests pin: silent at rest, audible during a
NoteOn..NoteOff window, silent after release decays past
threshold, no allocation in `process()`.

---

## Phase W7 — Wire into rawdaw-app

**Goal.** Replace the lead track's `SineNode` with the new
`WavetableSynthNode`. Bass / drums / pad stay sines until they get
their proper synths (physical / drum / synth-v0).

**Steps.**

- `rawdaw-app/Cargo.toml` adds `rawdaw-synth-wavetable` dep.
- `audio::configure_graph` swaps the lead track's instrument node
  to `WavetableSynthNode::new()` instead of `SineNode::new()`.
- Visual + audio verify via Rinch MCP: launch app, hit Play,
  confirm lead track sounds noticeably less harsh + wavetable-
  shaped. Bass / drums / pad still sine, so the rest of the mix
  doesn't change.

**Done when (✅ met).** End-to-end audible difference on the lead track.
All three clippy gates clean, `cargo test --workspace` green.

---

## Phase W8 — Plan + memory updates

**Goal.** Close out v0.

**Steps.**

- This doc's per-phase "what landed" + "deviations" subsections
  filled.
- `project_status` memory updated: v0 done; next session = grow
  the wavetable synth (multi-osc + FM/AM + second envelope + mod
  matrix + factory bank), or pivot to the physical / drum synth.

---

## Out of scope (v1+ growth)

These are deliberate v0 omissions, called out so the v0 done-when
isn't muddled with them:

- Multi-oscillator + FM/AM modulation between oscillators
- Mip-mapped band-limited wavetable lookup (anti-aliasing)
- Wavetable position morphing (multiple tables per wavetable)
- Second ADSR for filter envelope
- Modulation matrix (free routing instead of hardcoded LFO→cutoff)
- Exponential envelope ramps
- LFO waveforms beyond sine
- LFO tempo sync
- Filter modes beyond lowpass (HP / BP / notch / shelf)
- Stereo unison / detune
- Sample import / wavetable import
- Synth editor UI panel
- MPE / micro-tuning
- Per-voice effects
