use systemdiff_core::{
    Artifact, ArtifactKey, CollectorStatus, RegistryDecodedValue, RegistryHive,
    RegistryStartupEntry, RegistryStartupKind, RegistryValueDecoding, RegistryValueName,
    RegistryView, RunOncePrefixSemantics, Snapshot, WindowsService,
};
use systemdiff_diff::{
    ArtifactChange, ChangeKind, DiffDocument, DiffOptions, DiffWarning, DiffWarningCode,
    InconclusiveReason, diff_snapshots,
};
use systemdiff_report::{render_technical, render_terminal, write_json};

const BEFORE_CAPTURED_AT: &str = "2026-08-11T00:00:00Z";
const AFTER_CAPTURED_AT: &str = "2026-08-11T00:05:00Z";
const RUN_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_ONCE_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\RunOnce";

fn decoded_string(value: &str) -> RegistryValueDecoding {
    RegistryValueDecoding::Decoded {
        value: RegistryDecodedValue::String {
            value: value.to_owned(),
        },
    }
}

fn run_entry(
    name: RegistryValueName,
    decoding: RegistryValueDecoding,
    hash_byte: char,
) -> Artifact {
    Artifact::RegistryStartup(RegistryStartupEntry {
        hive: RegistryHive::CurrentUser,
        registry_view: RegistryView::Shared,
        key_path: RUN_PATH.to_owned(),
        value_name: name,
        startup_kind: RegistryStartupKind::Run,
        run_once_prefix: None,
        value_type: if matches!(&decoding, RegistryValueDecoding::Decoded { .. }) {
            1
        } else {
            3
        },
        content_sha256: hash_byte.to_string().repeat(64),
        decoding,
        raw_evidence: None,
    })
}

fn run_once_entry(name: &str, command: &str, hash_byte: char) -> Artifact {
    let prefix = if name.starts_with('!') {
        RunOncePrefixSemantics::DeferDeletionUntilAfterRun
    } else if name.starts_with('*') {
        RunOncePrefixSemantics::RunInSafeMode
    } else {
        RunOncePrefixSemantics::NoDocumentedPrefix
    };
    Artifact::RegistryStartup(RegistryStartupEntry {
        hive: RegistryHive::LocalMachine,
        registry_view: RegistryView::Registry64,
        key_path: RUN_ONCE_PATH.to_owned(),
        value_name: RegistryValueName::decoded(name),
        startup_kind: RegistryStartupKind::RunOnce,
        run_once_prefix: Some(prefix),
        value_type: 1,
        content_sha256: hash_byte.to_string().repeat(64),
        decoding: decoded_string(command),
        raw_evidence: None,
    })
}

fn key(scope_id: &str, canonical_id: &str) -> ArtifactKey {
    ArtifactKey {
        collector_id: "windows.registry.startup".to_owned(),
        scope_id: scope_id.to_owned(),
        artifact_kind: "registry_startup".to_owned(),
        canonical_id: canonical_id.to_owned(),
    }
}

fn change(
    change_id: &str,
    scope_id: &str,
    canonical_id: &str,
    change: ChangeKind,
) -> ArtifactChange {
    ArtifactChange {
        change_id: change_id.to_owned(),
        key: key(scope_id, canonical_id),
        change,
    }
}

fn windows_service(
    service_name: &str,
    display_name: Option<&str>,
    start_type: u32,
    delayed_auto_start: bool,
) -> Artifact {
    Artifact::WindowsService(WindowsService {
        service_name: service_name.to_owned(),
        display_name: display_name.map(str::to_owned),
        service_type: 0x10,
        start_type,
        error_control: 1,
        binary_path: "C:\\Program Files\\Example\\service.exe --service".to_owned(),
        account: Some("NT AUTHORITY\\LocalService".to_owned()),
        dependencies: vec!["RpcSs".to_owned(), "+NetworkProvider".to_owned()],
        load_order_group: Some("ExampleGroup".to_owned()),
        tag_id: Some(7),
        delayed_auto_start,
        description: Some("Provides an example background service.".to_owned()),
    })
}

