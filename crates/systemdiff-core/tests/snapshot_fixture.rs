use systemdiff_core::{
    Artifact, CollectorStatus, REGISTRY_RAW_EVIDENCE_MAX_CAPTURE_BYTES, RegistryDecodedValue,
    RegistryHive, RegistryRawEvidence, RegistryStartupEntry, RegistryValueDecoding, RegistryView,
    Snapshot, SnapshotValidationError,
};

fn before_snapshot() -> Snapshot {
    serde_json::from_str(include_str!("../../../fixtures/snapshots/before-v1.json"))
        .expect("the before fixture must deserialize")
}

#[test]
fn draft_v1_fixture_validates_and_round_trips() {
    let snapshot = before_snapshot();
    snapshot.validate().expect("the fixture must be valid");

    let json = serde_json::to_string_pretty(&snapshot).expect("snapshot must serialize");
    let reparsed: Snapshot = serde_json::from_str(&json).expect("snapshot must deserialize");

    assert_eq!(reparsed, snapshot);
}

#[test]
fn registry_fixture_uses_typed_decoding_without_raw_evidence() {
    for fixture in [
        include_str!("../../../fixtures/snapshots/before-v1.json"),
        include_str!("../../../fixtures/snapshots/after-v1.json"),
    ] {
        let snapshot: Snapshot =
            serde_json::from_str(fixture).expect("snapshot fixture must deserialize");
        for observation in snapshot.observations {
            let Artifact::RegistryStartup(entry) = observation.artifact else {
                continue;
            };
            assert_eq!(entry.value_type, 1);
            assert_eq!(entry.content_sha256.len(), 64);
            assert!(entry.raw_evidence.is_none());
            assert!(matches!(
                entry.decoding,
                RegistryValueDecoding::Decoded {
                    value: RegistryDecodedValue::String { .. }
                }
            ));
        }
    }
}

#[test]
fn registry_decoding_round_trips_typed_values_and_compact_raw_evidence() {
    let decoded_values = [
        (
            1,
            RegistryDecodedValue::String {
                value: "example".to_owned(),
            },
        ),
        (
            2,
            RegistryDecodedValue::ExpandString {
                value: "%LOCALAPPDATA%\\Example".to_owned(),
            },
        ),
        (
            7,
            RegistryDecodedValue::MultiString {
                values: vec!["one".to_owned(), "two".to_owned()],
            },
        ),
        (4, RegistryDecodedValue::Dword { value: 42 }),
        (11, RegistryDecodedValue::Qword { value: 42 }),
    ];

    for (value_type, decoded_value) in decoded_values {
        let entry = RegistryStartupEntry {
            hive: RegistryHive::CurrentUser,
            registry_view: RegistryView::Shared,
            key_path: "Software\\Example".to_owned(),
            value_name: "Synthetic".to_owned(),
            value_type,
            content_sha256: "0".repeat(64),
            decoding: RegistryValueDecoding::Decoded {
                value: decoded_value,
            },
            raw_evidence: None,
        };

        let json = serde_json::to_string(&entry).expect("registry evidence must serialize");
        let reparsed: RegistryStartupEntry =
            serde_json::from_str(&json).expect("registry evidence must deserialize");
        assert_eq!(reparsed, entry);
    }

    let entry = RegistryStartupEntry {
        hive: RegistryHive::CurrentUser,
        registry_view: RegistryView::Shared,
        key_path: "Software\\Example".to_owned(),
        value_name: "Synthetic".to_owned(),
        value_type: 4,
        content_sha256: "e8a4b2ee7ede79a3afb332b5b6cc3d952a65fd8cffb897f5d18016577c33d7cc"
            .to_owned(),
        decoding: RegistryValueDecoding::Decoded {
            value: RegistryDecodedValue::Dword { value: 42 },
        },
        raw_evidence: Some(RegistryRawEvidence {
            content_hex: "2a000000".to_owned(),
            captured_byte_count: 4,
            original_byte_count: 4,
            truncated: false,
        }),
    };
    let json = serde_json::to_value(&entry).expect("registry evidence must serialize");
    assert!(json["raw_evidence"]["content_hex"].is_string());
    assert!(!json["raw_evidence"]["content_hex"].is_array());
}

#[test]
fn invalid_registry_evidence_invariants_are_rejected() {
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

    let mut invalid_hash = before_snapshot();
    registry_entry(&mut invalid_hash).content_sha256 = "not-a-sha256".to_owned();
    assert!(matches!(
        invalid_hash.validate(),
        Err(SnapshotValidationError::InvalidRegistryEvidence { .. })
    ));

    let mut mismatched_type = before_snapshot();
    registry_entry(&mut mismatched_type).decoding = RegistryValueDecoding::Decoded {
        value: RegistryDecodedValue::Dword { value: 42 },
    };
    assert!(matches!(
        mismatched_type.validate(),
        Err(SnapshotValidationError::InvalidRegistryEvidence { .. })
    ));

    let mut inconsistent_raw = before_snapshot();
    registry_entry(&mut inconsistent_raw).raw_evidence = Some(RegistryRawEvidence {
        content_hex: "2a000000".to_owned(),
        captured_byte_count: 4,
        original_byte_count: 8,
        truncated: false,
    });
    assert!(matches!(
        inconsistent_raw.validate(),
        Err(SnapshotValidationError::InvalidRegistryEvidence { .. })
    ));

    let mut oversized_raw = before_snapshot();
    registry_entry(&mut oversized_raw).raw_evidence = Some(RegistryRawEvidence {
        content_hex: String::new(),
        captured_byte_count: REGISTRY_RAW_EVIDENCE_MAX_CAPTURE_BYTES + 1,
        original_byte_count: REGISTRY_RAW_EVIDENCE_MAX_CAPTURE_BYTES + 1,
        truncated: false,
    });
    assert!(matches!(
        oversized_raw.validate(),
        Err(SnapshotValidationError::InvalidRegistryEvidence { .. })
    ));
}

#[test]
fn duplicate_observation_identity_is_rejected() {
    let mut snapshot = before_snapshot();
    snapshot.observations.push(snapshot.observations[0].clone());

    assert!(matches!(
        snapshot.validate(),
        Err(SnapshotValidationError::DuplicateObservation(_))
    ));
}

#[test]
fn unknown_document_type_is_rejected() {
    let mut snapshot = before_snapshot();
    snapshot.document_type = "example.snapshot".to_owned();

    assert!(matches!(
        snapshot.validate(),
        Err(SnapshotValidationError::UnexpectedDocumentType { .. })
    ));
}

#[test]
fn failed_aggregate_cannot_claim_complete_scope_coverage() {
    let mut snapshot = before_snapshot();
    let services = snapshot
        .collectors
        .iter_mut()
        .find(|run| run.id == "windows.services")
        .expect("services run must exist");
    services.status = CollectorStatus::Failed;

    assert!(matches!(
        snapshot.validate(),
        Err(SnapshotValidationError::InconsistentCollectorStatus { .. })
    ));
}

#[test]
fn unavailable_scope_cannot_emit_observations_from_partial_collector() {
    let mut snapshot = before_snapshot();
    let tasks = snapshot
        .collectors
        .iter_mut()
        .find(|run| run.id == "windows.scheduled_tasks")
        .expect("scheduled tasks run must exist");
    tasks.coverage[0].status = CollectorStatus::PermissionDenied;

    assert!(matches!(
        snapshot.validate(),
        Err(SnapshotValidationError::ObservationFromUnavailableScope { .. })
    ));
}
