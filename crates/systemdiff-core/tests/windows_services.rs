use systemdiff_core::{
    Artifact, CollectorStatus, Snapshot, SnapshotValidationError, WindowsService,
    windows_service_identity,
};

fn before_snapshot() -> Snapshot {
    serde_json::from_str(include_str!("../../../fixtures/snapshots/before-v1.json"))
        .expect("the before fixture must deserialize")
}

fn service_observation(snapshot: &mut Snapshot) -> &mut systemdiff_core::Observation {
    snapshot
        .observations
        .iter_mut()
        .find(|observation| matches!(observation.artifact, Artifact::WindowsService(_)))
        .expect("fixture must contain service evidence")
}

fn service(snapshot: &mut Snapshot) -> &mut WindowsService {
    let Artifact::WindowsService(service) = &mut service_observation(snapshot).artifact else {
        unreachable!()
    };
    service
}

#[test]
fn service_identity_has_fixed_exact_utf16_vectors() {
    assert_eq!(
        windows_service_identity(&"LegacyService".encode_utf16().collect::<Vec<_>>()),
        "46561d2f6078ffa19958353b4dd219719a0316486a319e9b30f5da83feee9a94"
    );
    assert_eq!(
        windows_service_identity(
            &"ExampleUpdaterService_1a2b3"
                .encode_utf16()
                .collect::<Vec<_>>()
        ),
        "1f8fb1c080b80b63fc87e55671c51dda7a3c16cf4a3c7dcbe7224c75ef1000c8"
    );
    assert_ne!(
        windows_service_identity(&"ExampleService".encode_utf16().collect::<Vec<_>>()),
        windows_service_identity(&"exampleservice".encode_utf16().collect::<Vec<_>>())
    );
}

#[test]
fn service_fixture_round_trips_known_absence_and_false_delayed_start() {
    let snapshot = before_snapshot();
    snapshot.validate().expect("fixture must validate");
    let json = serde_json::to_string_pretty(&snapshot).expect("snapshot must serialize");
    let reparsed: Snapshot = serde_json::from_str(&json).expect("snapshot must deserialize");
    assert_eq!(reparsed, snapshot);

    let entry = reparsed
        .observations
        .iter()
        .find_map(|observation| match &observation.artifact {
            Artifact::WindowsService(service) => Some(service),
            _ => None,
        })
        .expect("fixture must contain service evidence");
    assert_eq!(entry.load_order_group, None);
    assert_eq!(entry.tag_id, None);
    assert!(!entry.delayed_auto_start);
}

#[test]
fn old_draft_service_wire_without_new_required_fields_is_rejected() {
    let json = include_str!("../../../fixtures/snapshots/before-v1.json")
        .replace("          \"load_order_group\": null,\n", "")
        .replace("          \"tag_id\": null,\n", "");
    assert!(serde_json::from_str::<Snapshot>(&json).is_err());
}

#[test]
fn invalid_service_identity_and_evidence_are_rejected() {
    let mut invalid = before_snapshot();
    service_observation(&mut invalid).canonical_id = "wrong".to_owned();
    assert!(matches!(
        invalid.validate(),
        Err(SnapshotValidationError::InvalidWindowsServiceEvidence {
            field: "canonical_id/service_name"
        })
    ));

    let mut invalid = before_snapshot();
    service(&mut invalid).service_name.clear();
    assert!(matches!(
        invalid.validate(),
        Err(SnapshotValidationError::InvalidWindowsServiceEvidence {
            field: "service_name"
        })
    ));

    let mut invalid = before_snapshot();
    service(&mut invalid).service_type = 1;
    assert!(matches!(
        invalid.validate(),
        Err(SnapshotValidationError::InvalidWindowsServiceEvidence {
            field: "service_type"
        })
    ));

    let mut invalid = before_snapshot();
    service(&mut invalid).dependencies.push(String::new());
    assert!(matches!(
        invalid.validate(),
        Err(SnapshotValidationError::InvalidWindowsServiceEvidence {
            field: "dependencies"
        })
    ));
}

#[test]
fn service_artifact_must_use_the_services_collector_and_scope() {
    let mut invalid = before_snapshot();
    service_observation(&mut invalid).scope_id = "machine.win32".to_owned();
    assert!(matches!(
        invalid.validate(),
        Err(SnapshotValidationError::MissingCoverage { .. })
            | Err(SnapshotValidationError::InvalidWindowsServiceEvidence { .. })
    ));
}

#[test]
fn real_services_scope_is_represented_as_partial() {
    let snapshot = before_snapshot();
    let run = snapshot
        .collectors
        .iter()
        .find(|run| run.id == "windows.services")
        .expect("services run must exist");
    assert_eq!(run.status, CollectorStatus::Partial);
    assert_eq!(run.coverage[0].status, CollectorStatus::Partial);
    assert_eq!(run.coverage[0].scope_id, "current_token.win32");
}
