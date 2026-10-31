//! Shared synthetic inventory fixtures for independent rule and decision tests.
use super::service::execute_at;
use crate::{
    config::View,
    config_dto::{Counts, Item, Kind, Observation, SkillMetadata},
    optimize_dto::*,
};
use std::path::Path;
mod evaluation;
mod rules;
mod stateless;
fn body_estimate(tokens: u64) -> crate::config_dto::ContentEstimate {
    crate::config_dto::ContentEstimate {
        tokens,
        encoding: "o200k_base".into(),
        method: "tiktoken-rs-0.12.0/o200k_base/ordinary-v1".into(),
        payload: "skillBody".into(),
        content_hash: "synthetic-body".into(),
        applicability: "referenceEncodingOnly".into(),
        tokenizer_version: Some("tiktoken-rs-0.12.0".into()),
    }
}
fn view() -> View {
    View {
        observation_versions: Default::default(),
        config_collection: Default::default(),
        hook_registry: Default::default(),
        snapshot: None,
        items: vec![Item {
            id: "object".into(),
            name: "synthetic".into(),
            kind: Kind::Skill,
            source_instance_id: "source".into(),
            path: "/synthetic/SKILL.md".into(),
            project: None,
            authorized_projects: vec![],
            source_contexts: vec![],
            native_key: None,
            configured_state: "discovered".into(),
            content_hash: "original".into(),
            observed_at: "2026-10-01T00:00:00Z".into(),
            current: true,
            stale: false,
            bytes: Some(16385),
            content_tokens: None,
            estimate_status: "unavailable".into(),
            characters: Some(16385),
            measurement_status: "complete".into(),
            bytes_source: Some("completeUtf8File".into()),
            estimate: None,
            body_token_estimate: None,
            body_estimate_status: "unknown".into(),
            skill_metadata: Some(SkillMetadata {
                status: "invalid".into(),
                description_characters: Some(501),
                issues: vec!["nameMissing".into()],
                diagnostics: vec![],
            }),
            usage_count: None,
            use_basis: None,
            last_record_at: None,
            observation: Observation::Unknown,
            counts: Counts::default(),
            related_turns: 0,
            related_tasks: 0,
            usage: None,
        }],
        issues: vec![],
        projects: vec!["/project".into(), "/other".into()],
        roots: vec![],
        project_roots: vec![],
        revision: "version".into(),
        checked: "2026-10-01T00:00:00Z".into(),
        history_status: "unavailable".into(),
        analysis: Default::default(),
    }
}
fn query(path: &Path, v: &View, r: Request) -> Response {
    execute_at(r, "config:test".into(), v, path).unwrap()
}
