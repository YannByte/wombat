use super::*;

fn model(name: &str) -> ModelRef {
    ModelRef {
        raw: Some(name.into()),
        ..Default::default()
    }
}
fn tokens(input: u64, cache: u64, create: u64, output: u64) -> TokenUsage {
    TokenUsage {
        input: Some(input),
        cache_read: Some(cache),
        cache_create: Some(create),
        output: Some(output),
        total: Some(input + cache + create + output),
        raw_input: Some(input + cache + create),
        reasoning: None,
    }
}

#[test]
fn official_short_request_has_exact_independent_category_amounts() {
    // Independent hand calculation in USD: .175 + .00875 + .14 = .32375.
    let result = price(&model("gpt-5.3-codex"), &tokens(100_000, 50_000, 0, 10_000));
    assert_eq!(result.cost.as_deref(), Some("0.32375"));
    assert_eq!(
        result
            .components
            .iter()
            .map(|c| c.cost.as_deref())
            .collect::<Vec<_>>(),
        [Some("0.175"), Some("0.00875"), Some("0"), Some("0.14")]
    );
    assert_eq!(result.status.as_ref(), "priced");
}

#[test]
fn current_codex_models_use_verified_standard_and_long_context_rates() {
    let sample = tokens(100_000, 50_000, 10_000, 20_000);
    let cases = [
        ("gpt-5.6-sol", "0.87", "2.58"),
        ("gpt-5.6-terra", "0.475", "1.35"),
        ("gpt-6-sol", "0.435", "1.29"),
        ("gpt-6.1-sol", "0.43", "1.28"),
        ("gpt-6-astra", "2.175", "6.45"),
    ];
    for (name, expected_short, expected_long) in cases {
        let short = price(&model(name), &sample);
        assert_eq!(short.cost.as_deref(), Some(expected_short), "{name}");
        let long = price(&model(name), &tokens(230_000, 50_000, 10_000, 20_000));
        assert_eq!(long.status.as_ref(), "priced", "{name}");
        assert_eq!(long.basis[0].condition.as_ref(), "longContext", "{name}");
        assert_eq!(
            long.components[0].rate_per_million.as_deref(),
            Some(match name {
                "gpt-5.6-sol" => "8",
                "gpt-5.6-terra" | "gpt-6-sol" | "gpt-6.1-sol" => "4",
                _ => "20",
            }),
            "{name}"
        );
        assert_eq!(long.cost.as_deref(), Some(expected_long), "{name}");
    }
}

#[test]
fn long_context_boundary_is_strict_and_applies_to_entire_request() {
    let at = price(&model("gpt-5.4"), &tokens(222_000, 50_000, 0, 10_000));
    assert_eq!(at.cost.as_deref(), Some("0.7175"));
    assert_eq!(at.basis[0].condition.as_ref(), "standard");
    let over = price(&model("gpt-5.4"), &tokens(222_001, 50_000, 0, 10_000));
    // 222001*5/1M + 50000*.5/1M + 10000*22.5/1M, not a marginal tier.
    assert_eq!(over.cost.as_deref(), Some("1.360005"));
    assert_eq!(over.basis[0].condition.as_ref(), "longContext");
    assert_eq!(over.basis[0].request_input_tokens, Some(272_001));
}

#[test]
fn sol_61_uses_its_own_cache_rate_and_keeps_unknown_conditions_unknown() {
    let at = price(&model("gpt-6.1-sol"), &tokens(222_000, 50_000, 0, 10_000));
    assert_eq!(at.cost.as_deref(), Some("0.549"));
    assert_eq!(at.basis[0].condition.as_ref(), "standard");
    assert_eq!(at.basis[0].match_method.as_ref(), "exact");
    let over = price(&model("gpt-6.1-sol"), &tokens(222_001, 50_000, 0, 10_000));
    assert_eq!(over.cost.as_deref(), Some("1.048004"));
    assert_eq!(over.basis[0].condition.as_ref(), "longContext");
    let mut unknown_length = tokens(100, 20, 0, 10);
    unknown_length.raw_input = None;
    let unknown = price(&model("gpt-6.1-sol"), &unknown_length);
    assert_eq!(unknown.cost, None);
    assert!(
        unknown
            .issues
            .iter()
            .any(|i| i.as_ref() == "requestContextUnknown")
    );
    for name in [
        "gpt-6.1-sol-2026-09-29",
        "openai/gpt-6.1-sol",
        "gpt-6.1-sol-preview",
    ] {
        assert_eq!(price(&model(name), &tokens(100, 20, 0, 10)).cost, None);
    }
}