fn service_change(change_id: &str, canonical_id: &str, change: ChangeKind) -> ArtifactChange {
    ArtifactChange {
        change_id: change_id.to_owned(),
        key: ArtifactKey {
            collector_id: "windows.services".to_owned(),
            scope_id: "current_token.win32".to_owned(),
            artifact_kind: "windows_service".to_owned(),
            canonical_id: canonical_id.to_owned(),
        },
        change,
    }
}

fn diff(changes: Vec<ArtifactChange>) -> DiffDocument {
    DiffDocument {
        document_type: "systemdiff.diff".to_owned(),
        schema_version: 1,
        before_captured_at: BEFORE_CAPTURED_AT.to_owned(),
        after_captured_at: AFTER_CAPTURED_AT.to_owned(),
        changes,
        warnings: Vec::new(),
    }
}

fn before_fixture() -> Snapshot {
    serde_json::from_str(include_str!("../../../fixtures/snapshots/before-v1.json"))
        .expect("the before fixture must deserialize")
}

fn after_fixture() -> Snapshot {
    serde_json::from_str(include_str!("../../../fixtures/snapshots/after-v1.json"))
        .expect("the after fixture must deserialize")
}

fn occurrence_count(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}

#[test]
fn added_run_and_run_once_entries_are_recognizable_without_opaque_ids() {
    let run = run_entry(
        RegistryValueName::decoded("ExampleUpdater"),
        decoded_string("C:\\Example\\updater.exe --background"),
        'a',
    );
    let run_once = run_once_entry("!FinishInstall", "C:\\Example\\finish-install.exe", 'b');
    let document = diff(vec![
        change(
            "change:v1:00000000",
            "current_user.shared.run",
            "opaque-run-identity",
            ChangeKind::Added { after: run },
        ),
        change(
            "change:v1:00000001",
            "local_machine.registry64.run_once",
            "opaque-run-once-identity",
            ChangeKind::Added { after: run_once },
        ),
    ]);

    let output = render_terminal(&document);

    assert!(output.contains("Added"));
    assert!(output.contains("ExampleUpdater"));
    assert!(output.contains("C:\\Example\\updater.exe --background"));
    assert!(output.contains("Run"));
    assert!(output.contains("!FinishInstall"));
    assert!(output.contains("C:\\Example\\finish-install.exe"));
    assert!(output.contains("RunOnce"));
    assert!(!output.contains("opaque-run-identity"));
    assert!(!output.contains(&"a".repeat(64)));
}

#[test]
fn modified_decoded_entry_shows_before_and_after_values() {
    let before = run_entry(
        RegistryValueName::decoded("ExampleApp"),
        decoded_string("C:\\Example\\example.exe --old"),
        'a',
    );
    let after = run_entry(
        RegistryValueName::decoded("ExampleApp"),
        decoded_string("C:\\Example\\example.exe --new"),
        'b',
    );
    let document = diff(vec![change(
        "change:v1:00000000",
        "current_user.shared.run",
        "example-app",
        ChangeKind::Modified { before, after },
    )]);

    let output = render_terminal(&document);

    assert!(output.contains("Modified"));
    assert!(output.contains("ExampleApp"));
    let before_index = output
        .find("C:\\Example\\example.exe --old")
        .expect("the previous decoded value must be visible");
    let after_index = output
        .find("C:\\Example\\example.exe --new")
        .expect("the new decoded value must be visible");
    assert!(
        before_index < after_index,
        "before evidence must precede after evidence"
    );
}

