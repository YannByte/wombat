//! Shared type-7 sample quantile, used by timing and usage populations.
pub(crate) fn quantile(sorted: &[f64], probability: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let h = (sorted.len() - 1) as f64 * probability;
    let lower = h.floor() as usize;
    let upper = h.ceil() as usize;
    Some(sorted[lower] + (sorted[upper] - sorted[lower]) * (h - lower as f64))
}

/// Exact type-7 values in linear selection work; no sorted population copy.
pub(crate) fn population_quantiles(values: &mut [f64]) -> (Option<f64>, Option<f64>) {
    let mut select = |probability: f64| {
        if values.is_empty() {
            return None;
        }
        let h = (values.len() - 1) as f64 * probability;
        let lower = h.floor() as usize;
        let upper = h.ceil() as usize;
        let a = *values.select_nth_unstable_by(lower, f64::total_cmp).1;
        let b = if lower == upper {
            a
        } else {
            *values.select_nth_unstable_by(upper, f64::total_cmp).1
        };
        Some(a + (b - a) * (h - lower as f64))
    };
    (select(0.5), select(0.9))
}

#[cfg(test)]
mod tests {
    #[test]
    fn selection_preserves_exact_type_seven_quantiles() {
        for size in 0..260 {
            let values: Vec<_> = (0..size)
                .map(|i| ((i * 997 + size * 13) % 51) as f64)
                .collect();
            let mut sorted = values.clone();
            sorted.sort_unstable_by(f64::total_cmp);
            let expected = (super::quantile(&sorted, 0.5), super::quantile(&sorted, 0.9));
            assert_eq!(super::population_quantiles(&mut values.clone()), expected);
        }
    }
}
