use super::*;
use serde_json::json;
fn capture(rates: Option<Value>, activity: Option<Value>) -> Capture {
    let account = json!({"requiresOpenaiAuth":true,"account":{"type":"chatgpt","email":"synthetic@example.invalid","planType":"pro"},"workspaceRouting":{"chatgptAccountId":"account-a"}});
    Capture {
        action: Action::Read,
        native_version: Some("0.160.0".into()),
        checked_at: "2026-10-03T00:00:00Z".into(),
        before: Some(account.clone()),
        after: Some(account),
        rates,
        activity,
        account_error: None,
        rates_error: None,
        activity_error: None,
    }
}
#[test]
fn actual_windows_and_activity_have_no_fixed_durations() {
    let out = normalize(capture(Some(json!({"accountId":"account-a","ordinaryUsageAllowed":true,"rateLimitsByLimitId":{"ordinary":{"limitName":"Synthetic","primary":{"usedPercent":23,"windowDurationMins":17,"resetsAt":1790985600}},"special":{"normalModelSlug":"synthetic-model","secondary":{"usedPercent":100,"windowDurationMins":61}}}})), Some(json!({"summary":{"lifetimeTokens":0,"currentStreakDays":2,"longestStreakDays":7,"peakDailyTokens":42,"longestRunningTurnSec":123}})))).unwrap();
    assert_eq!(out.windows.len(), 2);
    assert_eq!(out.windows[0].duration_minutes, Some(17));
    assert_eq!(out.windows[0].status, "reset_due");
    assert_eq!(
        out.windows[0].used_percent, 23.0,
        "a past reset does not restore allowance"
    );
    assert_eq!(out.windows[1].duration_minutes, Some(61));
    assert_eq!(
        out.ordinary_usage_allowed,
        Some(true),
        "special-bucket exhaustion is independent"
    );
    assert_eq!(out.summary.as_ref().unwrap().lifetime_tokens, Some(0));
    assert_eq!(out.summary.unwrap().longest_running_turn_seconds, Some(123));
    let serialized = serde_json::to_string(&out.identity).unwrap();
    assert!(serialized.contains("sy***@example.invalid"));
    assert!(!serialized.contains("synthetic@example.invalid"));
}
#[test]
fn legacy_single_bucket_is_never_used_as_fallback() {
    let out = normalize(capture(
        Some(json!({"rateLimits":{"primary":{"usedPercent":0,"windowDurationMins":300}}})),
        None,
    ))
    .unwrap();
    assert!(out.windows.is_empty());
    assert_eq!(
        out.allowance.error_code.as_deref(),
        Some("WINDOWS_NOT_PROVIDED")
    );
    assert!(out.ordinary_usage_allowed.is_none());
}
#[test]
fn account_switch_drops_all_late_usage() {
    let mut c = capture(
        Some(json!({"rateLimitsByLimitId":{}})),
        Some(json!({"summary":{"lifetimeTokens":999}})),
    );
    c.after.as_mut().unwrap()["workspaceRouting"]["chatgptAccountId"] = json!("account-b");
    let out = normalize(c).unwrap();
    assert_eq!(out.account.error_code.as_deref(), Some("ACCOUNT_CHANGED"));
    assert!(out.identity.is_none() && out.windows.is_empty() && out.summary.is_none());
}
#[test]
fn foreign_bucket_does_not_block_independent_activity() {
    let out = normalize(capture(
        Some(json!({"accountId":"account-b","rateLimitsByLimitId":{}})),
        Some(json!({"summary":{"currentStreakDays":3}})),
    ))
    .unwrap();
    assert_eq!(out.allowance.error_code.as_deref(), Some("ACCOUNT_CHANGED"));
    assert_eq!(out.activity.status, "available");
    assert_eq!(out.summary.unwrap().current_streak_days, Some(3));
}
#[test]
fn failed_sections_never_become_zero_or_success() {
    let mut c = capture(None, None);
    c.rates_error = Some("CODEX_TIMEOUT".into());
    c.activity_error = Some("CODEX_REQUEST_REJECTED".into());
    let out = normalize(c).unwrap();
    assert_eq!(out.account.status, "available");
    assert_eq!(out.allowance.error_code.as_deref(), Some("CODEX_TIMEOUT"));
    assert_eq!(
        out.activity.error_code.as_deref(),
        Some("CODEX_REQUEST_REJECTED")
    );
    assert!(out.windows.is_empty() && out.summary.is_none());
}