#[test]
fn modified_native_evidence_does_not_claim_an_unchanged_command_changed() {
    let before = run_entry(
        RegistryValueName::decoded("ExampleApp"),
        decoded_string("C:\\Example\\example.exe"),
        'a',
    );
    let mut after = run_entry(
        RegistryValueName::decoded("ExampleApp"),
        decoded_string("C:\\Example\\example.exe"),
        'b',
    );
    if let Artifact::RegistryStartup(entry) = &mut after {
        entry.value_type = 2;
        entry.decoding = RegistryValueDecoding::Decoded {
            value: RegistryDecodedValue::ExpandString {
                value: "C:\\Example\\example.exe".to_owned(),
            },
        };
    }
    let document = diff(vec![change(
        "change:v1:00000000",
        "current_user.shared.run",
        "example-app",
        ChangeKind::Modified { before, after },
    )]);

    let output = render_terminal(&document);

    assert!(output.contains("Registry startup evidence changed"));
    assert!(!output.contains("Startup command changed"));
}

#[test]
fn removed_entry_is_confirmed_but_inconclusive_absence_stays_uncertain() {
    let artifact = run_entry(
        RegistryValueName::decoded("LegacyUpdater"),
        decoded_string("C:\\Legacy\\updater.exe"),
        'a',
    );
    let removed = diff(vec![change(
        "change:v1:00000000",
        "current_user.shared.run",
        "legacy-updater",
        ChangeKind::Removed {
            before: artifact.clone(),
        },
    )]);
    let inconclusive = diff(vec![change(
        "change:v1:00000000",
        "current_user.shared.run",
        "legacy-updater",
        ChangeKind::Inconclusive {
            before: Some(artifact),
            after: None,
            reason: InconclusiveReason::CoverageIncomplete,
        },
    )]);

    let removed_output = render_terminal(&removed);
    let inconclusive_output = render_terminal(&inconclusive);

    assert!(removed_output.contains("Removed"));
    assert!(removed_output.contains("LegacyUpdater"));
    assert!(inconclusive_output.contains("LegacyUpdater"));
    assert!(inconclusive_output.contains("Could not confirm 1 possible change"));
    assert!(
        inconclusive_output
            .to_ascii_lowercase()
            .contains("inconclusive")
    );
    assert!(
        inconclusive_output
            .to_ascii_lowercase()
            .contains("coverage")
    );
    assert!(!inconclusive_output.contains("Removed"));
}

#[test]
fn undecoded_registry_data_is_not_presented_as_a_command() {
    let artifact = run_entry(
        RegistryValueName::decoded("BinaryStartupValue"),
        RegistryValueDecoding::UnsupportedType,
        'c',
    );
    let document = diff(vec![change(
        "change:v1:00000000",
        "current_user.shared.run",
        "binary-value",
        ChangeKind::Added { after: artifact },
    )]);

    let output = render_terminal(&document);

    assert!(output.contains("BinaryStartupValue"));
    assert!(output.to_ascii_lowercase().contains("not decoded"));
    assert!(!output.contains("Command:"));
    assert!(!output.contains(&"c".repeat(64)));
}

#[test]
fn invalid_utf16_and_unnamed_value_names_have_explicit_human_labels() {
    let invalid_name = RegistryValueName::InvalidUtf16 {
        utf16le_hex: "00d85800".to_owned(),
    };
    let invalid = run_entry(
        invalid_name,
        decoded_string("C:\\Example\\invalid.exe"),
        'a',
    );
    let unnamed = run_entry(
        RegistryValueName::decoded(""),
        decoded_string("C:\\Example\\default.exe"),
        'b',
    );
    let document = diff(vec![
        change(
            "change:v1:00000000",
            "current_user.shared.run",
            "invalid-name",
            ChangeKind::Added { after: invalid },
        ),
        change(
            "change:v1:00000001",
            "current_user.shared.run",
            "unnamed-value",
            ChangeKind::Added { after: unnamed },
        ),
    ]);

    let output = render_terminal(&document);

    assert!(output.contains("Name could not be decoded as UTF-16"));
    assert!(output.contains("Default value (unnamed)"));
    assert!(!output.contains("invalid-name"));
}

