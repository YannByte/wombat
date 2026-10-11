use super::*;
use std::sync::Arc;

pub(super) fn row(tokens: TokenUsage) -> PricedMeasurement {
    let token_unavailable_reasons = tokens.unavailable_reasons(TokenUnavailableReason::Missing);
    let model = ModelRef {
        raw: Some("gpt-5.4".into()),
        provider: Some("openai".into()),
        api_provider: None,
        pricing_model: None,
    };
    let price = crate::pricing::price(&model, &tokens);
    PricedMeasurement {
        fact: Arc::new(Measurement {
            id: "synthetic-measurement".into(),
            agent_kind: "codex".into(),
            source_instance_id: "synthetic-source".into(),
            thread_id: Some("synthetic-thread".into()),
            turn_id: Some("synthetic-turn".into()),
            response_id: None,
            timestamp: None,
            interval_end: None,
            grain: "response".into(),
            time_precision: "unknown".into(),
            model,
            reasoning_effort: None,
            tokens,
            token_unavailable_reasons,
            pricing_context_conflict: false,
            request_scoped: true,
            reported_cost: None,
            service_tier: None,
            sequence: 1,
            evidence: vec![],
        }),
        price: Arc::new(price),
    }
}

fn incomplete(raw: u64, cache: u64) -> TokenUsage {
    TokenUsage {
        raw_input: Some(raw),
        cache_read: Some(cache),
        output: Some(10),
        total: Some(raw + 10),
        ..Default::default()
    }
}

#[test]
fn native_input_survives_missing_breakdown_without_changing_tokens_or_price() {
    let measurement = row(incomplete(100, 60));
    let before = measurement.clone();
    let summary = summarize(&[&measurement]).unwrap();
    assert_eq!(summary.input_total, Some(100));
    assert_eq!(summary.cache_hit_rate, Some(0.6));
    assert_eq!(summary.tokens, before.fact.tokens);
    assert_eq!(summary.price, *before.price);
    assert_eq!(summary.tokens.input, None);
    assert_eq!(summary.tokens.cache_create, None);
    assert_eq!(summary.tokens.total, Some(110));
}

#[test]
fn zero_input_has_no_ratio_and_missing_cache_is_not_zero() {
    let zero = row(incomplete(0, 0));
    let summary = summarize(&[&zero]).unwrap();
    assert_eq!(summary.input_total, Some(0));
    assert_eq!(summary.cache_hit_rate, None);
    let missing_cache = row(TokenUsage {
        raw_input: Some(100),
        ..Default::default()
    });
    let summary = summarize(&[&missing_cache]).unwrap();
    assert_eq!(summary.input_total, Some(100));
    assert_eq!(summary.cache_hit_rate, None);
    assert_eq!(summary.tokens.cache_read, None);
}

#[test]
fn token_analysis_retains_each_field_subtotal_and_coverage_independently() {
    let known = row(TokenUsage {
        raw_input: Some(50),
        output: Some(10),
        total: Some(60),
        ..Default::default()
    });
    let missing = row(TokenUsage::default());
    let summary = summarize(&[&known, &missing]).unwrap();

    assert_eq!(summary.tokens.raw_input, None);
    assert_eq!(summary.tokens.total, None);
    assert_eq!(summary.token_analysis.method_version, 1);
    let raw_input = summary.token_analysis.field(TokenField::RawInput);
    assert_eq!(raw_input.observed_subtotal, Some(50));
    assert_eq!(raw_input.covered_records, 1);
    assert_eq!(raw_input.missing_records, 1);
    assert_eq!(raw_input.conflicting_records, 0);
    let output = summary.token_analysis.field(TokenField::Output);
    assert_eq!(output.observed_subtotal, Some(10));
    assert_eq!(output.covered_records, 1);
    assert_eq!(output.missing_records, 1);
    let total = summary.token_analysis.field(TokenField::Total);
    assert_eq!(total.observed_subtotal, Some(60));
    assert_eq!(total.covered_records, 1);
    assert_eq!(total.missing_records, 1);
}