#[test]
fn reasoning_is_a_subset_and_never_charged_twice() {
    let mut usage = tokens(10, 20, 0, 30);
    let baseline = price(&model("gpt-5.3-codex"), &usage);
    usage.reasoning = Some(25);
    assert_eq!(price(&model("gpt-5.3-codex"), &usage).cost, baseline.cost);
    usage.reasoning = Some(31);
    assert_eq!(
        price(&model("gpt-5.3-codex"), &usage).status.as_ref(),
        "unknown"
    );
}

#[test]
fn daily_total_is_sum_of_request_prices_not_daily_tier() {
    let record = price(&model("gpt-5.4"), &tokens(3_000, 0, 0, 1_000));
    let daily = sum_prices(std::iter::repeat_n(&record, 100)).unwrap();
    assert_eq!(daily.cost.as_deref(), Some("2.25"));
    assert_eq!(daily.basis.len(), 1);
    assert_eq!(daily.basis[0].request_input_tokens, None);
    let wrong_daily_request = price(&model("gpt-5.4"), &tokens(300_000, 0, 0, 100_000));
    assert_eq!(wrong_daily_request.cost.as_deref(), Some("3.75"));
}

#[test]
fn unscoped_cumulative_difference_does_not_invent_long_context_tier() {
    let result = price_with_context(
        &model("gpt-5.4"),
        &tokens(300_000, 0, 0, 100_000),
        &PricingContext {
            request_scoped: false,
            model_conflicted: false,
        },
    );
    assert_eq!(result.status.as_ref(), "unknown");
    assert!(result.cost.is_none());
    assert!(
        result
            .issues
            .iter()
            .any(|i| i.as_ref() == "requestContextUnknown")
    );
    // A constant-price model can still price an aggregate.
    let result = price_with_context(
        &model("gpt-5.3-codex"),
        &tokens(300_000, 0, 0, 100_000),
        &PricingContext {
            request_scoped: false,
            model_conflicted: false,
        },
    );
    assert_eq!(result.cost.as_deref(), Some("1.925"));
}

#[test]
fn parts_only_requests_do_not_establish_a_tier_but_constant_rates_remain_available() {
    for input in [272_000, 272_001] {
        let mut usage = tokens(input, 0, 0, 10);
        usage.raw_input = None;
        let tiered = price(&model("gpt-5.4"), &usage);
        assert_eq!(tiered.basis[0].request_input_tokens, None);
        assert_eq!(tiered.basis[0].condition.as_ref(), "conditionUnknown");
        assert_eq!(tiered.status.as_ref(), "unknown");
        assert_eq!(tiered.cost, None);
        assert_eq!(tiered.known_cost, "0");
        assert_eq!(tiered.components[1].cost.as_deref(), Some("0"));
        assert_eq!(tiered.components[2].cost.as_deref(), Some("0"));
        assert!(
            tiered
                .issues
                .iter()
                .any(|i| i.as_ref() == "requestContextUnknown")
        );
        assert!(
            !tiered
                .issues
                .iter()
                .any(|i| i.as_ref() == "catalogPriceMissing")
        );
        let constant = price(&model("gpt-5.3-codex"), &usage);
        assert_eq!(constant.status.as_ref(), "priced");
        assert_eq!(
            constant.cost.as_deref(),
            Some(if input == 272_000 {
                "0.47614"
            } else {
                "0.47614175"
            })
        );
    }
}

