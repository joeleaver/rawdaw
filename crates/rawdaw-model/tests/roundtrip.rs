//! Serialize the tiny project to RON and back; assert structural equality.
//!
//! This is the contract that the project file format depends on. If this
//! test breaks, the data model has changed in a way that affects on-disk
//! project files — bump `SCHEMA_VERSION` and add a migration.

mod common;

use rawdaw_model::*;

#[test]
fn project_roundtrips_through_save_load() {
    let original = common::build_tiny_project();

    let serialized = original.save().unwrap();

    // Sanity-check a few distinctive substrings so we notice if the on-disk
    // shape changes unexpectedly.
    assert!(serialized.contains(&format!("schema_version: {SCHEMA_VERSION}")));
    assert!(serialized.contains("name: \"Untitled\""));
    assert!(serialized.contains("default_key:"));
    assert!(serialized.contains("Functional("));
    assert!(serialized.contains("Chord("));
    assert!(serialized.contains("Drum("));
    assert!(serialized.contains("stripped"));

    let deserialized = Project::load(&serialized).unwrap();
    assert_eq!(original, deserialized);
}

#[test]
fn v2_project_round_trip_preserves_name() {
    // The v2 field-add: `Project.name`. A non-default value must
    // survive save → load — pins both the serialize and the
    // deserialize sides of the new field.
    let mut original = common::build_tiny_project();
    original.name = "My Verse".into();

    let serialized = original.save().unwrap();
    assert!(serialized.contains("name: \"My Verse\""));

    let deserialized = Project::load(&serialized).unwrap();
    assert_eq!(deserialized.name, "My Verse");
    assert_eq!(original, deserialized);
}

/// Strip a top-level pretty-printed RON field by name. Handles both
/// single-line `name: "Untitled",` and multi-line nested
/// `master_chain: (...),` blocks by counting paren/bracket depth from
/// the opening line until it balances. Used by the migration tests
/// below to derive an older-schema RON from the current pretty-printer
/// output rather than hand-crafting it, so the fixtures track the
/// model's evolution.
fn strip_field(ron: &str, field_name: &str) -> String {
    let prefix_pat = format!("{field_name}:");
    let mut out: Vec<&str> = Vec::new();
    let mut skipping = false;
    let mut depth: i32 = 0;
    for line in ron.lines() {
        if !skipping && line.trim_start().starts_with(&prefix_pat) {
            // Begin skipping. Count delimiters on the opening line.
            skipping = true;
            depth = delim_balance(line);
            if depth == 0 {
                // Single-line value (e.g. `name: "Untitled",`).
                skipping = false;
            }
            continue;
        }
        if skipping {
            depth += delim_balance(line);
            if depth <= 0 {
                skipping = false;
            }
            continue;
        }
        out.push(line);
    }
    out.join("\n")
}

