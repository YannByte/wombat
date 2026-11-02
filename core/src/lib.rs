// Reclaim freed scan pages in the long-lived native host.
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Configure the native host before it starts long-lived work.
#[doc(hidden)]
pub fn configure_allocator() {
    // The pinned mimalloc 2.3.2 header defines mi_option_disallow_arena_alloc=27.
    // The Rust binding omits that option. Direct OS segments can be unmapped
    // after large scans instead of retaining reusable pages in a 1 GiB arena.
    const DISALLOW_ARENA_ALLOC: libmimalloc_sys::mi_option_t = 27;
    // SAFETY: this is a valid option in the locked native dependency; it changes
    // future allocation policy without invalidating existing allocations.
    unsafe { libmimalloc_sys::mi_option_set(DISALLOW_ARENA_ALLOC, 1) }
}

mod account;
pub mod account_dto;
pub mod adapters;
pub mod agent_dto;
mod artifacts;
pub mod artifacts_dto;
pub mod collection;
mod config;
pub mod config_dto;
mod conversation;
pub mod conversation_dto;
pub mod conversation_source;
pub mod directories;
pub mod dto;
mod file_metadata;
mod host_environment;
mod host_process;
pub mod live;
mod live_index;
mod log_io;
pub mod monitor;
pub(crate) mod observation_versions;
pub(crate) mod operation_association;
mod optimize;
pub mod optimize_dto;
pub mod preferences;
pub mod pricing;
pub mod pricing_sync;
pub mod project_dto;
mod query_cache;
pub mod session_events;
pub(crate) mod session_relations;
pub mod setup_dto;
mod shared_text;
pub mod skill_dto;
mod statistics;
pub mod storage;
pub mod timing;
pub mod timing_dto;
pub mod usage_app;
pub mod usage_app_dto;
pub(crate) mod usage_observations;
pub mod usage_store;
use anyhow::Result;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    env,
    path::{Component, Path, PathBuf},
};
pub fn hash(value: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(value.as_ref()))
}

pub fn home() -> PathBuf {
    std::env::home_dir().unwrap_or_else(|| PathBuf::from("."))
}
pub fn absolute(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        env::current_dir()?.join(path)
    };
    let mut result = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                result.pop();
            }
            Component::CurDir => {}
            part => result.push(part.as_os_str()),
        }
    }
    Ok(result)
}

fn contract_schema<T: schemars::JsonSchema>(version: u32) -> Result<Value> {
    let mut schema = serde_json::to_value(
        schemars::generate::SchemaSettings::draft07()
            .into_generator()
            .into_root_schema_for::<T>(),
    )?;
    schema["$comment"] = serde_json::json!(format!("Wombat request protocol {version}"));
    Ok(schema)
}

