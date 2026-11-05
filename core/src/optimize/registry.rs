//! Stable rule directory; evaluation belongs to the fixed-input evaluator.
use crate::{
    config_dto::{Item, Kind},
    optimize_dto::*,
};
pub(super) fn catalog() -> Vec<RuleDefinition> {
    Rule::ALL
        .into_iter()
        .map(|rule| RuleDefinition {
            rule: rule.id().into(),
            version: RuleParameters::default().version,
            kinds: rule.kinds().to_vec(),
            basis: rule.basis().into(),
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(super) enum Rule {
    MissingInstruction,
    FileSize,
    SkillFormat,
    DescriptionStandard,
    DescriptionSize,
    BodyTokens,
    LocalReference,
    ExactInstructionBlocks,
    DeclaredCopyDrift,
    HookTarget,
}
impl Rule {
    /// Only implemented checks are exposed.
    pub(super) fn dependencies(self) -> Dependencies {
        let (evidence, method) = match self {
            Self::MissingInstruction => (Evidence::Existence, "authorized-existence-v1"),
            Self::FileSize => (Evidence::FileMeasurement, "file-bytes-v1"),
            Self::SkillFormat | Self::DescriptionStandard | Self::DescriptionSize => {
                (Evidence::SkillMetadata, "skill-frontmatter-v1")
            }
            Self::BodyTokens => (
                Evidence::BodyMeasurement,
                "tiktoken-rs-0.12.0/o200k_base/ordinary-v1",
            ),
            Self::LocalReference => (Evidence::References, "authorized-local-reference-v1"),
            Self::ExactInstructionBlocks => (Evidence::Blocks, "exact-instruction-block-v1"),
            Self::DeclaredCopyDrift => (Evidence::Relations, "raw-utf8/identity-v1"),
            Self::HookTarget => (Evidence::HostTargets, "trusted-enabled-hook-target-v1"),
        };
        Dependencies {
            semantics_version: if matches!(
                evidence,
                Evidence::FileMeasurement | Evidence::References
            ) {
                2
            } else {
                1
            },
            evidence,
            method,
            max_key_bytes: super::cache::MAX_ENTRY_BYTES,
            max_dependency_rows: 1024,
            max_candidate_rows: 4096,
        }
    }
    pub(super) const ALL: [Self; 10] = [
        Self::MissingInstruction,
        Self::FileSize,
        Self::SkillFormat,
        Self::DescriptionStandard,
        Self::DescriptionSize,
        Self::BodyTokens,
        Self::LocalReference,
        Self::ExactInstructionBlocks,
        Self::DeclaredCopyDrift,
        Self::HookTarget,
    ];
    pub(super) fn id(self) -> &'static str {
        match self {
            Self::MissingInstruction => "missingInstruction",
            Self::FileSize => "fileSize",
            Self::SkillFormat => "skillFormat",
            Self::DescriptionStandard => "descriptionStandard",
            Self::DescriptionSize => "descriptionSize",
            Self::BodyTokens => "bodyTokens",
            Self::LocalReference => "localReference",
            Self::ExactInstructionBlocks => "exactInstructionBlocks",
            Self::DeclaredCopyDrift => "declaredCopyDrift",
            Self::HookTarget => "hookTarget",
        }
    }
    fn kinds(self) -> &'static [Kind] {
        match self {
            Self::MissingInstruction | Self::FileSize => &[Kind::Rule],
            Self::SkillFormat
            | Self::DescriptionStandard
            | Self::DescriptionSize
            | Self::BodyTokens => &[Kind::Skill],
            Self::LocalReference | Self::ExactInstructionBlocks | Self::DeclaredCopyDrift => {
                &[Kind::Rule, Kind::Skill]
            }
            Self::HookTarget => &[Kind::Hook],
        }
    }
    fn basis(self) -> &'static str {
        match self {
            Self::MissingInstruction => "authorizedExistenceCheck",
            Self::FileSize | Self::DescriptionSize => "productReminder",
            Self::SkillFormat | Self::DescriptionStandard => "agentSkillsSpecification",
            Self::BodyTokens => "referenceEncodingOnly",
            Self::LocalReference => "authorizedMarkdownResources",
            Self::ExactInstructionBlocks => "staticExactBlocks",
            Self::DeclaredCopyDrift => "explicitSourceCopyRelation",
            Self::HookTarget => "effectiveTrustedEnabledHookRegistry",
        }
    }
    pub(super) fn applies(self, kind: &Kind) -> bool {
        self.kinds().contains(kind)
    }
}

#[derive(Clone, Copy, serde::Serialize)]
pub(super) enum Evidence {
    Existence,
    FileMeasurement,
    SkillMetadata,
    BodyMeasurement,
    References,
    Blocks,
    Relations,
    HostTargets,
}
impl Evidence {
    /// Each check consumes its own observation. A failed content read does not
    /// invalidate authorized filesystem size, and cannot make an unsupported
    /// algorithm appear to be waiting for more content.
    pub(super) fn accepts(self, item: &Item) -> bool {
        match self {
            Self::Existence => matches!(item.measurement_status.as_str(), "complete" | "missing"),
            Self::FileMeasurement => {
                item.measurement_status == "complete"
                    || (item.measurement_status == "unavailable"
                        && item.bytes_source.as_deref() == Some("filesystemMetadata")
                        && item.bytes.is_some())
            }
            Self::SkillMetadata
            | Self::BodyMeasurement
            | Self::References
            | Self::Blocks
            | Self::Relations => item.measurement_status == "complete",
            Self::HostTargets => true,
        }
    }
}
#[derive(serde::Serialize)]
pub(super) struct Dependencies {
    pub semantics_version: u32,
    pub evidence: Evidence,
    pub method: &'static str,
    /// Reuse work is bounded separately from the collector's analysis budgets.
    pub max_key_bytes: usize,
    pub max_dependency_rows: usize,
    pub max_candidate_rows: usize,
}