#[test]
fn zero_categories_remain_zero_without_tier_evidence_and_missing_is_not_zero() {
    let mut zero = tokens(0, 0, 0, 0);
    zero.raw_input = None;
    let priced = price(&model("gpt-5.4"), &zero);
    assert_eq!(priced.status.as_ref(), "priced");
    assert_eq!(priced.cost.as_deref(), Some("0"));
    assert_eq!(priced.basis[0].condition.as_ref(), "conditionUnknown");
    zero.output = None;
    zero.total = None;
    let missing = price(&model("gpt-5.4"), &zero);
    assert_eq!(missing.cost, None);
    assert_eq!(missing.components[0].cost.as_deref(), Some("0"));
    assert_eq!(missing.components[3].tokens, None);
    assert_eq!(missing.components[3].cost, None);
    let native_zero = price(&model("gpt-5.4"), &tokens(0, 0, 0, 0));
    assert_eq!(native_zero.cost.as_deref(), Some("0"));
    assert_eq!(native_zero.basis[0].condition.as_ref(), "standard");
    assert_eq!(native_zero.basis[0].request_input_tokens, Some(0));
    let mut contradictory = tokens(100, 0, 0, 10);
    contradictory.raw_input = Some(0);
    let invalid = price(&model("gpt-5.4"), &contradictory);
    assert_eq!(invalid.status.as_ref(), "unknown");
    assert_eq!(invalid.cost, None);
    assert!(
        invalid
            .issues
            .iter()
            .any(|i| i.as_ref() == "inconsistentTokenCounts")
    );
    assert!(
        invalid
            .components
            .iter()
            .all(|component| component.cost.is_none())
    );
}

#[test]
fn reliable_native_input_prices_known_categories_despite_missing_breakdown() {
    let usage = TokenUsage {
        raw_input: Some(272_001),
        cache_read: Some(50_000),
        output: Some(10_000),
        total: Some(282_001),
        ..Default::default()
    };
    let result = price(&model("gpt-5.4"), &usage);
    assert_eq!(result.basis[0].condition.as_ref(), "longContext");
    assert_eq!(result.basis[0].request_input_tokens, Some(272_001));
    assert_eq!(result.status.as_ref(), "partial");
    assert_eq!(result.cost, None);
    assert_eq!(result.known_cost, "0.25");
    assert_eq!(result.components[0].tokens, None);
    assert_eq!(result.components[0].cost, None);
    assert_eq!(result.components[1].cost.as_deref(), Some("0.025"));
    assert_eq!(result.components[2].tokens, None);
    assert_eq!(result.components[2].cost, None);
    assert_eq!(result.components[3].cost.as_deref(), Some("0.225"));
}

#[test]
fn replay_completed_parts_cannot_reconstruct_conflicting_request_length() {
    use crate::adapters::{
        collect,
        contract::{DiscoveryRequest, RunContext},
    };
    use serde_json::json;
    let first = json!({"type":"event_msg","payload":{
        "type":"token_usage_record","thread_id":"synthetic-thread",
        "response_id":"synthetic-response","usage":{
            "input_tokens":272001,"cache_write_input_tokens":0,
            "output_tokens":10,"total_tokens":272011
        }
    }});
    let second = json!({"type":"event_msg","payload":{
        "type":"token_usage_record","thread_id":"synthetic-thread",
        "response_id":"synthetic-response","usage":{
            "input_tokens":272000,"cached_input_tokens":50000,
            "cache_write_input_tokens":0,"output_tokens":10,"total_tokens":272010
        }
    }});
    for records in [[&first, &second], [&second, &first]] {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir(root.path().join("sessions")).unwrap();
        let metadata = json!({"type":"session_meta","payload":{"id":"synthetic-thread"}});
        std::fs::write(
            root.path().join("sessions/synthetic.jsonl"),
            [&metadata, records[0], records[1]]
                .iter()
                .map(|record| format!("{record}\n"))
                .collect::<String>(),
        )
        .unwrap();
        let collected = collect(
            &DiscoveryRequest {
                roots: vec![root.path().into()],
            },
            &RunContext::default(),
        );
        assert_eq!(collected.measurements.len(), 1);
        assert!(
            collected
                .issues
                .iter()
                .any(|i| i.code == "measurementConflict")
        );
        let fact = &collected.measurements[0];
        assert_eq!(fact.tokens.raw_input, None);
        assert_eq!(fact.tokens.total, None);
        assert_eq!(fact.tokens.input, Some(222_000));
        assert_eq!(fact.tokens.cache_read, Some(50_000));
        assert_eq!(fact.tokens.cache_create, Some(0));
        assert!(fact.request_scoped);
        let result = price_with_context(
            &model("gpt-5.4"),
            &fact.tokens,
            &PricingContext {
                request_scoped: fact.request_scoped,
                model_conflicted: fact.pricing_context_conflict,
            },
        );
        assert_eq!(result.basis[0].condition.as_ref(), "conditionUnknown");
        assert_eq!(result.basis[0].request_input_tokens, None);
        assert_eq!(result.status.as_ref(), "unknown");
        assert_eq!(result.cost, None);
        assert!(
            result
                .components
                .iter()
                .all(|component| component.rate_per_million.is_none())
        );
        assert_eq!(result.components[2].cost.as_deref(), Some("0"));
    }
}

