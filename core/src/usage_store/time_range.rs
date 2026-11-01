//! Complete scalar time bounds; pruning never assumes source time is ordered.
use super::*;
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeRange {
    pub first: Option<String>,
    pub last: Option<String>,
    pub undated: bool,
}
impl TimeRange {
    pub(crate) fn build<'a>(times: impl IntoIterator<Item = Option<&'a str>>) -> Self {
        let mut range = Self::default();
        for time in times {
            range.observe(time);
        }
        range
    }
    pub(crate) fn observe(&mut self, time: Option<&str>) {
        let time = time
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|t| {
                t.to_utc()
                    .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
            });
        if let Some(time) = time {
            if self.first.as_ref().is_none_or(|first| time < *first) {
                self.first = Some(time.clone());
            }
            if self.last.as_ref().is_none_or(|last| time > *last) {
                self.last = Some(time);
            }
        } else {
            self.undated = true;
        }
    }
    pub(crate) fn may_match(&self, scope: &crate::usage_app_dto::Scope, tz: chrono_tz::Tz) -> bool {
        if scope.undated == Some(true) {
            return self.undated;
        }
        if scope.since.is_none() && scope.until.is_none() {
            return self.first.is_some() || self.undated;
        }
        let date = |at: &Option<String>| {
            at.as_deref()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|t| t.with_timezone(&tz).date_naive().to_string())
        };
        match (date(&self.first), date(&self.last)) {
            (Some(first), Some(last)) => {
                !scope.since.as_ref().is_some_and(|s| last < *s)
                    && !scope.until.as_ref().is_some_and(|u| first >= *u)
            }
            _ => false,
        }
    }
    pub(super) fn validate(&self, count: usize) -> Result<()> {
        let valid = |s: &str| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .is_some_and(|t| {
                    t.to_utc()
                        .to_rfc3339_opts(chrono::SecondsFormat::Nanos, true)
                        == s
                })
        };
        if self.first.is_some() != self.last.is_some()
            || self.first.as_deref().is_some_and(|s| !valid(s))
            || self.last.as_deref().is_some_and(|s| !valid(s))
            || self
                .first
                .as_ref()
                .zip(self.last.as_ref())
                .is_some_and(|(a, b)| a > b)
            || count == 0 && self != &Self::default()
            || count > 0 && self.first.is_none() && !self.undated
        {
            return Err(corrupt("time summary invalid"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_pruning_handles_unordered_offsets_dst_and_undated_values() {
        let range = TimeRange::build([
            Some("2026-11-01T01:30:00-05:00"),
            None,
            Some("2026-11-01T01:15:00-04:00"),
        ]);
        range.validate(3).unwrap();
        let scope = |since: &str, until: &str| {
            serde_json::from_value(serde_json::json!({"since":since,"until":until})).unwrap()
        };
        assert!(range.may_match(
            &scope("2026-11-01", "2026-11-02"),
            chrono_tz::America::New_York
        ));
        assert!(!range.may_match(
            &scope("2026-10-31", "2026-11-01"),
            chrono_tz::America::New_York
        ));
        assert!(range.may_match(
            &serde_json::from_value(serde_json::json!({"undated":true})).unwrap(),
            chrono_tz::UTC
        ));
        assert!(
            !TimeRange::build([None]).may_match(&scope("2026-11-01", "2026-11-02"), chrono_tz::UTC)
        );
        assert!(!TimeRange::default().may_match(&Default::default(), chrono_tz::UTC));
    }
}