#[test]
fn observed_zero_stays_distinct_from_a_complete_zero_total() {
    let known_zero = row(TokenUsage {
        raw_input: Some(0),
        ..Default::default()
    });
    let missing = row(TokenUsage::default());
    let summary = summarize(&[&known_zero, &missing]).unwrap();
    let raw_input = summary.token_analysis.field(TokenField::RawInput);

    assert_eq!(summary.input_total, None);
    assert_eq!(raw_input.observed_subtotal, Some(0));
    assert_eq!(raw_input.covered_records, 1);
    assert_eq!(raw_input.missing_records, 1);
    assert_eq!(
        summarize(&[&known_zero])
            .unwrap()
            .token_analysis
            .field(TokenField::RawInput)
            .observed_subtotal,
        Some(0)
    );
    assert_eq!(
        summarize(&[&missing])
            .unwrap()
            .token_analysis
            .field(TokenField::RawInput)
            .observed_subtotal,
        None
    );
}

#[test]
fn empty_scope_keeps_legacy_complete_zero_but_has_no_observed_subtotal() {
    let summary = summarize(&[]).unwrap();
    assert_eq!(summary.measurement_count, 0);
    assert_eq!(summary.tokens.total, Some(0));
    assert_eq!(summary.input_total, Some(0));
    let total = summary.token_analysis.field(TokenField::Total);
    assert_eq!(total.observed_subtotal, None);
    assert_eq!(total.covered_records, 0);
    assert_eq!(total.missing_records, 0);
}

#[test]
fn calculated_total_keeps_native_evidence_price_and_coverage_separate() {
    let native = row(TokenUsage {
        raw_input: Some(100),
        output: Some(20),
        total: Some(119),
        ..Default::default()
    });
    let calculated = row(TokenUsage {
        raw_input: Some(200),
        output: Some(30),
        reasoning: Some(10),
        ..Default::default()
    });
    let before = serde_json::to_value(&calculated.fact).unwrap();
    let calculated_only = summarize(&[&calculated]).unwrap();
    let native_only = summarize(&[&native]).unwrap();
    assert_eq!(
        max_available_tokens([&calculated_only, &native_only]),
        Some(230)
    );
    assert_eq!(
        consumption_order(&calculated_only, &native_only, &Some(Sort::Tokens)),
        std::cmp::Ordering::Less
    );
    let summary = summarize(&[&native, &calculated]).unwrap();
    let analysis = summary.token_analysis.total_analysis.as_ref().unwrap();
    assert_eq!(summary.tokens.total, None);
    assert_eq!(
        summary.token_analysis.fields.total.observed_subtotal,
        Some(119)
    );
    assert_eq!(analysis.subtotal, Some(349));
    assert_eq!(analysis.recorded_records, 1);
    assert_eq!(analysis.calculated_records, 1);
    assert_eq!(analysis.covered_records, 2);
    assert_eq!(analysis.unavailable_records, 0);
    assert_eq!(summary.complete_token_total(), Some(349));
    assert_eq!(serde_json::to_value(&calculated.fact).unwrap(), before);
    assert_eq!(summarize(&[&calculated]).unwrap().price, *calculated.price);
    // Native values, including a discrepancy with categories, always win.
    assert_eq!(
        summarize(&[&native]).unwrap().available_token_subtotal(),
        Some(119)
    );

    let missing = row(TokenUsage::default());
    let partial = summarize(&[&native, &calculated, &missing]).unwrap();
    assert_eq!(partial.available_token_subtotal(), Some(349));
    assert_eq!(partial.complete_token_total(), None);
    assert_eq!(
        share(
            summary.complete_token_total(),
            partial.complete_token_total()
        ),
        None
    );
}

