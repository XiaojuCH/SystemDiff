use std::collections::BTreeSet;
use systemdiff_core::{
    Artifact, RegistryStartupEntry, RegistryStartupKind, RegistryValueName, RegistryView,
    RunOncePrefixSemantics, Snapshot, SnapshotValidationError,
};

fn before_snapshot() -> Snapshot {
    serde_json::from_str(include_str!("../../../fixtures/snapshots/before-v1.json"))
        .expect("the before fixture must deserialize")
}

fn registry_entry(snapshot: &mut Snapshot) -> &mut RegistryStartupEntry {
    snapshot
        .observations
        .iter_mut()
        .find_map(|observation| match &mut observation.artifact {
            Artifact::RegistryStartup(entry) => Some(entry),
            _ => None,
        })
        .expect("fixture must contain Registry evidence")
}

#[test]
fn registry_view_serialized_names_are_stable_and_round_trip() {
    let cases = [
        (RegistryView::Shared, "shared"),
        (RegistryView::Native, "native"),
        (RegistryView::Registry32, "registry32"),
        (RegistryView::Registry64, "registry64"),
    ];

    for (view, name) in cases {
        let json = serde_json::to_string(&view).expect("Registry view must serialize");
        assert_eq!(json, format!("\"{name}\""));
        let reparsed: RegistryView =
            serde_json::from_str(&json).expect("Registry view must deserialize");
        assert_eq!(reparsed, view);
    }

    assert!(serde_json::from_str::<RegistryView>("\"process_default\"").is_err());
}

#[test]
fn registry_startup_semantic_names_are_stable() {
    for (kind, name) in [
        (RegistryStartupKind::Run, "run"),
        (RegistryStartupKind::RunOnce, "run_once"),
    ] {
        assert_eq!(
            serde_json::to_string(&kind).expect("startup kind must serialize"),
            format!("\"{name}\"")
        );
    }

    for (semantics, name) in [
        (
            RunOncePrefixSemantics::NoDocumentedPrefix,
            "no_documented_prefix",
        ),
        (
            RunOncePrefixSemantics::DeferDeletionUntilAfterRun,
            "defer_deletion_until_after_run",
        ),
        (RunOncePrefixSemantics::RunInSafeMode, "run_in_safe_mode"),
        (RunOncePrefixSemantics::Undocumented, "undocumented"),
    ] {
        assert_eq!(
            serde_json::to_string(&semantics).expect("prefix semantics must serialize"),
            format!("\"{name}\"")
        );
    }
}

#[test]
fn documented_run_once_prefixes_validate_and_round_trip() {
    let cases = [
        ("Foo", RunOncePrefixSemantics::NoDocumentedPrefix),
        ("!Foo", RunOncePrefixSemantics::DeferDeletionUntilAfterRun),
        ("*Foo", RunOncePrefixSemantics::RunInSafeMode),
    ];

    for (value_name, semantics) in cases {
        let mut snapshot = before_snapshot();
        let entry = registry_entry(&mut snapshot);
        entry.key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce".to_owned();
        entry.value_name = RegistryValueName::decoded(value_name);
        entry.startup_kind = RegistryStartupKind::RunOnce;
        entry.run_once_prefix = Some(semantics);

        snapshot
            .validate()
            .unwrap_or_else(|error| panic!("{value_name} must validate: {error}"));
        let json = serde_json::to_string(&snapshot).expect("Snapshot must serialize");
        let reparsed: Snapshot = serde_json::from_str(&json).expect("Snapshot must deserialize");
        assert_eq!(reparsed, snapshot);
    }
}

#[test]
fn undocumented_run_once_prefix_forms_remain_uninterpreted() {
    for value_name in ["!*Foo", "*!Foo", "!!Foo", "**Foo", "!", "*"] {
        let mut snapshot = before_snapshot();
        let entry = registry_entry(&mut snapshot);
        entry.key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce".to_owned();
        entry.value_name = RegistryValueName::decoded(value_name);
        entry.startup_kind = RegistryStartupKind::RunOnce;
        entry.run_once_prefix = Some(RunOncePrefixSemantics::Undocumented);

        snapshot
            .validate()
            .unwrap_or_else(|error| panic!("{value_name} must remain valid raw evidence: {error}"));
        assert_eq!(
            registry_entry(&mut snapshot).value_name.decoded_value(),
            Some(value_name)
        );
    }
}

