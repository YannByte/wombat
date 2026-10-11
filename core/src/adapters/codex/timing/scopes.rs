//! Primitive scope observations, independent of previously projected metadata.
use super::*;

impl Facts {
    pub(in crate::adapters::codex) fn thread(
        &mut self,
        source: &SourceInstance,
        upstream: &str,
        timestamp: Option<&str>,
        cwd: Option<&str>,
        report: &mut SourceReport,
    ) -> String {
        let id = stable_id(&["codex", &source.id, "thread", upstream]);
        let value = Thread {
            id: id.clone(),
            agent_kind: "codex".into(),
            source_instance_id: source.id.clone(),
            upstream_id: upstream.into(),
            title: None,
            project: cwd.map(safe_text),
            started_at: timestamp.map(str::to_owned),
            last_activity_at: timestamp.map(str::to_owned),
        };
        self.record_thread(value, cwd, report);
        id
    }

    pub(in crate::adapters::codex) fn thread_activity(
        &mut self,
        source: &SourceInstance,
        upstream: &str,
        timestamp: Option<&str>,
        report: &mut SourceReport,
    ) {
        self.record_thread(
            Thread {
                id: stable_id(&["codex", &source.id, "thread", upstream]),
                agent_kind: "codex".into(),
                source_instance_id: source.id.clone(),
                upstream_id: upstream.into(),
                title: None,
                project: None,
                started_at: None,
                last_activity_at: timestamp.map(str::to_owned),
            },
            None,
            report,
        );
    }

    fn record_thread(&mut self, value: Thread, cwd: Option<&str>, report: &mut SourceReport) {
        let Some(evidence) = self.scope_evidence(report) else {
            return;
        };
        record(
            self,
            Some(value.id.clone()),
            None,
            SafePayload::Thread {
                value: Box::new(value.into_shared()),
                project_path: cwd.map(Into::into),
                evidence: evidence.clone(),
            },
            Vec::new(),
            report,
            &evidence,
        );
    }

    pub(in crate::adapters::codex) fn turn(
        &mut self,
        thread: &str,
        upstream: &str,
        timestamp: Option<&str>,
        status: Option<&str>,
        report: &mut SourceReport,
    ) -> String {
        let id = stable_id(&[thread, "turn", upstream]);
        let Some(evidence) = self.scope_evidence(report) else {
            return id;
        };
        let value = Turn {
            id: id.clone(),
            thread_id: thread.into(),
            upstream_id: upstream.into(),
            ordinal: 0,
            started_at: timestamp.map(str::to_owned),
            ended_at: matches!(status, Some("completed" | "interrupted" | "failed"))
                .then(|| timestamp.map(str::to_owned))
                .flatten(),
            last_activity_at: timestamp.map(str::to_owned),
            status: status.unwrap_or("unknown").into(),
        };
        record(
            self,
            Some(thread.into()),
            Some(id.clone()),
            SafePayload::Turn {
                value: Box::new(value.into_shared()),
                evidence: evidence.clone(),
            },
            Vec::new(),
            report,
            &evidence,
        );
        id
    }

    fn scope_evidence(&self, report: &mut SourceReport) -> Option<EvidenceRef> {
        if let Some(context) = &self.event_context {
            Some(context.evidence.clone())
        } else {
            issue(
                report,
                "missingEventContext",
                "事件缺少来源位置，无法建立投影",
                None,
            );
            None
        }
    }
}