#[test]
fn conflicting_model_context_preserves_counts_but_cannot_select_or_fetch_rates() {
    let usage = tokens(100, 20, 0, 10);
    for name in ["gpt-5.4", "gpt-5.3-codex", "synthetic-new-model"] {
        // A cleared conflicting provider looks absent; the independent conflict
        // observation must prevent treating that absence as permission to price.
        let original = model(name);
        let result = price_with_context(
            &original,
            &usage,
            &PricingContext {
                request_scoped: true,
                model_conflicted: true,
            },
        );
        assert_eq!(original.raw.as_deref(), Some(name));
        assert_eq!(original.provider, None);
        assert_eq!(result.status.as_ref(), "unknown");
        assert_eq!(result.cost, None);
        assert_eq!(result.known_cost, "0");
        assert!(result.basis.is_empty());
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.as_ref() == "modelContextConflict")
        );
        assert!(
            !result
                .issues
                .iter()
                .any(|i| matches!(i.as_ref(), "unverifiedModel" | "catalogPriceMissing"))
        );
        assert_eq!(
            result
                .components
                .iter()
                .map(|component| component.tokens)
                .collect::<Vec<_>>(),
            [Some(100), Some(20), Some(0), Some(10)]
        );
        assert!(
            result
                .components
                .iter()
                .all(|component| component.rate_per_million.is_none())
        );
        assert_eq!(result.components[2].cost.as_deref(), Some("0"));
    }
    assert_eq!(
        price(&model("gpt-5.4"), &usage).cost.as_deref(),
        Some("0.000405")
    );
}

#[test]
fn conflicting_model_context_does_not_turn_missing_categories_into_free_usage() {
    let zero = tokens(0, 0, 0, 0);
    let context = PricingContext {
        request_scoped: true,
        model_conflicted: true,
    };
    let known_zero = price_with_context(&model("gpt-5.4"), &zero, &context);
    assert_eq!(known_zero.cost.as_deref(), Some("0"));
    assert_eq!(known_zero.status.as_ref(), "priced");
    assert!(
        known_zero
            .issues
            .iter()
            .any(|i| i.as_ref() == "modelContextConflict")
    );
    let mut missing = zero.clone();
    missing.output = None;
    missing.total = None;
    let result = price_with_context(&model("gpt-5.4"), &missing, &context);
    assert_eq!(result.cost, None);
    assert_eq!(result.components[0].cost.as_deref(), Some("0"));
    assert_eq!(result.components[3].tokens, None);
    assert_eq!(result.components[3].cost, None);
}

#[test]
fn missing_category_unknown_model_and_zero_are_distinct() {
    let mut usage = tokens(100, 0, 0, 20);
    usage.cache_create = None;
    let partial = price(&model("gpt-5.3-codex"), &usage);
    assert_eq!(partial.status.as_ref(), "partial");
    assert!(partial.cost.is_none());
    assert_eq!(partial.known_cost, "0.000455");
    let unknown = price(&model("gpt-unverified"), &tokens(100, 0, 0, 20));
    assert_eq!(unknown.status.as_ref(), "unknown");
    assert!(unknown.cost.is_none());
    let zero = price(&model("gpt-unverified"), &tokens(0, 0, 0, 0));
    assert_eq!(zero.status.as_ref(), "priced");
    assert_eq!(zero.cost.as_deref(), Some("0"));
}

