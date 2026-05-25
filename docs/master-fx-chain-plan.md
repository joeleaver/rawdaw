# Master FX chain plan (v1)

rawdaw's master output today is `Mixer → GainNode → cpal` with the
gain hardcoded at `-12 dB` (0.25) — see the `MASTER_GAIN` doc-comment
in `crates/rawdaw-app/src/audio/mod.rs`, which explicitly flags this
as a placeholder: *"Replace with a real master-channel strip + a
soft-clipper once `rawdaw-fx` grows more nodes."*

This plan implements that master-channel strip. The first inhabitant
is a configurable soft-clipper (memoryless tanh limiter); EQ /
reverb / delay land as separate FX nodes over subsequent passes once
the chain infrastructure is in place.

The work is structured the same way as `synth-ui-integration-plan.md`:
an X0 design-decision phase locks the architectural choices, then
X1–X8 implement against those choices in small phases that each
compile, test, and ship audibly-equivalent output until the chain
itself starts shaping signal at X3+.

## Status

- X0 ✅ this document.
- X1 ✅ landed 2026-05-25. New `rawdaw-model::master_fx` module
  (`MasterFxData::SoftClip(SoftClipData)`, `SoftClipData
  { format_version, threshold }` with `Default` returning 0.7,
  `MasterChainData { format_version, fx: Vec<MasterFxData> }`
  with `Default` returning the single-entry safety-net
  soft-clipper chain). `Project.master_chain` field added with
  `#[serde(default)]`. `SCHEMA_VERSION` bumped 2 → 3;
  `check_loadable` extended to accept `1..=3`;
  `migrate_to_current` got a `v2 → v3` hop (bump only — serde
  default fills the new field during deserialize).
  Workspace tests: 708 → 715 (+5 master_fx unit tests + 2
  project migration tests in `tests/roundtrip.rs`).
- X2 — not started.
- X3 — not started.
- X4 — not started.
- X5 — not started.
- X6 — not started.
- X7 — not started.
- X8 — not started.

## Phase X0 — Plan + design decisions ◀ this doc

Lock the architectural choices below. No code changes. The decisions
are detailed in the "Design decisions locked in X0" section near the
bottom; the per-phase summary above each phase below assumes them.

**Done when:** This doc lands on `main` and Joe signs off on the
design-decision section.

## Phase X1 — `rawdaw-model::master_fx` types

Add the serialized model types for the master chain. No engine,
synth, or UI changes yet — this is pure model surface.

- New module `rawdaw-model::master_fx` with:
  - `MasterFxData` enum — tagged variants per FX kind. v1 ships
    `SoftClip(SoftClipData)` only; future variants
    (`Eq(EqData)`, `Reverb(ReverbData)`, …) extend the enum.
  - `SoftClipData { format_version: u32, threshold: f32 }` —
    plain serde struct, f32 only, `Default` returns the same
    threshold the runtime `SoftClipNode` defaults to (0.7).
  - `MasterChainData { format_version: u32, fx: Vec<MasterFxData> }`
    — ordered list, evaluated in order from gain → cpal.
- `Project` gains `master_chain: MasterChainData` field. Default for
  fresh projects = single-entry chain `[SoftClip(SoftClipData::default())]`
  so the safety net is on by default.
- Round-1 fixture (`build_round1_project`) picks up the default
  automatically — no fixture change needed if the field has a
  `#[serde(default)]` attribute on Project.
- Tests: ron round-trip for `MasterChainData` (empty + single-entry +
  multi-entry); `Project` round-trip with the new field; default-
  value pins for `SoftClipData` (threshold = 0.7).

**Done when:** New module compiles + tests pass; `Project` carries
the field; legacy serialized projects (without the field) deserialize
to the default chain via `#[serde(default)]`. Audio behavior unchanged
(no node consumes the new field yet). Workspace test count grows by
~6–8.

## Phase X2 — Runtime `SoftClipPatch` + `SoftClipNode` lives in `rawdaw-fx`

