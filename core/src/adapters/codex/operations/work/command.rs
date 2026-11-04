//! Codex 89c8bcf37d64be69e4c8286f4541c1a84ed312a4, protocol/items.rs
//! CommandExecutionItem, protocol/parse_command.rs and utils/path-uri/src/lib.rs.
//! Paginated ItemCompleted persists parsed_cmd/cwd/source; exec begin/end are transient.
//! Native Read is best-effort source classification, not an independent shell dispatch.
use super::*;
use serde::de::{SeqAccess, Visitor};

#[derive(Default)]
struct ParsedItems {
    values: Vec<ParsedCommand>,
    gaps: Vec<WorkGap>,
    bytes: usize,
    limited: bool,
}
fn gap(gaps: &mut Vec<WorkGap>, value: WorkGap) {
    if !gaps.contains(&value) {
        gaps.push(value);
    }
}
struct ParsedVisitor;
impl<'de> Visitor<'de> for ParsedVisitor {
    type Value = ParsedItems;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("native parsed command observations")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ParsedItems, A::Error> {
        #[derive(Deserialize)]
        struct Entry<'a> {
            #[serde(rename = "type")]
            kind: Option<String>,
            #[serde(borrow)]
            path: Option<&'a RawValue>,
        }
        let mut out = ParsedItems::default();
        while let Some(raw) = seq.next_element::<&RawValue>()? {
            if out.limited {
                continue;
            }
            if out.values.len() >= WORK_PATH_LIMIT {
                out.values.clear();
                out.limited = true;
                gap(&mut out.gaps, WorkGap::ResourceLimit);
                continue;
            }
            let entry = serde_json::from_str::<Entry>(raw.get()).ok();
            let kind = entry.as_ref().and_then(|v| v.kind.as_deref());
            let mut path = None;
            if matches!(kind, Some("read" | "list_files" | "search")) {
                match entry
                    .as_ref()
                    .and_then(|v| v.path)
                    .map(|v| serde_json::from_str::<String>(v.get()))
                    .transpose()
                {
                    Ok(Some(value)) if valid_work_path(&value) => path = Some(value),
                    Ok(None) => {
                        if kind == Some("read") {
                            gap(&mut out.gaps, WorkGap::MissingReadPath);
                        }
                    }
                    _ => gap(&mut out.gaps, WorkGap::InvalidField),
                }
            }
            let value = match kind {
                Some("read") => ParsedCommand::Read { path },
                Some("list_files") => ParsedCommand::ListFiles { path },
                Some("search") => ParsedCommand::Search { path },
                Some("unknown") => ParsedCommand::Unknown,
                _ => {
                    gap(
                        &mut out.gaps,
                        if entry.is_none() {
                            WorkGap::InvalidField
                        } else {
                            WorkGap::UnknownVariant
                        },
                    );
                    ParsedCommand::Unknown
                }
            };
            out.bytes = out.bytes.saturating_add(value.path().map_or(0, str::len));
            if out.bytes > WORK_PATH_BYTES {
                out.values.clear();
                out.limited = true;
                gap(&mut out.gaps, WorkGap::ResourceLimit);
            } else {
                out.values.push(value);
            }
        }
        Ok(out)
    }
}
impl<'de> Deserialize<'de> for ParsedItems {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_seq(ParsedVisitor)
    }
}
pub(in crate::adapters::codex::operations) fn observe(
    item: &Payload<'_>,
    raw_item: Option<&RawValue>,
    receiver: Option<&str>,
    terminal: bool,
    report: &mut SourceReport,
    evidence: &EvidenceRef,
) -> (
    WorkObservation,
    crate::adapters::contract::OperationMatchObservation,
) {
    let mut gaps = Vec::new();
    let source = match item.source {
        None => {
            gap(&mut gaps, WorkGap::MissingCommandSource);
            None
        }
        Some(raw) => match serde_json::from_str::<String>(raw.get()).as_deref() {
            Ok("agent") => Some(CommandSource::Agent),
            Ok("user_shell") => Some(CommandSource::UserShell),
            Ok("unified_exec_startup") => Some(CommandSource::UnifiedExecStartup),
            Ok("unified_exec_interaction") => Some(CommandSource::UnifiedExecInteraction),
            Ok(_) => {
                gap(&mut gaps, WorkGap::UnknownVariant);
                Some(CommandSource::Unknown)
            }
            Err(_) => {
                gap(&mut gaps, WorkGap::InvalidField);
                None
            }
        },
    };
    let cwd = match item.cwd.as_deref() {
        None => {
            gap(&mut gaps, WorkGap::MissingCommandCwd);
            None
        }
        Some(value) if !valid_work_path(value) => {
            gap(&mut gaps, WorkGap::InvalidField);
            None
        }
        Some(value) if value.len() > WORK_PATH_BYTES => {
            gap(&mut gaps, WorkGap::ResourceLimit);
            None
        }
        Some(value) => Some(value.to_owned()),
    };
    let parsed_commands = match item.parsed_cmd {
        None => {
            gap(&mut gaps, WorkGap::MissingParsedCommands);
            None
        }
        Some(raw) => match serde_json::from_str::<ParsedItems>(raw.get()) {
            Err(_) => {
                gap(&mut gaps, WorkGap::InvalidField);
                None
            }
            Ok(parsed) => {
                for value in parsed.gaps {
                    gap(&mut gaps, value);
                }
                if parsed.limited
                    || parsed
                        .bytes
                        .saturating_add(cwd.as_ref().map_or(0, String::len))
                        > WORK_PATH_BYTES
                {
                    gap(&mut gaps, WorkGap::ResourceLimit);
                    None
                } else {
                    Some(parsed.values)
                }
            }
        },
    };
    if !gaps.is_empty() {
        issue(
            report,
            "workObservationPartial",
            "命令结构化观察含缺口，保留未知元数据",
            Some(evidence.clone()),
        );
    }
    let supported = super::matching::command_shape(raw_item);
    let mut matching = super::matching::observe(
        if supported { item.command } else { None },
        receiver,
        source.as_ref(),
        cwd.as_deref(),
        parsed_commands.as_deref(),
    );
    if !supported && item.command.is_some() {
        matching
            .gaps
            .retain(|gap| *gap != MatchGap::MissingParameters);
        matching.gaps.push(MatchGap::UnsupportedParameters);
    }
    // Keep rejection evidence even if later generic outcome reconciliation
    // selects a stronger failure status from a contradictory exit code.
    if item.status.as_deref() == Some("declined")
        && !matching.gaps.contains(&MatchGap::UnconfirmedRead)
    {
        matching.gaps.push(MatchGap::UnconfirmedRead);
    }
    (
        WorkObservation {
            format_version: WORK_OBSERVATION_VERSION,
            stage: if terminal {
                WorkStage::Terminal
            } else {
                WorkStage::Proposed
            },
            data: WorkData::Command {
                cwd,
                source,
                parsed_commands,
            },
            gaps,
        },
        matching,
    )
}