#[test]
fn cache_write_without_official_rate_remains_unpriced() {
    let result = price(&model("gpt-5.3-codex"), &tokens(100, 0, 3, 20));
    assert_eq!(result.status.as_ref(), "partial");
    assert_eq!(result.known_cost, "0.000455");
    assert!(result.components[2].cost.is_none());
}

#[test]
fn official_snapshot_aliases_are_explicit_no_fuzzy_model_matching() {
    let usage = tokens(1000, 0, 0, 100);
    let canonical = price(&model("gpt-5.4"), &usage);
    let dated = price(&model("gpt-5.4-2026-03-05"), &usage);
    assert_eq!(dated.cost, canonical.cost);
    assert_eq!(dated.basis[0].match_method.as_ref(), "officialSnapshot");
    let mini = price(&model("gpt-5.4-mini-2026-03-17"), &usage);
    assert_eq!(mini.cost.as_deref(), Some("0.0012"));
    assert_eq!(mini.basis[0].match_method.as_ref(), "officialSnapshot");
    for name in [
        "gpt-5.4-fast",
        "GPT-5.4",
        "gpt-5.4-2026-12-01",
        "proxy/gpt-5.4",
        "gpt-5.4 ",
    ] {
        assert_eq!(
            price(&model(name), &usage).status.as_ref(),
            "unknown",
            "{name}"
        );
    }
    let mut fake_alias = model("other-model");
    fake_alias.pricing_model = Some("gpt-5.4".into());
    assert_eq!(price(&fake_alias, &usage).status.as_ref(), "unknown");
}

#[test]
fn explicit_other_provider_does_not_borrow_openai_price() {
    let mut reference = model("gpt-5.4");
    reference.provider = Some("other".into());
    assert_eq!(
        price(&reference, &tokens(100, 0, 0, 1)).status.as_ref(),
        "unknown"
    );
    reference.provider = Some("openai".into());
    reference.api_provider = Some("proxy".into());
    assert_eq!(
        price(&reference, &tokens(100, 0, 0, 1)).status.as_ref(),
        "unknown"
    );
}

#[test]
fn aggregate_preserves_partial_unknown_and_exact_sub_cent_amounts() {
    let one = price(&model("gpt-5.3-codex"), &tokens(0, 1, 0, 0));
    assert_eq!(one.cost.as_deref(), Some("0.000000175"));
    let many = sum_prices(std::iter::repeat_n(&one, 10_000)).unwrap();
    assert_eq!(many.cost.as_deref(), Some("0.00175"));
    let unknown = price(&model("unverified"), &tokens(0, 100, 0, 0));
    let partial = sum_prices([&many, &unknown]).unwrap();
    assert_eq!(partial.status.as_ref(), "partial");
    assert!(partial.cost.is_none());
    assert_eq!(partial.known_cost, "0.00175");
    assert_eq!(sum_prices([&unknown]).unwrap().status.as_ref(), "unknown");
    assert_eq!(sum_prices([]).unwrap().cost.as_deref(), Some("0"));
}

#[test]
fn unsupported_price_policy_is_rejected() {
    let mut recorded = empty_price();
    recorded.policy = "unsupported".into();
    assert!(sum_prices([&recorded]).is_err());
}
fn exact_price(cost: &str) -> PriceResult {
    let amount = parse_amount(cost).unwrap();
    let mut result = empty_price();
    result.cost = Some(decimal_string(amount));
    result.known_cost = decimal_string(amount);
    result
}

#[test]
fn invalid_counts_and_overflow_fail_without_fabricating_amounts() {
    let mut usage = tokens(100, 20, 0, 10);
    usage.total = Some(100);
    assert_eq!(price(&model("gpt-5.4"), &usage).status.as_ref(), "unknown");
    let mut amount = exact_price("79228162514264337593543950335");
    assert!(sum_prices([&amount, &amount]).is_err());
    amount.known_cost = "bad".into();
    assert!(sum_prices([&amount]).is_err());
}

#[test]
fn exact_decimal_boundary_rejects_loss_and_reads_scientific_numbers() {
    assert_eq!(exact_price("1.75e-7").cost.as_deref(), Some("0.000000175"));
    assert!(parse_amount("0.00000000000000000000000000001").is_err());
    let large = exact_price("79228162514264337593543950335");
    let tiny = exact_price("0.000000175");
    assert!(sum_prices([&large, &tiny]).is_err());
    let mut corrupt = tiny;
    corrupt.cost = None;
    assert!(sum_prices([&corrupt]).is_err());
}

