# Engine-wiring milestone plan

Replace `rawdaw-app`'s static UI fixture with a real `rawdaw_model::Project`,
translate the project's realized events into engine commands at launch, surface
the engine's playhead reactively, and hook the top-bar transport buttons.

**Engineering constraints** (from `CLAUDE.md`).

- Architectural correctness over shortcuts. Always.
- Unlimited time and budget — pay the cost of doing it right.
- ~700-line cap per source file. Split by concern when approached.
- No `unwrap()` outside tests / proven-impossible cases. No silent swallowing.
- UI work uses the `rinch:rinch` skill and the `rinch` MCP server.

**Cadence.** Each phase ends with `cargo run -p rawdaw-app` showing concrete
progress, clippy clean (default + `--no-default-features` + `--features
cpal-driver`), and `cargo test --workspace` green.

---

## Phase E0 — Plan + fixture-sharing decision ✅ this doc

**Goal.** Document the phased approach and lock the cross-crate
fixture-sharing decision before any code moves.

**Decision: public `fixtures` module on `rawdaw-model`, gated behind a
`fixtures` Cargo feature.**

Alternatives considered.

- **Separate `rawdaw-fixtures` crate.** Cleanest for crate boundaries but
  adds a third crate that exists only to host `Project` constructors. The
  fixtures live on top of `rawdaw-model` types; a thin wrapper crate is
  bureaucratic without a clear separation gain.
- **rawdaw-app builds its own `Project` independently.** Keeps the model
  crate purely a library, but duplicates the fixture-construction work
  between rawdaw-app (runtime fixture) and rawdaw-model's `tests/common/`
  (test fixtures). The point of this milestone is to *eliminate* the
  duplication.

The public-module-with-feature-gate path:

- `rawdaw-model/src/fixtures/` exposes `build_tiny_project()` (the existing
  test fixture, moved out of `tests/common/`) and `build_round1_project()`
  (new — mirrors the current UI fixture).
- Cargo feature `fixtures` gates the module. Test binaries (`tests/`)
  enable it via `[dev-dependencies] rawdaw-model = { ..., features =
  ["fixtures"] }`; rawdaw-app enables it via a regular `features = [
  "fixtures"]` so the fixture data ships in the binary.
- The existing `tests/common/mod.rs` becomes a one-line re-export so the
  existing test imports keep working.

**Done when.** This file is committed and references it elsewhere
(`docs/design/README.md`, the project_status memory) point at it.

---

## Phase E1 — Move existing fixture + add `build_round1_project()` ✅ done

**Goal.** Land the cross-crate fixture sharing decided in E0. Build the
real `Project` equivalent of the rawdaw-app UI fixture.

**Steps.**

- Add `fixtures` Cargo feature to `rawdaw-model` (no default features
  added — release builds stay lean).
- Move `tests/common/mod.rs` → `src/fixtures/mod.rs` under
  `#[cfg(feature = "fixtures")]`. Rename `build_tiny_project()` to
  `build_tiny_project()` (no change) and re-export from the crate root.
  Update `tests/common/mod.rs` to a one-line `pub use
  rawdaw_model::fixtures::build_tiny_project;` shim (or delete the file
  and update test imports).
- Add `build_round1_project()` — builds a `Project` whose
  patterns / chord-loops / sections / arrangement match the current
  `rawdaw-app::fixture::round1()` UI fixture. Five sections (intro,
  verse base, verse stripped, verse base, chorus) across 24 bars, four
  tracks (bass / lead / drums / pad), patterns `bass-main`, `lead-main`,
  `drums-main` (with `fill` variant), `pad-bed`.
- Add fixture-invariant tests pinning the round-1 shape: arrangement
  length, section / track / pattern counts, the chorus drums `fill`
  schedule entry, the stripped variant overrides.

**Done when.**

- `cargo test --workspace` green; the new `build_round1_project` tests
  pass.
- `cargo clippy --workspace --all-targets --features fixtures` and `cargo
  clippy --workspace --all-targets` both clean (with-feature and
  without-feature builds).
- rawdaw-app still compiles and renders the same UI (still using its own
  static fixture — E2 wires the real one through).

**Deviations from the original plan.**
- **Returned `(Project, Round1Keys)` tuple.** Plan said
  `build_round1_project() -> Project`; the actual signature exposes a
  `Round1Keys` bundle of every named id (tracks / patterns / sections /
  chord-loops / drum-kit) so downstream callers don't have to re-scan
  by name. The engine routing in Phase E3 needs `TrackId`s by name —
  this avoids a parallel lookup table.