#[test]
fn alternative_total_does_not_repair_conflicts_or_guess_counter_intervals() {
    for reason in [
        TokenUnavailableReason::Missing,
        TokenUnavailableReason::Invalid,
        TokenUnavailableReason::Conflicting,
        TokenUnavailableReason::Indeterminate,
    ] {
        let mut value = row(TokenUsage {
            raw_input: Some(100),
            output: Some(10),
            ..Default::default()
        });
        Arc::make_mut(&mut value.fact)
            .token_unavailable_reasons
            .total = Some(reason);
        let summary = summarize(&[&value]).unwrap();
        assert_eq!(summary.tokens.total, None);
        assert_eq!(summary.available_token_subtotal(), Some(110));
        assert_eq!(value.fact.token_unavailable_reasons.total, Some(reason));
        let native_coverage = summary.token_analysis.fields.total.clone();
        assert_eq!(native_coverage.covered_records, 0);
        assert_eq!(
            native_coverage.missing_records
                + native_coverage.invalid_records
                + native_coverage.conflicting_records
                + native_coverage.indeterminate_records,
            1
        );
        Arc::make_mut(&mut value.fact).request_scoped = false;
        Arc::make_mut(&mut value.fact).grain = "interval".into();
        assert_eq!(
            summarize(&[&value]).unwrap().available_token_subtotal(),
            None
        );
    }
    let incomplete = row(TokenUsage {
        input: Some(50),
        cache_read: Some(60),
        cache_create: Some(0),
        output: Some(10),
        ..Default::default()
    });
    assert_eq!(
        summarize(&[&incomplete])
            .unwrap()
            .available_token_subtotal(),
        None
    );
}

#[test]
fn zero_overflow_and_native_replacement_do_not_double_count_calculations() {
    let zero = row(TokenUsage {
        raw_input: Some(0),
        output: Some(0),
        ..Default::default()
    });
    let summary = summarize(&[&zero]).unwrap();
    assert_eq!(summary.available_token_subtotal(), Some(0));
    assert_eq!(summary.complete_token_total(), Some(0));
    let overflow = row(TokenUsage {
        raw_input: Some(MAX_SAFE_INTEGER),
        output: Some(1),
        ..Default::default()
    });
    let summary = summarize(&[&overflow]).unwrap();
    assert_eq!(summary.input_total, Some(MAX_SAFE_INTEGER));
    assert_eq!(summary.tokens.output, Some(1));
    let analysis = summary.token_analysis.total_analysis.unwrap();
    assert_eq!(analysis.subtotal, None);
    assert_eq!(analysis.overflow_records, 1);
    assert_eq!(analysis.unavailable_records, 1);
    let mut value = row(TokenUsage {
        raw_input: Some(100),
        output: Some(10),
        ..Default::default()
    });
    let old = summarize(&[&value]).unwrap();
    assert_eq!(
        old.token_analysis
            .total_analysis
            .as_ref()
            .unwrap()
            .calculated_records,
        1
    );
    let fact = Arc::make_mut(&mut value.fact);
    fact.tokens.total = Some(112);
    fact.token_unavailable_reasons.total = None;
    let new = summarize(&[&value]).unwrap();
    assert_eq!(new.available_token_subtotal(), Some(112));
    assert_eq!(
        new.token_analysis
            .total_analysis
            .as_ref()
            .unwrap()
            .calculated_records,
        0
    );
    assert_eq!(old.available_token_subtotal(), Some(110));
}

#[test]
fn captured_native_only_review_summaries_keep_their_original_analysis() {
    let native = row(TokenUsage {
        total: Some(10),
        ..Default::default()
    });
    let mut captured = serde_json::to_value(summarize(&[&native]).unwrap()).unwrap();
    captured["tokenAnalysis"]
        .as_object_mut()
        .unwrap()
        .remove("totalAnalysis");
    let restored: UsageSummary = serde_json::from_value(captured.clone()).unwrap();
    assert!(restored.token_analysis.total_analysis.is_none());
    assert_eq!(restored.available_token_subtotal(), Some(10));
    assert_eq!(serde_json::to_value(restored).unwrap(), captured);
}

