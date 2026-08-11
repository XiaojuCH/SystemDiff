#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use systemdiff_core::{
    Artifact, ArtifactKey, CollectorStatus, Observation, Snapshot, SnapshotValidationError,
};

pub const DIFF_DOCUMENT_TYPE: &str = "systemdiff.diff";
pub const DIFF_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffOptions {
    pub include_unchanged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffDocument {
    pub document_type: String,
    pub schema_version: u32,
    pub before_captured_at: String,
    pub after_captured_at: String,
    pub changes: Vec<ArtifactChange>,
    pub warnings: Vec<DiffWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactChange {
    pub change_id: String,
    pub key: ArtifactKey,
    pub change: ChangeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum ChangeKind {
    Added {
        after: Artifact,
    },
    Removed {
        before: Artifact,
    },
    Modified {
        before: Artifact,
        after: Artifact,
    },
    Unchanged {
        artifact: Artifact,
    },
    Inconclusive {
        before: Option<Artifact>,
        after: Option<Artifact>,
        reason: InconclusiveReason,
    },
}

impl ChangeKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Added { .. } => "added",
            Self::Removed { .. } => "removed",
            Self::Modified { .. } => "modified",
            Self::Unchanged { .. } => "unchanged",
            Self::Inconclusive { .. } => "inconclusive",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InconclusiveReason {
    CoverageIncomplete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffWarning {
    pub code: DiffWarningCode,
    pub collector_id: String,
    pub scope_id: String,
    pub before_status: Option<CollectorStatus>,
    pub after_status: Option<CollectorStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffWarningCode {
    CoverageIncomplete,
}

pub fn diff_snapshots(
    before: &Snapshot,
    after: &Snapshot,
    options: DiffOptions,
) -> Result<DiffDocument, DiffError> {
    before.validate().map_err(DiffError::InvalidBefore)?;
    after.validate().map_err(DiffError::InvalidAfter)?;

    ensure_collector_versions_are_compatible(before, after)?;

    let before_observations = observation_index(before);
    let after_observations = observation_index(after);
    let keys: BTreeSet<_> = before_observations
        .keys()
        .chain(after_observations.keys())
        .cloned()
        .collect();

    let mut changes = Vec::new();
    for (ordinal, key) in keys.into_iter().enumerate() {
        let before_observation = before_observations.get(&key).copied();
        let after_observation = after_observations.get(&key).copied();
        let coverage_complete = before.scope_status(&key) == Some(CollectorStatus::Complete)
            && after.scope_status(&key) == Some(CollectorStatus::Complete);

        let change = classify_change(
            before_observation,
            after_observation,
            coverage_complete,
            options,
        );
        if let Some(change) = change {
            changes.push(ArtifactChange {
                change_id: change_id(ordinal),
                key,
                change,
            });
        }
    }

    Ok(DiffDocument {
        document_type: DIFF_DOCUMENT_TYPE.to_owned(),
        schema_version: DIFF_SCHEMA_VERSION,
        before_captured_at: before.captured_at.clone(),
        after_captured_at: after.captured_at.clone(),
        changes,
        warnings: coverage_warnings(before, after),
    })
}

fn ensure_collector_versions_are_compatible(
    before: &Snapshot,
    after: &Snapshot,
) -> Result<(), DiffError> {
    let before_versions: BTreeMap<_, _> = before
        .collectors
        .iter()
        .map(|run| (run.id.as_str(), run.version))
        .collect();
    for after_run in &after.collectors {
        if let Some(before_version) = before_versions.get(after_run.id.as_str())
            && *before_version != after_run.version
        {
            return Err(DiffError::IncompatibleCollectorVersion {
                collector_id: after_run.id.clone(),
                before_version: *before_version,
                after_version: after_run.version,
            });
        }
    }
    Ok(())
}

fn observation_index(snapshot: &Snapshot) -> BTreeMap<ArtifactKey, &Observation> {
    snapshot
        .observations
        .iter()
        .map(|observation| (observation.key(), observation))
        .collect()
}

fn classify_change(
    before: Option<&Observation>,
    after: Option<&Observation>,
    coverage_complete: bool,
    options: DiffOptions,
) -> Option<ChangeKind> {
    match (before, after) {
        (Some(before), Some(after)) if before.artifact == after.artifact => {
            options.include_unchanged.then(|| ChangeKind::Unchanged {
                artifact: before.artifact.clone(),
            })
        }
        (Some(before), Some(after)) => Some(ChangeKind::Modified {
            before: before.artifact.clone(),
            after: after.artifact.clone(),
        }),
        (None, Some(after)) if coverage_complete => Some(ChangeKind::Added {
            after: after.artifact.clone(),
        }),
        (Some(before), None) if coverage_complete => Some(ChangeKind::Removed {
            before: before.artifact.clone(),
        }),
        (before, after) => Some(ChangeKind::Inconclusive {
            before: before.map(|observation| observation.artifact.clone()),
            after: after.map(|observation| observation.artifact.clone()),
            reason: InconclusiveReason::CoverageIncomplete,
        }),
    }
}

fn coverage_warnings(before: &Snapshot, after: &Snapshot) -> Vec<DiffWarning> {
    let before_statuses = coverage_index(before);
    let after_statuses = coverage_index(after);
    let scopes: BTreeSet<_> = before_statuses
        .keys()
        .chain(after_statuses.keys())
        .cloned()
        .collect();

    scopes
        .into_iter()
        .filter_map(|(collector_id, scope_id)| {
            let before_status = before_statuses
                .get(&(collector_id.clone(), scope_id.clone()))
                .copied();
            let after_status = after_statuses
                .get(&(collector_id.clone(), scope_id.clone()))
                .copied();
            let complete = before_status == Some(CollectorStatus::Complete)
                && after_status == Some(CollectorStatus::Complete);
            (!complete).then_some(DiffWarning {
                code: DiffWarningCode::CoverageIncomplete,
                collector_id,
                scope_id,
                before_status,
                after_status,
            })
        })
        .collect()
}

fn coverage_index(snapshot: &Snapshot) -> BTreeMap<(String, String), CollectorStatus> {
    snapshot
        .collectors
        .iter()
        .flat_map(|run| {
            run.coverage.iter().map(move |coverage| {
                let effective_status = match run.status {
                    CollectorStatus::Complete | CollectorStatus::Partial => coverage.status,
                    status => status,
                };
                (
                    (run.id.clone(), coverage.scope_id.clone()),
                    effective_status,
                )
            })
        })
        .collect()
}

fn change_id(ordinal: usize) -> String {
    format!("change:v1:{ordinal:08}")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffError {
    InvalidBefore(SnapshotValidationError),
    InvalidAfter(SnapshotValidationError),
    IncompatibleCollectorVersion {
        collector_id: String,
        before_version: u32,
        after_version: u32,
    },
}

impl fmt::Display for DiffError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBefore(error) => write!(formatter, "invalid before snapshot: {error}"),
            Self::InvalidAfter(error) => write!(formatter, "invalid after snapshot: {error}"),
            Self::IncompatibleCollectorVersion {
                collector_id,
                before_version,
                after_version,
            } => write!(
                formatter,
                "incompatible collector version for {collector_id}: before={before_version}, after={after_version}"
            ),
        }
    }
}

impl Error for DiffError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidBefore(error) | Self::InvalidAfter(error) => Some(error),
            Self::IncompatibleCollectorVersion { .. } => None,
        }
    }
}
