# rawdaw design docs

Living design documents for rawdaw. Updated as decisions are made; open questions tracked explicitly.

- [`overview.md`](overview.md) — what rawdaw is, primary user, in-scope vs out-of-scope.
- [`composition-model.md`](composition-model.md) — the structural composition model (sections, chord loops, patterns, tracks, clips). The defining feature.
- [`drum-patterns.md`](drum-patterns.md) — drum-specific pattern, kit, and multi-out mixer model.
- [`section-variants.md`](section-variants.md) — section variants: shared structure with controlled differences across `chorus`, `chorus-final`, etc.
- [`chord-loops.md`](chord-loops.md) — chord vocabulary, functional/Roman canonical with absolute escape hatch, slash chords, section scheduling.
- [`realization.md`](realization.md) — the realization pass: how structural objects become sample-timed MIDI events.
- [`engine.md`](engine.md) — audio engine: DAG of AudioNodes, RT/non-RT boundary, command queue, offline driver, cpal driver (sketched).
- [`architecture.md`](architecture.md) — engine layering, RT/non-RT boundary, plugin-host boundary, crate layout.
- [`ui-principles.md`](ui-principles.md) — durable UI principles; every design round inherits them.
- [`ui-brief-r1.md`](ui-brief-r1.md) — round-1 brief: the main window at rest, fed to claude.ai/design for HTML mockups.
- [`open-questions.md`](open-questions.md) — running list of unresolved decisions.
