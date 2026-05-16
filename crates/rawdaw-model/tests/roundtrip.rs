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
    assert!(serialized.contains("default_key:"));
    assert!(serialized.contains("Functional("));
    assert!(serialized.contains("Chord("));
    assert!(serialized.contains("Drum("));
    assert!(serialized.contains("stripped"));

    let deserialized = Project::load(&serialized).unwrap();
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

    // Surgically rewrite the version to a value we don't support.
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