Mirror the U3a/U3b pattern for synths: runtime patch type in the FX
crate, `From<SoftClipData>` conversion, `with_patch(patch)` /
`with_patch_publishers(patch, pubs)` constructors. Adds the actual
DSP node.

- `crates/rawdaw-fx/src/softclip.rs` — `SoftClipNode` with the
  tanh-knee shape (already drafted in this conversation). Defaults
  to threshold 0.7 / ~ -3.1 dBFS.
- Runtime `SoftClipPatch { threshold: f32 }` with `From<SoftClipData>`.
  Stays in `rawdaw-fx` so the model crate doesn't take a `rawdaw-fx`
  dep.
- `SoftClipPublishers { version: Arc<AtomicU64>, snapshot:
  Arc<Mutex<SoftClipPatch>> }` — mirror of `WavetablePublishers` /
  `DrumPublishers`.
- `SoftClipParam` enum with one variant for v1: `Threshold`.
  Encode/decode against the engine's `[u8; 8]` path; first byte
  reserved as the FX-kind discriminant for chain-position-independent
  routing (see X0 design decisions).
- `SoftClipNode::apply_event` — Param arm decodes via
  `SoftClipParam::decode`, applies to the runtime patch, writes the
  snapshot, bumps the version.
- Tests: ADSR-style threshold clamps (0.001..0.999); publisher
  contract round-trip; encode/decode round-trip; end-to-end Param
  event → audio (apply a Threshold event mid-stream and verify the
  output amplitude character changes).

**Done when:** `cargo test -p rawdaw-fx` is green; the node still
isn't wired into the engine graph (next phase). Workspace test count
grows by ~10–12.

## Phase X3 — Engine graph: chain wiring

Lay down the chain in `configure_graph`. Until X7, the default chain
is empty so audio remains byte-identical to today; this phase ships
the *infrastructure* for putting an FX list between the master gain
and cpal.

- `NodeId` allocation: master gain at `N+1` (unchanged); FX nodes at
  `N+2..=N+1+chain_len`; cpal reads from the *last* node in the chain
  (or from master gain when the chain is empty).
- Construct one node per `MasterFxData` variant from
  `project.master_chain`. The match arm picks the runtime constructor
  + builds publishers.
- Connect: `gain.out → fx[0].in`, `fx[k].out → fx[k+1].in`, …,
  `fx[last].out` is the new `ConfiguredGraph.master`.
- `ConfiguredGraph` gains `master_fx_publishers: Vec<(NodeId,
  MasterFxKind, MasterFxPublishers)>` (mirror of `wavetable_publishers`).
- AudioResources keeps the chain length + per-slot kind for UI
  dispatch. New per-slot `MasterFxEditorHandle { node_id, kind,
  patch_signal }` table.
- `debug_assert!` on chain length matching the project's
  `master_chain.fx.len()` at the graph boundary.
- Tests: empty chain → cpal reads from master gain (audio identical
  to round-1); 1-entry chain → cpal reads from soft-clip; 2-entry
  synthetic chain (two soft-clips in series) → second clip's NodeId
  is the master.

**Done when:** Round-1 with the default `[SoftClip]` chain reads
through the soft-clipper but produces audibly-clean output (default
threshold + current -12 dB master gain keeps the clipper inactive).
Workspace test count grows by ~5–7.

## Phase X4 — Host-side pollers + `push_master_fx_param`

Spawn one poller per chain slot mirroring the audio-thread patch
into a reactive `Signal<MasterFxPatch>`. Add the host-side
`push_master_fx_param(slot, param, value)` helper. No UI yet — the
UI in X5 reads these handles.

- `crates/rawdaw-app/src/audio/master_fx_poller.rs` — direct mirror
  of `wavetable_poller.rs` / `drum_poller.rs`. 20 Hz polling
  cadence; `Signal::send` of patch snapshots.
- `AudioResources` gains `master_fx_handles: Rc<Vec<MasterFxEditorHandle>>`
  (Vec, not BTreeMap, since slots are dense integer indices into the
  chain).