pub fn dispatch(op: &str, args: &Value) -> Result<Value> {
    match op {
        "schema_artifacts_request" => {
            contract_schema::<artifacts_dto::Request>(artifacts_dto::REQUEST_VERSION)
        }
        "schema_artifacts_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<artifacts_dto::Response>(),
        )?),
        "schema_monitor_request" => contract_schema::<monitor::Request>(monitor::REQUEST_VERSION),
        "schema_monitor_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<monitor::Response>(),
        )?),
        "schema_agent_error" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<agent_dto::ErrorOutput>(),
        )?),
        "schema_agent_request" => contract_schema::<agent_dto::Request>(agent_dto::REQUEST_VERSION),
        "schema_agent_description" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<agent_dto::Description>(),
        )?),
        "schema_setup_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<setup_dto::Request>(),
        )?),
        "schema_setup_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<setup_dto::Response>(),
        )?),
        "collection" => Ok(serde_json::to_value(collection::dispatch(
            serde_json::from_value(args.clone())?,
        )?)?),
        "collection_ingest" => {
            collection::ingest(args.clone())?;
            Ok(serde_json::json!({"accepted":true}))
        }
        "schema_collection_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<collection::Request>(),
        )?),
        "schema_collection_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<collection::Response>(),
        )?),
        "local_query" => live::dispatch(args.clone()),
        "cleanup_rebuild" => {
            #[derive(serde::Deserialize)]
            #[serde(rename_all = "camelCase", deny_unknown_fields)]
            struct Cleanup {
                request_id: String,
            }
            let request: Cleanup = serde_json::from_value(args.clone()).map_err(|_| {
                dto::operation_error("INVALID_ARGUMENT", "Invalid temporary cleanup request")
            })?;
            Ok(serde_json::json!({"removed":live::cleanup_rebuild(&request.request_id)?}))
        }
        "schema_conversation_request" => {
            contract_schema::<conversation_dto::Request>(conversation_dto::REQUEST_VERSION)
        }
        "schema_conversation_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<conversation_dto::Response>(),
        )?),
        "schema_timing_request" => {
            contract_schema::<timing_dto::Request>(timing_dto::REQUEST_VERSION)
        }
        "schema_timing_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<timing_dto::Response>(),
        )?),
        "schema_timing_local_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<timing_dto::LocalResponse>(),
        )?),
        "schema_timing_error_output" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<timing_dto::TimingErrorOutput>(),
        )?),
        "schema_timing_share_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<timing_dto::ShareResponse>(),
        )?),
        "schema_skill_discovery" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<skill_dto::Discovery>(),
        )?),
        "native_hook_context" => Ok(serde_json::to_value(config::hooks::context(
            serde_json::from_value(args.clone())?,
        )?)?),
        "native_account" => Ok(serde_json::to_value(account::capture(
            serde_json::from_value(args.clone())?,
        )?)?),
        "account_history" => Ok(serde_json::to_value(account::history::read(
            serde_json::from_value(args.clone())?,
        )?)?),
        "schema_account_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<account_dto::Request>(),
        )?),
        "schema_account_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<account_dto::Response>(),
        )?),
        "directories" => Ok(serde_json::to_value(directories::dispatch(
            serde_json::from_value(args.clone())?,
        )?)?),
        "schema_directories_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<directories::Request>(),
        )?),
        "schema_directories_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<directories::Response>(),
        )?),
        "schema_analysis_declaration" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<optimize_dto::AnalysisDeclaration>(),
        )?),
        "preferences" => Ok(serde_json::to_value(preferences::dispatch(
            serde_json::from_value(args.clone())?,
        )?)?),
        "schema_preferences_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<preferences::Request>(),
        )?),
        "schema_preferences_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<preferences::Response>(),
        )?),
        "schema_cli_history_read" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<agent_dto::HistoryRead>(),
        )?),
        "schema_cli_help" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<agent_dto::Help>(),
        )?),
        "schema_project_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<project_dto::Request>(),
        )?),
        "schema_project_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<project_dto::Response>(),
        )?),
        "schema_optimize_request" => {
            contract_schema::<optimize_dto::Request>(optimize_dto::REQUEST_VERSION)
        }
        "schema_optimize_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<optimize_dto::Response>(),
        )?),
        "schema_config_request" => {
            contract_schema::<config_dto::Request>(config_dto::REQUEST_VERSION)
        }
        "schema_config_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<config_dto::Response>(),
        )?),
        "schema_live_request" => contract_schema::<live::Request>(live::REQUEST_VERSION),
        "schema_live_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<live::Response>(),
        )?),
        "prices" => pricing_sync::dispatch(args),
        "schema_pricing_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<pricing_sync::Request>(),
        )?),
        "schema_pricing_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<pricing_sync::Response>(),
        )?),
        "schema_usage_app" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<usage_app_dto::Response>(),
        )?),
        "schema_usage_request" => {
            contract_schema::<usage_app_dto::Request>(usage_app_dto::REQUEST_VERSION)
        }
        _ => Err(dto::operation_error(
            "INVALID_ARGUMENT",
            format!("未知内核操作：{op}"),
        )),
    }
}