#[test]
fn persisted_price_replays_without_catalog_recalculation() {
    let mut saved = price(&model("gpt-5.5-2026-04-23"), &tokens(100_000, 0, 0, 1_000));
    assert_eq!(saved.cost.as_deref(), Some("0.53"));
    saved.price_revision = "prior-catalog".into();
    let encoded = serde_json::to_string(&saved).unwrap();
    let decoded: PriceResult = serde_json::from_str(&encoded).unwrap();
    let summary = sum_prices([&decoded]).unwrap();
    assert_eq!(summary.cost.as_deref(), Some("0.53"));
    assert_eq!(summary.price_revision.as_ref(), "prior-catalog");
}

#[test]
fn catalog_pins_supported_rates_aliases_and_auditable_metadata() {
    let info = catalog_info();
    assert_eq!(info.revision, PRICE_REVISION);
    assert_eq!(info.policy, PRICE_POLICY);
    assert_eq!(info.hash.len(), 64);
    let mut identities = BTreeSet::new();
    for entry in &catalog().models {
        assert!(identities.insert(&entry.id));
        for alias in &entry.aliases {
            assert!(identities.insert(alias));
        }
        assert!(entry.source.starts_with("https://developers.openai.com/"));
        for rates in
            std::iter::once(&entry.rates).chain(entry.long_context.iter().map(|tier| &tier.rates))
        {
            for rate in [
                &rates.input,
                &rates.cache_read,
                &rates.cache_create,
                &rates.output,
            ]
            .into_iter()
            .flatten()
            {
                assert!(parse_amount(rate).is_ok());
            }
        }
    }
}

#[test]
fn shared_price_metadata_keeps_exact_amounts_and_serialization() {
    let mut first = price(&model("gpt-5.4"), &tokens(100, 20, 0, 10));
    let mut second = price(&model("gpt-5.4"), &tokens(200, 30, 0, 20));
    let original = serde_json::to_value([&first, &second]).unwrap();
    let mut strings = PriceParts::default();
    strings.compact(&mut first);
    strings.compact(&mut second);
    assert_eq!(serde_json::to_value([&first, &second]).unwrap(), original);
    assert!(Arc::ptr_eq(&first.basis[0].source, &second.basis[0].source));
    assert!(Arc::ptr_eq(
        &first.components[0].category,
        &second.components[0].category
    ));
    assert_ne!(first.cost, second.cost);
    assert_ne!(
        first.basis[0].request_input_tokens,
        second.basis[0].request_input_tokens
    );
}

#[test]
fn distinct_request_totals_share_only_equal_categories() {
    let mut first = price(&model("gpt-5.4"), &tokens(100, 20, 0, 10));
    let mut second = price(&model("gpt-5.4"), &tokens(200, 20, 0, 10));
    let expected = serde_json::to_value([&first, &second]).unwrap();
    let mut pool = PriceParts::default();
    pool.compact(&mut first);
    pool.compact(&mut second);
    assert!(!Arc::ptr_eq(&first.components[0], &second.components[0]));
    for index in 1..4 {
        assert!(Arc::ptr_eq(
            &first.components[index],
            &second.components[index]
        ));
    }
    assert_eq!(serde_json::to_value([&first, &second]).unwrap(), expected);
    let mut other_rate = second.clone();
    Arc::make_mut(&mut other_rate.components[1]).rate_per_million = None;
    pool.compact(&mut other_rate);
    assert!(!Arc::ptr_eq(
        &first.components[1],
        &other_rate.components[1]
    ));
    assert!(second.components[1].rate_per_million.is_some());
}

