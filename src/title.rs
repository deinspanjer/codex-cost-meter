use std::str::FromStr;

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Metric {
    Cost,
    TotalTokens,
    InputTokens,
    OutputTokens,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MetricList(Vec<Metric>);

impl Default for MetricList {
    fn default() -> Self {
        Self(vec![Metric::Cost, Metric::TotalTokens])
    }
}

#[derive(Debug, Error)]
pub(crate) enum TitleError {
    #[error("title metrics cannot be empty")]
    EmptyMetrics,
    #[error("all cannot be combined")]
    CombinedAll,
    #[error("duplicate title metric")]
    DuplicateMetric,
    #[error("unknown title metric: {0}")]
    UnknownMetric(String),
    #[error("title width cannot fit a visible base and metric suffix")]
    TooNarrow,
}

impl FromStr for MetricList {
    type Err = TitleError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.is_empty() {
            return Err(TitleError::EmptyMetrics);
        }

        let mut metrics = Vec::new();
        for name in value.split(',') {
            if name == "all" {
                if value != "all" {
                    return Err(TitleError::CombinedAll);
                }
                return Ok(Self(vec![
                    Metric::Cost,
                    Metric::TotalTokens,
                    Metric::InputTokens,
                    Metric::OutputTokens,
                ]));
            }
            let metric = match name {
                "cost" => Metric::Cost,
                "total-tokens" => Metric::TotalTokens,
                "input-tokens" => Metric::InputTokens,
                "output-tokens" => Metric::OutputTokens,
                _ => return Err(TitleError::UnknownMetric(name.into())),
            };
            if metrics.contains(&metric) {
                return Err(TitleError::DuplicateMetric);
            }
            metrics.push(metric);
        }
        Ok(Self(metrics))
    }
}

pub(crate) struct TitleFormat {
    width: usize,
    metrics: MetricList,
}

impl TitleFormat {
    pub(crate) fn new(width: usize, metrics: MetricList) -> Self {
        Self { width, metrics }
    }

    pub(crate) fn compose(
        &self,
        title: &str,
        stats: &crate::report::StatsReport,
    ) -> Result<String, TitleError> {
        let suffix = self
            .metrics
            .0
            .iter()
            .map(|metric| match metric {
                Metric::Cost => format_cost(
                    stats.known_model_cost_usd,
                    stats.estimated_cost_usd.is_none(),
                ),
                Metric::TotalTokens => {
                    format_tokens(stats.input_tokens.saturating_add(stats.output_tokens), '⇄')
                }
                Metric::InputTokens => format_tokens(stats.input_tokens, '⇥'),
                Metric::OutputTokens => format_tokens(stats.output_tokens, '↦'),
            })
            .collect::<Vec<_>>()
            .join(" · ");
        let reserved = suffix.chars().count() + " · ".chars().count();
        let available = self
            .width
            .checked_sub(reserved)
            .ok_or(TitleError::TooNarrow)?;
        let base = strip_canonical_suffix(title).trim_end();
        if base.is_empty() {
            return Err(TitleError::TooNarrow);
        }
        let base = if base.chars().count() <= available {
            base.to_owned()
        } else {
            if available < 2 {
                return Err(TitleError::TooNarrow);
            }
            let truncated = base.chars().take(available - 1).collect::<String>();
            let truncated = truncated.trim_end();
            if truncated.is_empty() {
                return Err(TitleError::TooNarrow);
            }
            format!("{truncated}…")
        };
        Ok(format!("{base} · {suffix}"))
    }

    pub(crate) fn matches_suffix(&self, title: &str) -> bool {
        let mut title = title;
        let mut metrics = Vec::new();
        while let Some((base, segment)) = title.rsplit_once(" · ") {
            let Some(metric) = canonical_metric(segment) else {
                break;
            };
            metrics.push(metric);
            title = base;
        }
        metrics.reverse();
        metrics == self.metrics.0
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }
}

fn strip_canonical_suffix(mut title: &str) -> &str {
    while let Some((base, segment)) = title.rsplit_once(" · ") {
        if canonical_metric(segment).is_none() {
            break;
        }
        title = base;
    }
    title
}

