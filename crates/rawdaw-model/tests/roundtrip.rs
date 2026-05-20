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

#[test]
fn v1_project_migrates_to_v2_with_default_name() {
    // composition-writability C4 migration contract: a v1 project
    // (no `name` field, schema_version: 1) loads into a v2 build by
    // defaulting `name` to "Untitled" via the `serde(default = ...)`
    // attribute and bumping the in-memory schema_version.
    //
    // The fixture is derived from a current empty project rather than
    // hand-crafted RON so it tracks the model's evolution — when a
    // future schema version adds another field, the same strip-and-
    // version-rewrite trick covers it.
    let current = Project::new(Scale::major(PitchClass::C));
    let v2_ron = current.save().unwrap();

    // Build a v1 RON by lowering the version line and stripping the
    // `name: "..."` line (v1 RON didn't carry that field). Filtering
    // by `starts_with("name:")` on the trimmed line is robust to
    // pretty-printer indentation; the project's nested types don't
    // currently carry a field literally named `name:` at the top
    // level of a struct (track names sit under `name:` too, but
    // there's only one top-level `Project.name` so trimming + line
    // filtering catches it cleanly given the empty-project fixture
    // has zero tracks).
    let v1_ron = v2_ron
        .replacen("schema_version: 2", "schema_version: 1", 1)
        .lines()
        .filter(|line| !line.trim_start().starts_with("name:"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(v1_ron.contains("schema_version: 1"));
    assert!(!v1_ron.contains("name:"));

    let migrated = Project::load(&v1_ron).expect("v1 → v2 migration succeeds");
    assert_eq!(migrated.schema_version, SCHEMA_VERSION);
    assert_eq!(migrated.name, "Untitled");
    // The rest of the project should round-trip identically (the
    // only change between v1 and v2 is the name field).
    assert_eq!(migrated.default_key, current.default_key);
    assert_eq!(migrated.tempo_map, current.tempo_map);
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
