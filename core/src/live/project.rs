//! Compose one project report from one incremental read and one current inventory.
use super::query::{Read, read};
use super::*;
use crate::project_dto::*;
use std::sync::atomic::AtomicBool;
fn failure(error: &anyhow::Error) -> Failure {
    Failure {
        code: crate::live_index::failure_code(error).into(),
        message: format!("{error:#}"),
    }
}
fn section<T>(result: Result<T>, partial: impl FnOnce(&T) -> bool) -> Section<T> {
    match result {
        Ok(result) => Section {
            state: if partial(&result) {
                State::Partial
            } else {
                State::Complete
            },
            result: Some(result),
            error: None,
        },
        Err(error) => Section {
            state: State::Unavailable,
            result: None,
            error: Some(failure(&error)),
        },
    }
}
fn unavailable<T>(error: &anyhow::Error) -> Result<T> {
    Err(operation_error(
        crate::live_index::failure_code(error),
        format!("{error:#}"),
    ))
}
pub(super) fn execute(
    request: crate::project_dto::Request,
    hooks: Option<&crate::config::hooks::Capture>,
    skills: Option<&crate::config::review::Capture>,
) -> Result<crate::project_dto::Response> {
    let parse = |v: &str| {
        chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d")
            .ok()
            .filter(|d| d.to_string() == v)
    };
    if !std::path::Path::new(&request.project_root).is_absolute()
        || !std::path::Path::new(&request.project_root).is_dir()
        || parse(&request.since).is_none()
        || parse(&request.until).is_none()
        || request.since >= request.until
        || request.timezone.parse::<chrono_tz::Tz>().is_err()
        || request.roots.len() > 64
    {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Project requires an existing absolute directory, valid dates and timezone",
        ));
    }
    let scope = usage_app_dto::Scope {
        project: Some(request.project_root.clone()),
        since: Some(request.since.clone()),
        until: Some(request.until.clone()),
        timezone: Some(request.timezone.clone()),
        source_instance_id: request.source_instance_id.clone(),
        ..Default::default()
    };
    let cfg = crate::config_dto::Request {
        review: true,
        roots: Some(request.roots.clone()),
        project_roots: Some(vec![request.project_root.clone()]),
        scope: crate::config_dto::Scope {
            project: scope.project.clone(),
            since: scope.since.clone(),
            until: scope.until.clone(),
            timezone: scope.timezone.clone(),
            source_instance_id: scope.source_instance_id.clone(),
            ..Default::default()
        },
        limit: Some(10),
        ..Default::default()
    };
    crate::config::validate(&cfg)?;
    crate::optimize::parameters(request.rule_overrides.clone())?;
    let collected = read(&request.roots, Mode::Auto, false, false, false);
    let mut inventory = match &collected {
        Ok(r) => r.snapshot.detailed_view().and_then(|details| {
            crate::config::prepare_observed(
                &cfg,
                Some(details.unwrap_or_else(|| Arc::clone(&r.snapshot))),
                "current".into(),
                hooks,
                skills,
            )
        }),
        Err(_) => crate::config::prepare_observed(&cfg, None, "unavailable".into(), hooks, skills),
    };
    if let (Ok(inventory), Err(error)) = (&mut inventory, &collected) {
        inventory.issues.push(crate::config_dto::Issue {
            code: crate::live_index::failure_code(error).into(),
            path: None,
        });
    }
    let configuration = section(
        match &inventory {
            Ok(v) => crate::config::execute(cfg, String::new(), v),
            Err(e) => unavailable(e),
        },
        |r| r.coverage.status == "partial",
    );
    let checks = section(
        match &inventory {
            Ok(v) => crate::optimize::execute(
                crate::optimize_dto::Request {
                    roots: Some(request.roots.clone()),
                    project_roots: Some(vec![request.project_root.clone()]),
                    project: Some(request.project_root.clone()),
                    source_instance_id: request.source_instance_id.clone(),
                    rule_overrides: request.rule_overrides.clone(),
                    limit: Some(20),
                    ..Default::default()
                },
                String::new(),
                v,
                None,
            ),
            Err(e) => unavailable(e),
        },
        |r| r.result_status == "partial",
    );
    let history = section(
        (|| match &collected {
            Ok(Read {
                snapshot,
                freshness,
            }) => {
                let mut result = crate::usage_app::execute_snapshot(
                    usage_app_dto::Request {
                        action: usage_app_dto::Action::Usage,
                        scope: scope.clone(),
                        limit: Some(1),
                        compact: Some(true),
                        ..Default::default()
                    },
                    snapshot.as_ref(),
                )?;
                result.freshness = Some(freshness.clone());
                Ok(super::Response {
                    output_version: 1,
                    result,
                    freshness: freshness.clone(),
                    additional: vec![],
                })
            }
            Err(e) => unavailable(e),
        })(),
        |r| r.result.quality.status == "partial",
    );
    let records = section(
        (|| match &collected {
            Ok(r) => {
                let mut query: crate::conversation_dto::Request = serde_json::from_value(
                    json!({"action":"records","roots":request.roots,"selection":{"projects":[request.project_root],"sources":request.source_instance_id.clone().into_iter().collect::<Vec<_>>(),"since":request.since,"until":request.until,"timezone":request.timezone},"limit":10}),
                )?;
                query.snapshot_id = r.freshness.revision.clone();
                crate::conversation::query(&r.snapshot, None, &query, &AtomicBool::new(false)).map(
                    |mut result| {
                        result.freshness = Some(r.freshness.clone());
                        result
                    },
                )
            }
            Err(e) => unavailable(e),
        })(),
        |r| r.partial,
    );
    let artifacts = section(
        crate::artifacts::execute(
            crate::artifacts_dto::Request::Inspect {
                project_root: request.project_root.clone(),
                paths: vec![],
                keep_paths: vec![],
                product_temporary: false,
                codex: false,
            },
            &|| false,
        ),
        |r| r.partial,
    );
    let partial = [
        &configuration.state,
        &checks.state,
        &history.state,
        &records.state,
        &artifacts.state,
    ]
    .iter()
    .any(|s| !matches!(s, State::Complete));
    let result = crate::project_dto::Response {
        output_version: 2,
        scope: request,
        partial,
        configuration,
        checks,
        history,
        records,
        artifacts,
    };
    if serde_json::to_vec(&result)?.len() > 2 * 1024 * 1024 {
        return Err(operation_error(
            "OUTPUT_LIMIT",
            "Project evidence exceeds 2 MiB; inspect selected components",
        ));
    }
    Ok(result)
}