- **`tests/common/mod.rs` kept as a re-export shim**, not deleted —
  the existing test imports (`mod common; common::build_tiny_project()`)
  keep working without touching `smoke.rs` / `roundtrip.rs`. Removing
  it would have been spec-pure but would have meant a parallel test
  rename.
- **Two-variant bass pattern.** The UI library lists `bass-main` as
  `Pitched · 2 variants`; the model's pattern body now carries `main`
  and `alt` (identical content) to match. Round-2 schedule UI doesn't
  exercise the `alt` variant — it's just there for the library count.
- **Pattern bodies are illustrative, not musical.** Each variant has
  1–4 events — enough for realize() to produce events Phase E3 can
  count, but deliberately not composition decisions. Real bass /
  melody / drum content will land when the round-3 pattern editor
  lets us *make* it.
- **`VariantId::new("__silent__")` placeholder for the silent
  sub-range.** The current model treats every `(BarRange, VariantId)`
  schedule entry as naming a real variant; the UI fixture's
  `(3..4, None)` shape (sub-range silence) doesn't yet have a
  first-class model representation. Using a sentinel variant id is
  the smallest viable stand-in; revisit when sub-range silences get
  a proper model type (likely round 3 alongside the schedule editor).

---

## Phase E2 — Presentation adapter; rawdaw-app reads the real Project ✅ done

**Goal.** Drive the UI's `Round1` view from the model's
`build_round1_project()` plus a small presentation overlay carrying the
UI-only fields the model doesn't own (colors, library meta strings,
round-2 cell realization decorations, top-bar transient state).

**What landed.**

- `rawdaw-app/Cargo.toml` enables the `fixtures` feature on
  `rawdaw-model`. The crate now ships the round-1 model fixture in
  the binary.
- `rawdaw-app/src/fixture/mod.rs` keeps the UI's view types
  (`Track`, `Pattern`, `ChordLoop`, `Section`, `Activation`,
  `Arrangement`) — but with `String` / `Vec` fields instead of
  `&'static str` / `&'static [_]` (Phase E2 step 1).
- `rawdaw-app/src/fixture/data.rs` is the adapter: calls
  `build_round1_project()` once, walks the resulting `Project` plus
  an inline `Overlay` keyed by model ids, and produces the UI's
  `Round1`. `OnceLock<Round1>` caches the result so every component
  pulls the same `&'static Round1`.
- `rawdaw-app/src/fixture/chord_naming.rs` carries the
  Roman / absolute / quality-suffix display helpers (split out to
  keep both files under the ~700-line cap).
- `rawdaw-model/src/fixtures/round1.rs` intro section now includes
  `pattern_ref: None` silent activations for bass / lead / drums so
  the round-1 inspector activation table can render the `silent`
  pill (vs the dashed `inherit` placeholder reserved for tracks
  absent from a section). New `silent_activation` helper + test.

**Deviations from the original plan.**

- **Kept the module name `fixture`** (plan said rename to
  `presentation`). The module is unambiguously an adapter now; the
  rename would have churned every import site without clarifying
  intent.
- **UI view types preserved.** Plan implied the UI would consume
  `rawdaw_model::*` types directly + an overlay. In practice the UI's
  `Track` / `Pattern` / etc. carry display-derived fields (role
  display labels, "Pitched · N variants" meta strings, asterisks
  computed against role defaults) that aren't 1:1 with model types.
  The adapter populates the UI types from model data; cleaner than
  threading model + overlay refs through every component.
- **`OnceLock` caches the full UI `Round1`**, not just the model
  `Project`. The adapter runs once on first call; subsequent
  `round1()` calls return a `&'static Round1` directly.
- **Cell realization decorations stay in the overlay.** The model's
  `RealizationParams` carries voicing + a u8-jitter humanization
  scalar; the UI's `Realization` carries voicing + octave +
  fractional humanization. The mapping isn't 1:1 yet — until the
  model grows richer realization fields, the per-cell display
  values live in the overlay keyed by
  `(SectionId, variant-name, TrackId)`.
- **Pinned-note counts flow through the overlay** but the UI's
  cell footer still reads a hardcoded `0` in `identity_column.rs`
  (a Phase 6 stub). The plumbing is in place; surfacing the real
  count is a follow-up.
- **Visible regression in intro's activation table.** Phase 8
  showed `bass · bass-main · silent` / `lead · lead-main · silent`
  / `drums · drums-main · silent` in the round-1 inspector when
  intro was selected. Post-E2 the rows still render and the
  `silent` pill is preserved, but no pattern name is shown — the
  model's `pattern_ref: None` state doesn't bind a pattern. The
  three round-2 artboards exercised in Phase 8 (verse base / verse
  stripped / chorus base) are visually identical; default render
  (verse@bar5 selected) matches exactly.