/// A completion may omit repeated dispatch metadata. Fill absent fields only;
/// disagreeing known fields invalidate the metadata rather than choose an endpoint.
pub(super) fn merge(previous: &mut WorkObservation, next: &WorkObservation) -> bool {
    let (
        WorkData::Command {
            cwd,
            source,
            parsed_commands,
        },
        WorkData::Command {
            cwd: new_cwd,
            source: new_source,
            parsed_commands: new_parsed,
        },
    ) = (&mut previous.data, &next.data)
    else {
        return false;
    };
    if next.stage == WorkStage::Terminal {
        previous.stage = WorkStage::Terminal;
    }
    let different =
        |left: &Option<_>, right: &Option<_>| left.is_some() && right.is_some() && left != right;
    let conflict = different(cwd, new_cwd)
        || (source.is_some() && new_source.is_some() && source != new_source)
        || (parsed_commands.is_some() && new_parsed.is_some() && parsed_commands != new_parsed);
    if conflict {
        *cwd = None;
        *source = None;
        *parsed_commands = None;
        gap(&mut previous.gaps, WorkGap::ConflictingObservation);
        return true;
    }
    if cwd.is_none() {
        cwd.clone_from(new_cwd);
    }
    if source.is_none() {
        source.clone_from(new_source);
    }
    if parsed_commands.is_none() {
        parsed_commands.clone_from(new_parsed);
    }
    for value in &next.gaps {
        gap(&mut previous.gaps, value.clone());
    }
    previous.gaps.retain(|value| match value {
        WorkGap::MissingCommandCwd => cwd.is_none(),
        WorkGap::MissingCommandSource => source.is_none(),
        WorkGap::MissingParsedCommands => parsed_commands.is_none(),
        _ => true,
    });
    false
}
