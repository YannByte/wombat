//! Official Markdown ingestion is deliberately limited to standard text/Codex
//! token tables. Host transport fetches the fixed URL; Rust owns interpretation,
//! validation, publication and the catalog used by subsequent offline refreshes.
use crate::{
    dto::operation_error,
    pricing::{self, Catalog, LongContext, ModelPrice, Rates},
};
use anyhow::Result;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io::Read, path::Path};

mod automatic;
use automatic::automatic_at;
pub use automatic::{Automatic, AutomaticStatus};

pub const SOURCE: &str = "https://developers.openai.com/api/docs/pricing.md";
pub const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
const LONG_NOTE: &str = "Short context: ≤272K input tokens. Long context: >272K input tokens.";
const HEADER: &str = "| Model | Short context input | Short context cached input | Short context cache writes | Short context output | Long context input | Long context cached input | Long context cache writes | Long context output |";
const CODEX_HEADER: &str = "| Category | Model | Input | Cached input | Output |";

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Status,
    Update,
    AutoUpdate,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub action: Action,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub automatic: Option<Automatic>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub download_required: bool,
    pub output_version: u32,
    pub action: Action,
    pub origin: String,
    pub updated: bool,
    pub source: String,
    pub source_hash: Option<String>,
    pub catalog_hash: String,
    pub catalog: Catalog,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Stored {
    fetched_at: String,
    document: String,
    source_hash: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HostRequest {
    action: Action,
    document: Option<String>,
    attempt_id: Option<String>,
    error_code: Option<String>,
}

fn changed(message: &str) -> anyhow::Error {
    operation_error(
        "PRICE_SOURCE_CHANGED",
        format!("官方价表格式无法确认：{message}；保留现有价表"),
    )
}
fn rate(text: &str) -> Result<Option<String>> {
    if text == "-" {
        return Ok(None);
    }
    let value = text
        .strip_prefix('$')
        .ok_or_else(|| changed("价格单位不是美元"))?;
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return Err(changed("无效价格"));
    }
    let amount = Decimal::from_str_exact(value).map_err(|_| changed("价格精度无效"))?;
    if amount < Decimal::ZERO || amount > Decimal::from(1_000_000) || amount.scale() > 9 {
        return Err(changed("价格超出支持范围"));
    }
    Ok(Some(amount.normalize().to_string()))
}
fn rates(cells: &[&str]) -> Result<Rates> {
    Ok(Rates {
        input: rate(cells[0])?,
        cache_read: rate(cells[1])?,
        cache_create: rate(cells[2])?,
        output: rate(cells[3])?,
    })
}
fn table<'a>(
    section: &'a str,
    heading: &str,
    header: &str,
    columns: usize,
) -> Result<Vec<Vec<&'a str>>> {
    let (_, tail) = section
        .split_once(heading)
        .ok_or_else(|| changed("缺少标准价格表"))?;
    let mut lines = tail.lines().map(str::trim).skip_while(|s| s.is_empty());
    if lines.next() != Some(header) {
        return Err(changed("表头发生变化"));
    }
    let separator = lines.next().ok_or_else(|| changed("表格不完整"))?;
    if separator
        .split('|')
        .filter(|s| !s.trim().is_empty())
        .count()
        != columns
        || !separator.chars().all(|c| "|-: ".contains(c))
    {
        return Err(changed("表格分隔无效"));
    }
    let mut rows = Vec::new();
    for line in lines {
        if !line.starts_with('|') {
            break;
        }
        let cells: Vec<_> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells.len() != columns || rows.len() >= 500 {
            return Err(changed("列数或模型数量异常"));
        }
        rows.push(cells);
    }
    if rows.is_empty() {
        return Err(changed("价格表为空"));
    }
    Ok(rows)
}
fn model(id: &str, rates: Rates, long_context: Option<LongContext>) -> Result<ModelPrice> {
    if id.is_empty()
        || id.len() > 100
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
    {
        return Err(changed("模型名称无法识别"));
    }
    if rates.input.is_none() || rates.output.is_none() {
        return Err(changed("缺少输入或输出单价"));
    }
    Ok(ModelPrice {
        id: id.into(),
        // Only aliases already verified in the bundled catalog are inherited.
        aliases: pricing::catalog()
            .models
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.aliases.clone())
            .unwrap_or_default(),
        source: SOURCE.into(),
        rates,
        long_context,
    })
}
pub(crate) fn parse(document: &str, fetched_at: &str) -> Result<Catalog> {
    if document.len() > MAX_DOCUMENT_BYTES || !document.starts_with("# Pricing\n") {
        return Err(changed("文档大小或标题无效"));
    }
    let (main, _) = document
        .split_once("Cyber models")
        .ok_or_else(|| changed("缺少模型分区"))?;
    if !main.contains("Prices per 1M tokens.") || !main.contains(LONG_NOTE) {
        return Err(changed("计价单位或长上下文阈值发生变化"));
    }
    let rows = table(main, "### Standard pricing data", HEADER, 9)?;
    let mut models = Vec::new();
    for cells in rows {
        let id = cells[0]
            .strip_suffix(" (<272K context length)")
            .unwrap_or(cells[0]);
        let base = rates(&cells[1..5])?;
        let long = rates(&cells[5..9])?;
        let long_context = if cells[5..9].iter().all(|s| *s == "-") {
            None
        } else {
            if long.input.is_none() || long.output.is_none() {
                return Err(changed("长上下文价格不完整"));
            }
            Some(LongContext {
                input_above: 272_000,
                rates: long,
            })
        };
        models.push(model(id, base, long_context)?);
    }
    let (_, specialized) = document
        .split_once("Specialized models")
        .ok_or_else(|| changed("缺少专用模型分区"))?;
    let before_table = specialized.split("###").next().unwrap_or_default();
    if !before_table.lines().any(|s| s.trim() == "Standard")
        || !before_table.contains("Prices per 1M tokens.")
    {
        return Err(changed("专用模型不是标准价格"));
    }
    for cells in table(
        specialized,
        "### Grouped Pricing Table data",
        CODEX_HEADER,
        5,
    )? {
        // Multimodal, future billing starts, embedding and tools need their own policies.
        if cells[0] == "Codex" {
            models.push(model(
                cells[1],
                Rates {
                    input: rate(cells[2])?,
                    cache_read: rate(cells[3])?,
                    cache_create: None,
                    output: rate(cells[4])?,
                },
                None,
            )?);
        }
    }
    let mut ids = BTreeSet::new();
    for m in &models {
        if !ids.insert(m.id.clone()) {
            return Err(changed("模型重复"));
        }
    }
    // An incomplete/retired section must not silently discard previously supported models.
    if pricing::catalog()
        .models
        .iter()
        .any(|m| !ids.contains(&m.id))
    {
        return Err(changed("缺少内置价表中的模型"));
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Catalog {
        revision: format!("openai-standard-live-v1-{}", &crate::hash(document)[..16]),
        verified_at: fetched_at.into(),
        policy: pricing::PRICE_POLICY.into(),
        currency: "USD".into(),
        models,
    })
}
fn response(stored: Option<&Stored>, action: Action, updated: bool) -> Result<Response> {
    let catalog = match stored {
        Some(s) => {
            if s.source_hash != crate::hash(&s.document)
                || chrono::DateTime::parse_from_rfc3339(&s.fetched_at).is_err()
            {
                return Err(changed("缓存校验失败"));
            }
            parse(&s.document, &s.fetched_at)?
        }
        None => pricing::catalog().clone(),
    };
    let catalog_hash = if stored.is_none() {
        pricing::catalog_info().hash
    } else {
        crate::hash(serde_json::to_vec(&catalog)?)
    };
    Ok(Response {
        automatic: None,
        download_required: false,
        output_version: 1,
        action,
        origin: if stored.is_some() {
            "downloaded"
        } else {
            "bundled"
        }
        .into(),
        updated,
        source: SOURCE.into(),
        source_hash: stored.map(|s| s.source_hash.clone()),
        catalog_hash,
        catalog,
    })
}
fn read_cache(root: &Path) -> Result<Option<Stored>> {
    let file = match fs::File::open(root.join("active.json")) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    file.take((MAX_DOCUMENT_BYTES * 3 + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_DOCUMENT_BYTES * 3 {
        return Err(operation_error("PRICE_CACHE_INVALID", "价表缓存过大"));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| operation_error("PRICE_CACHE_INVALID", "价表缓存损坏，请重新更新价格"))
}
pub(crate) fn current_at(root: &Path) -> Result<Response> {
    response(read_cache(root)?.as_ref(), Action::Status, false)
        .map_err(|e| operation_error("PRICE_CACHE_INVALID", e.to_string()))
}
pub fn current() -> Result<Response> {
    current_at(&crate::storage::data_home()?.join("prices"))
}
fn update_at(root: &Path, document: String) -> Result<Response> {
    let stored = Stored {
        fetched_at: chrono::Utc::now().to_rfc3339(),
        source_hash: crate::hash(&document),
        document,
    };
    let result = response(Some(&stored), Action::Update, true)?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(root)?;
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(root.join("update.lock"))?;
    lock.try_lock()
        .map_err(|_| operation_error("UPDATE_BUSY", "价表正在更新"))?;
    if let Ok(previous) = current_at(root)
        && previous.source_hash.as_ref() == Some(&stored.source_hash)
    {
        return Ok(Response {
            action: Action::Update,
            updated: false,
            ..previous
        });
    }
    let bytes = serde_json::to_vec(&stored)?;
    crate::storage::atomic_write(&root.join(format!("{}.json", stored.source_hash)), &bytes)?;
    crate::storage::atomic_write(&root.join("active.json"), &bytes)?;
    Ok(result)
}
pub fn dispatch(args: &serde_json::Value) -> Result<serde_json::Value> {
    let request: HostRequest = serde_json::from_value(args.clone())
        .map_err(|_| operation_error("INVALID_ARGUMENT", "无效价表请求"))?;
    let root = crate::storage::data_home()?.join("prices");
    if request.action == Action::AutoUpdate {
        return Ok(serde_json::to_value(automatic_at(
            &root,
            request,
            chrono::Utc::now().timestamp_millis(),
        )?)?);
    }
    if request.attempt_id.is_some() || request.error_code.is_some() {
        return Err(operation_error("INVALID_ARGUMENT", "无效自动价表请求"));
    }
    let result = match (request.action, request.document) {
        (Action::Status, None) => current()?,
        (Action::Update, Some(document)) => {
            update_at(&crate::storage::data_home()?.join("prices"), document)?
        }
        _ => {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "价表更新需要官方文档；查询不接受文档",
            ));
        }
    };
    Ok(serde_json::to_value(result)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::contract::{ModelRef, TokenUsage};
    const SAMPLE: &str = include_str!("../tests/fixtures/official-pricing-synthetic.txt");
    #[test]
    fn official_tables_select_standard_rates_and_preserve_unknowns() {
        let catalog = parse(SAMPLE, "2026-09-30T00:00:00Z").unwrap();
        assert_eq!(catalog.models.len(), 10);
        let m = ModelRef {
            raw: Some("gpt-6-sol".into()),
            ..Default::default()
        };
        let tokens = TokenUsage {
            input: Some(100_000),
            cache_read: Some(50_000),
            cache_create: Some(10_000),
            output: Some(20_000),
            raw_input: Some(160_000),
            total: Some(180_000),
            reasoning: Some(0),
        };
        let priced = pricing::price_with_catalog(
            &m,
            &tokens,
            &pricing::PricingContext {
                request_scoped: true,
                model_conflicted: false,
            },
            &catalog,
            "synthetic",
        );
        assert_eq!(priced.cost.as_deref(), Some("0.4125"));
        assert_eq!(priced.price_revision.as_ref(), catalog.revision);
        assert_eq!(priced.basis[0].source.as_ref(), SOURCE);
        let long = TokenUsage {
            input: Some(230_000),
            raw_input: Some(290_000),
            total: Some(310_000),
            ..tokens.clone()
        };
        assert_eq!(
            pricing::price_with_catalog(
                &m,
                &long,
                &pricing::PricingContext {
                    request_scoped: true,
                    model_conflicted: false,
                },
                &catalog,
                "synthetic"
            )
            .cost
            .as_deref(),
            Some("1.245")
        );
        let mini = catalog
            .models
            .iter()
            .find(|m| m.id == "gpt-5.4-mini")
            .unwrap();
        assert!(mini.rates.cache_create.is_none());
        assert_eq!(mini.aliases, ["gpt-5.4-mini-2026-03-17"]);
        let sol = catalog
            .models
            .iter()
            .find(|m| m.id == "gpt-6.1-sol")
            .unwrap();
        assert_eq!(sol.rates.cache_read.as_deref(), Some("0.1"));
        assert_eq!(
            sol.long_context
                .as_ref()
                .unwrap()
                .rates
                .cache_read
                .as_deref(),
            Some("0.2")
        );
        assert!(sol.aliases.is_empty());
        assert!(
            !catalog
                .models
                .iter()
                .any(|m| m.id == "excluded-future-billing" || m.id == "codex-auto-review")
        );
    }
    #[test]
    fn format_drift_partial_tables_and_bad_prices_fail_closed() {
        for document in [
            SAMPLE.replace("### Standard pricing data", "### Batch pricing data"),
            SAMPLE.replace("Short context input", "Input"),
            SAMPLE.replace(LONG_NOTE, "Short context: ≤300K input tokens."),
            SAMPLE.replace("$1.23", "$-1"),
            SAMPLE.replace("$1.23", "$1e3"),
            SAMPLE.replace("$1.23", "$1000001"),
            SAMPLE.replace("$1.23", "Free"),
            SAMPLE.replace("gpt-6-sol |", "new-id |"),
            SAMPLE.replace("gpt-6.1-sol |", "new-sol |"),
            SAMPLE.replace("synthetic-new |", "gpt-6-sol |"),
            SAMPLE.replace("$4 | $0.4 | $0.5 | $15", "- | $0.4 | $0.5 | $15"),
            SAMPLE.replace(
                "### Standard pricing data\n\n",
                "### Standard pricing data\nmissing\n",
            ),
        ] {
            assert!(parse(&document, "2026-09-30").is_err());
        }
    }
    #[test]
    fn publish_is_versioned_idempotent_and_failed_update_preserves_active() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(current_at(root.path()).unwrap().origin, "bundled");
        let first = update_at(root.path(), SAMPLE.into()).unwrap();
        assert!(first.updated);
        assert!(!update_at(root.path(), SAMPLE.into()).unwrap().updated);
        let bytes = fs::read(root.path().join("active.json")).unwrap();
        assert!(update_at(root.path(), "# Pricing\nInvalid".into()).is_err());
        assert_eq!(fs::read(root.path().join("active.json")).unwrap(), bytes);
        let second = update_at(root.path(), SAMPLE.replace("$1.23", "$2.34")).unwrap();
        assert_ne!(first.catalog.revision, second.catalog.revision);
        assert!(
            root.path()
                .join(format!("{}.json", first.source_hash.unwrap()))
                .exists()
        );
        assert_eq!(
            current_at(root.path()).unwrap().catalog_hash,
            second.catalog_hash
        );
        fs::write(root.path().join("active.json"), "{}").unwrap();
        assert!(current_at(root.path()).is_err());
        assert!(update_at(root.path(), SAMPLE.into()).is_ok());
    }
}
