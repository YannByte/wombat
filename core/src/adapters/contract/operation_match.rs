//! Body-free matching observations. Digests establish equal requests, never equal effects.
use super::*;
pub const OPERATION_MATCH_VERSION: u32 = 2;
pub const MATCH_STRING_BYTES: usize = 64 * 1024;
pub const MATCH_TARGET_LIMIT: usize = 4096;
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchGap {
    MissingParameters,
    UnsupportedParameters,
    MissingReceiver,
    MissingHistoricalCwd,
    UnresolvedReadTarget,
    UnconfirmedRead,
    ResourceLimit,
    ConflictingObservation,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum SourcePathPlatform {
    Posix,
    Windows,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadMatchTarget {
    /// Historical lexical target; no current filesystem, inode or content identity.
    pub path: String,
    pub platform: SourcePathPlatform,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationMatchObservation {
    pub format_version: u32,
    /// Only protocol-owned execution within an explicitly identified source thread.
    pub receiver_owner: Option<String>,
    pub request_fingerprint: Option<String>,
    /// Complete model-visible callable and arguments; never a decoded raw MCP service identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function_request_fingerprint: Option<String>,
    /// Native read labels verified against a supported, determinate invocation.
    pub read_targets: Vec<ReadMatchTarget>,
    /// Search/no-match exit codes cannot establish failures for retry matching.
    pub expected_nonzero: bool,
    pub gaps: Vec<MatchGap>,
}
impl OperationMatchObservation {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        if self.format_version != OPERATION_MATCH_VERSION {
            return Err(crate::dto::operation_error(
                "UNSUPPORTED_VERSION",
                "不支持此操作匹配观察版本",
            ));
        }
        anyhow::ensure!(
            self.receiver_owner.as_deref().is_none_or(|s| !s.is_empty()
                && s.len() <= MATCH_STRING_BYTES
                && !s.chars().any(char::is_control)),
            "invalid matching receiver"
        );
        anyhow::ensure!(
            self.request_fingerprint
                .iter()
                .chain(self.function_request_fingerprint.iter())
                .all(|s| s.len() == 64
                    && s.bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))),
            "invalid request fingerprint"
        );
        anyhow::ensure!(
            self.read_targets.len() <= MATCH_TARGET_LIMIT,
            "matching target limit"
        );
        let mut bytes = 0usize;
        let mut targets = std::collections::BTreeSet::new();
        for target in &self.read_targets {
            anyhow::ensure!(
                valid_work_path(&target.path) && targets.insert(&target.path),
                "invalid matching target"
            );
            let absolute = match target.platform {
                SourcePathPlatform::Posix => target.path.starts_with('/'),
                SourcePathPlatform::Windows => {
                    !target.path.contains('\\')
                        && ((target.path.starts_with("//")
                            && target
                                .path
                                .split('/')
                                .filter(|part| !part.is_empty())
                                .count()
                                >= 2)
                            || (target
                                .path
                                .as_bytes()
                                .first()
                                .is_some_and(u8::is_ascii_alphabetic)
                                && target.path.as_bytes().get(1) == Some(&b':')
                                && target.path.as_bytes().get(2) == Some(&b'/')))
                }
            };
            anyhow::ensure!(
                absolute && !target.path.split('/').any(|part| part == ".."),
                "unlocated matching target"
            );
            bytes = bytes.saturating_add(target.path.len());
        }
        anyhow::ensure!(bytes <= MATCH_STRING_BYTES, "matching target byte limit");
        anyhow::ensure!(
            !self.gaps.contains(&MatchGap::ConflictingObservation)
                || (self.receiver_owner.is_none()
                    && self.request_fingerprint.is_none()
                    && self.function_request_fingerprint.is_none()
                    && self.read_targets.is_empty()),
            "conflicting matching fields must be absent"
        );
        Ok(())
    }
}

impl Operation {
    pub(crate) fn validate_matching(&self) -> anyhow::Result<()> {
        if let Some(result) = &self.text_result {
            if result.method_version != 1 {
                return Err(crate::dto::operation_error(
                    "UNSUPPORTED_VERSION",
                    "Unknown text result observation format",
                ));
            }
            anyhow::ensure!(
                result.hash.as_ref().is_none_or(
                    |hash| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
                ) && (!result.conflicting || result.hash.is_none()),
                "invalid text result digest"
            );
        }
        let Some(m) = &self.matching else {
            return Ok(());
        };
        m.validate()?;
        anyhow::ensure!(
            m.receiver_owner
                .as_deref()
                .is_none_or(|owner| owner == self.thread_id.as_ref()),
            "matching receiver scope mismatch"
        );
        match self.kind.as_ref() {
            "command" => {
                anyhow::ensure!(
                    m.function_request_fingerprint.is_none(),
                    "commands cannot retain callable matching"
                );
                let Some(WorkObservation {
                    data: WorkData::Command { cwd, source, .. },
                    ..
                }) = &self.work
                else {
                    anyhow::ensure!(
                        m.receiver_owner.is_none()
                            && m.request_fingerprint.is_none()
                            && m.read_targets.is_empty(),
                        "command matching requires native work evidence"
                    );
                    return Ok(());
                };
                anyhow::ensure!(
                    m.receiver_owner.is_none()
                        || matches!(
                            source,
                            Some(CommandSource::Agent | CommandSource::UnifiedExecStartup)
                        ),
                    "matching receiver requires agent execution source"
                );
                anyhow::ensure!(
                    m.request_fingerprint.is_none() || cwd.is_some(),
                    "matching request requires historical cwd"
                );
            }
            "mcp" | "mcpTool" | "mcpResource" | "mcpDiscovery" | "mcpUnclassified"
            | "mcpConflict" => {
                anyhow::ensure!(
                    m.read_targets.is_empty() && !m.expected_nonzero,
                    "MCP matching cannot establish filesystem reads or expected command exits"
                );
                anyhow::ensure!(
                    m.request_fingerprint.is_none()
                        || self.server.as_deref().is_some_and(valid_service_identity)
                            && self.tool.as_deref().is_some_and(valid_service_identity),
                    "MCP matching requires service identity"
                );
                anyhow::ensure!(
                    self.kind.as_ref() != "mcpConflict"
                        || m.request_fingerprint.is_none()
                            && m.function_request_fingerprint.is_none()
                            && m.receiver_owner.is_none(),
                    "conflicting MCP matching fields must be absent"
                );
            }
            _ => anyhow::bail!("unsupported operation matching kind"),
        }
        Ok(())
    }
}

fn valid_service_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}
