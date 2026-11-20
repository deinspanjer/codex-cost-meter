//! Independent, observation-dated token-credit catalog. Never consult API rates.
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::pricing::{CostResult, ServiceTier, Usage};

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Card {
    pub observed_at: String,
    pub source: String,
    pub rows: Vec<Row>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Row {
    pub model: String,
    pub input: f64,
    pub cached_input: f64,
    pub output: f64,
    pub fast: Option<f64>,
    pub ultrafast: Option<f64>,
}

pub(crate) fn cards() -> &'static [Card] {
    static CARDS: std::sync::OnceLock<Vec<Card>> = std::sync::OnceLock::new();
    CARDS.get_or_init(|| {
        serde_json::from_str(include_str!("../data/credit-rates.json"))
            .expect("embedded credit card must be valid")
    })
}

pub(crate) struct Estimate {
    pub amount: CostResult,
    pub reason: Option<&'static str>,
    pub source: Option<String>,
    pub observed_at: Option<String>,
}

pub(crate) fn cost(
    model: &str,
    at: Option<OffsetDateTime>,
    tier: &ServiceTier,
    usage: Usage,
) -> Estimate {
    let unknown = |reason| Estimate {
        amount: CostResult {
            known: 0.0,
            complete: None,
        },
        reason: Some(reason),
        source: None,
        observed_at: None,
    };
    if usage
        .cached_input
        .checked_add(usage.cache_write_input)
        .is_none_or(|n| n > usage.input)
    {
        return unknown("invalid_token_partition");
    }
    // No credit-specific model proxies or free workload policy are inferred.
    if model == "codex-auto-review" {
        return unknown("unsupported_workload_identity");
    }
    // Gross input, not uncached input or accumulated session context.
    if usage.input > 272_000 {
        return unknown("credit_context_rule_unavailable");
    }
    let cards = cards();
    if !cards
        .iter()
        .any(|card| card.rows.iter().any(|row| row.model == model))
    {
        return unknown("credit_model_unavailable");
    }
    let Some(card) = cards.iter().rev().find(|card| {
        card.rows.iter().any(|row| row.model == model)
            && at.is_none_or(|at| {
                at >= OffsetDateTime::parse(&card.observed_at, &Rfc3339).expect("catalog date")
            })
    }) else {
        return unknown("credit_history_unavailable");
    };
    let Some(row) = card.rows.iter().find(|row| row.model == model) else {
        return unknown("credit_model_unavailable");
    };
    let factor = match tier {
        ServiceTier::Standard | ServiceTier::AssumedStandard => Some(1.0),
        ServiceTier::Fast => row.fast,
        ServiceTier::Ultrafast => row.ultrafast,
        ServiceTier::Unpriced(_) => None,
    };
    let Some(factor) = factor else {
        return unknown("credit_speed_unavailable");
    };
    let known = ((usage.input - usage.cached_input) as f64 * row.input
        + usage.cached_input as f64 * row.cached_input
        + usage.output as f64 * row.output)
        / 1_000_000.0
        * factor;
    Estimate {
        amount: CostResult {
            known,
            complete: Some(known),
        },
        reason: None,
        source: Some(card.source.clone()),
        observed_at: Some(card.observed_at.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn credits_partition_inclusive_counters_without_api_write_or_reasoning_fees() {
        let usage = Usage {
            input: 100_000,
            cached_input: 80_000,
            cache_write_input: 10_000,
            output: 10_000,
        };
        let at = Some(datetime!(2026-11-02 00:00 UTC));
        // (20k*50 + 80k*2.5 + 10k*250) / million.
        assert_eq!(
            cost("gpt-6.1-sol", at, &ServiceTier::Standard, usage)
                .amount
                .complete,
            Some(3.7)
        );
        assert_eq!(
            cost("gpt-6.1-sol", at, &ServiceTier::Fast, usage)
                .amount
                .complete,
            Some(7.4)
        );
        assert_eq!(
            cost("gpt-6.1-sol", at, &ServiceTier::Ultrafast, usage)
                .amount
                .complete,
            Some(22.200000000000003)
        );
    }

    #[test]
    fn evidence_gaps_do_not_inherit_api_or_subscription_rules() {
        let at = Some(datetime!(2026-11-02 00:00 UTC));
        let usage = Usage {
            input: 272_000,
            ..Usage::default()
        };
        assert!(
            cost("gpt-6.1-sol", at, &ServiceTier::Standard, usage)
                .amount
                .complete
                .is_some()
        );
        assert_eq!(
            cost(
                "gpt-6.1-sol",
                at,
                &ServiceTier::Standard,
                Usage {
                    input: 272_001,
                    ..usage
                }
            )
            .amount
            .known,
            0.0
        );
        assert!(
            cost(
                "gpt-6.1-sol",
                at,
                &ServiceTier::Standard,
                Usage {
                    input: 272_001,
                    ..usage
                }
            )
            .amount
            .complete
            .is_none()
        );
        assert!(
            cost(
                "gpt-6.1-sol",
                Some(datetime!(2026-10-01 00:00 UTC)),
                &ServiceTier::Standard,
                usage
            )
            .amount
            .complete
            .is_none()
        );
        assert!(
            cost("codex-auto-review", at, &ServiceTier::Standard, usage)
                .amount
                .complete
                .is_none()
        );
        assert!(
            cost("gpt-5.6-sol", at, &ServiceTier::Fast, usage)
                .amount
                .complete
                .is_none()
        );
    }
}
