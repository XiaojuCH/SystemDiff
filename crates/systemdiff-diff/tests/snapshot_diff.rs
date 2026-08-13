use systemdiff_core::{Artifact, CollectorStatus, RegistryValueDecoding, Snapshot};
use systemdiff_diff::{ChangeKind, DiffError, DiffOptions, diff_snapshots};

fn snapshots() -> (Snapshot, Snapshot) {
    let before = serde_json::from_str(include_str!("../../../fixtures/snapshots/before-v1.json"))
        .expect("the before fixture must deserialize");
    let after = serde_json::from_str(include_str!("../../../fixtures/snapshots/after-v1.json"))
        .expect("the after fixture must deserialize");
    (before, after)
}

fn registry_snapshots() -> (Snapshot, Snapshot) {
    let before = serde_json::from_str(include_str!(
        "../../../fixtures/snapshots/registry-before-v1.json"
    ))
    .expect("the Registry before fixture must deserialize");
    let after = serde_json::from_str(include_str!(
        "../../../fixtures/snapshots/registry-after-v1.json"
    ))
    .expect("the Registry after fixture must deserialize");
    (before, after)
}

#[test]
fn broad_fixture_classifies_confirmed_and_inconclusive_changes() {
    let (before, after) = snapshots();
    let diff = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("synthetic snapshots must be comparable");

    assert!(
        diff.changes
            .iter()
            .any(|change| matches!(&change.change, ChangeKind::Added { .. }))
    );
    assert!(
        diff.changes
            .iter()
            .any(|change| matches!(&change.change, ChangeKind::Modified { .. }))
    );
    assert!(diff.changes.iter().any(|change| {
        change.key.collector_id == "windows.scheduled_tasks"
            && matches!(&change.change, ChangeKind::Inconclusive { .. })
    }));
    assert_eq!(diff.warnings.len(), 2);
    assert!(
        diff.warnings
            .iter()
            .any(|warning| warning.collector_id == "windows.services")
    );
    assert!(
        diff.warnings
            .iter()
            .any(|warning| warning.collector_id == "windows.scheduled_tasks")
    );
}

fn service_fixture_snapshot(path: &str) -> Snapshot {
    let text = match path {
        "before" => include_str!("../../../fixtures/snapshots/services-added-before-v1.json"),
        "after" => include_str!("../../../fixtures/snapshots/services-added-after-v1.json"),
        _ => unreachable!(),
    };
    serde_json::from_str(text).expect("focused Services fixture must deserialize")
}

#[test]
fn complete_service_scope_confirms_added_and_removed() {
    let before = service_fixture_snapshot("before");
    let after = service_fixture_snapshot("after");
    let added = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("complete service fixture must compare");
    assert_eq!(added.changes.len(), 1);
    assert!(matches!(added.changes[0].change, ChangeKind::Added { .. }));

    let removed = diff_snapshots(&after, &before, DiffOptions::default())
        .expect("reversed complete service fixture must compare");
    assert_eq!(removed.changes.len(), 1);
    assert!(matches!(
        removed.changes[0].change,
        ChangeKind::Removed { .. }
    ));
}

#[test]
fn directly_observed_service_fields_compare_as_modified_under_partial_coverage() {
    let mut before = service_fixture_snapshot("after");
    let mut after = before.clone();
    for snapshot in [&mut before, &mut after] {
        snapshot.collectors[0].status = CollectorStatus::Partial;
        snapshot.collectors[0].coverage[0].status = CollectorStatus::Partial;
        snapshot.collectors[0]
            .diagnostics
            .push(systemdiff_core::Diagnostic {
                code: "service_visibility_best_effort".to_owned(),
                message: "Synthetic current-token coverage is incomplete.".to_owned(),
                stage: Some("enumerate".to_owned()),
                native_code: None,
                scope_id: Some("current_token.win32".to_owned()),
            });
    }
    let Artifact::WindowsService(service) = &mut after.observations[0].artifact else {
        unreachable!()
    };
    service.start_type = 2;
    service.binary_path = r#"%ProgramFiles%\Example\new-service.exe --service"#.to_owned();
    service.description = Some("Updated description only where stated.".to_owned());

    let diff = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("direct service evidence must compare");
    assert!(matches!(
        diff.changes[0].change,
        ChangeKind::Modified { .. }
    ));
}