- `push_master_fx_param(slot: usize, param: SoftClipParam, value: f32)`
  routes the event to the right NodeId.
- Tests: publisher contract (version + snapshot bump on Param apply,
  shared via cloned publishers); push helper ok/err for in-range vs
  out-of-range slot; chain-length matches handle count.

**Done when:** Audio thread publishes patch state to the host on
every Param apply; host-side helper can address Param events at the
right NodeId without UI intervention. Workspace test count grows by
~5.

## Phase X5 — Selection model: a third axis for the master strip

The Inspector branches on (`selected_idx`, `selected_track`) today.
Add a third axis for master-FX selection, mutually exclusive with
the other two like the existing pair.

- `AppState.selected_master_fx: Signal<Option<usize>>` — `Some(slot)`
  → Inspector renders the master-FX editor for that slot. `None` →
  master strip is unselected.
- `AppState::select_master_fx(idx)` enforces the mutex with
  `set_selected_idx` / `select_track`.
- `regions/tracks_pane.rs` gains a "Master" entry (separator + one
  row, fixed at the bottom of the list) — clicking selects slot 0 of
  the master chain.
- `regions/inspector/mod.rs` adds a fourth branch: `selected_idx =
  None && selected_track = None && selected_master_fx = Some(slot)`.
  Routes to the new `MasterFxEditor` (X6).
- `inspector_selection_keys` in `app.rs` returns a key derived from
  the third axis too, so Inspector re-mounts on master-FX
  selection.
- Tests: mutex behavior across all three axes; pane-width sentinel
  for master-FX mode (consistent with the synth-editor 600px
  widening from U4).

**Done when:** Clicking "Master" in the tracks pane lights up
selection state and triggers the Inspector to render an empty
placeholder. Workspace test count grows by ~4.

## Phase X6 — `MasterFxEditor` dispatch + `SoftClipEditor` body

Build the inspector body. Mirror of U5's `WavetableEditor` / U7's
`DrumEditor` — a per-FX-kind dispatcher that renders the right
editor for the selected slot.

- `regions/inspector/master_fx_editor.rs` — chain breadcrumb
  ("Master ▸ slot 0 ▸ SoftClip") + per-slot editor dispatch on
  `MasterFxKind`.
- `regions/inspector/soft_clip_editor.rs` — single threshold slider
  (range 0.001..0.999, step 0.01, label format "−X.X dBFS").
  Per-control `Signal<f64>` re-binds via `Effect` on the slot's
  `patch_signal` (U9 lesson applied — slider visually re-syncs on
  external pushes).
- Threshold slider pushes `SoftClipParam::Threshold` via the X4
  helper.
- Tests: editor renders for default patch; slider→param round-trip
  via the host-side push path mirrors what U5's filter-cutoff test
  does.

**Done when:** Selecting the master strip shows a one-slider editor
that drives the live threshold. Verified visually via the rinch
MCP (drag → soft-clip threshold change → audible character change on
loud passages). Workspace test count grows by ~3.

## Phase X7 — Default chain wiring + master-gain calibration

With the soft-clipper now interactive and protective, the -12 dB
master gain can be relaxed. The original gain was a workaround for
the missing soft-clipper — see the existing `MASTER_GAIN` doc-
comment.

- `MASTER_GAIN` raises from `0.25` to a higher value (calibration
  candidate: `0.5` (~-6 dB) or `0.7` (~-3 dB)). Pick by listening
  through a round-1 loop with the soft-clipper engaging on
  transients — the goal is comfortably-loud chord stacks that don't
  trip the soft-clip on every voice, only on peak transients.
- Default chain in `MasterChainData::default()` confirms it ships
  with `[SoftClip(default())]` (locked in X1; X7 just verifies
  audibly).
- Audio→listening verification documented in the plan deviations
  section. This is a calibration phase, not a code phase — fully
  expect a small handful of constant tweaks based on what sounds
  right.

