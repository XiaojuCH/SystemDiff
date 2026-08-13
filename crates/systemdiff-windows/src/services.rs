use std::collections::BTreeMap;
use systemdiff_core::{
    Artifact, CollectionContext, CollectionOutcome, Collector, CollectorDescriptor, CollectorRun,
    CollectorStatus, Diagnostic, Observation, PrivilegeRequirement, ScopeCoverage,
    WINDOWS_SERVICES_COLLECTOR_ID, WINDOWS_SERVICES_COLLECTOR_VERSION, WINDOWS_SERVICES_SCOPE_ID,
    WindowsService, windows_service_identity,
};

pub const SERVICES_COLLECTOR_ID: &str = WINDOWS_SERVICES_COLLECTOR_ID;
pub const SERVICES_COLLECTOR_VERSION: u32 = WINDOWS_SERVICES_COLLECTOR_VERSION;
pub const MAX_SERVICES_PER_SCOPE: usize = 4_096;
pub const MAX_SERVICE_EVIDENCE_BYTES: usize = 32 * 1024;
pub const MAX_SERVICES_COLLECTOR_EVIDENCE_BYTES: usize = 16 * 1024 * 1024;
const MAX_CONFIG_READS: usize = 3;

pub fn descriptor() -> CollectorDescriptor {
    CollectorDescriptor {
        id: SERVICES_COLLECTOR_ID.to_owned(),
        version: SERVICES_COLLECTOR_VERSION,
        description: "Win32 service configuration visible to the current token, excluding drivers."
            .to_owned(),
        privilege: PrivilegeRequirement::ObjectAclDependent,
    }
}

pub struct WindowsServicesCollector;

impl Collector for WindowsServicesCollector {
    fn descriptor(&self) -> CollectorDescriptor {
        descriptor()
    }

