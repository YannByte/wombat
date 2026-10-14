//! Internal per-file observation coverage; never a public transport schema.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

pub const WATERMARK_FORMAT_VERSION: u32 = 2;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OriginalSource {
    pub path: String,
    pub physical_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatermarkState {
    Complete,
    Partial,
    Missing,
    Failed,
}

/// A finite vocabulary prevents raw diagnostics or source content from entering coverage.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WatermarkIssue {
    IncompleteTail,
    InvalidRecord,
    InvalidTimestamp,
    SourceChanged,
    SourceUnreadable,
    ResourceLimit,
    Cancelled,
    SourceMissing,
    SourceSyncFailed,
    InvalidSourceField,
}
impl WatermarkIssue {
    pub(crate) fn from_code(code: &str) -> Self {
        match code {
            "incompleteTail" => Self::IncompleteTail,
            "invalidRecord" => Self::InvalidRecord,
            "invalidTimestamp" => Self::InvalidTimestamp,
            "sourceChanged" => Self::SourceChanged,
            "sourceUnreadable" => Self::SourceUnreadable,
            "resourceLimit" | "issueLimit" => Self::ResourceLimit,
            "sourceMissing" => Self::SourceMissing,
            "sourceSyncFailed" => Self::SourceSyncFailed,
            _ => Self::InvalidSourceField,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceWatermark {
    pub original_source: Option<OriginalSource>,
    pub format_version: u32,
    pub source_instance_id: String,
    /// The exact file identity used by session_events::Position.
    pub file_id: String,
    /// None for empty files or observations before any generation was established.
    pub generation: Option<String>,
    /// Last committed complete newline boundary, including invalid records skipped there.
    /// Failed/Missing retain this historical position; they never delete observed facts.
    pub committed_offset: u64,
    /// Captured file length for this observation, unknown after failure or disappearance.
    pub observed_bytes: Option<u64>,
    /// Collection time of this observation, independent of the retained commit position.
    pub observed_at: String,
    pub state: WatermarkState,
    pub issue_codes: Vec<WatermarkIssue>,
}
impl SourceWatermark {
    pub(crate) fn validate(&self) -> Result<()> {
        if let Some(source) = &self.original_source {
            ensure!(
                !source.physical_id.is_empty()
                    && crate::hash(source.path.as_bytes()) == self.file_id,
                "invalid original source identity"
            );
        }
        if self.format_version != WATERMARK_FORMAT_VERSION {
            return Err(crate::dto::operation_error(
                "UNSUPPORTED_VERSION",
                "不支持此来源水位版本",
            ));
        }
        ensure!(
            !self.source_instance_id.is_empty() && !self.file_id.is_empty(),
            "missing watermark identity"
        );
        ensure!(
            self.generation.as_ref().is_none_or(|v| !v.is_empty()),
            "empty watermark generation"
        );
        ensure!(
            chrono::DateTime::parse_from_rfc3339(&self.observed_at).is_ok(),
            "invalid watermark observation time"
        );
        ensure!(
            self.issue_codes.len() <= 10,
            "watermark issue limit exceeded"
        );
        ensure!(
            !self
                .issue_codes
                .iter()
                .enumerate()
                .any(|(i, v)| self.issue_codes[..i].contains(v)),
            "duplicate watermark issue"
        );
        if matches!(
            self.state,
            WatermarkState::Complete | WatermarkState::Partial
        ) {
            ensure!(
                self.observed_bytes
                    .is_some_and(|n| self.committed_offset <= n),
                "invalid watermark byte coverage"
            );
        }
        if self.state == WatermarkState::Complete {
            ensure!(
                self.observed_bytes == Some(self.committed_offset) && self.issue_codes.is_empty(),
                "incomplete watermark claims complete"
            );
        }
        Ok(())
    }
    pub(crate) fn unavailable(
        &self,
        state: WatermarkState,
        issue: WatermarkIssue,
        at: &str,
    ) -> Self {
        Self {
            observed_bytes: None,
            observed_at: at.into(),
            state,
            issue_codes: vec![issue],
            ..self.clone()
        }
    }
}

pub(crate) fn validate_watermarks(rows: &[SourceWatermark]) -> Result<()> {
    let mut keys = std::collections::BTreeSet::new();
    for row in rows {
        row.validate()?;
        ensure!(
            keys.insert((&row.source_instance_id, &row.file_id)),
            "duplicate file watermark"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
