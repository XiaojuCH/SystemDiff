#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use systemdiff_diff::ArtifactChange;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    pub finding_id: String,
    pub rule_id: String,
    pub rule_version: u32,
    pub change_id: String,
    pub classification: Classification,
    pub confidence: Confidence,
    pub explanation_key: String,
    pub explanation_parameters: BTreeMap<String, String>,
    pub reason_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    Informational,
    Expected,
    Noteworthy,
    Suspicious,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

pub trait Rule {
    fn id(&self) -> &'static str;
    fn version(&self) -> u32;
    fn evaluate(&self, change: &ArtifactChange) -> Vec<Finding>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use systemdiff_core::{
        ArtifactKey, RegistryDecodedValue, RegistryHive, RegistryStartupEntry,
        RegistryValueDecoding, RegistryView,
    };
    use systemdiff_diff::ChangeKind;

    struct ExampleRule;

    impl Rule for ExampleRule {
        fn id(&self) -> &'static str {
            "example.registry-startup-added"
        }

        fn version(&self) -> u32 {
            1
        }

        fn evaluate(&self, change: &ArtifactChange) -> Vec<Finding> {
            if !matches!(&change.change, ChangeKind::Added { .. }) {
                return Vec::new();
            }

            vec![Finding {
                finding_id: format!("finding:{}:{}", self.id(), change.change_id),
                rule_id: self.id().to_owned(),
                rule_version: self.version(),
                change_id: change.change_id.clone(),
                classification: Classification::Noteworthy,
                confidence: Confidence::Medium,
                explanation_key: "finding.registry_startup.added".to_owned(),
                explanation_parameters: BTreeMap::new(),
                reason_ids: vec!["registry_startup_added".to_owned()],
            }]
        }
    }

    #[test]
    fn finding_references_change_without_replacing_evidence() {
        let change = ArtifactChange {
            change_id: "change:v1:synthetic".to_owned(),
            key: ArtifactKey {
                collector_id: "windows.registry.startup".to_owned(),
                scope_id: "current_user.shared".to_owned(),
                artifact_kind: "registry_startup".to_owned(),
                canonical_id: "synthetic".to_owned(),
            },
            change: ChangeKind::Added {
                after: systemdiff_core::Artifact::RegistryStartup(RegistryStartupEntry {
                    hive: RegistryHive::CurrentUser,
                    registry_view: RegistryView::Shared,
                    key_path: "Software\\Example".to_owned(),
                    value_name: "Synthetic".to_owned(),
                    value_type: 1,
                    content_sha256:
                        "04c9e304d22dd63d40474ebbb8ca4cb383a68b755876aced0b32ad9e54ec82bf"
                            .to_owned(),
                    decoding: RegistryValueDecoding::Decoded {
                        value: RegistryDecodedValue::String {
                            value: "C:\\Example.exe".to_owned(),
                        },
                    },
                    raw_evidence: None,
                }),
            },
        };

        let findings = ExampleRule.evaluate(&change);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].change_id, change.change_id);
        assert_eq!(findings[0].rule_id, ExampleRule.id());
    }
}