fn canonical_metric(segment: &str) -> Option<Metric> {
    if is_cost(segment) {
        return Some(Metric::Cost);
    }
    for (prefix, metric) in [
        ('⇄', Metric::TotalTokens),
        ('⇥', Metric::InputTokens),
        ('↦', Metric::OutputTokens),
    ] {
        if segment.strip_prefix(prefix).is_some_and(is_compact_token) {
            return Some(metric);
        }
    }
    None
}

fn is_cost(segment: &str) -> bool {
    let Some(value) = segment.strip_prefix('$') else {
        return false;
    };
    let value = value.strip_suffix('+').unwrap_or(value);
    let Some((whole, cents)) = value.rsplit_once('.') else {
        return false;
    };
    cents.len() == 2 && cents.bytes().all(|byte| byte.is_ascii_digit()) && is_grouped_integer(whole)
}

fn is_grouped_integer(value: &str) -> bool {
    let mut groups = value.split(',');
    let Some(first) = groups.next() else {
        return false;
    };
    if first.is_empty()
        || first.len() > 3
        || !first.bytes().all(|byte| byte.is_ascii_digit())
        || (first.len() > 1 && first.starts_with('0'))
        || (first == "0" && value.contains(','))
    {
        return false;
    }
    groups.all(|group| group.len() == 3 && group.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_compact_token(value: &str) -> bool {
    let (number, unit) = value
        .strip_suffix('K')
        .or_else(|| value.strip_suffix('M'))
        .or_else(|| value.strip_suffix('B'))
        .map_or((value, false), |number| (number, true));
    let Some((whole, decimal)) = number.split_once('.') else {
        return number.len() <= 3
            && !number.is_empty()
            && number.bytes().all(|byte| byte.is_ascii_digit())
            && (number == "0" || !number.starts_with('0'))
            && (!unit || number != "0");
    };
    unit && whole.len() <= 3
        && !whole.is_empty()
        && whole.bytes().all(|byte| byte.is_ascii_digit())
        && decimal.len() == 1
        && decimal.bytes().all(|byte| byte.is_ascii_digit())
        && decimal != "0"
        && whole != "0"
        && !whole.starts_with('0')
}

fn format_cost(cost: f64, incomplete: bool) -> String {
    let cents = (cost * 100.0).round() as u64;
    let whole = cents / 100;
    let mut grouped = String::new();
    for (index, digit) in whole.to_string().chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    let whole = grouped.chars().rev().collect::<String>();
    format!(
        "${whole}.{:02}{}",
        cents % 100,
        if incomplete { "+" } else { "" }
    )
}

fn format_tokens(tokens: u64, prefix: char) -> String {
    let value = if tokens < 1_000 {
        tokens.to_string()
    } else {
        let (divisor, unit) = if tokens >= 1_000_000_000 {
            (1_000_000_000, 'B')
        } else if tokens >= 1_000_000 {
            (1_000_000, 'M')
        } else {
            (1_000, 'K')
        };
        format_scaled(tokens, divisor, unit)
    };
    format!("{prefix}{value}")
}

fn format_scaled(tokens: u64, divisor: u64, unit: char) -> String {
    let tenths = ((u128::from(tokens) * 10) + (u128::from(divisor) / 2)) / u128::from(divisor);
    if tenths >= 10_000 && unit != 'B' {
        return format_scaled(tokens, divisor * 1_000, if unit == 'K' { 'M' } else { 'B' });
    }
    let whole = tenths / 10;
    let decimal = tenths % 10;
    if decimal == 0 {
        format!("{whole}{unit}")
    } else {
        format!("{whole}.{decimal}{unit}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::str::FromStr;

    use super::{Metric, MetricList, TitleFormat};
    use crate::report::StatsReport;

    fn stats(input_tokens: u64, output_tokens: u64, cost: f64, incomplete: bool) -> StatsReport {
        StatsReport {
            rollout_count: 1,
            majority_turn_model: None,
            majority_reasoning_level: None,
            input_tokens,
            input_cache_write_tokens: 0,
            input_cache_read_tokens: 0,
            reasoning_tokens: 0,
            output_tokens,
            turns: 1,
            completed_or_aborted_turns: 1,
            incomplete_turns: 0,
            total_turn_duration_seconds: 0.0,
            estimated_cost_usd: (!incomplete).then_some(cost),
            known_model_cost_usd: cost,
            unpriced_models: BTreeMap::new(),
            unattributed_usage_tokens: None,
            incomplete_input: incomplete,
        }
    }

    #[test]
    fn parses_metric_lists() {
        fn assert_parse(input: &str, expected: Result<Vec<Metric>, &str>) {
            match (MetricList::from_str(input), expected) {
                (Ok(actual), Ok(expected)) => assert_eq!(actual.0, expected),
                (Err(actual), Err(expected)) => assert_eq!(actual.to_string(), expected),
                (actual, expected) => panic!("{input:?}: got {actual:?}, expected {expected:?}"),
            }
        }

        for (input, expected) in [
            (
                "cost,total-tokens",
                Ok(vec![Metric::Cost, Metric::TotalTokens]),
            ),
            (
                "output-tokens,cost",
                Ok(vec![Metric::OutputTokens, Metric::Cost]),
            ),
            (
                "all",
                Ok(vec![
                    Metric::Cost,
                    Metric::TotalTokens,
                    Metric::InputTokens,
                    Metric::OutputTokens,
                ]),
            ),
            ("", Err("title metrics cannot be empty")),
            ("cost,cost", Err("duplicate title metric")),
            ("other", Err("unknown title metric: other")),
            ("all,cost", Err("all cannot be combined")),
        ] {
            assert_parse(input, expected);
        }
    }

    #[test]
    fn composes_bounded_titles() {
        for (width, metrics, title, input, output, cost, incomplete, expected) in [
            (
                65,
                "cost,total-tokens",
                "Task title",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("Task title · $12.34 · ⇄1.4M"),
            ),
            (
                65,
                "cost",
                "Task title",
                1_200_000,
                200_000,
                12.34,
                true,
                Ok("Task title · $12.34+"),
            ),
            (
                65,
                "output-tokens,cost",
                "Task title",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("Task title · ↦200K · $12.34"),
            ),
            (
                65,
                "cost,total-tokens",
                "Task title · $0.50",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("Task title · $12.34 · ⇄1.4M"),
            ),
            (
                65,
                "cost,total-tokens",
                "Task title · $0.50 · ⇥1.2M · ⇄1.4M",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("Task title · $12.34 · ⇄1.4M"),
            ),
            (
                65,
                "cost",
                "Keep · $0.50 here",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("Keep · $0.50 here · $12.34"),
            ),
            (
                65,
                "cost",
                "Keep · 12.34",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("Keep · 12.34 · $12.34"),
            ),
            (
                65,
                "cost",
                "Keep · $0,000.50",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("Keep · $0,000.50 · $12.34"),
            ),
            (
                65,
                "cost",
                "Keep · ⇄1.0M",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("Keep · ⇄1.0M · $12.34"),
            ),
            (
                65,
                "input-tokens,output-tokens",
                "Task title",
                2_000,
                999,
                12.34,
                false,
                Ok("Task title · ⇥2K · ↦999"),
            ),
            (
                65,
                "total-tokens",
                "Task title",
                1_500_000_000,
                0,
                12.34,
                false,
                Ok("Task title · ⇄1.5B"),
            ),
            (
                24,
                "cost,total-tokens",
                "🦀 title",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("🦀 title · $12.34 · ⇄1.4M"),
            ),
            (
                22,
                "cost,total-tokens",
                "Task title",
                1_200_000,
                200_000,
                12.34,
                false,
                Ok("Task… · $12.34 · ⇄1.4M"),
            ),
            (
                17,
                "cost,total-tokens",
                "Task title",
                1_200_000,
                200_000,
                12.34,
                false,
                Err("title width cannot fit a visible base and metric suffix"),
            ),
        ] {
            let format = TitleFormat::new(width, MetricList::from_str(metrics).unwrap());
            let actual = format
                .compose(title, &stats(input, output, cost, incomplete))
                .map_err(|error| error.to_string());
            assert_eq!(
                actual.as_ref().map(String::as_str).map_err(String::as_str),
                expected,
                "{metrics} on {title:?}"
            );
            if let Ok(actual) = actual {
                assert!(actual.chars().count() <= width);
            }
        }
    }

    #[test]
    fn recognizes_the_requested_canonical_suffix_in_order() {
        let format = TitleFormat::new(65, MetricList::default());

        assert!(format.matches_suffix("Task title · $12.34 · ⇄1.4M"));
        assert!(!format.matches_suffix("Task title · ⇄1.4M · $12.34"));
        assert!(!format.matches_suffix("Task title · $12.34"));
    }
}