/// Net opening-vs-closing of `( [ { ` over `) ] }` on a single line.
/// Ignores chars inside string literals (RON uses `"..."` only).
fn delim_balance(line: &str) -> i32 {
    let mut depth: i32 = 0;
    let mut in_str = false;
    let mut escape = false;
    for ch in line.chars() {
        if escape {
            escape = false;
            continue;
        }
        if in_str {
            match ch {
                '\\' => escape = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match ch {
            '"' => in_str = true,
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            _ => {}
        }
    }
    depth
}

#[test]
fn v1_project_migrates_to_current_with_default_name_and_master_chain() {
    // composition-writability C4 + master-fx-chain X1 migration
    // contract: a v1 project (no `name`, no `master_chain` field,
    // schema_version: 1) loads into the current build by defaulting
    // both missing fields via `serde(default = ...)` attributes and
    // bumping the in-memory schema_version through each migration
    // step in turn.
    //
    // The fixture is derived from a current empty project rather than
    // hand-crafted RON so it tracks the model's evolution — when a
    // future schema version adds another field, the same strip-and-
    // version-rewrite trick covers it.
    let current = Project::new(Scale::major(PitchClass::C));
    let current_ron = current.save().unwrap();

    // Build a v1 RON by lowering the version line and stripping every
    // field that didn't exist in v1: `name` (added v2) and
    // `master_chain` (added v3). `strip_field` handles both the
    // single-line `name: "Untitled",` and the multi-line nested
    // `master_chain: (...),` block correctly.
    let v1_ron = strip_field(
        &strip_field(&current_ron, "master_chain"),
        "name",
    )
    .replacen(
        &format!("schema_version: {SCHEMA_VERSION}"),
        "schema_version: 1",
        1,
    );
    assert!(v1_ron.contains("schema_version: 1"));
    assert!(!v1_ron.contains("name:"));
    assert!(!v1_ron.contains("master_chain:"));

    let migrated = Project::load(&v1_ron).expect("v1 → current migration succeeds");
    assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    assert_eq!(migrated.name, "Untitled");
    assert_eq!(migrated.master_chain, MasterChainData::default());
    // The rest of the project should round-trip identically.
    assert_eq!(migrated.default_key, current.default_key);
    assert_eq!(migrated.tempo_map, current.tempo_map);
}

#[test]
fn v2_project_migrates_to_v3_with_default_master_chain() {
    // master-fx-chain X1 migration contract: a v2 project (carries
    // `name`, no `master_chain` field, schema_version: 2) loads into
    // the current build by defaulting `master_chain` to the safety-
    // net single-entry soft-clipper chain via
    // [`MasterChainData::default`] and bumping the in-memory
    // schema_version.
    //
    // Pinning v2 → v3 specifically (rather than only via the v1 →
    // current test above) protects the v3 migration's individual
    // hop: if a future schema bump replaced the v1 → v2 step
    // entirely, the v1 → current test would still pass while this
    // test would catch a broken v2 → v3 hop.
    let current = Project::new(Scale::major(PitchClass::C));
    let current_ron = current.save().unwrap();

    let v2_ron = strip_field(&current_ron, "master_chain").replacen(
        &format!("schema_version: {SCHEMA_VERSION}"),
        "schema_version: 2",
        1,
    );
    assert!(v2_ron.contains("schema_version: 2"));
    assert!(!v2_ron.contains("master_chain:"));
    // v2 carried `name`, so it should survive the strip.
    assert!(v2_ron.contains("name:"));

    let migrated = Project::load(&v2_ron).expect("v2 → v3 migration succeeds");
    assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    assert_eq!(migrated.master_chain, MasterChainData::default());
    assert_eq!(migrated.name, current.name);
}

#[test]
fn project_round_trip_preserves_custom_master_chain() {
    // master-fx-chain X1 round-trip contract: a non-default
    // master_chain (here: a two-entry chain with a softer threshold)
    // must survive save → load. Pins both the serialize and
    // deserialize sides of the new field.
    let mut original = common::build_tiny_project();
    original.master_chain = MasterChainData {
        format_version: MASTER_CHAIN_FORMAT_VERSION,
        fx: vec![
            MasterFxData::SoftClip(SoftClipData {
                format_version: SOFT_CLIP_FORMAT_VERSION,
                threshold: 0.5,
            }),
            MasterFxData::SoftClip(SoftClipData {
                format_version: SOFT_CLIP_FORMAT_VERSION,
                threshold: 0.95,
            }),
        ],
    };

    let serialized = original.save().unwrap();
    assert!(serialized.contains("master_chain:"));
    assert!(serialized.contains("SoftClip("));

    let deserialized = Project::load(&serialized).unwrap();
    assert_eq!(deserialized.master_chain, original.master_chain);
    assert_eq!(original, deserialized);
}

#[test]
fn project_roundtrips_compactly() {
    let original = common::build_tiny_project();
    let compact = ron::ser::to_string(&original).unwrap();
    let deserialized: Project = ron::de::from_str(&compact).unwrap();
    assert_eq!(original, deserialized);
}

#[test]
fn loading_an_unsupported_version_fails() {
    let project = common::build_tiny_project();
    let serialized = project.save().unwrap();

    // Surgically rewrite the version to a value we don't support. `+999`
    // guarantees we're past the loadable range (currently {1, 2}); when a
    // future v3 lands, the migration check still rejects vN+999.
    let fake_version = SCHEMA_VERSION + 999;
    let tampered = serialized.replacen(
        &format!("schema_version: {SCHEMA_VERSION}"),
        &format!("schema_version: {fake_version}"),
        1,
    );
    assert_ne!(
        tampered, serialized,
        "test setup: replacement must have changed the string"
    );

    match Project::load(&tampered) {
        Err(LoadError::UnsupportedSchemaVersion { found, expected }) => {
            assert_eq!(found, fake_version);
            assert_eq!(expected, SCHEMA_VERSION);
        }
        other => panic!("expected UnsupportedSchemaVersion, got {other:?}"),
    }
}

#[test]
fn fresh_project_has_current_schema_version() {
    let project = Project::new(Scale::major(PitchClass::C));
    assert_eq!(project.schema_version, SCHEMA_VERSION);
}

/// Dev-only: print the serialized RON for eyeballing. Run with:
///   cargo test -p rawdaw-model --test roundtrip dump_ron -- --ignored --nocapture
#[test]
#[ignore]
fn dump_ron() {
    let project = common::build_tiny_project();
    println!("{}", project.save().unwrap());
}