#[test]
fn multiple_changes_share_one_registry_group_and_empty_diff_is_calm() {
    let first = run_entry(
        RegistryValueName::decoded("First"),
        decoded_string("C:\\Example\\first.exe"),
        'a',
    );
    let second = run_once_entry("Second", "C:\\Example\\second.exe", 'b');
    let document = diff(vec![
        change(
            "change:v1:00000000",
            "current_user.shared.run",
            "first",
            ChangeKind::Added { after: first },
        ),
        change(
            "change:v1:00000001",
            "local_machine.registry64.run_once",
            "second",
            ChangeKind::Added { after: second },
        ),
    ]);

    let output = render_terminal(&document);
    let empty_output = render_terminal(&diff(Vec::new()));

    assert_eq!(occurrence_count(&output, "Registry startup changes"), 1);
    assert!(
        output.find("First").expect("First must render")
            < output.find("Second").expect("Second must render")
    );
    assert!(empty_output.contains("No changes"));
}

#[test]
fn partial_coverage_warning_is_prominent_and_preserves_status_context() {
    let artifact = run_entry(
        RegistryValueName::decoded("MaybeGone"),
        decoded_string("C:\\Example\\maybe.exe"),
        'a',
    );
    let mut document = diff(vec![change(
        "change:v1:00000000",
        "current_user.shared.run",
        "maybe-gone",
        ChangeKind::Inconclusive {
            before: Some(artifact),
            after: None,
            reason: InconclusiveReason::CoverageIncomplete,
        },
    )]);
    document.warnings.push(DiffWarning {
        code: DiffWarningCode::CoverageIncomplete,
        collector_id: "windows.registry.startup".to_owned(),
        scope_id: "current_user.shared.run".to_owned(),
        before_status: Some(CollectorStatus::Complete),
        after_status: Some(CollectorStatus::Partial),
    });

    let output = render_terminal(&document);

    assert!(output.to_ascii_lowercase().contains("coverage"));
    assert!(output.contains("Current-user Run startup"));
    assert!(!output.contains("current_user.shared.run"));
    assert!(output.to_ascii_lowercase().contains("complete"));
    assert!(output.to_ascii_lowercase().contains("partial"));
    assert!(!output.contains("Removed"));
}

#[test]
fn observed_control_characters_cannot_inject_lines_or_ansi_sequences() {
    let artifact = run_entry(
        RegistryValueName::decoded("Evil\nName\u{1b}[31m\u{2028}\u{202e}"),
        decoded_string("C:\\Example\\evil.exe\r\nInjected\targument\u{7}"),
        'a',
    );
    let document = diff(vec![change(
        "change:v1:00000000",
        "current_user.shared.run",
        "control-value",
        ChangeKind::Added { after: artifact },
    )]);
    let before = before_fixture();
    let after = after_fixture();

    for output in [
        render_terminal(&document),
        render_technical(&document, &before, &after),
    ] {
        assert!(
            !output.contains('\u{1b}'),
            "raw ANSI escape must never reach output"
        );
        assert!(!output.contains('\r'));
        assert!(!output.contains('\t'));
        assert!(!output.contains('\u{7}'));
        assert!(!output.contains('\u{202e}'));
        assert!(!output.contains('\u{2028}'));
        assert!(
            !output
                .lines()
                .any(|line| line == "Name" || line == "Injected")
        );
        assert!(output.contains("Evil\\nName"));
        assert!(output.contains("\\u{202e}"));
        assert!(output.contains("\\u{2028}"));
        assert!(output.contains("\\r\\nInjected\\targument"));
    }
}