#[test]
fn token_analysis_keeps_unavailable_categories_disjoint() {
    let known = row(TokenUsage {
        raw_input: Some(5),
        ..Default::default()
    });
    let unavailable = |reason| {
        let mut value = row(TokenUsage::default());
        Arc::make_mut(&mut value.fact)
            .token_unavailable_reasons
            .raw_input = Some(reason);
        value
    };
    let missing = row(TokenUsage::default());
    let rows = [
        known,
        missing,
        unavailable(TokenUnavailableReason::Conflicting),
        unavailable(TokenUnavailableReason::Invalid),
        unavailable(TokenUnavailableReason::Indeterminate),
    ];
    let summary = summarize(&rows.iter().collect::<Vec<_>>()).unwrap();
    let raw_input = summary.token_analysis.field(TokenField::RawInput);
    assert_eq!(raw_input.observed_subtotal, Some(5));
    assert_eq!(raw_input.covered_records, 1);
    assert_eq!(raw_input.missing_records, 1);
    assert_eq!(raw_input.conflicting_records, 1);
    assert_eq!(raw_input.invalid_records, 1);
    assert_eq!(raw_input.indeterminate_records, 1);
    assert_eq!(
        raw_input.covered_records
            + raw_input.missing_records
            + raw_input.conflicting_records
            + raw_input.invalid_records
            + raw_input.indeterminate_records,
        summary.measurement_count as u64,
    );
}

#[test]
fn inconsistent_token_value_and_reason_is_rejected() {
    let mut inconsistent = row(TokenUsage {
        raw_input: Some(5),
        ..Default::default()
    });
    Arc::make_mut(&mut inconsistent.fact)
        .token_unavailable_reasons
        .raw_input = Some(TokenUnavailableReason::Conflicting);
    let error = summarize(&[&inconsistent]).unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<crate::dto::OperationError>()
            .unwrap()
            .code,
        "INVALID_LEDGER"
    );
}

#[test]
fn recorded_token_max_includes_partial_buckets_without_claiming_complete_totals() {
    let partial_count = row(TokenUsage {
        total: Some(1_000),
        ..Default::default()
    });
    let partial_gap = row(TokenUsage::default());
    let complete = row(TokenUsage {
        total: Some(500),
        ..Default::default()
    });
    let partial = summarize(&[&partial_count, &partial_gap]).unwrap();
    let complete = summarize(&[&complete]).unwrap();

    assert_eq!(partial.tokens.total, None);
    assert_eq!(
        partial
            .token_analysis
            .field(TokenField::Total)
            .observed_subtotal,
        Some(1_000)
    );
    assert_eq!(max_available_tokens([&partial, &complete]), Some(1_000));
    assert_eq!(max_available_tokens(std::iter::empty()), None);
    // The query computes this from all groups before it paginates the response.
    assert_eq!(max_available_tokens([&partial, &complete]), Some(1_000));
    assert_eq!(max_available_tokens([&complete]), Some(500));
    assert_eq!(
        consumption_order(&partial, &complete, &Some(Sort::Tokens)),
        std::cmp::Ordering::Less
    );
}

#[test]
fn impossible_categories_cannot_produce_a_ratio_even_in_a_larger_mixed_total() {
    let valid = row(incomplete(1000, 60));
    for tokens in [
        incomplete(100, 101),
        TokenUsage {
            cache_create: Some(50),
            ..incomplete(100, 60)
        },
        TokenUsage {
            input: Some(30),
            cache_create: Some(0),
            ..incomplete(100, 60)
        },
    ] {
        let invalid = row(tokens);
        let summary = summarize(&[&invalid, &valid]).unwrap();
        assert_eq!(summary.input_total, Some(1100));
        assert_eq!(summary.cache_hit_rate, None);
    }
}

