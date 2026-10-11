//! Bounded rollout reads, unchanged-file checks and parser boundaries.
use super::*;
use crate::file_metadata::FileMetadata;
#[derive(Default, Serialize, Deserialize)]
pub(super) struct State {
    pub(super) event_generation: Option<String>,
    /// Replacement identity seed; not an observed generation until a valid row exists.
    pub(super) pending_event_generation: Option<String>,
    pub(super) thread: Option<String>,
    pub(super) turn: Option<String>,
    pub(super) model: ModelRef,
    pub(super) effort: Option<String>,
    pub(super) context_conflicts: Vec<crate::session_events::MeasurementContextField>,
    pub(super) thread_context_conflicts: Vec<crate::session_events::MeasurementContextField>,
    pub(super) thread_model: ModelRef,
    pub(super) thread_effort: Option<String>,
    pub(super) previous: Option<TokenObservation>,
    pub(super) ordinal: u64,
    pub(super) epoch: u64,
    pub(super) uncertain_counter: bool,
    pub(super) available_skills: BTreeMap<String, String>,
    pub(super) turn_had_operation: bool,
}
impl State {
    pub(super) fn break_context(&mut self) {
        self.turn = None;
        self.model = ModelRef::default();
        self.effort = None;
        self.context_conflicts.clear();
        self.thread_context_conflicts.clear();
        self.thread_model = ModelRef::default();
        self.thread_effort = None;
        self.previous = None;
        self.available_skills.clear();
        self.turn_had_operation = false;
        self.epoch += 1;
    }
    pub(super) fn corrupt_boundary(&mut self) {
        self.break_context();
        self.uncertain_counter = true;
    }
}

pub(super) fn timestamp(text: Option<&str>) -> Option<String> {
    crate::session_events::Time::from_source(text)
        .0
        .timestamp
        .as_deref()
        .map(str::to_owned)
}
pub(super) fn precision(text: Option<&str>) -> String {
    let fraction = text
        .and_then(|text| text.split_once('.'))
        .map(|(_, fraction)| fraction.chars().take_while(char::is_ascii_digit).count())
        .unwrap_or(0);
    if fraction > 6 {
        "nanosecond"
    } else if fraction > 3 {
        "microsecond"
    } else if fraction > 0 {
        "millisecond"
    } else if text.is_some_and(|t| t.len() >= 19) {
        "second"
    } else {
        "unknown"
    }
    .into()
}

pub(super) fn read_file(
    path: &Path,
    source: &SourceInstance,
    context: &RunContext,
    facts: &mut Facts,
    report: &mut SourceReport,
) {
    read_file_from(path, source, context, facts, report, None, None);
}

pub(super) fn read_file_from(
    path: &Path,
    source: &SourceInstance,
    context: &RunContext,
    facts: &mut Facts,
    report: &mut SourceReport,
    checkpoint: Option<&mut incremental::Checkpoint>,
    capture: Option<&FileMetadata>,
) {
    let _ = read_file_flushing(
        path,
        source,
        context,
        facts,
        report,
        checkpoint,
        capture,
        |_, _| Ok(()),
    );
}

