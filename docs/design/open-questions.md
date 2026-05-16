# Open Questions

Running list of design decisions not yet resolved. Move items into the relevant doc with a "Resolved" note once decided.

## Composition model

- **Pattern length vs. clip duration.** Patterns have a length; clips occupy a section-relative time range. If a 2-bar pattern is placed in a 4-bar slot, does it auto-loop, stretch, or play once and stop? Default loop is probably right, with explicit "play once" available.

### Resolved (moved into composition-model.md / drum-patterns.md / realization.md / section-variants.md)

- ~~**Multi-track patterns.**~~ Resolved as no — drums use a multi-voice `Pattern::Drum` variant within a single drum track.
- ~~**Drum patterns specifically.**~~ Designed in `drum-patterns.md`.
- ~~**Pattern variants deferred?**~~ No — first-class in v1.
- ~~**Per-note override addressing & orphans on pattern edit.**~~ Stable `NoteId` per pattern event; overrides reference IDs; orphans warned in UI and ignored by realization. See `realization.md`.
- ~~**Section variants.**~~ Designed in `section-variants.md`. Variant-as-sparse-override on a section base; shallow merge semantics; flat (base-only) inheritance.
- ~~**Pitched pattern event vocabulary for melodies.**~~ Expanded to `ScaleDegree | ChordDegree | Absolute | Chromatic | Rest`, with per-event `OctaveSpec`. See `composition-model.md` and `realization.md`.
- ~~**Chord-loop representation, secondary dominants, slashes, section scheduling.**~~ Designed in `chord-loops.md`. Functional as canonical, `in_key` for tonicization/borrowing, independent `BassSpec`, multi-loop sections via `Vec<(BarRange, ChordLoopRef)>`.

## Engine / RT

- **Lookahead window for realization.** How far ahead of the playhead should the realization pass run? Trade-off: longer = more tolerant of UI hiccups, but more re-work if user changes a chord that's already been realized into the engine's event queue. Probably 100–500ms with re-realization on structural edits.
- **Routing change cost.** Topo-sorting on every routing change is fine for v1, but if it stalls audio during big graph edits, we may need double-buffered graphs (process current while building next, atomic swap).
- **JACK vs. ALSA vs. PipeWire on Linux.** `cpal` handles them, but default backend and user-facing config are open.

## Built-in instruments

- **Synth scope.** What kind of synth(s)? Subtractive is the obvious starting point. FM, wavetable, additive are all possible. Joe wants to build a couple — worth a design discussion of its own when we get there.
- **Sampler scope.** Just SFZ/SF2 playback (`oxisynth`)? Or our own sample player with simple multisample mapping? Probably both: oxisynth for SoundFont compatibility, our own simpler one for drums and one-shots.

## UI

- **Library panel layout.** Side-docked? Modal? Tabbed? Open question — sketch some mockups before deciding.
- **Realization context display in piano roll.** How do we show the chord/scale context above derived notes without clutter? Faint chord-symbol band along the top of the editor is a candidate.
- **Section editing UX.** When you click a section block in the arrangement view, do you (a) zoom into a section detail view, (b) get a section inspector panel, (c) expand the block inline? Affects information density.

## Project file format

- **Format.** Some structured text (TOML/JSON/RON) for human-diffable projects, with a binary sidecar for large blobs (sample audio, cached realizations)? Or single binary? Text-with-blobs is friendlier for version control, which matters for composition projects you keep coming back to.
- **Schema versioning.** Plan for migrations from day one — the structural model will evolve and we'll want to read old projects.

## v2+ directions (intentionally deferred, design for compatibility)

- **Sub-patterns / motif composition.** Patterns reference other patterns as motifs with transforms (transpose, invert, retrograde, augment/diminish). Would make `PatternKind` extensible. v1 flat patterns + variants + expanded event vocabulary cover the common cases; revisit if melodic motif development becomes a felt limitation.
- **Chord-loop variants.** Symmetric with pattern/section variants. Skip in v1; clone chord loops instead. Revisit if cloning becomes painful.
- **Per-arrangement-instance overrides.** Third-level merge (base → variant → instance). Skip — make another section variant instead.
- **Bar addressing relative to section end** (e.g. `last_bar`) in activation variant schedules. Useful if length variants make absolute indices painful.
- **Multi-channel layouts beyond stereo.** Mono for tight kicks, surround for whatever later.

## Out-of-scope reconsiderations (don't enable lightly)

- Audio recording / clip-level audio editing.
- Notation view.
- Plugin hosting (CLAP first when added).
- Web/WASM target.
- Live performance / clip-launch view.