#[test]
fn mixed_complete_and_incomplete_breakdowns_conserve_native_input_totals() {
    let native = row(incomplete(100, 60));
    let parts = row(TokenUsage {
        raw_input: Some(100),
        input: Some(50),
        cache_read: Some(20),
        cache_create: Some(30),
        output: Some(10),
        total: Some(110),
        ..Default::default()
    });
    let summary = summarize(&[&native, &parts]).unwrap();
    assert_eq!(summary.input_total, Some(200));
    assert_eq!(summary.cache_hit_rate, Some(0.4));
    assert_eq!(summary.tokens.raw_input, Some(200));
    assert_eq!(summary.tokens.input, None);
    assert_eq!(summary.tokens.cache_create, None);
    assert_eq!(summary.tokens.total, Some(220));
    let absent = row(TokenUsage::default());
    let partial = summarize(&[&native, &absent]).unwrap();
    assert_eq!(partial.input_total, None);
    assert_eq!(partial.cache_hit_rate, None);
}

#[test]
fn native_input_aggregation_rejects_unsafe_counts_and_aggregate_overflow() {
    let one = row(TokenUsage {
        raw_input: Some(1),
        ..Default::default()
    });
    for large in [MAX_SAFE_INTEGER, u64::MAX] {
        let large = row(TokenUsage {
            raw_input: Some(large),
            ..Default::default()
        });
        let error = summarize(&[&large, &one]).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "RESOURCE_LIMIT"
        );
    }
}

