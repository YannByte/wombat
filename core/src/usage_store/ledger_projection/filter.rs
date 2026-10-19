//! Conservative SQL candidates. Exact timezone, coverage and accounting rules
//! stay in the shared reducers; these predicates may only exclude impossible rows.
use crate::usage_app_dto::Scope;
use rusqlite::types::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub(crate) struct Scan<'a> {
    pub(super) filter: &'a Filter,
    pub(super) select: Option<&'a dyn Fn(&super::LedgerGroup) -> bool>,
}
impl<'a> Scan<'a> {
    pub(crate) fn new(
        filter: &'a Filter,
        select: Option<&'a dyn Fn(&super::LedgerGroup) -> bool>,
    ) -> Self {
        Self { filter, select }
    }
}

#[derive(Default)]
pub(crate) struct Filter {
    clauses: Vec<String>,
    pub(super) values: Vec<Value>,
}
impl Filter {
    pub(crate) fn scope(scope: &Scope, projects: &BTreeMap<&str, Option<&str>>) -> Self {
        let mut filter = Self::default();
        for (field, value) in [
            ("agent", &scope.agent_kind),
            ("source", &scope.source_instance_id),
            ("model", &scope.model),
            ("effort", &scope.reasoning_effort),
        ] {
            if let Some(value) = value {
                filter.bind(
                    format!("json_extract(catalog,'$.key.{field}') = ?"),
                    value.clone(),
                );
            }
        }
        for (field, unknown) in [
            ("model", scope.model_unknown),
            ("effort", scope.effort_unknown),
        ] {
            if unknown == Some(true) {
                filter
                    .clauses
                    .push(format!("json_extract(catalog,'$.key.{field}') IS NULL"));
            }
        }
        if let Some(thread) = &scope.thread_id {
            filter.bind("thread = ?".into(), thread.clone());
        }
        if scope.project.is_some() || scope.project_unknown == Some(true) {
            if scope.project.is_none() {
                let assigned: Vec<_> = projects
                    .iter()
                    .filter(|(_, project)| project.is_some())
                    .map(|(id, _)| *id)
                    .collect();
                filter.bind(
                    "(thread IS NULL OR thread NOT IN (SELECT value FROM json_each(?)))".into(),
                    serde_json::to_string(&assigned).expect("string list serialization"),
                );
            } else {
                let threads = projects
                    .iter()
                    .filter(|(_, project)| {
                        scope
                            .project
                            .as_deref()
                            .is_none_or(|p| **project == Some(p))
                            && (scope.project_unknown != Some(true) || project.is_none())
                    })
                    .map(|(id, _)| (*id).to_owned())
                    .collect();
                filter.threads(threads, scope.project.is_none());
            }
        }
        let hour = "json_extract(catalog,'$.key.time.Hour')";
        let day = "json_extract(catalog,'$.key.time.Date')";
        if scope.undated == Some(true) {
            filter
                .clauses
                .push(format!("{hour} IS NULL AND {day} IS NULL"));
        } else if scope.since.is_some() || scope.until.is_some() {
            let mut hours = vec![format!("{hour} IS NOT NULL")];
            let mut days = vec![format!("{day} IS NOT NULL")];
            // Every chrono-tz UTC offset is within a day. This envelope also
            // admits midnight rollbacks; exact local dates are checked later.
            for (bound, op, padding) in [(&scope.since, ">=", -24_i64), (&scope.until, "<", 24_i64)]
            {
                if let Some(bound) = bound
                    // Keep existing lexical scope semantics for signed years.
                    && bound.len() == 10
                    && let Ok(date) = crate::usage_app::scope::date(bound)
                {
                    let value = date
                        .and_hms_opt(0, 0, 0)
                        .unwrap()
                        .and_utc()
                        .timestamp()
                        .div_euclid(3600)
                        + padding;
                    hours.push(format!("{hour} {op} ?"));
                    filter.values.push(Value::Integer(value));
                }
            }
            for (bound, op) in [(&scope.since, ">="), (&scope.until, "<")] {
                if let Some(bound) = bound {
                    days.push(format!("{day} {op} ?"));
                    filter.values.push(Value::Text(bound.clone()));
                }
            }
            filter.clauses.push(format!(
                "(({}) OR ({}))",
                hours.join(" AND "),
                days.join(" AND ")
            ));
        }
        filter
    }
    pub(crate) fn tasks(threads: BTreeSet<String>) -> Self {
        let mut filter = Self::default();
        // Unassigned measurements still contribute to the selected summary.
        filter.threads(threads, true);
        filter
    }
    pub(crate) fn either(mut self, other: Self) -> Self {
        if self.clauses.is_empty() || other.clauses.is_empty() {
            return Self::default();
        }
        self.clauses = vec![format!(
            "(({}) OR ({}))",
            self.clauses.join(" AND "),
            other.clauses.join(" AND ")
        )];
        self.values.extend(other.values);
        self
    }
    fn threads(&mut self, threads: BTreeSet<String>, unassigned: bool) {
        let json = serde_json::to_string(&threads).expect("string set serialization");
        self.bind(
            if unassigned {
                "(thread IS NULL OR thread IN (SELECT value FROM json_each(?)))"
            } else {
                "thread IN (SELECT value FROM json_each(?))"
            }
            .into(),
            json,
        );
    }
    fn bind(&mut self, clause: String, value: String) {
        self.clauses.push(clause);
        self.values.push(Value::Text(value));
    }
    pub(super) fn sql(&self) -> String {
        let mut parameter = 7;
        self.clauses
            .iter()
            .map(|clause| {
                let mut parts = clause.split('?');
                let mut out = format!(" AND {}", parts.next().unwrap());
                for part in parts {
                    parameter += 1;
                    out.push_str(&format!("?{parameter}{part}"));
                }
                out
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidates_preserve_unknowns_and_bound_text_as_data() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        db.execute_batch("CREATE TABLE measurement_summaries(source TEXT,block INTEGER,part INTEGER,thread TEXT,catalog TEXT); CREATE INDEX model ON measurement_summaries(source,json_extract(catalog,'$.key.model'),block,part);").unwrap();
        for (id, thread, model, time) in [
            (
                1,
                Some("known"),
                Some("gpt-5.4"),
                serde_json::json!({"Hour":497711}),
            ),
            (
                2,
                Some("unattributed"),
                None,
                serde_json::json!({"Date":"2026-10-11"}),
            ),
            (
                3,
                Some("absent-owner"),
                Some("x' OR 1=1 --"),
                serde_json::json!("Unknown"),
            ),
            (4, None, None, serde_json::json!({"Hour":0})),
        ] {
            let catalog = serde_json::json!({"key":{"agent":"codex","source":"s","thread":thread,"model":model,"effort":null,"time":time}}).to_string();
            db.execute(
                "INSERT INTO measurement_summaries VALUES('s',?1,0,?2,?3)",
                rusqlite::params![id, thread, catalog],
            )
            .unwrap();
        }
        let projects = BTreeMap::from([("known", Some("/project")), ("unattributed", None)]);
        let read = |scope: Scope| {
            let filter = Filter::scope(&scope, &projects);
            let sql = format!(
                "SELECT block FROM measurement_summaries WHERE source=?1 AND ?7 IS NULL {} ORDER BY block",
                filter.sql()
            );
            let mut values = vec![Value::Text("s".into())];
            values.extend(std::iter::repeat_n(Value::Null, 6));
            values.extend(filter.values);
            db.prepare(&sql)
                .unwrap()
                .query_map(rusqlite::params_from_iter(values), |r| r.get::<_, i64>(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<_>>>()
                .unwrap()
        };
        let scope = |value| serde_json::from_value(value).unwrap();
        assert_eq!(
            read(scope(serde_json::json!({"projectUnknown":true}))),
            [2, 3, 4]
        );
        assert_eq!(
            read(scope(serde_json::json!({"modelUnknown":true}))),
            [2, 4]
        );
        assert_eq!(
            read(scope(serde_json::json!({"model":"x' OR 1=1 --"}))),
            [3]
        );
        assert_eq!(read(scope(serde_json::json!({"undated":true}))), [3]);
        assert_eq!(
            read(scope(serde_json::json!({"since":"+10000-01-01"}))),
            [1, 2, 4]
        );
        assert_eq!(
            read(scope(
                serde_json::json!({"since":"2026-10-11","until":"2026-10-12"})
            )),
            [1, 2]
        );
    }
}