#[test]
fn streaming_categories_match_independent_exact_totals_and_mixed_rates() {
    let rows: Vec<_> = (1..=1000)
        .map(|n| price(&model("gpt-5.4"), &tokens(n, 0, 0, 2)))
        .collect();
    let sum = sum_prices(&rows).unwrap();
    // 500500 uncached input * $2.5/M + 2000 output * $15/M.
    assert_eq!(sum.cost.as_deref(), Some("1.28125"));
    assert_eq!(sum.components[0].cost.as_deref(), Some("1.25125"));
    assert_eq!(sum.components[0].tokens, Some(500500));
    assert_eq!(sum.components[3].tokens, Some(2000));
    let partitions: Vec<_> = rows
        .chunks(73)
        .map(|part| sum_prices(part).unwrap())
        .collect();
    assert_eq!(sum_prices(&partitions).unwrap(), sum);
    let mut missing_rate = rows[0].clone();
    Arc::make_mut(&mut missing_rate.components[0]).rate_per_million = None;
    for values in [
        [&rows[0], &missing_rate, &rows[1]],
        [&missing_rate, &rows[0], &rows[1]],
    ] {
        assert!(
            sum_prices(values).unwrap().components[0]
                .rate_per_million
                .is_none()
        );
    }
}

#[test]
fn damaged_component_amounts_and_statuses_cannot_hide_behind_valid_total() {
    let row = price(&model("gpt-5.4"), &tokens(100, 20, 0, 10));
    for (status, cost, known) in [
        ("unexpected", Some("0"), "0"),
        ("priced", Some("0.01"), "0.02"),
        ("unknown", None, "0.01"),
        ("partial", Some("0"), "0"),
        ("priced", Some("-1"), "-1"),
    ] {
        let mut damaged = row.clone();
        let component = Arc::make_mut(&mut damaged.components[0]);
        component.status = status.into();
        component.cost = cost.map(str::to_owned);
        component.known_cost = known.into();
        assert!(sum_prices([&damaged]).is_err());
    }
    assert!(sum_prices([&row]).is_ok());
}

#[test]
fn automatic_price_detection_requires_repairable_missing_rates() {
    let tokens = TokenUsage {
        input: Some(100),
        cache_read: Some(0),
        cache_create: Some(0),
        output: Some(10),
        total: Some(110),
        raw_input: Some(100),
        ..Default::default()
    };
    let model = ModelRef {
        raw: Some("synthetic-new".into()),
        provider: Some("openai".into()),
        ..Default::default()
    };
    let missing = |model: &ModelRef, tokens: &TokenUsage| {
        price(model, tokens)
            .issues
            .iter()
            .any(|s| s.as_ref() == "catalogPriceMissing")
    };
    assert!(missing(&model, &tokens));
    assert!(!missing(
        &ModelRef {
            raw: None,
            ..model.clone()
        },
        &tokens
    ));
    assert!(!missing(
        &ModelRef {
            api_provider: Some("third-party".into()),
            ..model.clone()
        },
        &tokens
    ));
    assert!(!missing(
        &model,
        &TokenUsage {
            total: Some(999),
            ..tokens.clone()
        }
    ));
    assert!(!missing(
        &ModelRef {
            raw: Some("gpt-5.4".into()),
            ..model.clone()
        },
        &tokens
    ));
    let zero = TokenUsage {
        input: Some(0),
        cache_read: Some(0),
        cache_create: Some(0),
        output: Some(0),
        total: Some(0),
        raw_input: Some(0),
        ..Default::default()
    };
    assert!(!missing(&model, &zero));
    let context_unknown = price_with_context(
        &ModelRef {
            raw: Some("gpt-5.4".into()),
            ..model
        },
        &tokens,
        &PricingContext {
            request_scoped: false,
            model_conflicted: false,
        },
    );
    assert!(
        !context_unknown
            .issues
            .iter()
            .any(|s| s.as_ref() == "catalogPriceMissing")
    );
}

#[test]
fn prepared_price_cache_overflow_keeps_every_charge_and_checks_uncached_values() {
    let one = price(&model("gpt-5.3-codex"), &tokens(100_000, 50_000, 0, 10_000));
    let mut rows = vec![one; 5000];
    let sum = sum_prices(rows.iter().chain([&rows[0], &rows[4999]])).unwrap();
    // 5,002 distinct ledger contributions at exactly USD 0.32375 each.
    assert_eq!(sum.cost.as_deref(), Some("1619.3975"));
    assert_eq!(sum.components[0].tokens, Some(500_200_000));
    rows[4999].currency = "invalid".into();
    assert!(sum_prices(&rows).is_err());
}
