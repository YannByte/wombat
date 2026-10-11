//! Revision-owned sufficient statistics, prepared before a live view is published.
//! Filters still select exact facts; only completely selected groups can be folded.
use super::*;
use std::collections::HashMap;

const MAX_ROWS: usize = 250_000;
const MAX_GROUPS: usize = 4096;
const MAX_BYTES: usize = 8 * 1024 * 1024;
const MIN_GROUP: usize = 16;

struct Group {
    usage: UsageSummary,
    total: TotalParts,
    input_consistent: bool,
}
impl Group {
    fn new(rows: &[&PricedMeasurement]) -> Result<Self> {
        Ok(Self {
            usage: summarize(rows)?,
            total: TotalParts::from_rows(rows)?,
            input_consistent: rows
                .iter()
                .all(|row| input_categories_consistent(&row.fact.tokens)),
        })
    }
}

#[derive(Default)]
pub(crate) struct SummaryIndex {
    // Addresses are identity keys only, never dereferenced. The owning immutable
    // snapshot keeps every indexed Arc alive and never mutates its price or fact.
    members: HashMap<usize, Member>,
    groups: Vec<Group>,
    threads: HashMap<String, crate::usage_app::conversations::ThreadSummary>,
}
struct Member {
    group: Option<usize>,
    time: crate::usage_app::scope::ObservedTime,
}
impl SummaryIndex {
    pub(crate) fn build(rows: &[&PricedMeasurement]) -> Result<Self> {
        let mut index = Self::default();
        if rows.len() > MAX_ROWS {
            return Ok(index);
        }
        let mut groups = HashMap::<_, Vec<&PricedMeasurement>>::new();
        for &row in rows {
            let time = crate::usage_app::scope::ObservedTime::parse(row);
            index.members.insert(
                row as *const PricedMeasurement as usize,
                Member { group: None, time },
            );
            let key = (
                row.fact.thread_id.as_deref(),
                row.fact.model.raw.as_deref(),
                row.fact.reasoning_effort.as_deref(),
                time.local_date(chrono_tz::UTC),
            );
            if groups.len() < MAX_GROUPS || groups.contains_key(&key) {
                groups.entry(key).or_default().push(row);
            }
        }
        // Stable admission under the byte budget; larger groups save more work.
        let mut groups: Vec<_> = groups
            .into_values()
            .filter(|rows| rows.len() >= MIN_GROUP)
            .collect();
        groups.sort_unstable_by(|a, b| {
            b.len()
                .cmp(&a.len())
                .then_with(|| a[0].fact.id.cmp(&b[0].fact.id))
        });
        let mut bytes = 0;
        for rows in groups {
            let group = Group::new(&rows)?;
            let size = serde_json::to_vec(&group.usage)?.len();
            if bytes + size > MAX_BYTES {
                continue;
            }
            bytes += size;
            let id = index.groups.len();
            for row in rows {
                index
                    .members
                    .get_mut(&(row as *const PricedMeasurement as usize))
                    .unwrap()
                    .group = Some(id);
            }
            index.groups.push(group);
        }
        let mut threads: Vec<_> = crate::usage_app::conversations::group_measurements(rows, |r| {
            r.fact.thread_id.as_deref()
        })
        .into_iter()
        .collect();
        threads.sort_unstable_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(b.0)));
        for (id, rows) in threads.into_iter().take(MAX_GROUPS) {
            if rows.len() < MIN_GROUP {
                continue;
            }
            let summary = crate::usage_app::conversations::prepare_thread(&rows)?;
            let size = serde_json::to_vec(&summary)?.len() + id.len();
            if bytes + size > MAX_BYTES {
                continue;
            }
            bytes += size;
            index.threads.insert(id.to_owned(), summary);
        }
        Ok(index)
    }
    pub(in crate::usage_app) fn thread(
        &self,
        id: &str,
    ) -> Option<&crate::usage_app::conversations::ThreadSummary> {
        self.threads.get(id)
    }
    pub(crate) fn local_date(&self, row: &PricedMeasurement, tz: Tz) -> Option<NaiveDate> {
        self.members
            .get(&(row as *const PricedMeasurement as usize))
            .map_or_else(|| local_date(row, tz), |member| member.time.local_date(tz))
    }
    pub(crate) fn summarize(&self, rows: &[&PricedMeasurement]) -> Result<UsageSummary> {
        if self.groups.is_empty() || rows.len() < MIN_GROUP
            // Canonical filtered sequences are strictly ordered. Other inputs,
            // including duplicates, use the general path without losing charges.
            || rows.windows(2).any(|pair| pair[0].fact.id >= pair[1].fact.id)
        {
            return summarize(rows);
        }
        let group_for = |row: &PricedMeasurement| {
            self.members
                .get(&(row as *const PricedMeasurement as usize))
                .and_then(|member| member.group)
        };
        let mut counts = vec![0_usize; self.groups.len()];
        for &row in rows {
            if let Some(id) = group_for(row) {
                counts[id] += 1;
            }
        }
        let complete = |id: usize| counts[id] == self.groups[id].usage.measurement_count;
        let mut selected = self
            .groups
            .iter()
            .enumerate()
            .filter_map(|(id, group)| complete(id).then_some(group))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return summarize(rows);
        }
        let remainder = rows
            .iter()
            .copied()
            .filter(|row| group_for(row).is_none_or(|id| !complete(id)))
            .collect::<Vec<_>>();
        let partial;
        if !remainder.is_empty() {
            partial = Group::new(&remainder)?;
            selected.push(&partial);
        }
        merge(&selected)
    }
}

