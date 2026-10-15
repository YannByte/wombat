//! Version-pinned configuration inventory and evidence queries, sharing the usage ledger.
mod analysis;
mod hook_targets;
pub(crate) mod hooks;
mod identity;
mod observations;
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
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
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
#[derive(Default)]
pub(crate) struct Store {
    views: VecDeque<(String, Instant, Arc<View>)>,
}
impl Store {
    pub(crate) fn evict_unshared_snapshots(&mut self) {
        self.views
            .retain(|(_, _, view)| view.snapshot.is_none() || Arc::strong_count(view) > 1);
    }
    pub fn has_views(&mut self) -> bool {
        self.views
            .retain(|(_, at, _)| at.elapsed() < Duration::from_secs(600));
        !self.views.is_empty()
    }
    pub fn get(&mut self, id: &str) -> Result<Arc<View>> {
        self.has_views();
        self.views
            .iter()
            .find(|(key, _, _)| key == id)
            .map(|(_, _, v)| Arc::clone(v))
            .ok_or_else(|| operation_error("VIEW_EXPIRED", "配置读取版本已过期，请刷新"))
    }
    pub fn snapshot(&mut self, id: &str) -> Option<(Arc<Snapshot>, String)> {
        self.has_views();
        self.views.iter().find_map(|(_, _, view)| {
            let snapshot = view.snapshot.as_ref()?;
            (snapshot.manifest.snapshot_ref.snapshot_id == id)
                .then(|| (Arc::clone(snapshot), view.checked.clone()))
        })
    }
    pub fn insert(&mut self, view: View) -> (String, Arc<View>) {
        let id = format!("config:{}", uuid::Uuid::new_v4());
        let view = Arc::new(view);
        while self.views.len() >= 8 {
            self.views.pop_front();
        }
        self.views
            .push_back((id.clone(), Instant::now(), Arc::clone(&view)));
        (id, view)
    }
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
        extension_activity: None,
        hook_registry: HookRegistry::default(),
        output_version: 1,
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
            status: "complete".into(),
            history_status: "notRequested".into(),
            issues: vec![],
            supported_evidence: Capabilities::default().evidence_types,
            absence_observable: false,
        },
    }
}
