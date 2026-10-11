//! Keep both native and alternative cohorts until the final selected scope is known.
use super::*;

#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
pub(super) struct TotalParts {
    recorded_sum: u128,
    calculated_sum: u128,
    recorded: u64,
    calculated: u64,
    unavailable: u64,
    overflow: u64,
}
impl TotalParts {
    pub(super) fn from_rows(rows: &[&PricedMeasurement]) -> Result<Self> {
        let mut parts = Self::default();
        for row in rows {
            let fact = &row.fact;
            if let Some(native) = fact.tokens.total {
                parts.recorded = checked_record_increment(parts.recorded)?;
                parts.recorded_sum += u128::from(native);
                continue;
            }
            let value = if fact.request_scoped && fact.grain.as_ref() == "response" {
                let pair = fact.tokens.raw_input.zip(fact.tokens.output);
                let value = pair.and_then(|(input, output)| {
                    input.checked_add(output).filter(|n| *n <= MAX_SAFE_INTEGER)
                });
                if pair.is_some() && value.is_none() {
                    parts.overflow = checked_record_increment(parts.overflow)?;
                }
                value
            } else {
                None
            };
            if let Some(value) = value {
                parts.calculated = checked_record_increment(parts.calculated)?;
                parts.calculated_sum += u128::from(value);
            } else {
                parts.unavailable = checked_record_increment(parts.unavailable)?;
            }
        }
        Ok(parts)
    }
    pub(super) fn merge(&mut self, other: &Self) {
        // Disjoint canonical rows bound counts by the addressable input length.
        self.recorded_sum += other.recorded_sum;
        self.calculated_sum += other.calculated_sum;
        self.recorded += other.recorded;
        self.calculated += other.calculated;
        self.unavailable += other.unavailable;
        self.overflow += other.overflow;
    }
    pub(super) fn finish(&self) -> AnalyzedTokenTotal {
        let combined = self.recorded_sum + self.calculated_sum;
        let overflow = combined > u128::from(MAX_SAFE_INTEGER);
        let calculated = if overflow { 0 } else { self.calculated };
        let covered = self.recorded + calculated;
        AnalyzedTokenTotal {
            method_version: 1,
            subtotal: (covered > 0).then_some(if overflow {
                self.recorded_sum as u64
            } else {
                combined as u64
            }),
            covered_records: covered,
            recorded_records: self.recorded,
            calculated_records: calculated,
            unavailable_records: self.unavailable + if overflow { self.calculated } else { 0 },
            overflow_records: self.overflow + if overflow { self.calculated } else { 0 },
        }
    }
}
