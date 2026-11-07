pub mod codex;
pub(crate) use codex::review_target;
pub mod contract;
pub(crate) mod shared_strings;

use contract::*;
use sha2::{Digest, Sha256};

pub fn stable_id(parts: &[&str]) -> String {
    let mut digest = Sha256::new();
    for part in parts {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    format!("{:x}", digest.finalize())
}

pub fn registry() -> Vec<Box<dyn AgentAdapter>> {
    vec![Box::new(codex::CodexAdapter)]
}

pub fn collect(request: &DiscoveryRequest, context: &RunContext) -> Collected {
    collect_with(&registry(), request, context)
}

/// No adapter-specific branch: unsupported detail levels remain absent facts.
pub fn collect_with(
    adapters: &[Box<dyn AgentAdapter>],
    request: &DiscoveryRequest,
    context: &RunContext,
) -> Collected {
    collect_planned(adapters, request, context, &ReadPlan::default())
}

pub(crate) fn collect_planned(
    adapters: &[Box<dyn AgentAdapter>],
    request: &DiscoveryRequest,
    context: &RunContext,
    plan: &ReadPlan,
) -> Collected {
    let mut result = Collected::default();
    for adapter in adapters {
        let discovered = adapter.discover(request);
        result.issues.extend(discovered.issues);
        for source in discovered.sources {
            let before = (
                result.watermarks.len(),
                result.title_observations.len(),
                result.threads.len(),
                result.turns.len(),
                result.measurements.len(),
                result.events.len(),
                result.operations.len(),
            );
            let report = adapter.collect(&source, plan, context, &mut result);
            // Streaming sinks can already have received a prefix when cancellation
            // arrives. A cancelled source does not publish that prefix as new facts.
            if report.status == "cancelled" {
                result.watermarks.truncate(before.0);
                result.title_observations.truncate(before.1);
                result.threads.truncate(before.2);
                result.turns.truncate(before.3);
                result.measurements.truncate(before.4);
                result.events.truncate(before.5);
                result.operations.truncate(before.6);
            }
            result.issues.extend(report.issues.iter().cloned());
            result.sources.push(report);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Cancelling;
    impl AgentAdapter for Cancelling {
        fn descriptor(&self) -> AdapterDescriptor {
            DailyOnly.descriptor()
        }
        fn discover(&self, r: &DiscoveryRequest) -> DiscoveryReport {
            DailyOnly.discover(r)
        }
        fn collect(
            &self,
            source: &SourceInstance,
            plan: &ReadPlan,
            context: &RunContext,
            sink: &mut dyn FactSink,
        ) -> SourceReport {
            let mut report = DailyOnly.collect(source, plan, context, sink);
            report.status = "cancelled".into();
            report
        }
    }
    #[test]
    fn cancelled_source_prefix_is_retracted_without_erasing_prior_source_facts() {
        let result = collect_with(
            &[Box::new(DailyOnly), Box::new(Cancelling)],
            &DiscoveryRequest { roots: vec![] },
            &RunContext::default(),
        );
        assert_eq!(result.sources.len(), 2);
        assert_eq!(result.sources[0].status, "complete");
        assert_eq!(result.sources[1].status, "cancelled");
        assert_eq!(result.measurements.len(), 1);
        assert_eq!(result.measurements[0].tokens.total, Some(110));
    }
    struct DailyOnly;
    impl AgentAdapter for DailyOnly {
        fn descriptor(&self) -> AdapterDescriptor {
            AdapterDescriptor {
                agent_kind: "test-daily".into(),
                adapter_version: "1".into(),
                capabilities: Capabilities {
                    usage: true,
                    measurement_grain: vec!["day".into()],
                    ..Capabilities::default()
                },
            }
        }
        fn discover(&self, _: &DiscoveryRequest) -> DiscoveryReport {
            DiscoveryReport {
                sources: vec![SourceInstance {
                    id: "fixture-source".into(),
                    agent_kind: "test-daily".into(),
                    root: "synthetic".into(),
                }],
                issues: vec![],
            }
        }
        fn collect(
            &self,
            source: &SourceInstance,
            _: &ReadPlan,
            _: &RunContext,
            sink: &mut dyn FactSink,
        ) -> SourceReport {
            sink.push(Fact::Measurement(
                Measurement {
                    id: stable_id(&[&source.agent_kind, &source.id, "same-upstream"]),
                    agent_kind: source.agent_kind.as_str().into(),
                    source_instance_id: source.id.as_str().into(),
                    thread_id: None,
                    turn_id: None,
                    response_id: None,
                    timestamp: Some("2026-09-29T00:00:00Z".into()),
                    interval_end: Some("2026-09-30T00:00:00Z".into()),
                    grain: "day".into(),
                    time_precision: "day".into(),
                    model: ModelRef {
                        raw: Some("synthetic-model".into()),
                        provider: Some("unknown-provider".into()),
                        ..ModelRef::default()
                    },
                    reasoning_effort: None,
                    tokens: TokenUsage {
                        input: Some(20),
                        cache_read: Some(80),
                        cache_create: Some(0),
                        output: Some(10),
                        reasoning: None,
                        total: Some(110),
                        raw_input: Some(100),
                    },
                    token_unavailable_reasons: TokenFields {
                        reasoning: Some(TokenUnavailableReason::Missing),
                        ..TokenFields::default()
                    },
                    pricing_context_conflict: false,
                    request_scoped: false,
                    reported_cost: Some("1.234".into()),
                    service_tier: None,
                    sequence: 1,
                    evidence: vec![],
                }
                .into(),
            ));
            let descriptor = self.descriptor();
            SourceReport {
                source: source.clone(),
                adapter_version: descriptor.adapter_version,
                source_versions: vec![],
                capabilities: descriptor.capabilities,
                status: "complete".into(),
                files_read: 1,
                bytes_read: 0,
                issues: vec![],
            }
        }
    }
    #[test]
    fn heterogeneous_source_requires_no_fake_thread_or_provider_inference() {
        let result = collect_with(
            &[Box::new(DailyOnly)],
            &DiscoveryRequest::default(),
            &RunContext::default(),
        );
        assert_eq!(result.measurements.len(), 1);
        assert!(result.threads.is_empty());
        assert!(result.turns.is_empty());
        let measurement = &result.measurements[0];
        assert_eq!(measurement.tokens.total, Some(110));
        assert_eq!(
            measurement.model.provider.as_deref(),
            Some("unknown-provider")
        );
        assert!(measurement.reasoning_effort.is_none());
        assert!(!result.sources[0].capabilities.turns);
        assert_ne!(
            measurement.id,
            stable_id(&["codex", "fixture-source", "same-upstream"])
        );
    }
}
