#![deny(unsafe_op_in_unsafe_fn)]

use systemdiff_core::{CollectorDescriptor, PrivilegeRequirement};

pub const REGISTRY_STARTUP_COLLECTOR_ID: &str = "windows.registry.startup";
pub const SERVICES_COLLECTOR_ID: &str = "windows.services";
pub const SCHEDULED_TASKS_COLLECTOR_ID: &str = "windows.scheduled_tasks";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectorPlan {
    pub descriptor: CollectorDescriptor,
    pub implementation: ImplementationStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImplementationStatus {
    Planned,
}

pub fn mvp_collector_plans() -> Vec<CollectorPlan> {
    vec![
        CollectorPlan {
            descriptor: CollectorDescriptor {
                id: REGISTRY_STARTUP_COLLECTOR_ID.to_owned(),
                version: 1,
                description: "Documented Run and RunOnce registry startup locations.".to_owned(),
                privilege: PrivilegeRequirement::StandardUserPartial,
            },
            implementation: ImplementationStatus::Planned,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn planned_collector_ids_are_unique_and_versioned() {
        let plans = mvp_collector_plans();
        let ids: BTreeSet<_> = plans
            .iter()
            .map(|plan| plan.descriptor.id.as_str())
            .collect();

        assert_eq!(plans.len(), ids.len());
        assert!(plans.iter().all(|plan| plan.descriptor.version > 0));
        assert!(
            plans
                .iter()
                .all(|plan| plan.implementation == ImplementationStatus::Planned)
        );
    }
}