#[test]
fn technical_mode_keeps_lossless_invalid_and_unnamed_value_names() {
    let invalid = run_entry(
        RegistryValueName::InvalidUtf16 {
            utf16le_hex: "00d85800".to_owned(),
        },
        decoded_string("C:\\Example\\invalid.exe"),
        'a',
    );
    let unnamed = run_entry(
        RegistryValueName::decoded(""),
        decoded_string("C:\\Example\\default.exe"),
        'b',
    );
    let document = diff(vec![
        change(
            "change:v1:00000000",
            "current_user.shared.run",
            "invalid-name",
            ChangeKind::Added { after: invalid },
        ),
        change(
            "change:v1:00000001",
            "current_user.shared.run",
            "unnamed-value",
            ChangeKind::Added { after: unnamed },
        ),
    ]);

    let output = render_technical(&document, &before_fixture(), &after_fixture());

    assert!(output.contains("value name encoding: invalid_utf16"));
    assert!(output.contains("value name UTF-16LE hex: 00d85800"));
    assert!(output.contains("value name encoding: decoded"));
    assert!(output.contains("value name: <empty>"));
}

#[test]
fn technical_mode_exposes_exact_evidence_versions_and_scoped_diagnostics() {
    let before = before_fixture();
    let after = after_fixture();
    let document = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("the broad fixtures must produce a deterministic diff");

    let output = render_technical(&document, &before, &after);

    for expected in [
        "windows.registry.startup",
        "version: 1",
        "current_user.shared.run",
        "registry_startup",
        "05b14fce21e4232e9c47bc29ba58737949d4c17c78c8f2efb289128644c60603",
        "current_user",
        "shared",
        RUN_PATH,
        "ExampleApp",
        "value type: 1",
        "decoded",
        r#"C:\\Example\\example.exe --background"#,
        r#"C:\\Example\\example.exe --background --updated"#,
        "b27cce45267b6100cdd3267ec6dcdf2023e6846bb3f0f66162ab179678f0727c",
        "e3f7174dd4ae12dc6be7e6d17d598e035cd03bf994008d3e8866c15183640f35",
        "task_folder_access_denied",
        "enumerate_folder",
        "-2147024891",
    ] {
        assert!(
            output.contains(expected),
            "technical output omitted {expected:?}\n{output}"
        );
    }
}

#[test]
fn technical_mode_lists_snapshot_coverage_even_without_observations_or_changes() {
    let before: Snapshot = serde_json::from_str(include_str!(
        "../../../fixtures/snapshots/registry-before-v1.json"
    ))
    .expect("the Registry before fixture must deserialize");
    let mut after = before.clone();
    after.captured_at = AFTER_CAPTURED_AT.to_owned();
    after.collectors[0].status = CollectorStatus::Partial;
    after.collectors[0].coverage[0].status = CollectorStatus::Partial;
    let document = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("an empty partial Registry scope must produce a Diff");
    assert!(document.changes.is_empty());
    assert_eq!(document.warnings.len(), 1);

    let output = render_technical(&document, &before, &after);

    assert!(output.contains("Before Snapshot collector coverage"));
    assert!(output.contains("After Snapshot collector coverage"));
    assert!(output.contains("collector ID: windows.registry.startup"));
    assert!(output.contains("version: 1"));
    assert!(output.contains("aggregate status: complete"));
    assert!(output.contains("aggregate status: partial"));
    assert!(output.contains("scope current_user.shared.run: complete"));
    assert!(output.contains("scope current_user.shared.run: partial"));
}