#[test]
fn partial_service_scope_never_turns_disappearance_into_removed() {
    let before = service_fixture_snapshot("after");
    let mut after = service_fixture_snapshot("before");
    after.collectors[0].status = CollectorStatus::Partial;
    after.collectors[0].coverage[0].status = CollectorStatus::Partial;
    after.collectors[0]
        .diagnostics
        .push(systemdiff_core::Diagnostic {
            code: "service_visibility_best_effort".to_owned(),
            message: "Synthetic current-token coverage is incomplete.".to_owned(),
            stage: Some("enumerate".to_owned()),
            native_code: None,
            scope_id: Some("current_token.win32".to_owned()),
        });

    let diff = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("partial service fixture must compare conservatively");
    assert!(matches!(
        diff.changes[0].change,
        ChangeKind::Inconclusive { .. }
    ));
}

#[test]
fn shuffled_observations_produce_identical_diff_documents() {
    let (before, after) = snapshots();
    let baseline = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("synthetic snapshots must be comparable");

    let mut shuffled_before = before;
    let mut shuffled_after = after;
    shuffled_before.observations.reverse();
    shuffled_after.observations.reverse();

    let shuffled = diff_snapshots(&shuffled_before, &shuffled_after, DiffOptions::default())
        .expect("shuffled snapshots must be comparable");
    assert_eq!(shuffled, baseline);

    let baseline_json = serde_json::to_string_pretty(&baseline).expect("diff must serialize");
    let shuffled_json = serde_json::to_string_pretty(&shuffled).expect("diff must serialize");
    assert_eq!(shuffled_json, baseline_json);
}

#[test]
fn unchanged_artifacts_are_opt_in() {
    let (before, _) = snapshots();

    let default_diff = diff_snapshots(&before, &before, DiffOptions::default())
        .expect("snapshot must compare with itself");
    assert!(default_diff.changes.is_empty());

    let verbose_diff = diff_snapshots(
        &before,
        &before,
        DiffOptions {
            include_unchanged: true,
        },
    )
    .expect("snapshot must compare with itself");
    assert_eq!(verbose_diff.changes.len(), before.observations.len());
    assert!(
        verbose_diff
            .changes
            .iter()
            .all(|change| matches!(&change.change, ChangeKind::Unchanged { .. }))
    );
}

#[test]
fn failed_collector_does_not_create_false_removals() {
    let (before, _) = snapshots();
    let mut after = before.clone();
    after.captured_at = "2026-08-11T00:10:00Z".to_owned();
    let services = after
        .collectors
        .iter_mut()
        .find(|run| run.id == "windows.services")
        .expect("services run must exist");
    services.status = CollectorStatus::Failed;
    services.coverage[0].status = CollectorStatus::Failed;
    after
        .observations
        .retain(|observation| observation.collector_id != "windows.services");

    let diff = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("failed coverage must produce a conservative diff");
    let service_change = diff
        .changes
        .iter()
        .find(|change| change.key.collector_id == "windows.services")
        .expect("the missing service must remain visible");

    assert!(matches!(
        &service_change.change,
        ChangeKind::Inconclusive { .. }
    ));
    assert!(
        diff.warnings
            .iter()
            .any(|warning| warning.collector_id == "windows.services")
    );
}

#[test]
fn directly_observed_modification_remains_modified_with_partial_coverage() {
    let (before, _) = snapshots();
    let mut after = before.clone();
    after.captured_at = "2026-08-11T00:10:00Z".to_owned();
    let task = after
        .observations
        .iter_mut()
        .find(|observation| observation.collector_id == "windows.scheduled_tasks")
        .expect("scheduled task observation must exist");
    let Artifact::ScheduledTask(task) = &mut task.artifact else {
        panic!("scheduled task Collector must emit a scheduled task artifact");
    };
    task.enabled = false;

    let diff = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("partial snapshots with direct evidence must compare");
    let task_change = diff
        .changes
        .iter()
        .find(|change| change.key.collector_id == "windows.scheduled_tasks")
        .expect("the modified task must be present");

    assert!(matches!(&task_change.change, ChangeKind::Modified { .. }));
    assert!(
        diff.warnings
            .iter()
            .any(|warning| warning.collector_id == "windows.scheduled_tasks")
    );
}

