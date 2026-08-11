#![deny(unsafe_op_in_unsafe_fn)]

mod platform;
mod registry;
mod win32;

use std::error::Error;
use std::fmt;
use systemdiff_core::{
    CollectionContext, Collector, RedactionMetadata, RedactionStatus, Snapshot, SnapshotMetadata,
    SnapshotValidationError, assemble_snapshot,
};

pub use registry::{
    MAX_REGISTRY_COLLECTOR_EVIDENCE_BYTES, MAX_REGISTRY_VALUE_DATA_BYTES,
    MAX_REGISTRY_VALUES_PER_SCOPE, REGISTRY_STARTUP_COLLECTOR_ID,
    REGISTRY_STARTUP_COLLECTOR_VERSION, RegistryStartupCollector,
};
use systemdiff_core::{CollectorDescriptor, PrivilegeRequirement};

pub const SERVICES_COLLECTOR_ID: &str = "windows.services";
pub const SCHEDULED_TASKS_COLLECTOR_ID: &str = "windows.scheduled_tasks";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectorPlan {
    pub descriptor: CollectorDescriptor,
    pub implementation: ImplementationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplementationStatus {
    Implemented,
    Planned,
}

pub fn mvp_collector_plans() -> Vec<CollectorPlan> {
    vec![
        CollectorPlan {
            descriptor: registry::descriptor(),
            implementation: ImplementationStatus::Implemented,
        },
        CollectorPlan {
            descriptor: CollectorDescriptor {
                id: SERVICES_COLLECTOR_ID.to_owned(),
                version: 1,
                description: "Win32 service configuration, excluding drivers.".to_owned(),
                privilege: PrivilegeRequirement::ObjectAclDependent,
            },
            implementation: ImplementationStatus::Planned,
        },
        CollectorPlan {
            descriptor: CollectorDescriptor {
                id: SCHEDULED_TASKS_COLLECTOR_ID.to_owned(),
                version: 1,
                description: "Task Scheduler 2.0 configuration visible to the current token."
                    .to_owned(),
                privilege: PrivilegeRequirement::ObjectAclDependent,
            },
            implementation: ImplementationStatus::Planned,
        },
    ]
}

pub fn capture_snapshot(
    captured_at: String,
    systemdiff_version: String,
) -> Result<Snapshot, CaptureError> {
    if !platform::is_supported() {
        return Err(CaptureError::UnsupportedPlatform);
    }
    let host = platform::host_metadata();
    let privilege = platform::privilege_state();
    let context = CollectionContext { privilege };
    let outcome = RegistryStartupCollector.collect(&context);
    assemble_snapshot(
        SnapshotMetadata {
            systemdiff_version,
            captured_at,
            host,
            privilege,
            redaction: RedactionMetadata {
                status: RedactionStatus::Unredacted,
                policy: None,
            },
        },
        vec![outcome],
    )
    .map_err(CaptureError::InvalidSnapshot)
}

#[derive(Debug)]
pub enum CaptureError {
    UnsupportedPlatform,
    InvalidSnapshot(SnapshotValidationError),
}

impl fmt::Display for CaptureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter.write_str(
                "snapshot collection requires Windows 10 version 1709 / Windows Server 2016 version 1709 or later",
            ),
            Self::InvalidSnapshot(error) => write!(formatter, "collected Snapshot is invalid: {error}"),
        }
    }
}

impl Error for CaptureError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::UnsupportedPlatform => None,
            Self::InvalidSnapshot(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn collector_ids_are_unique_versioned_and_registry_is_implemented() {
        let plans = mvp_collector_plans();
        let ids: BTreeSet<_> = plans
            .iter()
            .map(|plan| plan.descriptor.id.as_str())
            .collect();

        assert_eq!(plans.len(), ids.len());
        assert!(plans.iter().all(|plan| plan.descriptor.version > 0));
        assert_eq!(plans[0].implementation, ImplementationStatus::Implemented);
        assert!(
            plans[1..]
                .iter()
                .all(|plan| plan.implementation == ImplementationStatus::Planned)
        );
    }
}