#[test]
fn connection_denial_is_visible_in_each_unavailable_section() {
    let mut c = capture(None, None);
    c.before = None;
    c.after = None;
    c.account_error = Some("LOCAL_ACCESS_DENIED".into());
    let out = normalize(c).unwrap();
    for section in [&out.account, &out.allowance, &out.activity] {
        assert_eq!(section.status, "unavailable");
        assert_eq!(section.error_code.as_deref(), Some("LOCAL_ACCESS_DENIED"));
    }
    assert!(out.identity.is_none() && out.windows.is_empty() && out.summary.is_none());
}
#[test]
fn malformed_account_and_invalid_percentages_are_visible() {
    let mut c = capture(None, None);
    c.before = Some(json!({}));
    assert_eq!(normalize(c).unwrap().account.status, "unavailable");
    let out = normalize(capture(Some(json!({"rateLimitsByLimitId":{"invalid":{"primary":{"usedPercent":-1}},"valid":{"secondary":{"usedPercent":0}}}})), None)).unwrap();
    assert_eq!(out.allowance.status, "partial");
    assert_eq!(out.windows.len(), 1);
    assert_eq!(out.windows[0].used_percent, 0.0);
}
#[test]
fn signed_out_and_unidentifiable_accounts_cannot_reuse_usage() {
    let mut c = capture(None, None);
    c.before = Some(json!({"requiresOpenaiAuth":true,"account":null}));
    c.after = c.before.clone();
    let out = normalize(c).unwrap();
    assert_eq!(out.account.status, "signed_out");
    assert!(out.identity.is_none());
    let mut c = capture(Some(json!({"rateLimitsByLimitId":{}})), None);
    c.before = Some(
        json!({"requiresOpenaiAuth":true,"account":{"type":"chatgpt","email":null,"planType":"pro"}}),
    );
    c.after = c.before.clone();
    assert_eq!(
        normalize(c).unwrap().allowance.error_code.as_deref(),
        Some("ACCOUNT_IDENTITY_UNAVAILABLE")
    );
}
#[test]
fn invalid_timestamps_and_unsafe_integers_remain_unknown() {
    let out = normalize(capture(Some(json!({"rateLimitsByLimitId":{"invalid":{"primary":{"usedPercent":20,"windowDurationMins":9007199254740992_u64,"resetsAt":9223372036854775807_i64}}}})), Some(json!({"summary":{"lifetimeTokens":9007199254740992_u64,"currentStreakDays":0}})))).unwrap();
    assert_eq!(out.allowance.status, "partial");
    assert_eq!(out.windows[0].duration_minutes, None);
    assert_eq!(out.windows[0].resets_at, None);
    assert_eq!(out.activity.status, "partial");
    assert_eq!(out.summary.as_ref().unwrap().lifetime_tokens, None);
    assert_eq!(out.summary.unwrap().current_streak_days, Some(0));
    let empty = normalize(capture(None, Some(json!({"summary":{}})))).unwrap();
    assert_eq!(empty.activity.status, "partial");
}

#[test]
fn native_balances_spend_and_reset_credits_keep_units_counts_and_zero() {
    let out = normalize(capture(Some(json!({"accountId":"account-a","rateLimitsByLimitId":{"special":{
        "normalModelSlug":"synthetic-model", "credits":{"balance":"0.0000000000000000001","hasCredits":true,"unlimited":false},
        "individualLimit":{"limit":"100.000","used":"0","remainingPercent":100,"resetsAt":1790985600},
        "spendControlReached":false,"rateLimitReachedType":"workspace_member_usage_limit_reached"
    }},"rateLimitResetCredits":{"availableCount":7,"credits":[{"id":"credit-a","title":"Synthetic credit","grantedAt":1790985600,"expiresAt":1790985601,"status":"available","resetType":"codexRateLimits"}]}})),None)).unwrap();
    assert_eq!(out.allowance.status, "available");
    assert!(out.windows.is_empty());
    let bucket = &out.buckets[0];
    assert_eq!(
        bucket.credits.as_ref().unwrap().balance.as_deref(),
        Some("0.0000000000000000001")
    );
    assert_eq!(bucket.credits.as_ref().unwrap().unlimited, Some(false));
    let spend = bucket.individual_limit.as_ref().unwrap();
    assert_eq!(spend.limit.as_deref(), Some("100.000"));
    assert_eq!(spend.used.as_deref(), Some("0"));
    assert_eq!(spend.status, "reset_due");
    assert_eq!(bucket.spend_control_reached, Some(false));
    let reset = out.reset_credits.unwrap();
    assert_eq!(
        reset.available_count,
        Some(7),
        "capped details cannot replace the count"
    );
    assert_eq!(
        reset.credits.unwrap()[0].status,
        "available",
        "native state is preserved, not redeemed by Wombat"
    );
}