#[test]
fn undecoded_registry_values_are_compared_by_full_content_hash() {
    let (mut before, _) = snapshots();
    let registry = before
        .observations
        .iter_mut()
        .find(|observation| observation.collector_id == "windows.registry.startup")
        .expect("Registry observation must exist");
    let Artifact::RegistryStartup(entry) = &mut registry.artifact else {
        panic!("Registry Collector must emit Registry evidence");
    };
    entry.value_type = 3;
    entry.content_sha256 = "a".repeat(64);
    entry.decoding = RegistryValueDecoding::NotApplicable;
    entry.raw_evidence = None;

    let mut after = before.clone();
    after.captured_at = "2026-08-11T00:10:00Z".to_owned();
    let registry = after
        .observations
        .iter_mut()
        .find(|observation| observation.collector_id == "windows.registry.startup")
        .expect("Registry observation must exist");
    let Artifact::RegistryStartup(entry) = &mut registry.artifact else {
        panic!("Registry Collector must emit Registry evidence");
    };
    entry.content_sha256 = "b".repeat(64);

    let diff = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("undecoded Registry evidence must compare");
    let registry_change = diff
        .changes
        .iter()
        .find(|change| change.key.collector_id == "windows.registry.startup")
        .expect("changed Registry evidence must be present");

    assert!(matches!(
        &registry_change.change,
        ChangeKind::Modified { .. }
    ));
}

#[test]
fn overlapping_collector_version_mismatch_is_rejected() {
    let (before, mut after) = snapshots();
    let registry = after
        .collectors
        .iter_mut()
        .find(|run| run.id == "windows.registry.startup")
        .expect("registry run must exist");
    registry.version = 2;
    for observation in &mut after.observations {
        if observation.collector_id == "windows.registry.startup" {
            observation.collector_version = 2;
        }
    }

    assert!(matches!(
        diff_snapshots(&before, &after, DiffOptions::default()),
        Err(DiffError::IncompatibleCollectorVersion { .. })
    ));
}

#[test]
fn registry_fixture_produces_exactly_one_added_startup_value() {
    let (before, after) = registry_snapshots();
    let diff = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("Registry fixtures must be comparable");

    assert!(diff.warnings.is_empty());
    assert_eq!(diff.changes.len(), 1);
    assert_eq!(diff.changes[0].key.collector_id, "windows.registry.startup");
    assert_eq!(diff.changes[0].key.scope_id, "current_user.shared.run");
    assert!(matches!(
        &diff.changes[0].change,
        ChangeKind::Added {
            after: Artifact::RegistryStartup(_)
        }
    ));
}

#[test]
fn partial_registry_scope_does_not_create_a_false_removal() {
    let (_, before) = registry_snapshots();
    let (mut after, _) = registry_snapshots();
    after.captured_at = "2026-08-11T00:02:00Z".to_owned();
    let registry = after
        .collectors
        .iter_mut()
        .find(|run| run.id == "windows.registry.startup")
        .expect("Registry run must exist");
    registry.status = CollectorStatus::Partial;
    registry
        .coverage
        .iter_mut()
        .find(|coverage| coverage.scope_id == "current_user.shared.run")
        .expect("HKCU Run scope must exist")
        .status = CollectorStatus::Partial;

    let diff = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("partial Registry coverage must produce a conservative diff");
    assert_eq!(diff.changes.len(), 1);
    assert!(matches!(
        &diff.changes[0].change,
        ChangeKind::Inconclusive { .. }
    ));
    assert!(
        diff.warnings
            .iter()
            .any(|warning| warning.scope_id == "current_user.shared.run")
    );
}
