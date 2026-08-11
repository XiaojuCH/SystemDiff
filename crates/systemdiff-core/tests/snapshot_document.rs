use systemdiff_core::{SnapshotDocumentError, decode_snapshot_document};

#[test]
fn supported_snapshot_v1_is_routed_to_the_current_wire_type() {
    let snapshot =
        decode_snapshot_document(include_bytes!("../../../fixtures/snapshots/before-v1.json"))
            .expect("the v1 fixture must be routed and decoded");

    assert_eq!(snapshot.document_type, "systemdiff.snapshot");
    assert_eq!(snapshot.schema_version, 1);
}

#[test]
fn unknown_document_type_is_rejected_before_snapshot_body_construction() {
    let error =
        decode_snapshot_document(br#"{"document_type":"systemdiff.diff","schema_version":1}"#)
            .expect_err("another document family must not route to Snapshot v1");

    assert!(matches!(
        error,
        SnapshotDocumentError::UnexpectedDocumentType { ref found }
            if found == "systemdiff.diff"
    ));
}

#[test]
fn unsupported_schema_is_rejected_before_snapshot_body_construction() {
    let error =
        decode_snapshot_document(br#"{"document_type":"systemdiff.snapshot","schema_version":2}"#)
            .expect_err("an unsupported Snapshot schema must not use the v1 wire type");

    assert!(matches!(
        error,
        SnapshotDocumentError::UnsupportedSchemaVersion { found: 2 }
    ));
}

#[test]
fn supported_header_with_incomplete_body_reports_a_v1_body_error() {
    let error =
        decode_snapshot_document(br#"{"document_type":"systemdiff.snapshot","schema_version":1}"#)
            .expect_err("the supported route must still require a complete v1 body");

    assert!(matches!(error, SnapshotDocumentError::InvalidSnapshotV1(_)));
}

#[test]
fn malformed_or_missing_header_is_rejected_as_a_header_error() {
    for input in [
        br#"{"document_type":"systemdiff.snapshot"}"#.as_slice(),
        br#"{"document_type": "systemdiff.snapshot""#.as_slice(),
    ] {
        let error = decode_snapshot_document(input)
            .expect_err("a malformed document header must be rejected");
        assert!(matches!(error, SnapshotDocumentError::InvalidHeader(_)));
    }
}
