mod account;
pub mod account_dto;
pub mod adapters;
pub mod agent_dto;
pub mod collection;
mod config;
pub mod config_dto;
mod conversation;
pub mod conversation_dto;
pub mod conversation_source;
pub mod directories;
pub mod dto;
mod file_metadata;
mod handoff;
pub mod handoff_dto;
pub mod live;
mod live_index;
#[cfg(windows)]
mod live_windows;
mod log_io;
pub mod monitor;
pub(crate) mod observation_versions;
pub(crate) mod operation_association;
mod optimize;
pub mod optimize_dto;
pub mod preferences;
pub mod pricing;
pub mod pricing_sync;
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
pub mod web_view_dto;
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

pub fn dispatch(op: &str, args: &Value) -> Result<Value> {
    match op {
        "monitor" => Ok(serde_json::to_value(monitor::dispatch(
            serde_json::from_value(args.clone())
                .map_err(|_| dto::operation_error("INVALID_ARGUMENT", "Invalid monitor request"))?,
        )?)?),
        "schema_monitor_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<monitor::Request>(),
        )?),
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
        "schema_agent_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<agent_dto::Request>(),
        )?),
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
        "conversation" => Ok(serde_json::to_value(conversation::dispatch(
            serde_json::from_value(args.clone()).map_err(|_| {
                dto::operation_error("INVALID_ARGUMENT", "Invalid conversation request")
            })?,
        )?)?),
        "schema_conversation_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<conversation_dto::Request>(),
        )?),
        "schema_conversation_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<conversation_dto::Response>(),
        )?),
        "timing" => Ok(serde_json::to_value(timing::dispatch(
            serde_json::from_value(args.clone())
                .map_err(|_| dto::operation_error("INVALID_ARGUMENT", "Invalid timing request"))?,
        )?)?),
        "schema_timing_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<timing_dto::Request>(),
        )?),
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
        "schema_web_view_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<web_view_dto::Request>(),
        )?),
        "schema_skill_installation" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<skill_dto::Installation>(),
        )?),
        "schema_skill_discovery" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<skill_dto::Discovery>(),
        )?),
        "schema_handoff_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<handoff_dto::Request>(),
        )?),
        "schema_handoff_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<handoff_dto::Response>(),
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
        "native_allowance_gate" => Ok(serde_json::to_value(account::gate::evaluate(
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
        "schema_optimize_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<optimize_dto::Request>(),
        )?),
        "schema_optimize_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<optimize_dto::Response>(),
        )?),
        "schema_config_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<config_dto::Request>(),
        )?),
        "schema_config_response" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<config_dto::Response>(),
        )?),
        "live_endpoint" => live::endpoint(),
        "schema_live_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<live::Request>(),
        )?),
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
        "usage_app" => usage_app::dispatch(args),
        "schema_usage_app" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<usage_app_dto::Response>(),
        )?),
        "schema_usage_request" => Ok(serde_json::to_value(
            schemars::generate::SchemaSettings::draft07()
                .into_generator()
                .into_root_schema_for::<usage_app_dto::Request>(),
        )?),
        _ => Err(dto::operation_error(
            "INVALID_ARGUMENT",
            format!("未知内核操作：{op}"),
        )),
    }
}