fn add(left: Option<u64>, right: Option<u64>) -> Result<Option<u64>> {
    match (left, right) {
        (Some(a), Some(b)) => a
            .checked_add(b)
            .filter(|n| *n <= MAX_SAFE_INTEGER)
            .map(Some)
            .ok_or_else(|| operation_error("RESOURCE_LIMIT", "Token合计超出安全整数范围")),
        _ => Ok(None),
    }
}
fn merge(groups: &[&Group]) -> Result<UsageSummary> {
    let mut result = summarize(&[])?;
    let mut total = TotalParts::default();
    let mut input_consistent = true;
    for group in groups {
        let usage = &group.usage;
        result.measurement_count += usage.measurement_count;
        result.input_total = add(result.input_total, usage.input_total)?;
        result.unpriced_tokens = add(result.unpriced_tokens, usage.unpriced_tokens)?;
        input_consistent &= group.input_consistent;
        total.merge(&group.total);
        for field in TokenField::ALL {
            result.tokens.set(
                field,
                add(result.tokens.get(field), usage.tokens.get(field))?,
            );
            let target = result.token_analysis.fields.get_mut(field);
            let source = usage.token_analysis.fields.get(field);
            target.covered_records += source.covered_records;
            target.missing_records += source.missing_records;
            target.conflicting_records += source.conflicting_records;
            target.invalid_records += source.invalid_records;
            target.indeterminate_records += source.indeterminate_records;
            target.observed_subtotal = if target.covered_records > 0 {
                add(
                    Some(target.observed_subtotal.unwrap_or(0)),
                    Some(source.observed_subtotal.unwrap_or(0)),
                )?
            } else {
                None
            };
        }
    }
    result.cache_hit_rate = input_consistent
        .then(|| share(result.tokens.cache_read, result.input_total))
        .flatten();
    result.token_analysis.total_analysis = Some(total.finish());
    result.price = crate::pricing::sum_prices(groups.iter().map(|g| &g.usage.price))
        .map_err(|e| operation_error("PRICING_ERROR", e))?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::usage_app::summary::tests::row;
    use std::sync::Arc;

    fn fixture(count: usize) -> Vec<PricedMeasurement> {
        (0..count)
            .map(|i| {
                let mut value = row(TokenUsage {
                    input: Some(70),
                    cache_read: Some(20),
                    cache_create: Some(10),
                    raw_input: Some(100),
                    output: Some(10),
                    reasoning: Some(3),
                    total: (i % 5 != 0).then_some(110),
                });
                let fact = Arc::make_mut(&mut value.fact);
                fact.id = format!("row-{i:05}");
                fact.thread_id = Some(format!("task-{}", i / 32).into());
                fact.timestamp = Some(
                    if i % 2 == 0 {
                        "2026-03-08T01:30:00Z"
                    } else {
                        "2026-03-08T20:30:00Z"
                    }
                    .into(),
                );
                value
            })
            .collect()
    }
    fn equal(index: &SummaryIndex, rows: &[&PricedMeasurement]) {
        assert_eq!(
            serde_json::to_value(index.summarize(rows).unwrap()).unwrap(),
            serde_json::to_value(summarize(rows).unwrap()).unwrap()
        );
    }
    #[test]
    fn exact_groups_partial_filters_duplicates_and_revision_changes() {
        let mut values = fixture(160);
        // Independent missing, conflicting and invalid values survive a fold.
        for (i, value) in values.iter_mut().enumerate() {
            if i % 13 == 0 {
                let fact = Arc::make_mut(&mut value.fact);
                fact.tokens.reasoning = None;
                fact.token_unavailable_reasons.reasoning = Some(match i % 3 {
                    0 => TokenUnavailableReason::Conflicting,
                    1 => TokenUnavailableReason::Invalid,
                    _ => TokenUnavailableReason::Indeterminate,
                });
            }
        }
        let rows = values.iter().collect::<Vec<_>>();
        let index = SummaryIndex::build(&rows).unwrap();
        assert_eq!(index.groups.len(), 5);
        equal(&index, &rows);
        equal(&index, &rows[..64]);
        equal(&index, &rows[7..103]);
        equal(
            &index,
            &rows
                .iter()
                .copied()
                .enumerate()
                .filter_map(|(i, r)| (i < 64 || i % 2 == 0).then_some(r))
                .collect::<Vec<_>>(),
        );
        equal(&index, &rows.iter().rev().copied().collect::<Vec<_>>());
        let duplicate = vec![rows[0]; 32];
        equal(&index, &duplicate);
        for tz in [
            chrono_tz::UTC,
            chrono_tz::America::New_York,
            chrono_tz::Asia::Shanghai,
        ] {
            let selected = rows
                .iter()
                .copied()
                .filter(|row| local_date(row, tz).unwrap().day() == 8)
                .collect::<Vec<_>>();
            equal(&index, &selected);
        }
        // A new immutable revision cannot reuse aggregates by fact ID alone.
        let mut next = fixture(160);
        Arc::make_mut(&mut next[0].fact).tokens.total = Some(210);
        Arc::make_mut(&mut next[0].fact)
            .token_unavailable_reasons
            .total = None;
        let refs = next.iter().collect::<Vec<_>>();
        equal(&index, &refs);
        let next_index = SummaryIndex::build(&refs).unwrap();
        equal(&next_index, &refs);
        equal(&index, &rows);
    }
    #[test]
    fn alternative_overflow_is_decided_after_groups_merge_and_limits_fall_back() {
        let mut values = fixture(32);
        for (i, value) in values.iter_mut().enumerate() {
            let tokens = if i == 16 {
                TokenUsage {
                    raw_input: Some(MAX_SAFE_INTEGER),
                    output: Some(0),
                    ..Default::default()
                }
            } else {
                TokenUsage {
                    total: Some(if i == 0 { 5 } else { 0 }),
                    ..Default::default()
                }
            };
            let mut changed = row(tokens);
            let fact = Arc::make_mut(&mut changed.fact);
            fact.id = format!("row-{i:05}");
            fact.thread_id = Some(format!("task-{}", i / 16).into());
            *value = changed;
        }
        let rows = values.iter().collect::<Vec<_>>();
        let index = SummaryIndex::build(&rows).unwrap();
        assert_eq!(index.groups.len(), 2);
        equal(&index, &rows);
        equal(&index, &rows[16..]);
        let total = index
            .summarize(&rows)
            .unwrap()
            .token_analysis
            .total_analysis
            .unwrap();
        assert_eq!(total.subtotal, Some(5));
        assert_eq!(total.calculated_records, 0);
        assert_eq!(total.overflow_records, 1);
        assert!(
            SummaryIndex::build(&vec![rows[0]; MAX_ROWS + 1])
                .unwrap()
                .groups
                .is_empty()
        );
    }
    #[test]
    fn parsed_time_index_keeps_offsets_date_precision_unknowns_and_timezone_boundaries() {
        let cases = [
            ("2026-03-08T06:59:59Z", "second"),
            ("2026-03-08T07:00:00Z", "second"),
            ("2026-11-01T05:59:59Z", "second"),
            ("2026-11-01T06:00:00Z", "second"),
            ("2026-10-11T00:30:00+08:00", "second"),
            ("2026-10-11", "date"),
            ("2026-10-11", "unknown"),
            ("invalid", "date"),
        ];
        let mut rows = fixture(cases.len());
        for (row, (time, precision)) in rows.iter_mut().zip(cases) {
            let fact = Arc::make_mut(&mut row.fact);
            fact.timestamp = Some(time.into());
            fact.time_precision = precision.into();
        }
        let index = SummaryIndex::build(&rows.iter().collect::<Vec<_>>()).unwrap();
        for row in &rows {
            for tz in [
                chrono_tz::UTC,
                chrono_tz::America::New_York,
                chrono_tz::Asia::Shanghai,
            ] {
                assert_eq!(index.local_date(row, tz), local_date(row, tz));
            }
        }
        assert_eq!(
            index
                .local_date(&rows[4], chrono_tz::UTC)
                .unwrap()
                .to_string(),
            "2026-10-10"
        );
        assert_eq!(
            index
                .local_date(&rows[5], chrono_tz::UTC)
                .unwrap()
                .to_string(),
            "2026-10-11"
        );
        assert!(index.local_date(&rows[6], chrono_tz::UTC).is_none());
    }
    #[test]
    fn prepared_thread_calendar_queries_equal_facts() {
        let root = tempfile::tempdir().unwrap();
        let prices = crate::pricing_sync::current_at(root.path()).unwrap();
        let values = fixture(160);
        let collected = Collected {
            measurements: values.iter().map(|r| Arc::clone(&r.fact)).collect(),
            threads: (0..5)
                .map(|i| Thread {
                    id: format!("task-{i}"),
                    upstream_id: format!("native-{i}"),
                    agent_kind: "codex".into(),
                    source_instance_id: "synthetic-source".into(),
                    title: None,
                    project: Some(format!("/synthetic/project-{}", i % 2)),
                    started_at: None,
                    last_activity_at: Some("2026-03-09T00:00:00Z".into()),
                })
                .collect(),
            ..Default::default()
        };
        let mut snapshot =
            crate::usage_store::memory(collected, "live:aggregate:test".into(), prices, None)
                .unwrap();
        assert!(snapshot.summary_index.thread("task-0").is_some());
        let queries: Vec<Request> = [
            serde_json::json!({"action":"threads","scope":{"allTime":true,"timezone":"UTC"},"sort":"tokens"}),
            serde_json::json!({"action":"threads","scope":{"project":"/synthetic/project-1","allTime":true,"timezone":"UTC"},"sort":"recent"}),
            serde_json::json!({"action":"threads","scope":{"since":"2026-03-08","until":"2026-03-09","timezone":"America/New_York"}}),
            serde_json::json!({"action":"usage","scope":{"allTime":true,"timezone":"America/New_York"},"group":"day"}),
            serde_json::json!({"action":"usage","scope":{"allTime":true,"timezone":"Asia/Shanghai"},"group":"month"}),
        ].into_iter().map(|q| serde_json::from_value(q).unwrap()).collect();
        let expected = queries
            .iter()
            .map(|q| crate::usage_app::execute_uncached(q.clone(), &snapshot).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(expected[0].summary.complete_token_total(), Some(17_600));
        snapshot.summary_index = SummaryIndex::default();
        for (request, accelerated) in queries.into_iter().zip(expected) {
            let direct = crate::usage_app::execute_uncached(request, &snapshot).unwrap();
            assert_eq!(
                serde_json::to_value(accelerated).unwrap(),
                serde_json::to_value(direct).unwrap()
            );
        }
    }
}