#[test]
fn malformed_dimensions_preserve_siblings_and_missing_reset_details() {
    let out = normalize(capture(Some(json!({"rateLimitsByLimitId":{"bad":{
        "primary":{"usedPercent":"invalid"},"secondary":{"usedPercent":0},
        "credits":{"balance":"NaN","hasCredits":false,"unlimited":false},
        "individualLimit":{"limit":"5","used":"invalid","remainingPercent":101,"resetsAt":9223372036854775807_i64}
    },"ok":{"primary":{"usedPercent":100}}},"rateLimitResetCredits":{"availableCount":0,"credits":null}})),None)).unwrap();
    assert_eq!(out.allowance.status, "partial");
    assert_eq!(out.windows.len(), 2);
    assert_eq!(out.windows[0].used_percent, 0.0);
    let bucket = &out.buckets[0];
    assert_eq!(bucket.credits.as_ref().unwrap().balance, None);
    assert_eq!(bucket.credits.as_ref().unwrap().has_credits, Some(false));
    assert_eq!(
        bucket.individual_limit.as_ref().unwrap().limit.as_deref(),
        Some("5")
    );
    assert_eq!(bucket.individual_limit.as_ref().unwrap().used, None);
    assert_eq!(
        bucket.individual_limit.as_ref().unwrap().remaining_percent,
        None
    );
    assert_eq!(bucket.individual_limit.as_ref().unwrap().resets_at, None);
    let reset = out.reset_credits.unwrap();
    assert_eq!(reset.available_count, Some(0));
    assert!(reset.credits.is_none());
    let out=normalize(capture(Some(json!({"rateLimitsByLimitId":{},"rateLimitResetCredits":{"availableCount":0,"credits":[]}})),None)).unwrap();
    assert!(out.reset_credits.unwrap().credits.unwrap().is_empty());
}

#[test]
fn reset_credit_details_are_bounded_and_foreign_accounts_drop_all_dimensions() {
    let rows:Vec<_>=(0..130).map(|n|json!({"id":format!("credit-{n}"),"grantedAt":1790985600,"status":"available","resetType":"codexRateLimits"})).collect();
    let rates = json!({"accountId":"account-a","rateLimitsByLimitId":{"bucket":{"credits":{"balance":"1","hasCredits":true,"unlimited":false}}},"rateLimitResetCredits":{"availableCount":130,"credits":rows}});
    let out = normalize(capture(Some(rates.clone()), None)).unwrap();
    assert_eq!(out.allowance.status, "partial");
    let reset = out.reset_credits.unwrap();
    assert_eq!(reset.available_count, Some(130));
    assert!(reset.details_truncated);
    assert_eq!(reset.credits.unwrap().len(), 128);
    let mut foreign = rates;
    foreign["accountId"] = json!("account-b");
    let out = normalize(capture(Some(foreign), None)).unwrap();
    assert!(out.buckets.is_empty() && out.reset_credits.is_none());
    assert_eq!(out.allowance.error_code.as_deref(), Some("ACCOUNT_CHANGED"));
}

#[test]
fn missing_native_required_fields_are_partial_instead_of_success() {
    let out = normalize(capture(Some(json!({"rateLimitsByLimitId":{"bucket":{"credits":{},"individualLimit":{}}},"rateLimitResetCredits":{"availableCount":1,"credits":[{"id":"credit-a"}]}})), None)).unwrap();
    assert_eq!(out.allowance.status, "partial");
    assert_eq!(out.buckets[0].status, "partial");
    assert_eq!(out.buckets[0].credits.as_ref().unwrap().has_credits, None);
    assert_eq!(
        out.buckets[0]
            .individual_limit
            .as_ref()
            .unwrap()
            .remaining_percent,
        None
    );
    let row = &out
        .reset_credits
        .as_ref()
        .unwrap()
        .credits
        .as_ref()
        .unwrap()[0];
    assert_eq!(row.status, "unknown");
    assert_eq!(row.reset_type, "unknown");
    assert_eq!(row.granted_at, None);
}
#[test]
fn model_restrictions_require_explicit_identity_bound_native_notice() {
    let raw = json!({"accountId":"account-a","rateLimitsByLimitId":{},"rateLimitUpsell":{"banner_type":"selected_model_limit","blocked_model_slug":"synthetic-model","reset_at":1791075600,"title":"untrusted copy","request_url":"https://example.invalid/action","ctas":[{"action":"execute"}]}});
    let out = normalize(capture(Some(raw.clone()), None)).unwrap();
    assert_eq!(
        out.model_restriction.as_ref().unwrap().model,
        "synthetic-model"
    );
    let serialized = serde_json::to_string(&out).unwrap();
    assert!(!serialized.contains("untrusted copy") && !serialized.contains("request_url"));
    for (field, value) in [
        ("banner_type", json!("unknown")),
        ("blocked_model_slug", Value::Null),
        ("blocked_model_slug", json!("bad\nmodel")),
        ("reset_at", json!("invalid")),
    ] {
        let mut raw = raw.clone();
        raw["rateLimitUpsell"][field] = value;
        assert!(
            normalize(capture(Some(raw), None))
                .unwrap()
                .model_restriction
                .is_none(),
            "{field}"
        );
    }
    let mut missing_id = raw.clone();
    missing_id["accountId"] = Value::Null;
    assert!(
        normalize(capture(Some(missing_id), None))
            .unwrap()
            .model_restriction
            .is_none()
    );
    let mut alias = raw;
    alias["rateLimitUpsell"]["blocked_model_slug"] = Value::Null;
    alias["rateLimitUpsell"]["model_slug"] = json!("synthetic-model");
    assert!(
        normalize(capture(Some(alias), None))
            .unwrap()
            .model_restriction
            .is_none()
    );
}
