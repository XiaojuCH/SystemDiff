use systemdiff_core::{
    Artifact, ArtifactKey, CollectorStatus, RegistryDecodedValue, RegistryHive,
    RegistryStartupEntry, RegistryStartupKind, RegistryValueDecoding, RegistryValueName,
    RegistryView, RunOncePrefixSemantics, Snapshot,
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