    fn collect(&self, context: &CollectionContext) -> CollectionOutcome {
        let mut source = crate::win32_services::Win32ServiceSource::new();
        collect_with_source(&mut source, context)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawServiceName {
    pub name_utf16: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RawServiceConfig {
    pub service_name_utf16: Vec<u16>,
    pub display_name_utf16: Option<Vec<u16>>,
    pub service_type: u32,
    pub start_type: u32,
    pub error_control: u32,
    pub binary_path_utf16: Vec<u16>,
    pub account_utf16: Option<Vec<u16>>,
    pub dependencies_utf16: Vec<Vec<u16>>,
    pub load_order_group_utf16: Option<Vec<u16>>,
    pub tag_id: Option<u32>,
    pub delayed_auto_start: bool,
    pub description_utf16: Option<Vec<u16>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServiceEnumeration {
    pub names: Vec<RawServiceName>,
    pub issues: Vec<ServiceFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ServiceFailureKind {
    AccessDenied,
    InvalidData,
    ResourceLimit,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ServiceFailure {
    pub kind: ServiceFailureKind,
    pub code: &'static str,
    pub message: &'static str,
    pub stage: &'static str,
    pub native_code: Option<i64>,
}

pub(crate) trait ServiceDataSource {
    fn enumerate(&mut self) -> Result<ServiceEnumeration, ServiceFailure>;
    fn read_config_once(
        &mut self,
        service_name_utf16: &[u16],
    ) -> Result<RawServiceConfig, ServiceFailure>;
}

fn collect_with_source<S: ServiceDataSource>(
    source: &mut S,
    _context: &CollectionContext,
) -> CollectionOutcome {
    collect_with_source_and_budget(source, MAX_SERVICES_COLLECTOR_EVIDENCE_BYTES)
}

fn collect_with_source_and_budget<S: ServiceDataSource>(
    source: &mut S,
    collector_budget: usize,
) -> CollectionOutcome {
    let enumeration = match source.enumerate() {
        Ok(enumeration) => enumeration,
        Err(failure) => return failed_outcome(failure),
    };

    let mut diagnostics = vec![Diagnostic {
        code: "service_visibility_best_effort".to_owned(),
        message:
            "Windows can silently omit services that the current token cannot query for status."
                .to_owned(),
        stage: Some("enumerate".to_owned()),
        native_code: None,
        scope_id: Some(WINDOWS_SERVICES_SCOPE_ID.to_owned()),
    }];
    diagnostics.extend(enumeration.issues.into_iter().map(diagnostic));
    let mut by_identity: BTreeMap<String, Vec<Vec<u16>>> = BTreeMap::new();
    for name in enumeration.names {
        by_identity
            .entry(windows_service_identity(&name.name_utf16))
            .or_default()
            .push(name.name_utf16);
    }

    let mut observations = Vec::new();
    let mut retained_bytes = 0_usize;
    for (canonical_id, names) in by_identity {
        if names.len() != 1 {
            diagnostics.push(diagnostic_for_identity(
                ServiceFailure {
                    kind: ServiceFailureKind::InvalidData,
                    code: "service_identity_collision",
                    message: "Multiple enumerated services produced one Collector identity.",
                    stage: "normalize",
                    native_code: None,
                },
                &canonical_id,
            ));
            continue;
        }
        let name = &names[0];
        let raw = match read_stable_config(source, name) {
            Ok(raw) => raw,
            Err(failure) => {
                diagnostics.push(diagnostic_for_identity(failure, &canonical_id));
                continue;
            }
        };
        let service = match normalize_service(raw) {
            Ok(service) => service,
            Err(failure) => {
                diagnostics.push(diagnostic_for_identity(failure, &canonical_id));
                continue;
            }
        };
        let evidence_bytes = service_evidence_bytes(&service).unwrap_or(usize::MAX);
        let next = retained_bytes.saturating_add(evidence_bytes);
        if evidence_bytes > MAX_SERVICE_EVIDENCE_BYTES || next > collector_budget {
            diagnostics.push(diagnostic_for_identity(
                ServiceFailure {
                    kind: ServiceFailureKind::ResourceLimit,
                    code: "service_resource_limit",
                    message: "Service evidence exceeded a SystemDiff capture budget.",
                    stage: "normalize",
                    native_code: None,
                },
                &canonical_id,
            ));
            continue;
        }
        retained_bytes = next;
        observations.push(Observation {
            collector_id: SERVICES_COLLECTOR_ID.to_owned(),
            collector_version: SERVICES_COLLECTOR_VERSION,
            scope_id: WINDOWS_SERVICES_SCOPE_ID.to_owned(),
            canonical_id,
            artifact: Artifact::WindowsService(service),
        });
    }
    observations.sort_by_key(|observation| observation.key());
    diagnostics.sort_by(|left, right| {
        (&left.code, &left.stage, left.native_code, &left.message).cmp(&(
            &right.code,
            &right.stage,
            right.native_code,
            &right.message,
        ))
    });
    diagnostics.dedup();

    CollectionOutcome {
        run: CollectorRun {
            id: SERVICES_COLLECTOR_ID.to_owned(),
            version: SERVICES_COLLECTOR_VERSION,
            status: CollectorStatus::Partial,
            coverage: vec![ScopeCoverage {
                scope_id: WINDOWS_SERVICES_SCOPE_ID.to_owned(),
                status: CollectorStatus::Partial,
            }],
            diagnostics,
        },
        observations,
    }
}

fn read_stable_config<S: ServiceDataSource>(
    source: &mut S,
    name: &[u16],
) -> Result<RawServiceConfig, ServiceFailure> {
    let mut previous = source.read_config_once(name)?;
    for _ in 1..MAX_CONFIG_READS {
        let current = source.read_config_once(name)?;
        if current == previous {
            return Ok(current);
        }
        previous = current;
    }
    Err(ServiceFailure {
        kind: ServiceFailureKind::Other,
        code: "service_changed_during_scan",
        message: "A service changed during bounded configuration reads.",
        stage: "query_config",
        native_code: None,
    })
}

fn normalize_service(raw: RawServiceConfig) -> Result<WindowsService, ServiceFailure> {
    let strict = |units: Vec<u16>| String::from_utf16(&units).map_err(|_| invalid_utf16_failure());
    let optional = |units: Option<Vec<u16>>| {
        units
            .map(strict)
            .transpose()
            .map(|value| value.filter(|text| !text.is_empty()))
    };
    let service_name = strict(raw.service_name_utf16)?;
    if service_name.is_empty() {
        return Err(invalid_data_failure("A service name was empty."));
    }
    let win32_base = raw.service_type & 0x30;
    if !matches!(win32_base, 0x10 | 0x20) || raw.service_type & 0x0f != 0 {
        return Err(invalid_data_failure(
            "A non-Win32 or driver-only service was returned by the Win32 filter.",
        ));
    }
    Ok(WindowsService {
        service_name,
        display_name: optional(raw.display_name_utf16)?,
        service_type: raw.service_type,
        start_type: raw.start_type,
        error_control: raw.error_control,
        binary_path: strict(raw.binary_path_utf16)?,
        account: optional(raw.account_utf16)?,
        dependencies: raw
            .dependencies_utf16
            .into_iter()
            .map(strict)
            .collect::<Result<_, _>>()?,
        load_order_group: optional(raw.load_order_group_utf16)?,
        tag_id: raw.tag_id,
        delayed_auto_start: raw.delayed_auto_start,
        description: optional(raw.description_utf16)?,
    })
}

fn service_evidence_bytes(service: &WindowsService) -> Option<usize> {
    let mut units = service.service_name.encode_utf16().count();
    for value in [
        service.display_name.as_deref(),
        Some(service.binary_path.as_str()),
        service.account.as_deref(),
        service.load_order_group.as_deref(),
        service.description.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        units = units.checked_add(value.encode_utf16().count())?;
    }
    for dependency in &service.dependencies {
        units = units.checked_add(dependency.encode_utf16().count())?;
    }
    units.checked_mul(2)
}

fn failed_outcome(failure: ServiceFailure) -> CollectionOutcome {
    let status = if failure.kind == ServiceFailureKind::AccessDenied {
        CollectorStatus::PermissionDenied
    } else {
        CollectorStatus::Failed
    };
    CollectionOutcome {
        run: CollectorRun {
            id: SERVICES_COLLECTOR_ID.to_owned(),
            version: SERVICES_COLLECTOR_VERSION,
            status,
            coverage: vec![ScopeCoverage {
                scope_id: WINDOWS_SERVICES_SCOPE_ID.to_owned(),
                status,
            }],
            diagnostics: vec![diagnostic(failure)],
        },
        observations: Vec::new(),
    }
}

fn diagnostic(failure: ServiceFailure) -> Diagnostic {
    Diagnostic {
        code: failure.code.to_owned(),
        message: failure.message.to_owned(),
        stage: Some(failure.stage.to_owned()),
        native_code: failure.native_code,
        scope_id: Some(WINDOWS_SERVICES_SCOPE_ID.to_owned()),
    }
}

fn diagnostic_for_identity(failure: ServiceFailure, canonical_id: &str) -> Diagnostic {
    let mut diagnostic = diagnostic(failure);
    diagnostic.message = format!("{} Artifact identity: {canonical_id}.", diagnostic.message);
    diagnostic
}

fn invalid_utf16_failure() -> ServiceFailure {
    invalid_data_failure("Service configuration contained malformed UTF-16.")
}

fn invalid_data_failure(message: &'static str) -> ServiceFailure {
    ServiceFailure {
        kind: ServiceFailureKind::InvalidData,
        code: "service_invalid_data",
        message,
        stage: "normalize",
        native_code: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use systemdiff_core::PrivilegeState;

    #[derive(Clone)]
    struct FakeSource {
        names: Result<ServiceEnumeration, ServiceFailure>,
        reads: BTreeMap<Vec<u16>, Vec<Result<RawServiceConfig, ServiceFailure>>>,
    }

    impl ServiceDataSource for FakeSource {
        fn enumerate(&mut self) -> Result<ServiceEnumeration, ServiceFailure> {
            self.names.clone()
        }

        fn read_config_once(
            &mut self,
            service_name_utf16: &[u16],
        ) -> Result<RawServiceConfig, ServiceFailure> {
            self.reads
                .get_mut(service_name_utf16)
                .and_then(|reads| (!reads.is_empty()).then(|| reads.remove(0)))
                .unwrap_or_else(|| Err(invalid_data_failure("Missing fake read.")))
        }
    }

    fn utf16(value: &str) -> Vec<u16> {
        value.encode_utf16().collect()
    }

    fn config(name: &str) -> RawServiceConfig {
        RawServiceConfig {
            service_name_utf16: utf16(name),
            display_name_utf16: Some(utf16("Example Service")),
            service_type: 0x10,
            start_type: 3,
            error_control: 1,
            binary_path_utf16: utf16(r#"%SystemRoot%\Example.exe --service"#),
            account_utf16: Some(utf16("LocalSystem")),
            dependencies_utf16: vec![utf16("RpcSs"), utf16("+NetworkProvider")],
            load_order_group_utf16: None,
            tag_id: None,
            delayed_auto_start: false,
            description_utf16: None,
        }
    }

    fn source_with(name: &str, reads: Vec<Result<RawServiceConfig, ServiceFailure>>) -> FakeSource {
        FakeSource {
            names: Ok(ServiceEnumeration {
                names: vec![RawServiceName {
                    name_utf16: utf16(name),
                }],
                issues: Vec::new(),
            }),
            reads: BTreeMap::from([(utf16(name), reads)]),
        }
    }

    fn context() -> CollectionContext {
        CollectionContext {
            privilege: PrivilegeState::StandardUser,
        }
    }

    #[test]
    fn successful_collection_is_best_effort_partial_and_preserves_raw_evidence() {
        let raw = config("ExampleService_1a2b3");
        let mut source = source_with("ExampleService_1a2b3", vec![Ok(raw.clone()), Ok(raw)]);
        let outcome = collect_with_source(&mut source, &context());
        assert_eq!(outcome.run.status, CollectorStatus::Partial);
        assert_eq!(outcome.observations.len(), 1);
        let Artifact::WindowsService(service) = &outcome.observations[0].artifact else {
            panic!("expected service")
        };
        assert_eq!(service.service_name, "ExampleService_1a2b3");
        assert_eq!(service.binary_path, r#"%SystemRoot%\Example.exe --service"#);
        assert_eq!(service.dependencies, ["RpcSs", "+NetworkProvider"]);
        assert!(
            outcome
                .run
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "service_visibility_best_effort" })
        );
    }

    #[test]
    fn query_failure_omits_atomic_observation_and_preserves_siblings() {
        let first = config("First");
        let failure = ServiceFailure {
            kind: ServiceFailureKind::AccessDenied,
            code: "service_access_denied",
            message: "Service configuration access was denied.",
            stage: "open_service",
            native_code: Some(5),
        };
        let mut source = FakeSource {
            names: Ok(ServiceEnumeration {
                names: vec![
                    RawServiceName {
                        name_utf16: utf16("Second"),
                    },
                    RawServiceName {
                        name_utf16: utf16("First"),
                    },
                ],
                issues: Vec::new(),
            }),
            reads: BTreeMap::from([
                (utf16("First"), vec![Ok(first.clone()), Ok(first)]),
                (utf16("Second"), vec![Err(failure)]),
            ]),
        };
        let outcome = collect_with_source(&mut source, &context());
        assert_eq!(outcome.observations.len(), 1);
        assert!(outcome.run.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == "service_access_denied"
                && diagnostic
                    .message
                    .contains(&windows_service_identity(&utf16("Second")))
                && !diagnostic.message.contains("Second")
        }));
    }

    #[test]
    fn per_service_failures_retain_distinct_private_identities() {
        let failure = ServiceFailure {
            kind: ServiceFailureKind::AccessDenied,
            code: "service_access_denied",
            message: "Service configuration access was denied.",
            stage: "open_service",
            native_code: Some(5),
        };
        let mut source = FakeSource {
            names: Ok(ServiceEnumeration {
                names: vec![
                    RawServiceName {
                        name_utf16: utf16("PrivateNameOne"),
                    },
                    RawServiceName {
                        name_utf16: utf16("PrivateNameTwo"),
                    },
                ],
                issues: Vec::new(),
            }),
            reads: BTreeMap::from([
                (utf16("PrivateNameOne"), vec![Err(failure.clone())]),
                (utf16("PrivateNameTwo"), vec![Err(failure)]),
            ]),
        };

        let outcome = collect_with_source(&mut source, &context());
        let failures = outcome
            .run
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "service_access_denied")
            .collect::<Vec<_>>();

        assert_eq!(failures.len(), 2);
        assert_ne!(failures[0].message, failures[1].message);
        assert!(
            failures
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("PrivateName"))
        );
    }

    #[test]
    fn changing_configuration_has_three_read_bound_and_is_omitted() {
        let mut first = config("Changing");
        let mut second = first.clone();
        let mut third = first.clone();
        first.start_type = 2;
        second.start_type = 3;
        third.start_type = 4;
        let mut source = source_with("Changing", vec![Ok(first), Ok(second), Ok(third)]);
        let outcome = collect_with_source(&mut source, &context());
        assert!(outcome.observations.is_empty());
        assert!(
            outcome
                .run
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "service_changed_during_scan" })
        );
    }

    #[test]
    fn malformed_utf16_and_driver_types_are_omitted_without_lossy_text() {
        let mut malformed = config("Malformed");
        malformed.description_utf16 = Some(vec![0xd800]);
        let mut source = source_with("Malformed", vec![Ok(malformed.clone()), Ok(malformed)]);
        let outcome = collect_with_source(&mut source, &context());
        assert!(outcome.observations.is_empty());
        assert!(
            outcome
                .run
                .diagnostics
                .iter()
                .any(|diagnostic| { diagnostic.code == "service_invalid_data" })
        );

        let mut driver = config("Driver");
        driver.service_type = 1;
        let mut source = source_with("Driver", vec![Ok(driver.clone()), Ok(driver)]);
        assert!(
            collect_with_source(&mut source, &context())
                .observations
                .is_empty()
        );
    }

    #[test]
    fn aggregate_budget_selection_is_deterministic_by_identity() {
        let one = config("One");
        let two = config("Two");
        let make = |reversed: bool| {
            let mut names = vec![
                RawServiceName {
                    name_utf16: utf16("One"),
                },
                RawServiceName {
                    name_utf16: utf16("Two"),
                },
            ];
            if reversed {
                names.reverse();
            }
            FakeSource {
                names: Ok(ServiceEnumeration {
                    names,
                    issues: Vec::new(),
                }),
                reads: BTreeMap::from([
                    (utf16("One"), vec![Ok(one.clone()), Ok(one.clone())]),
                    (utf16("Two"), vec![Ok(two.clone()), Ok(two.clone())]),
                ]),
            }
        };
        let one_budget = service_evidence_bytes(&normalize_service(one.clone()).unwrap()).unwrap();
        let mut forward = make(false);
        let mut reverse = make(true);
        let left = collect_with_source_and_budget(&mut forward, one_budget);
        let right = collect_with_source_and_budget(&mut reverse, one_budget);
        assert_eq!(left, right);
        assert_eq!(left.observations.len(), 1);
    }
}