**Done when:** Round-1 plays at a louder, fuller volume with the
soft-clip catching transients cleanly. Master-gain constant
captured + the listen test recorded.

## Phase X8 — Plan + memory updates

Mirror U9 — close-out + cross-references resolved.

- Per-phase ✅ markers + deviations recorded above.
- Memory `project_status.md` updated: master FX milestone CLOSED,
  picklist item #9 (`fx polish`) marked partial — soft-clipper ✅,
  EQ/reverb/delay still pending as separate FX nodes that plug into
  the chain.
- Cross-reference in `synth-ui-integration-plan.md` (the U7 note
  about future master FX) marked resolved if applicable.
- Consider a follow-on plan for "Chain editing UX" — drag-to-reorder,
  "+ Add FX" picker, per-FX bypass — if it would be needed before
  the next FX kind lands.

**Done when:** All eight X phases ✅; memory updated; the next
picklist item is clearly the next bite.

---

## Design decisions locked in X0

### One master chain. No per-track FX yet.

Per-track FX chains (insert FX between an instrument and the mixer)
are a strict superset of the master chain — same data shape, more
slots. They also pull on track-model surface (TrackKind has to grow
an FX field) and the chord/loop model (sends/returns vs. inserts).

**Pick master-only for v1.** Per-track FX is a separate planning
milestone once the master chain has shipped and shaped how the
domain wants to express FX. Inserts on every track is doable but
unnecessary work today — the current pain point is the missing
master limiter, not per-track shaping.

### Ordered `Vec<MasterFxData>` chain, length 0+.

Even though v1 ships a single soft-clip slot, the model is a list
from day one. Adding EQ → SoftClip → Reverb later is appending
variants, not refactoring schema.

```text
struct MasterChainData {
    format_version: u32,
    fx: Vec<MasterFxData>,
}

enum MasterFxData {
    SoftClip(SoftClipData),
    // EQ(EqData),    // future
    // Reverb(ReverbData),
    // Delay(DelayData),
}
```