- **The `__silent__` variant id sentinel survived.** Phase E1
  flagged it as needing a first-class model representation; the
  adapter still translates it back to `Option::None` for the UI's
  schedule entries. Resolution deferred to a later round.

**Verification.**

- 30 rawdaw-app unit tests pass — new tests pin the intro
  silent-vs-active shape, chord-name resolution in C major
  (`["I","V","vi","IV"]` → `["C","G","Am","F"]`), and the
  five-step / 24-bar arrangement.
- `cargo test --workspace` green.
- Clippy clean across default / `--no-default-features` /
  `--features cpal-driver` builds.
- Visual verification via the rinch MCP: default arrangement
  view + inspector match Phase 8; verse / verse@stripped / chorus
  blocks render the expected variant chips and inspector
  activation tables; section editor for verse (artboards A + B)
  and chorus (artboard C) shows identical layout, realization
  values, and variant schedules to Phase 8.

---

## Phase E3 — Engine instantiation + per-track sine node graph ✅ done

**Goal.** At app launch, build a `rawdaw_engine::Engine`, set up an audio
graph with a per-track instrument node (`SineNode` for now), translate
the realized events from the Project, and feed them into the engine's
event queue. **No audio thread yet.** The engine is ready to render but
nothing is calling `process_block`.

**What landed.**

- `rawdaw-engine/src/nodes/mixer.rs` — new `MixerNode`: N stereo
  inputs declared at construction → 1 stereo summing output. Sized
  per construction call so a 4-track round-1 project gets a 4-input
  mixer, etc. Tests pin zero / one / many inputs and the descriptor
  channel shape.
- `rawdaw-app/src/audio.rs` — new `AudioResources` Rinch store
  (`Rc<RefCell<Engine>>` + `NodeId master` + `Rc<TrackRouting>` +
  initial event count). `AudioResources::build()` calls
  `build_round1_project()`, installs the mixer at `NodeId(0)` plus
  one `SineNode` at `NodeId(i+1)` per track in a single
  `GraphCommand::Batch` (one topo recompute), then runs
  `realize() → translate_events() → push_event` to arm the engine.
- `rawdaw-app/src/app.rs` — `main_window` installs the
  `AudioResources` store alongside `AppState` at app launch.
- `rawdaw-app/src/main.rs` — `audio` module added; architecture
  doc-comment updated.

**Deviations from the original plan.**

- **`Rc<RefCell<Engine>>` instead of bare `Engine` in the store.**
  rinch's `create_store<T: Clone + 'static>` requires the type to be
  cheaply cloneable. The engine isn't `Clone`; wrapping it in
  `Rc<RefCell<>>` gives the store a Clone shape without contention
  overhead (single-threaded UI). Phase E4 splits the engine — at
  that point the `AudioEngine` moves to the audio thread and only
  the `EngineHandle` remains here.
- **`Rc<TrackRouting>` instead of inline `TrackRouting`.** Same
  reason: keep the store cheap to clone. The routing is immutable
  after build.
- **Aggregate type is named `AudioResources`, not `BuiltEngine`.**
  The plan called the aggregate `BuiltEngine`; the actual struct
  carries more than just the engine (routing, event count) and the
  store is consumed via `use_store::<AudioResources>()`, so the
  more descriptive name reads better at call sites.
- **No UI handler consumes the store yet.** The plan says "the
  handle stays accessible from the UI layer." For E3 that's a
  preparation step — phases E4 (cpal callback), E5 (playhead read),
  and E6 (transport buttons) are the actual consumers. `_audio = …`
  binds the store builder; the fields are marked `#[allow(dead_code)]`
  pending those phases.
- **Master is the mixer.** The plan said "first cut can be a custom
  `MixerNode` that's the topo-root" — that's what landed. No
  passthrough alternative.
- **Sample rate hardcoded to 48 kHz, block size 256.** Matches the
  engine's `tests/render.rs`. Phase E4 picks up the actual sample
  rate from the opened cpal device.

**Verification.**

- `audio::tests::round_1_pushes_one_block_event_per_realized_event`
  is the milestone done-when's cross-check: builds the resources,
  realizes the model project independently, asserts
  `initial_event_count == realize(&project).len()`. Companion tests
  pin every track routes to a sine, the NodeId layout
  (master=0, sines=1..=N), and that the built engine renders offline
  without panicking.
- 113 workspace tests pass (rawdaw-app 34, rawdaw-engine 29
  including 4 new mixer tests, rawdaw-model 12 + 21 integration,
  plus crate-tests).
- Clippy clean across default / `--no-default-features` /
  `--features cpal-driver`.
