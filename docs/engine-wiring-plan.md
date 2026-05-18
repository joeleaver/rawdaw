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

## Phase E1 — Move existing fixture + add `build_round1_project()`

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

---

## Phase E2 — Presentation adapter; rawdaw-app reads the real Project

**Goal.** Replace `rawdaw-app::fixture` types with model types where the
model is authoritative (`Track`, `Pattern`, `ChordLoop`, `Section`,
`Arrangement`, …). Keep a separate "presentation overlay" for UI-only
fields (pattern colors, section colors, role labels, instance counts).

**Steps.**

- Add `rawdaw-app::presentation` (renamed from `fixture`) with two halves:
  - `model`: re-exports `rawdaw_model::fixtures::build_round1_project()`,
    cached behind a `OnceLock<Project>` so every component pulls the same
    instance.
  - `overlay`: pattern & section colors, role labels — the data the UI
    needs that the model doesn't carry. Keyed by `PatternId` /
    `SectionId`.
- Adapter functions matching the UI's existing lookup helpers:
  `section_by_key(name)`, `pattern_by_name(name)`, `chord_loop_by_name`,
  etc., but built on top of the real `Project` plus the overlay.
- Update every rawdaw-app site that imports from `crate::fixture` to
  import from `crate::presentation`.
- Round-2's fixture-invariant tests move to operate on the real Project.

**Done when.** `cargo run -p rawdaw-app` shows the same UI as Phase 8
(verse base / stripped / chorus all match artboards A / B / C). The
old `crate::fixture` module is gone; everything reads from the real
Project + overlay. No regressions in clippy or tests.

---

## Phase E3 — Engine instantiation + per-track sine node graph

**Goal.** At app launch, build a `rawdaw_engine::Engine`, set up an audio
graph with a per-track instrument node (`SineNode` for now), translate
the realized events from the Project, and feed them into the engine's
event queue. **No audio thread yet.** The engine is ready to render but
nothing is calling `process_block`.

**Steps.**

- `rawdaw-app::audio` module: owns the `Engine`-construction logic and
  exposes a `BuiltEngine { engine, master_node, routing }` aggregate.
- Construct the graph:
  - Master node (a sum / passthrough; first cut can be a custom
    `MixerNode` that's the topo-root).
  - One `SineNode` per project track, connected into the master.
  - `TrackRouting: BTreeMap<TrackId, NodeId>` maps each project track to
    its sine.
- Realize the Project to `TimedEvent`s via `rawdaw_model::realize::*`,
  then `translate_events(..., &routing)` → `Vec<BlockEvent>`. Push every
  event into the engine's event queue.
- Store the built engine in `AppState` (or a Rinch store dedicated to
  audio resources). The handle stays accessible from the UI layer
  without crossing the `AppState`/audio boundary unsafely — at this
  phase the engine is single-threaded so the convenience `Engine`
  wrapper is fine.

**Done when.** App launches successfully; engine builds without panics;
the realized event count matches what `tests/render.rs` produces for the
same project (engine-test cross-check via a unit test).

**Risks.**

- The realize pass may surface gaps the fixture didn't exercise (e.g.
  `PatternBody::Chord` block events, which the current model has design
  for but no implementation). Track those gaps as `TODO`s; if any are
  load-bearing for E4 playback, defer them to a follow-on milestone.

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