Considered: hardcoding a single soft-clip slot ("`master_softclip:
SoftClipData` on Project"). Smaller v1 but locks the wrong shape;
the second FX kind would require model-level migration that the
list shape avoids.

### Default chain is `[SoftClip(default())]`, not empty.

Fresh projects boot with the safety net active. Two reasons:

1. **Audibly safer.** Cpal hard-clips at ±1.0; a project that
   accidentally builds a hot mix shouldn't blow out a listener's
   ears on first preview.
2. **Discoverable.** A user who clicks the master strip wants to
   see *something* there. An empty chain hides the feature.

Considered: empty default + a "+ Add SoftClip" button in the UI.
Pushes the discovery burden onto the user; loses the safety net for
anyone who doesn't know it's there.

### Runtime types in `rawdaw-fx`, serialized mirrors in `rawdaw-model`.

Direct port of the two-layer pattern locked in by U0 for synth
patches:

1. Runtime `SoftClipPatch` lives in `rawdaw-fx` (where the
   `SoftClipNode` lives). Holds `f32` directly; `Copy + Default`.
2. Serialized `SoftClipData` lives in `rawdaw-model::master_fx`.
   Plain serde struct with `format_version: u32`.
3. `From<SoftClipData>` in `rawdaw-fx`.

`rawdaw-model` does *not* gain a dep on `rawdaw-fx` — the model
stays leaf-domain. Same rationale as why the synth two-layer pattern
keeps `rawdaw-model` free of `rawdaw-dsp`.

### Parameter event paths get an FX-kind discriminant.

The existing `[u8; 8]` `ParamEvent` path is already 8 bytes — three
of them currently unused for the per-synth enums. The master-FX
arm needs to encode (a) which kind of FX (SoftClip vs future EQ vs
…), (b) which parameter inside that FX. So:

```text
byte[0] = FX-kind tag (0 = SoftClip, 1 = EQ, …)
byte[1..] = per-FX-kind subpath (1 byte for SoftClipParam::Threshold)
```

The slot index is *not* in the path — the engine routes by NodeId
already (each FX has its own NodeId). The host knows which slot
maps to which NodeId via the `master_fx_handles` table.

Considered: a global `FxParam` enum that's a tagged union across
all FX kinds. Same wire format but harder to maintain — each FX
crate would have to coordinate with a central enum. Per-kind enums
keep the locality.

### Publishers + pollers mirror Wavetable/Drum directly.

`SoftClipPublishers { version: Arc<AtomicU64>, snapshot:
Arc<Mutex<SoftClipPatch>> }` — identical shape to
`WavetablePublishers` / `DrumPublishers`. Same RT trade-off
documented at U5 (Mutex blocking) applies; same future migration to
triple-buffer / arc-swap is shared with the synth publishers (when
that lands, every publisher type moves together).

The host-side poller is also a near-clone of
`wavetable_poller.rs` / `drum_poller.rs`. At least one of the three
should be extracted into a generic `Poller<P>` over a
`PublisherSnapshot` trait in a follow-on refactor — but only after
all three are written, so the shape of the abstraction is informed
by the actual use sites.

### TracksPane gains a "Master" pseudo-track row at the bottom.

The selection axes today are section-block (arrangement) and
project-track (synth editor). Master-FX is a third axis; the user
needs a click target somewhere.

Considered:

- (A) **Bottom row of TracksPane** ("Master" with a visual
  separator). Lowest UI cost; consistent with the existing pane;
  reads naturally as part of the project's tracks list.
- (B) **Dedicated Master strip region** on the right side of the
  arrangement, before the Inspector. Mirrors Ableton's master track
  visual. More chrome to design; more space cost; more code.
- (C) **Top-bar slot.** Tiny; hard to discover.

**Pick A** for v1. Migrate to B during the deferred UI redesign
pass if Joe's reference DAWs (Ableton in particular) make a strong
case for it.

### Inspector branches: 4 states, not 3.

```text
selected_idx       → SelectedInspector  (existing)
selected_track     → SynthEditor        (existing)
selected_master_fx → MasterFxEditor     (new)
all three None     → InspectorEmpty     (existing)
```

The three "Some" axes are mutually exclusive — selecting any one
clears the other two. Mirror the `set_selected_idx` /
`select_track` mutex pattern with a third helper
`select_master_fx`.

### Audio→UI slider re-bind from U9 applies straight away.

Per-control `Signal<f64>` Effects subscribed to the slot's
`patch_signal` — same pattern as the U9 wavetable/drum editor
re-bind. So:

- Threshold slider visually re-syncs if an external source (a
  preset apply, automation, MIDI Learn) writes to the patch.
- `apply_*_preset`-style host-side `Signal::set` shortcuts also
  apply when a future preset system covers master FX.

This carry-over is "free" — the U9 pattern is already in the
inspector editors and just needs to be replicated.

---

## Future plans needed

- **Chain editing UX** — drag-to-reorder slots, "+ Add FX" picker,
  per-slot bypass, per-slot delete. Not in v1; the chain length is
  fixed at project-construct time. The next FX kind (probably EQ)
  will force this question.
- **Per-track FX chains** — same architecture, more click targets.
  Deferred to a separate plan once master FX has shaped the domain.
- **EQ / reverb / delay nodes** — each is its own DSP design
  problem. Each lands as a new `MasterFxData` variant + a new
  editor + a new param enum. The chain architecture from this plan
  hosts them without further model migration.
- **Soft-clipper shape selection** — current pick is tanh-knee.
  Polynomial / cubic / asymmetric shapes are a future option-pick
  in the editor.

## Out of scope (deferred)

- Sidechaining / aux sends. Routing model is a separate plan.
- Per-channel meters. Visual-only; UI redesign territory.
- Parameter automation (lane-based time-varying params). Orthogonal
  to the chain itself; lands when the round-3 automation plan does.
- Save-as-preset for FX patches. Same shape as the U8 synth preset
  bank; lands once a project save/load milestone establishes the
  user-preset directory format.