#[test]
fn technical_multi_string_keeps_element_boundaries_and_empty_values() {
    let mut artifact = run_entry(
        RegistryValueName::decoded("MultiValue"),
        RegistryValueDecoding::Decoded {
            value: RegistryDecodedValue::MultiString {
                values: vec!["a, b".to_owned(), String::new(), "c".to_owned()],
            },
        },
        'a',
    );
    if let Artifact::RegistryStartup(entry) = &mut artifact {
        entry.value_type = 7;
    }
    let document = diff(vec![change(
        "change:v1:00000000",
        "current_user.shared.run",
        "multi-value",
        ChangeKind::Added { after: artifact },
    )]);

    let output = render_technical(&document, &before_fixture(), &after_fixture());

    assert!(output.contains(r#"multi_string (3 elements): [0]="a, b"; [1]=""; [2]="c""#));
}

#[test]
fn service_changes_have_a_dedicated_factual_human_group() {
    let added = windows_service("ExampleUpdater", Some("Example Update Service"), 2, true);
    let removed = windows_service("LegacyAgent", None, 4, false);
    let document = diff(vec![
        service_change(
            "change:v1:00000000",
            "service-a",
            ChangeKind::Added { after: added },
        ),
        service_change(
            "change:v1:00000001",
            "service-b",
            ChangeKind::Removed { before: removed },
        ),
    ]);

    let output = render_terminal(&document);

    assert_eq!(occurrence_count(&output, "Windows service changes"), 1);
    assert!(!output.contains("Other evidence changes"));
    for expected in [
        "Example Update Service",
        "Added (Windows service)",
        "Service name: ExampleUpdater",
        "Automatic (delayed start)",
        r#"C:\Program Files\Example\service.exe --service"#,
        r#"NT AUTHORITY\LocalService"#,
        "LegacyAgent",
        "Removed (Windows service)",
        "Disabled",
    ] {
        assert!(
            output.contains(expected),
            "human output omitted {expected:?}"
        );
    }
    assert!(!output.contains("service-a"));
    assert!(!output.to_ascii_lowercase().contains("malicious"));
    assert!(!output.to_ascii_lowercase().contains("suspicious"));
}

#[test]
fn modified_service_lists_only_fields_that_changed() {
    let before = windows_service("ExampleUpdater", Some("Example Update Service"), 3, false);
    let mut after = before.clone();
    let Artifact::WindowsService(after_service) = &mut after else {
        unreachable!("helper must return a service")
    };
    after_service.start_type = 2;
    after_service.delayed_auto_start = true;
    after_service.description = Some("Updated description".to_owned());
    let document = diff(vec![service_change(
        "change:v1:00000000",
        "service-a",
        ChangeKind::Modified { before, after },
    )]);

    let output = render_terminal(&document);

    assert!(output.contains("Modified (Windows service)"));
    assert!(output.contains("Start:"));
    assert!(output.contains("Before: Manual (on demand)"));
    assert!(output.contains("After:  Automatic (delayed start)"));
    assert!(output.contains("Delayed automatic start configured:"));
    assert!(output.contains("Before: No"));
    assert!(output.contains("After:  Yes"));
    assert!(output.contains("Description:"));
    assert!(output.contains("Before: Provides an example background service."));
    assert!(output.contains("After:  Updated description"));
    assert!(!output.contains("Binary path:"));
    assert!(!output.contains("Account:"));
    assert!(!output.contains("Dependencies:"));
    assert!(!output.contains("Load-order group:"));
    assert!(!output.contains("Tag ID:"));
    assert!(!output.contains("Error control:"));
}

#[test]
fn delayed_flag_change_is_visible_for_non_automatic_service() {
    let before = windows_service("ManualService", None, 3, false);
    let mut after = before.clone();
    let Artifact::WindowsService(after_service) = &mut after else {
        unreachable!("helper must return a service")
    };
    after_service.delayed_auto_start = true;
    let document = diff(vec![service_change(
        "change:v1:00000000",
        "manual-service",
        ChangeKind::Modified { before, after },
    )]);

    let output = render_terminal(&document);

    assert!(output.contains("Delayed automatic start configured:"));
    assert!(output.contains("Before: No"));
    assert!(output.contains("After:  Yes"));
    assert!(!output.contains("Start:"));
}

#[test]
fn inconclusive_service_absence_does_not_claim_removal() {
    let observed = windows_service("MaybePresent", Some("Maybe Present Service"), 3, false);
    let mut document = diff(vec![service_change(
        "change:v1:00000000",
        "service-a",
        ChangeKind::Inconclusive {
            before: Some(observed),
            after: None,
            reason: InconclusiveReason::CoverageIncomplete,
        },
    )]);
    document.warnings.push(DiffWarning {
        code: DiffWarningCode::CoverageIncomplete,
        collector_id: "windows.services".to_owned(),
        scope_id: "current_token.win32".to_owned(),
        before_status: Some(CollectorStatus::Partial),
        after_status: Some(CollectorStatus::Partial),
    });

    let output = render_terminal(&document);

    assert!(output.contains("Inconclusive (Windows service)"));
    assert!(
        output
            .to_ascii_lowercase()
            .contains("current-token service coverage was incomplete in the after snapshot")
    );
    assert!(output.contains("removal could not be confirmed"));
    assert!(output.contains("could not be confirmed"));
    assert!(output.contains("Windows services visible to the current token"));
    assert!(!output.contains("windows.services/current_token.win32"));
    assert!(!output.contains("Removed"));
}

#[test]
fn human_service_start_labels_keep_unknown_native_values_factual() {
    let document = diff(vec![
        service_change(
            "change:v1:00000000",
            "boot",
            ChangeKind::Added {
                after: windows_service("BootService", None, 0, false),
            },
        ),
        service_change(
            "change:v1:00000001",
            "system",
            ChangeKind::Added {
                after: windows_service("SystemService", None, 1, false),
            },
        ),
        service_change(
            "change:v1:00000002",
            "unknown",
            ChangeKind::Added {
                after: windows_service("FutureService", None, 99, false),
            },
        ),
    ]);

    let output = render_terminal(&document);

    assert!(output.contains("Boot start"));
    assert!(output.contains("System start"));
    assert!(output.contains("Unknown (raw start type 99)"));
}

#[test]
fn technical_service_output_preserves_every_field_and_known_absence() {
    let artifact = windows_service("ExampleUpdater", Some("Example Update Service"), 99, false);
    let document = diff(vec![service_change(
        "change:v1:00000000",
        "service-a",
        ChangeKind::Added { after: artifact },
    )]);

    let output = render_technical(&document, &before_fixture(), &after_fixture());

    for expected in [
        "service name: ExampleUpdater",
        r#"display name: "Example Update Service""#,
        "service type: 16",
        "start type: 99 (unknown native value)",
        "error control: 1 (normal)",
        r#"binary path: C:\Program Files\Example\service.exe --service"#,
        r#"account: "NT AUTHORITY\\LocalService""#,
        "dependencies (2):",
        "[0]: RpcSs",
        "[1]: +NetworkProvider",
        r#"load-order group: "ExampleGroup""#,
        "tag ID: 7",
        "delayed auto-start: false",
        r#"description: "Provides an example background service.""#,
    ] {
        assert!(
            output.contains(expected),
            "technical service output omitted {expected:?}\n{output}"
        );
    }

    let mut absent = windows_service("MinimalService", None, 3, false);
    let Artifact::WindowsService(service) = &mut absent else {
        unreachable!("helper must return a service")
    };
    service.account = None;
    service.dependencies.clear();
    service.load_order_group = None;
    service.tag_id = None;
    service.description = None;
    let absent_document = diff(vec![service_change(
        "change:v1:00000001",
        "service-b",
        ChangeKind::Added { after: absent },
    )]);
    let absent_output = render_technical(&absent_document, &before_fixture(), &after_fixture());

    assert!(absent_output.contains("display name: none"));
    assert!(absent_output.contains("account: none"));
    assert!(absent_output.contains("dependencies: none"));
    assert!(absent_output.contains("load-order group: none"));
    assert!(absent_output.contains("tag ID: none"));
    assert!(absent_output.contains("description: none"));
}

#[test]
fn service_evidence_cannot_inject_terminal_lines_or_bidi_controls() {
    let artifact = Artifact::WindowsService(WindowsService {
        service_name: "Evil\nService\u{202e}".to_owned(),
        display_name: Some("Display\r\nInjected\u{1b}[31m".to_owned()),
        service_type: 0x10,
        start_type: 2,
        error_control: 1,
        binary_path: "C:\\evil\tservice.exe\u{2028}".to_owned(),
        account: Some("Account\u{2066}".to_owned()),
        dependencies: vec!["Dep\nOne".to_owned()],
        load_order_group: Some("Group\u{202a}".to_owned()),
        tag_id: None,
        delayed_auto_start: false,
        description: Some("Description\rTwo".to_owned()),
    });
    let document = diff(vec![service_change(
        "change:v1:00000000",
        "service-a",
        ChangeKind::Added { after: artifact },
    )]);

    for output in [
        render_terminal(&document),
        render_technical(&document, &before_fixture(), &after_fixture()),
    ] {
        assert!(!output.contains('\u{1b}'));
        assert!(!output.contains('\r'));
        assert!(!output.contains('\t'));
        assert!(!output.contains('\u{202e}'));
        assert!(!output.contains('\u{2028}'));
        assert!(!output.contains('\u{2066}'));
        assert!(!output.contains('\u{202a}'));
        assert!(output.contains("Display\\r\\nInjected\\u{1b}[31m"));
        assert!(output.contains("Evil\\nService\\u{202e}"));
        assert!(output.contains("C:\\evil\\tservice.exe\\u{2028}"));
    }
}

#[test]
fn json_output_remains_the_pretty_serialized_diff_with_one_trailing_newline() {
    let artifact = run_entry(
        RegistryValueName::decoded("JsonContract"),
        decoded_string("C:\\Example\\json.exe"),
        'd',
    );
    let mut document = diff(vec![change(
        "change:v1:00000000",
        "current_user.shared.run",
        "json-contract",
        ChangeKind::Added { after: artifact },
    )]);
    document.warnings.push(DiffWarning {
        code: DiffWarningCode::CoverageIncomplete,
        collector_id: "windows.registry.startup".to_owned(),
        scope_id: "local_machine.registry64.run".to_owned(),
        before_status: Some(CollectorStatus::Complete),
        after_status: Some(CollectorStatus::PermissionDenied),
    });

    let mut bytes = Vec::new();
    write_json(&mut bytes, &document).expect("representative diff JSON must render");
    let output = String::from_utf8(bytes).expect("JSON output must be UTF-8");
    let expected = serde_json::to_string_pretty(&document).expect("Diff must serialize") + "\n";
    assert_eq!(output, expected);

    let value: serde_json::Value = serde_json::from_str(&output).expect("output must be JSON");
    assert_eq!(value["document_type"], "systemdiff.diff");
    assert_eq!(value["schema_version"], 1);
    assert_eq!(value["changes"][0]["change"]["change"], "added");
    assert_eq!(
        value["changes"][0]["change"]["after"]["evidence"]["value_name"]["value"],
        "JsonContract"
    );
    assert_eq!(value["warnings"][0]["code"], "coverage_incomplete");
}

#[test]
fn registry_demo_fixture_matches_the_published_human_transcript() {
    let before: Snapshot = serde_json::from_str(include_str!(
        "../../../fixtures/snapshots/registry-before-v1.json"
    ))
    .expect("the Registry before fixture must deserialize");
    let after: Snapshot = serde_json::from_str(include_str!(
        "../../../fixtures/snapshots/registry-after-v1.json"
    ))
    .expect("the Registry after fixture must deserialize");
    let document = diff_snapshots(&before, &after, DiffOptions::default())
        .expect("the Registry demo fixtures must diff");

    assert_eq!(
        render_terminal(&document),
        include_str!("../../../fixtures/reports/registry-added-human.txt")
    );
}