- Rinch MCP visual verification: UI unchanged from post-E2 state.
  App launches; engine builds without panicking on the real
  project.

**Out-of-scope, flagged for later:**

- The realize pass currently does not exercise `PatternBody::Chord`
  block events (model design exists, no implementation). Round-1
  fixture doesn't reach this code path, so it isn't blocking — but
  E4 might want to widen the cross-check test to include a project
  that does.

---

## Phase E4 — cpal driver + audio thread wiring

**Goal.** Spawn the cpal audio thread, hook the callback to
`AudioEngine::process_block`, and produce continuous (silent until E5
kicks transport) audio output.

**Steps.**

- Move the `cpal-driver` feature behind the default features of
  `rawdaw-app` (already exists as an opt-in flag on `rawdaw-engine` per
  the existing engine plan).
- At app startup, after E3's engine build: `engine.split()` →
  `(AudioEngine, EngineHandle)`. Move the `AudioEngine` into the cpal
  callback closure; keep the `EngineHandle` in a Rinch store accessible
  to UI handlers.
- Surface stream errors via a channel back to the host (the engine
  README calls this out as an acknowledged debt — fix it here instead
  of swallowing). UI shows a non-blocking error banner if the stream
  drops.
- The engine starts with transport paused; audio thread runs but
  outputs silence until E6 starts playback.

**Done when.** App launches with audio thread running; no buffer
underruns on a quiet system (verify via the `RenderResult` xrun counter
if exposed). `cargo clippy --workspace --features cpal-driver` clean.

---

## Phase E5 — Reactive playhead

**Goal.** Drive the arrangement view's playhead from the engine's
actual transport position instead of the hardcoded bar 5 beat 2.

**Steps.**

- The audio thread updates a `std::sync::atomic::AtomicU64` (packed
  `SampleTime`) at every block boundary.
- A Rinch `Signal<u64>` mirrors the atomic. A small `Effect` polls the
  atomic on a frame timer (target ~60Hz UI refresh) and `send()`s the
  new value when it changes.
- `regions/arrangement.rs::playhead_percent` reads the signal and maps
  `SampleTime` → bars → percent via the project's tempo map.

**Done when.** With the engine paused, the playhead is stationary at
its initial position (sample 0 → bar 1). When E6 lands, hitting play
makes the playhead scrub across the arrangement.

**Risks.**

- The frame-timer polling pattern is a known anti-pattern; the rinch
  framework may grow a native "audio-thread → UI signal" bridge. Document
  this as a re-evaluation point.

---

## Phase E6 — Play / pause / stop wiring

**Goal.** The top bar's transport buttons drive the engine's playback
state. Pressing play starts the audio engine consuming events from the
queue; pause halts the consumption; stop resets the transport to
sample 0.

**Steps.**

- Add a `Transport` state machine to `AudioEngine` (Playing / Paused /
  Stopped). Transition commands flow through the existing
  `GraphCommand` queue (or a sibling `TransportCommand` queue if
  conflating audio-graph mutations with transport state would be
  surprising).
- Top-bar buttons in `regions/topbar.rs` get onclick handlers that
  push the appropriate commands via the `EngineHandle`.
- Spacebar shortcut for play / pause (lift from round-1 follow-up list
  if `keyboard shortcuts` was on it — wire just this one).

**Done when.** Click play → audible sine output (per the project's
realized events); pause → silence + playhead frozen; stop → silence +
playhead at sample 0; click play again → resumes from sample 0. The
arrangement's playhead from E5 advances in real time.

---

## Phase E7 — Polish + final sweep

**Goal.** Close out the milestone.

**Steps.**

- File-size audit. Anything approaching the 700-line cap gets split.
- All three clippy gates (default, `--no-default-features`,
  `--features cpal-driver`) clean.
- `cargo test --workspace` green.
- Memory + plan-doc updated. Status memory reflects the milestone is
  done; next priorities (real instruments via `rawdaw-sampler`, tempo
  ramps, plugin host) are listed.

---

## Out of scope (later milestones)

- Real instruments (`rawdaw-sampler`: SF2/SFZ via oxisynth + drum sample
  player). The sine-per-track is a plumbing placeholder.
- Tempo ramps. The engine's `ProcessContext` only honors a constant BPM
  (first `BpmEvent`). Tempo-map honoring is part of "engine iteration 4".
- Plugin hosting (CLAP-first). Explicitly deferred.
- Worker-pool parallel graph scheduling. Deferred until single-threaded
  hits a bottleneck.
- Project file I/O — load / save UI affordances. The model layer supports
  it; surfacing it in the UI is its own UX exercise.
- Editing UI (round 3 — pattern editor / piano roll).