#[test]
fn inconsistent_run_once_evidence_is_rejected() {
    let cases = [
        ("!Foo", Some(RunOncePrefixSemantics::NoDocumentedPrefix)),
        (
            "*Foo",
            Some(RunOncePrefixSemantics::DeferDeletionUntilAfterRun),
        ),
        ("Foo", Some(RunOncePrefixSemantics::RunInSafeMode)),
        ("Foo", None),
    ];

    for (value_name, semantics) in cases {
        let mut snapshot = before_snapshot();
        let entry = registry_entry(&mut snapshot);
        entry.key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce".to_owned();
        entry.value_name = RegistryValueName::decoded(value_name);
        entry.startup_kind = RegistryStartupKind::RunOnce;
        entry.run_once_prefix = semantics;

        assert!(matches!(
            snapshot.validate(),
            Err(SnapshotValidationError::InvalidRegistryEvidence {
                field: "value_name/run_once_prefix"
            })
        ));
    }

    let mut run_snapshot = before_snapshot();
    registry_entry(&mut run_snapshot).run_once_prefix =
        Some(RunOncePrefixSemantics::NoDocumentedPrefix);
    assert!(matches!(
        run_snapshot.validate(),
        Err(SnapshotValidationError::InvalidRegistryEvidence {
            field: "startup_kind/run_once_prefix"
        })
    ));

    let mut mismatched_key = before_snapshot();
    registry_entry(&mut mismatched_key).startup_kind = RegistryStartupKind::RunOnce;
    assert!(matches!(
        mismatched_key.validate(),
        Err(SnapshotValidationError::InvalidRegistryEvidence {
            field: "key_path/startup_kind"
        })
    ));
}

#[test]
fn full_run_once_value_names_keep_distinct_observation_identities() {
    let mut snapshot = before_snapshot();
    let template_index = snapshot
        .observations
        .iter()
        .position(|observation| matches!(&observation.artifact, Artifact::RegistryStartup(_)))
        .expect("fixture must contain Registry evidence");
    let template = snapshot.observations.remove(template_index);

    let cases = [
        (
            "Foo",
            RunOncePrefixSemantics::NoDocumentedPrefix,
            "e300b1f49c3d61d973561e229a5b174ff27312a0df7c72801f6db2e8bd256a9e",
        ),
        (
            "!Foo",
            RunOncePrefixSemantics::DeferDeletionUntilAfterRun,
            "246bb80ad302e1d428b58825421b6ec88d372e0e7d68dcf60185332d7607d833",
        ),
        (
            "*Foo",
            RunOncePrefixSemantics::RunInSafeMode,
            "23c891afd1729eb817401b675d713aea7b9acfa5e9be7103a3c3e522c7edec94",
        ),
    ];
    for (value_name, semantics, canonical_id) in cases {
        let mut observation = template.clone();
        observation.canonical_id = canonical_id.to_owned();
        let Artifact::RegistryStartup(entry) = &mut observation.artifact else {
            unreachable!("template must contain Registry evidence");
        };
        entry.key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce".to_owned();
        entry.value_name = RegistryValueName::decoded(value_name);
        entry.startup_kind = RegistryStartupKind::RunOnce;
        entry.run_once_prefix = Some(semantics);
        snapshot.observations.push(observation);
    }

    snapshot
        .validate()
        .expect("full prefixed names must remain distinct identities");
    let keys: BTreeSet<_> = snapshot
        .observations
        .iter()
        .filter(|observation| matches!(&observation.artifact, Artifact::RegistryStartup(_)))
        .map(|observation| observation.key())
        .collect();
    assert_eq!(keys.len(), 3);
    for expected in cases.map(|(_, _, canonical_id)| canonical_id) {
        assert!(keys.iter().any(|key| key.canonical_id == expected));
    }
}

#[test]
fn unnamed_value_is_stable_evidence_not_marker_corruption() {
    let mut snapshot = before_snapshot();
    let entry = registry_entry(&mut snapshot);
    entry.key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce".to_owned();
    entry.value_name = RegistryValueName::decoded("");
    entry.startup_kind = RegistryStartupKind::RunOnce;
    entry.run_once_prefix = Some(RunOncePrefixSemantics::NoDocumentedPrefix);

    snapshot
        .validate()
        .expect("the unnamed value must remain valid evidence");
    let json = serde_json::to_string(&snapshot).expect("Snapshot must serialize");
    let reparsed: Snapshot = serde_json::from_str(&json).expect("Snapshot must deserialize");
    assert_eq!(reparsed, snapshot);
}

#[test]
fn invalid_utf16_value_name_round_trips_losslessly() {
    let units = [u16::from(b'!'), 0xd800, u16::from(b'X')];
    let name = RegistryValueName::from_utf16_units(&units);
    assert!(matches!(name, RegistryValueName::InvalidUtf16 { .. }));
    assert_eq!(name.utf16_units().as_deref(), Some(units.as_slice()));

    let mut snapshot = before_snapshot();
    let entry = registry_entry(&mut snapshot);
    entry.key_path = "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce".to_owned();
    entry.value_name = name;
    entry.startup_kind = RegistryStartupKind::RunOnce;
    entry.run_once_prefix = Some(RunOncePrefixSemantics::DeferDeletionUntilAfterRun);
    snapshot
        .validate()
        .expect("invalid UTF-16 name evidence must remain lossless");

    let json = serde_json::to_string(&snapshot).expect("Snapshot must serialize");
    let reparsed: Snapshot = serde_json::from_str(&json).expect("Snapshot must deserialize");
    assert_eq!(reparsed, snapshot);
}
