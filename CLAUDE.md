# rawdaw — engineering rules

These rules govern how rawdaw is built. They override default instincts toward
speed and expediency.

## 1. Architectural correctness over shortcuts. Always.

When the choice is between the fast way and the right way, choose the right
way. No shortcuts. No "fix later" comments. No half-built abstractions. No
silent compromises on type safety, error handling, or domain modeling.

If the right approach is more code, more crates, more tests, more refactors,
or a bigger redesign — that's what we do. We surface the cost and pay it.

This applies in particular to:

- **Data modeling.** Model the domain correctly even when serialization, UI,
  or onboarding gets harder.
- **Crate boundaries.** Split crates when responsibilities differ. Don't
  shove things into one crate "for now."
- **Abstractions.** Build the abstraction when the alternative is duplication
  that will diverge. "Only two cases" is not a reason to skip.
- **Errors.** No `unwrap()` outside tests / proven-impossible cases. No
  silently swallowed errors. No fallbacks that hide an invariant violation.
- **Tests.** Write the test that proves the invariant, not the easy one.

## 2. Unlimited time and budget

There is no ship date. Decisions are made on correctness, not expediency.
A feature that takes 3 weeks to do right and 3 days to do badly takes 3
weeks.

Corollary: don't add features just because they're easy, and don't skip
features just because they're hard. The bar is "what does this DAW need
to actually be the thing we want it to be?" — not "what fits in a sprint?"

## 3. ~700-line cap per source file. Stop and refactor.

When a file approaches ~700 lines, stop and refactor before adding more. A
file that big almost always has multiple responsibilities entangled. Split
by concern, not by line count alone.

This applies to all source files (Rust modules, tests, design docs).

## 4. UI work: use the Rinch skill and MCP

The desktop UI is Rinch (Rust). When writing or reviewing any code under
`crates/rawdaw-app/` — or any file using `rsx!`, `Signal`,
`#[component]`, or Rinch components:

- **Skill.** The `rinch:rinch` Claude skill carries the 14 must-follow
  rules for the framework (rsx-only DOM, `{|| ...}` reactivity wrapper,
  `Signal` is `Copy`, etc.). Invoke it on every fresh Rinch session and
  re-check it when something compiles but silently misbehaves.
- **MCP.** The `rinch` MCP server (configured in `.mcp.json`) attaches to
  a running rawdaw window for live screenshots, DOM inspection,
  simulated clicks/keys, and computed-style queries. Use it to verify
  rsx output visually rather than guessing — Rinch updates are
  surgical and fast, but almost nothing about a *wrong* layout shows
  up at compile time.
- **Source of truth.** Rinch lives at `../rinch/` (sibling clone). When
  the skill or this file is insufficient, read Rinch's own examples
  (`../rinch/examples/`) and components crate
  (`../rinch/crates/rinch-components/`) directly. The `#[component]`
  macro's constraints (no reference-typed params, type-derived defaults
  only, `children` is a magic param name) come from
  `../rinch/crates/rinch-macros/src/lib.rs` — read the source when in
  doubt.