#[allow(clippy::too_many_arguments)]
pub(super) fn read_file_flushing(
    path: &Path,
    source: &SourceInstance,
    context: &RunContext,
    facts: &mut Facts,
    report: &mut SourceReport,
    mut checkpoint: Option<&mut incremental::Checkpoint>,
    capture: Option<&FileMetadata>,
    mut flush: impl FnMut(&mut Facts, bool) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let evidence_path: Arc<str> = path.to_string_lossy().as_ref().into();
    let file_id = crate::hash(evidence_path.as_bytes());
    let first_issue = report.issues.len();
    let observed_at = chrono::Utc::now().to_rfc3339();
    let prior_offset = checkpoint.as_ref().map_or(0, |c| c.offset);
    let prior_generation = checkpoint
        .as_ref()
        .and_then(|c| c.state.event_generation.clone());
    let mut watermark = SourceWatermark {
        format_version: WATERMARK_FORMAT_VERSION,
        source_instance_id: source.id.clone(),
        file_id: file_id.clone(),
        generation: prior_generation.clone(),
        committed_offset: prior_offset,
        observed_bytes: None,
        observed_at,
        state: WatermarkState::Failed,
        issue_codes: vec![WatermarkIssue::SourceUnreadable],
    };
    facts.watermarks.insert(file_id.clone(), watermark.clone());
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(_) => {
            issue(
                report,
                "sourceUnreadable",
                "无法读取日志文件",
                Some(EvidenceRef {
                    file: Arc::clone(&evidence_path),
                    line: 0,
                }),
            );
            return Ok(());
        }
    };
    let opened = FileMetadata::from_file(&file).ok();
    if capture.is_some_and(|expected| {
        opened
            .as_ref()
            .is_none_or(|current| !incremental::contains_capture(expected, current))
    }) {
        issue(report, "sourceChanged", "日志在读取期间被替换或截断", None);
        return Ok(());
    }
    let before = capture.cloned().or(opened);
    let length = before.as_ref().map_or(0, |m| m.len());
    watermark.observed_bytes = before.as_ref().map(|m| m.len());
    facts.watermarks.insert(file_id.clone(), watermark.clone());
    if length > context.max_bytes.saturating_sub(report.bytes_read) {
        issue(report, "resourceLimit", "日志文件超出剩余读取范围", None);
        watermark.issue_codes = vec![WatermarkIssue::ResourceLimit];
        facts.watermarks.insert(file_id.clone(), watermark);
        return Ok(());
    }
    // Freeze this read to the observed prefix; later appends belong to the next refresh.
    let offset = checkpoint.as_ref().map_or(0, |c| c.offset);
    if file.seek(SeekFrom::Start(offset)).is_err() {
        issue(report, "sourceUnreadable", "无法定位日志增量", None);
        return Ok(());
    }
    let mut reader = BufReader::new(file.take(length.saturating_sub(offset)));
    let event_source: Arc<str> = source.id.as_str().into();
    let event_file: Arc<str> = file_id.as_str().into();
    let mut buffer = Vec::new();
    let mut state = checkpoint
        .as_mut()
        .map_or_else(State::default, |c| std::mem::take(&mut c.state));
    let mut line_number = checkpoint.as_ref().map_or(0, |c| c.line);
    let mut consumed = offset;
    let mut newline_offset = offset;
    let mut unclosed_record = false;
    let mut pending_tail = false;
    let mut valid_records = 0;
    loop {
        flush(facts, false)?;
        if context.is_cancelled() {
            report.status = "cancelled".into();
            break;
        }
        let row = match crate::log_io::next_line(&mut reader, &mut buffer) {
            Ok(Some(row)) => row,
            Ok(None) => break,
            Err(_) => {
                issue(report, "sourceUnreadable", "日志读取中断", None);
                break;
            }
        };
        // A live cursor only commits complete newline-delimited records. No state is
        // changed by a partial tail, even when its current prefix is valid JSON.
        if checkpoint.is_some() && !row.bytes().ends_with(b"\n") {
            report.bytes_read += row.bytes().len() as u64;
            pending_tail = true;
            issue(
                report,
                "incompleteTail",
                "日志尾行尚未写完",
                Some(EvidenceRef {
                    file: Arc::clone(&evidence_path),
                    line: line_number + 1,
                }),
            );
            break;
        }
        if row.bytes().ends_with(b"\n") {
            newline_offset = consumed + row.bytes().len() as u64;
        } else {
            // Direct reads keep accepting valid final JSON; watermark coverage still
            // ends at the last newline, independently of those observed facts.
            unclosed_record = true;
        }
        line_number += 1;
        consumed += row.bytes().len() as u64;
        report.bytes_read += row.bytes().len() as u64;
        let evidence = EvidenceRef {
            file: Arc::clone(&evidence_path),
            line: line_number,
        };
        if row.bytes().iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let record: Envelope = match serde_json::from_slice(row.bytes()) {
            Ok(record) => record,
            Err(_) => {
                let tail = !row.bytes().ends_with(b"\n");
                issue(
                    report,
                    if tail {
                        "incompleteTail"
                    } else {
                        "invalidRecord"
                    },
                    if tail {
                        "日志尾行尚未写完"
                    } else {
                        "日志记录格式无效"
                    },
                    Some(evidence.clone()),
                );
                if let Some(generation) = &state.event_generation {
                    timing::discontinuity(
                        facts,
                        crate::session_events::Position {
                            source_instance_id: Arc::clone(&event_source),
                            file_id: Arc::clone(&event_file),
                            generation: generation.clone().into(),
                            byte_offset: consumed - row.bytes().len() as u64,
                            ordinal: 0,
                        },
                        state.thread.clone(),
                        report,
                        &evidence,
                    );
                }
                state.corrupt_boundary();
                continue;
            }
        };
        let payload: Payload = match serde_json::from_str(record.payload.get()) {
            Ok(payload) => payload,
            Err(_) => {
                issue(
                    report,
                    "invalidRecord",
                    "日志字段格式无效",
                    Some(evidence.clone()),
                );
                if let Some(generation) = &state.event_generation {
                    timing::discontinuity(
                        facts,
                        crate::session_events::Position {
                            source_instance_id: Arc::clone(&event_source),
                            file_id: Arc::clone(&event_file),
                            generation: generation.clone().into(),
                            byte_offset: consumed - row.bytes().len() as u64,
                            ordinal: 0,
                        },
                        state.thread.clone(),
                        report,
                        &evidence,
                    );
                }
                state.corrupt_boundary();
                continue;
            }
        };
        valid_records += 1;
        let time = timestamp(record.timestamp);
        if record.timestamp.is_some() && time.is_none() {
            issue(
                report,
                "invalidTimestamp",
                "日志时间无效，保留未归日计量",
                Some(evidence.clone()),
            );
        }
        let fingerprint = format!(
            "{:x}",
            Sha256::digest(row.bytes().strip_suffix(b"\n").unwrap_or(row.bytes()))
        );
        let pending_generation = state.pending_event_generation.take();
        let generation = state
            .event_generation
            .get_or_insert_with(|| {
                pending_generation.unwrap_or_else(|| {
                    let physical = before.as_ref().map(incremental::physical_identity);
                    crate::hash(
                        serde_json::to_vec(&(physical, &fingerprint))
                            .expect("serializable identity"),
                    )
                })
            })
            .clone();
        let position = crate::session_events::Position {
            source_instance_id: Arc::clone(&event_source),
            file_id: Arc::clone(&event_file),
            generation: generation.into(),
            byte_offset: consumed - row.bytes().len() as u64,
            ordinal: 0,
        };
        facts.event_context = Some(timing::Context::new(
            position,
            record.timestamp,
            record.kind,
            payload.kind.as_deref(),
            &evidence,
            record.metadata,
        ));
        process(
            record.kind,
            payload,
            record.payload,
            time,
            record.timestamp,
            evidence,
            fingerprint,
            &mut state,
            facts,
            source,
            report,
        );
        facts.event_context = None;
    }
    if valid_records > 0 || length == 0 {
        report.files_read += 1;
    }
    let after = FileMetadata::read(path).ok();
    if (consumed < length && !pending_tail)
        || after.as_ref().is_none_or(|m| m.len() < length)
        || before
            .as_ref()
            .zip(after.as_ref())
            .is_some_and(|(a, b)| file_changed(a, b))
    {
        issue(report, "sourceChanged", "日志在读取期间被替换或截断", None);
    }
    if unclosed_record
        && !report.issues[first_issue..]
            .iter()
            .any(|i| i.code == "incompleteTail")
    {
        issue(
            report,
            "incompleteTail",
            "日志尾行缺少完整换行，保留已观察事实但读取覆盖不完整",
            Some(EvidenceRef {
                file: evidence_path,
                line: line_number,
            }),
        );
    }
    let mut codes = Vec::new();
    for issue in &report.issues[first_issue..] {
        let code = WatermarkIssue::from_code(&issue.code);
        if !codes.contains(&code) {
            codes.push(code);
        }
    }
    if (pending_tail || unclosed_record) && !codes.contains(&WatermarkIssue::IncompleteTail) {
        codes.push(WatermarkIssue::IncompleteTail);
    }
    if context.is_cancelled() {
        codes.push(WatermarkIssue::Cancelled);
    }
    if report.issues.len() >= 1000 && !codes.contains(&WatermarkIssue::ResourceLimit) {
        codes.push(WatermarkIssue::ResourceLimit);
    }
    let failed = codes.iter().any(|c| {
        matches!(
            c,
            WatermarkIssue::SourceChanged | WatermarkIssue::SourceUnreadable
        )
    });
    watermark.issue_codes = codes;
    watermark.state = if failed {
        WatermarkState::Failed
    } else if !watermark.issue_codes.is_empty() || newline_offset != length {
        WatermarkState::Partial
    } else {
        WatermarkState::Complete
    };
    if !failed {
        watermark.generation = state.event_generation.clone();
        watermark.committed_offset = newline_offset;
    }
    facts.watermarks.insert(file_id, watermark);
    if let Some(checkpoint) = checkpoint {
        checkpoint.offset = consumed;
        checkpoint.line = line_number;
        checkpoint.state = state;
    }
    flush(facts, true)?;
    Ok(())
}

pub(super) fn file_changed(a: &FileMetadata, b: &FileMetadata) -> bool {
    a.physical != b.physical || (a.len() == b.len() && a.modified().ok() != b.modified().ok())
}