fn collect(rows: &[serde_json::Value]) -> Vec<PricedMeasurement> {
    let directory = tempfile::tempdir().unwrap();
    std::fs::create_dir(directory.path().join("sessions")).unwrap();
    std::fs::write(
        directory.path().join("sessions/synthetic.jsonl"),
        rows.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    let collected = adapters::collect(
        &DiscoveryRequest {
            roots: vec![directory.path().into()],
        },
        &RunContext::default(),
    );
    collected
        .measurements
        .into_iter()
        .map(|measurement| {
            let price = crate::pricing::price_with_context(
                &measurement.model,
                &measurement.tokens,
                &crate::pricing::PricingContext {
                    request_scoped: measurement.request_scoped,
                    model_conflicted: measurement.pricing_context_conflict,
                },
            );
            PricedMeasurement {
                fact: measurement,
                price: Arc::new(price),
            }
        })
        .collect()
}

fn metadata() -> serde_json::Value {
    serde_json::json!({"type":"session_meta","payload":{"id":"synthetic-thread"}})
}

#[test]
fn reconciled_conflicts_stay_null_instead_of_becoming_reconstructed_counts() {
    let direct = |raw, cache, total| {
        serde_json::json!({"type":"event_msg","payload":{
            "type":"token_usage_record","thread_id":"synthetic-thread",
            "response_id":"synthetic-response","usage":{
                "input_tokens":raw,"cached_input_tokens":cache,
                "cache_write_input_tokens":0,"output_tokens":10,"total_tokens":total
            }
        }})
    };
    let rows = collect(&[metadata(), direct(100, 60, 110), direct(110, 70, 120)]);
    assert_eq!(rows.len(), 1);
    let summary = summarize(&rows.iter().collect::<Vec<_>>()).unwrap();
    assert_eq!(summary.tokens.raw_input, None);
    assert_eq!(summary.tokens.cache_read, None);
    assert_eq!(summary.tokens.total, None);
    assert_eq!(summary.input_total, None);
    assert_eq!(summary.cache_hit_rate, None);
    let rows = collect(&[metadata(), direct(100, 60, 110), direct(100, 60, 111)]);
    let summary = summarize(&rows.iter().collect::<Vec<_>>()).unwrap();
    assert_eq!(summary.tokens.total, None);
    assert_eq!(summary.input_total, Some(100));
    assert_eq!(summary.cache_hit_rate, Some(0.6));
}

#[test]
fn categories_completed_by_replay_do_not_restore_conflicting_native_input() {
    let first = serde_json::json!({"type":"event_msg","payload":{
        "type":"token_usage_record","thread_id":"synthetic-thread",
        "response_id":"synthetic-response","usage":{
            "input_tokens":100,"cache_write_input_tokens":0,
            "output_tokens":10,"total_tokens":110
        }
    }});
    let second = serde_json::json!({"type":"event_msg","payload":{
        "type":"token_usage_record","thread_id":"synthetic-thread",
        "response_id":"synthetic-response","usage":{
            "input_tokens":110,"cached_input_tokens":60,"cache_write_input_tokens":0,
            "output_tokens":10,"total_tokens":120
        }
    }});
    for events in [
        [metadata(), first.clone(), second.clone()],
        [metadata(), second.clone(), first.clone()],
    ] {
        let rows = collect(&events);
        assert_eq!(rows.len(), 1);
        let summary = summarize(&rows.iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(summary.tokens.raw_input, None);
        assert_eq!(summary.tokens.input, Some(50));
        assert_eq!(summary.tokens.cache_read, Some(60));
        assert_eq!(summary.tokens.cache_create, Some(0));
        assert_eq!(summary.tokens.total, None);
        assert_eq!(summary.input_total, None);
        assert_eq!(summary.cache_hit_rate, None);
    }
    // TokenUsage alone cannot prove whether absent native input was never
    // recorded or nulled by reconciliation. Complete parts do not authorize it.
    let parts_only = row(TokenUsage {
        input: Some(50),
        cache_read: Some(60),
        cache_create: Some(0),
        ..Default::default()
    });
    let summary = summarize(&[&parts_only]).unwrap();
    assert_eq!(summary.input_total, None);
    assert_eq!(summary.cache_hit_rate, None);
}

#[test]
fn cumulative_differences_keep_the_ledger_and_price_conserved() {
    let counter = |input, cache, output| {
        serde_json::json!({"type":"event_msg","payload":{
            "type":"token_count","info":{"total_token_usage":{
                "input_tokens":input,"cached_input_tokens":cache,
                "output_tokens":output,"total_tokens":input+output
            }}
        }})
    };
    let rows = collect(&[metadata(), counter(100, 60, 10), counter(300, 120, 40)]);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].fact.tokens.cache_create, Some(0));
    let summaries = rows
        .iter()
        .map(|row| summarize(&[row]).unwrap())
        .collect::<Vec<_>>();
    let summary = summarize(&rows.iter().collect::<Vec<_>>()).unwrap();
    assert_eq!(summary.input_total, Some(300));
    assert_eq!(summary.tokens.input, Some(180));
    assert_eq!(summary.tokens.cache_read, Some(120));
    assert_eq!(summary.tokens.cache_create, Some(0));
    assert_eq!(summary.tokens.output, Some(40));
    assert_eq!(summary.tokens.total, Some(340));
    assert_eq!(summary.cache_hit_rate, Some(0.4));
    assert_eq!(
        summaries
            .iter()
            .map(|s| s.input_total.unwrap())
            .sum::<u64>(),
        300
    );
    assert_eq!(
        summary.price,
        crate::pricing::sum_prices(rows.iter().map(|r| r.price.as_ref())).unwrap()
    );
}

#[test]
fn optional_calculation_overflow_preserves_native_subtotals_and_categories() {
    let native = row(TokenUsage {
        total: Some(5),
        ..Default::default()
    });
    let alternative = row(TokenUsage {
        raw_input: Some(MAX_SAFE_INTEGER),
        output: Some(0),
        ..Default::default()
    });
    let first = summarize(&[&native, &alternative]).unwrap();
    let reverse = summarize(&[&alternative, &native]).unwrap();
    assert_eq!(first.token_analysis, reverse.token_analysis);
    assert_eq!(first.available_token_subtotal(), Some(5));
    assert_eq!(first.complete_token_total(), None);
    assert_eq!(
        first.token_analysis.fields.raw_input.observed_subtotal,
        Some(MAX_SAFE_INTEGER)
    );
    let total = first.token_analysis.total_analysis.unwrap();
    assert_eq!(total.recorded_records, 1);
    assert_eq!(total.calculated_records, 0);
    assert_eq!(total.covered_records, 1);
    assert_eq!(total.unavailable_records, 1);
    assert_eq!(total.overflow_records, 1);
}
