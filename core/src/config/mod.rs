//! Version-pinned configuration inventory and evidence queries, sharing the usage ledger.
mod analysis;
mod hook_targets;
pub(crate) mod hooks;
mod identity;
mod observations;
pub(crate) mod review;
pub(crate) use observations::{ConfigCollection, ObservationVersions};
pub(crate) mod measure;
mod references;
mod scan;
use crate::{
    adapters::contract::{AgentAdapter, DiscoveryRequest, MAX_SAFE_INTEGER},
    config_dto::*,
    dto::operation_error,
    usage_store::{PricedMeasurement, Snapshot},
};
use anyhow::Result;
use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::Tz;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, OnceLock},
};

pub(crate) struct View {
    pub snapshot: Option<Arc<Snapshot>>,
    pub items: Vec<Item>,
    pub issues: Vec<Issue>,
    pub projects: Vec<String>,
    pub roots: Vec<String>,
    pub project_roots: Vec<String>,
    pub revision: String,
    pub checked: String,
    pub history_status: String,
    pub analysis: analysis::Analysis,
    pub hook_registry: HookRegistry,
    pub config_collection: ConfigCollection,
    pub observation_versions: OnceLock<ObservationVersions>,
}

mod inventory;
mod query;
mod scope;
#[cfg(test)]
mod tests;
pub(crate) use inventory::prepare_observed;
pub(crate) use query::execute;
#[cfg(test)]
use scope::normalize;
pub(crate) use scope::validate;
use scope::{applicable, in_time, matches_row, normalize_at, usage};
pub(crate) fn capabilities() -> Response {
    Response {
        explanation: None,
        extension_activity: None,
        extension_review: None,
        hook_registry: HookRegistry::default(),
        output_version: 3,
        action: Action::Capabilities,
        capabilities: Capabilities::default(),
        read_view: None,
        usage_revision: None,
        config_revision: String::new(),
        checked_at: Utc::now().to_rfc3339(),
        scope: Scope::default(),
        authorized_projects: vec![],
        authorized_source_roots: vec![],
        host_restart_command: None,
        summary: Summary {
            current_items: 0,
            historical_items: 0,
            observed_items: 0,
            usage: None,
        },
        items: vec![],
        evidence: vec![],
        related_scopes: vec![],
        page: crate::usage_app_dto::Page {
            offset: 0,
            limit: 50,
            total: 0,
            next_offset: None,
        },
        coverage: Coverage {
            history_read_complete: None,
            status: "complete".into(),
            history_status: "notRequested".into(),
            issues: vec![],
            supported_evidence: Capabilities::default().evidence_types,
            absence_observable: false,
        },
    }
}
