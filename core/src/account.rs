//! Parse current native account responses without credentials or legacy quota fallback.
use crate::{account_dto::*, dto::operation_error};
use anyhow::Result;
use chrono::DateTime;
use serde::Deserialize;
use serde_json::Value;
mod allowance;
pub(crate) mod gate;
pub(crate) mod history;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Capture {
    pub action: Action,
    pub native_version: Option<String>,
    pub checked_at: String,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub rates: Option<Value>,
    pub activity: Option<Value>,
    pub account_error: Option<String>,
    pub rates_error: Option<String>,
    pub activity_error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeAccountResponse {
    #[serde(rename = "requiresOpenaiAuth")]
    _requires_openai_auth: bool,
    account: Option<NativeAccount>,
    workspace_routing: Option<Routing>,
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum NativeAccount {
    ApiKey,
    Chatgpt {
        email: Option<String>,
        #[serde(rename = "planType")]
        plan: String,
    },
    AmazonBedrock,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Routing {
    chatgpt_account_id: String,
}
#[derive(Deserialize)]
struct Activity {
    summary: NativeSummary,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeSummary {
    lifetime_tokens: Option<u64>,
    current_streak_days: Option<u64>,
    longest_streak_days: Option<u64>,
    peak_daily_tokens: Option<u64>,
    longest_running_turn_sec: Option<u64>,
}

fn section(status: &str, checked: Option<&str>, code: Option<&str>) -> Section {
    Section {
        status: status.into(),
        checked_at: checked.map(str::to_owned),
        error_code: code.map(str::to_owned),
    }
}
fn safe_text(s: &str) -> Option<String> {
    (!s.is_empty() && s.len() <= 512 && !s.chars().any(char::is_control)).then(|| s.to_owned())
}
fn safe_integer(value: Option<u64>) -> Option<u64> {
    value.filter(|v| *v <= 9_007_199_254_740_991)
}
fn identity(account: &NativeAccountResponse) -> Option<Identity> {
    let (kind, email, plan) = match account.account.as_ref()? {
        NativeAccount::ApiKey => ("api_key", None, None),
        NativeAccount::AmazonBedrock => ("amazon_bedrock", None, None),
        NativeAccount::Chatgpt { email, plan } => ("chatgpt", email.as_deref(), safe_text(plan)),
    };
    let source_id = account
        .workspace_routing
        .as_ref()
        .map(|r| r.chatgpt_account_id.as_str());
    let id = crate::hash(serde_json::to_vec(&(kind, source_id, email, &plan)).ok()?);
    let masked_email = email.and_then(|e| {
        let e = safe_text(e)?;
        let (name, domain) = e.rsplit_once('@')?;
        if name.is_empty() || domain.is_empty() || name.contains('@') {
            return None;
        }
        Some(format!(
            "{}***@{}",
            name.chars().take(2).collect::<String>(),
            domain
        ))
    });
    Some(Identity {
        id,
        kind: kind.into(),
        masked_email,
        plan,
    })
}

pub(crate) fn normalize(c: Capture) -> Result<Response> {
    let observed = DateTime::parse_from_rfc3339(&c.checked_at)
        .map_err(|_| operation_error("INVALID_ARGUMENT", "Invalid account observation time"))?;
    let mut out = Response {
        history: None,
        history_error_code: None,
        output_version: 1,
        action: c.action,
        native_version: c.native_version,
        account: section(
            "unavailable",
            None,
            c.account_error.as_deref().or(Some("ACCOUNT_UNAVAILABLE")),
        ),
        allowance: section(
            "unavailable",
            None,
            c.rates_error
                .as_deref()
                .or(c.account_error.as_deref())
                .or(Some("ALLOWANCE_UNAVAILABLE")),
        ),
        activity: section(
            "unavailable",
            None,
            c.activity_error
                .as_deref()
                .or(c.account_error.as_deref())
                .or(Some("ACTIVITY_UNAVAILABLE")),
        ),
        identity: None,
        windows: vec![],
        buckets: vec![],
        reset_credits: None,
        ordinary_usage_allowed: None,
        model_restriction: None,
        summary: None,
    };
    let accounts = c.before.zip(c.after).and_then(|(a, b)| {
        Some((
            serde_json::from_value::<NativeAccountResponse>(a).ok()?,
            serde_json::from_value::<NativeAccountResponse>(b).ok()?,
        ))
    });
    let Some((before, after)) = accounts else {
        return Ok(out);
    };
    let before_id = identity(&before);
    let after_id = identity(&after);
    if before_id.as_ref().map(|i| &i.id) != after_id.as_ref().map(|i| &i.id) {
        out.account.error_code = Some("ACCOUNT_CHANGED".into());
        return Ok(out);
    }
    out.identity = after_id;
    out.account = section(
        if out.identity.is_some() {
            "available"
        } else {
            "signed_out"
        },
        Some(&c.checked_at),
        None,
    );
    if out.identity.as_ref().is_none_or(|i| i.kind != "chatgpt") {
        out.allowance = section("unsupported", None, Some("SUBSCRIPTION_ACCOUNT_REQUIRED"));
        out.activity = section("unsupported", None, Some("SUBSCRIPTION_ACCOUNT_REQUIRED"));
        return Ok(out);
    }
    if after.workspace_routing.is_none()
        && !matches!(&after.account, Some(NativeAccount::Chatgpt { email: Some(email), .. }) if safe_text(email).is_some())
    {
        out.allowance.error_code = Some("ACCOUNT_IDENTITY_UNAVAILABLE".into());
        out.activity.error_code = Some("ACCOUNT_IDENTITY_UNAVAILABLE".into());
        return Ok(out);
    }
    if let Some(rates) = c.rates {
        allowance::read(
            &mut out,
            rates,
            after
                .workspace_routing
                .as_ref()
                .map(|r| r.chatgpt_account_id.as_str()),
            &c.checked_at,
            observed.timestamp(),
        );
    }
    if let Some(activity) = c.activity {
        match serde_json::from_value::<Activity>(activity) {
            Ok(activity) => {
                let raw = activity.summary;
                let values = [
                    raw.lifetime_tokens,
                    raw.current_streak_days,
                    raw.longest_streak_days,
                    raw.peak_daily_tokens,
                    raw.longest_running_turn_sec,
                ];
                let safe = values.map(safe_integer);
                let complete = values == safe && safe.iter().any(Option::is_some);
                out.summary = Some(Summary {
                    lifetime_tokens: safe[0],
                    current_streak_days: safe[1],
                    longest_streak_days: safe[2],
                    peak_daily_tokens: safe[3],
                    longest_running_turn_seconds: safe[4],
                });
                out.activity = section(
                    if complete { "available" } else { "partial" },
                    Some(&c.checked_at),
                    (!complete).then_some("NATIVE_DATA_INCOMPLETE"),
                );
            }
            Err(_) => out.activity.error_code = Some("NATIVE_PROTOCOL_ERROR".into()),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;

pub(crate) fn capture(c: Capture) -> Result<Response> {
    if c.action == Action::History {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "History never captures native account data",
        ));
    }
    let at = c.checked_at.clone();
    let mut out = normalize(c)?;
    if let Err(error) =
        crate::storage::data_home().and_then(|root| history::record(&root, &out, &at))
    {
        out.history_error_code = Some(
            if error
                .downcast_ref::<crate::dto::OperationError>()
                .is_some_and(|e| e.code == "UNSUPPORTED_VERSION")
            {
                "UNSUPPORTED_VERSION"
            } else {
                "HISTORY_UNAVAILABLE"
            }
            .into(),
        );
    }
    Ok(out)
}
