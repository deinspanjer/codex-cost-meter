use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use time::macros::datetime;
use time::{Date, OffsetDateTime};

const FIRST_PERSISTED_FAST_RELEASE: OffsetDateTime = datetime!(2026-07-09 16:47:12 UTC);

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct Usage {
    pub input: u64,
    pub cached_input: u64,
    pub cache_write_input: u64,
    pub output: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum ServiceTier {
    Standard,
    AssumedStandard,
    Fast,
    Unpriced(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CostResult {
    pub known: f64,
    pub complete: Option<f64>,
}

pub(crate) struct Catalog {
    as_of: String,
    source: String,
    histories: HashMap<String, Vec<PricePoint>>,
    long_context_thresholds: HashMap<String, u64>,
    proxies: HashMap<String, String>,
    undated_proxies: HashSet<String>,
}

#[derive(Clone, Copy, Deserialize)]
struct PricePoint {
    #[serde(deserialize_with = "deserialize_date")]
    effective_from: Date,
    input: Option<f64>,
    cached_input: Option<f64>,
    cache_write_input: Option<f64>,
    output: Option<f64>,
    fast: Option<Rates>,
    long_context: Option<TierRates>,
}

#[derive(Clone, Copy, Deserialize)]
struct Rates {
    input: Option<f64>,
    cached_input: Option<f64>,
    cache_write_input: Option<f64>,
    output: Option<f64>,
}

#[derive(Clone, Copy, Deserialize)]
struct TierRates {
    standard: Rates,
    fast: Option<Rates>,
}

impl PricePoint {
    fn rates(self, long_context: bool, tier: &ServiceTier) -> Option<Rates> {
        if long_context {
            let rates = self.long_context?;
            return match tier {
                ServiceTier::Standard | ServiceTier::AssumedStandard => Some(rates.standard),
                ServiceTier::Fast => rates.fast,
                ServiceTier::Unpriced(_) => None,
            };
        }
        match tier {
            ServiceTier::Standard | ServiceTier::AssumedStandard => Some(Rates {
                input: self.input,
                cached_input: self.cached_input,
                cache_write_input: self.cache_write_input,
                output: self.output,
            }),
            ServiceTier::Fast => self.fast,
            ServiceTier::Unpriced(_) => None,
        }
    }

    fn rate_sets(self) -> impl Iterator<Item = Rates> {
        let mut rates = vec![Rates {
            input: self.input,
            cached_input: self.cached_input,
            cache_write_input: self.cache_write_input,
            output: self.output,
        }];
        rates.extend(self.fast);
        if let Some(long_context) = self.long_context {
            rates.push(long_context.standard);
            rates.extend(long_context.fast);
        }
        rates.into_iter()
    }
}

fn deserialize_date<'de, D>(deserializer: D) -> Result<Date, D::Error>
where
    D: Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Date::parse(
        &value,
        time::macros::format_description!("[year]-[month]-[day]"),
    )
    .map_err(serde::de::Error::custom)
}

#[derive(Deserialize)]
struct RawCatalog {
    #[serde(default)]
    as_of: String,
    #[serde(default)]
    source: String,
    histories: HashMap<String, Vec<PricePoint>>,
    #[serde(default)]
    long_context_thresholds: HashMap<String, u64>,
    proxies: HashMap<String, String>,
    undated_proxies: HashSet<String>,
}

#[derive(Debug, Error)]
pub(crate) enum PricingError {
    #[error("invalid pricing JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("negative rate for {model} effective {effective_from}")]
    NegativeRate { model: String, effective_from: Date },
    #[error("non-increasing effective date for {model}: {effective_from}")]
    NonIncreasingEffectiveDate { model: String, effective_from: Date },
    #[error("proxy {proxy} has no target history: {target}")]
    UnresolvedProxy { proxy: String, target: String },
    #[error("undated proxy is not a proxy: {0}")]
    InvalidUndatedProxy(String),
    #[error("invalid long-context threshold model: {0}")]
    InvalidLongContextThreshold(String),
}

impl Catalog {
    pub(crate) fn embedded() -> Result<Self, PricingError> {
        Self::parse(include_str!("../data/model-prices.json"))
    }

    fn parse(prices: &str) -> Result<Self, PricingError> {
        let raw: RawCatalog = serde_json::from_str(prices)?;

        for (model, history) in &raw.histories {
            let mut previous = None;
            for point in history {
                if point.rate_sets().any(|rates| {
                    [
                        rates.input,
                        rates.cached_input,
                        rates.cache_write_input,
                        rates.output,
                    ]
                    .into_iter()
                    .flatten()
                    .any(|rate| rate < 0.0)
                }) {
                    return Err(PricingError::NegativeRate {
                        model: model.clone(),
                        effective_from: point.effective_from,
                    });
                }
                if previous.is_some_and(|date| point.effective_from <= date) {
                    return Err(PricingError::NonIncreasingEffectiveDate {
                        model: model.clone(),
                        effective_from: point.effective_from,
                    });
                }
                previous = Some(point.effective_from);
            }
        }

        for (model, threshold) in &raw.long_context_thresholds {
            if *threshold == 0 || !raw.histories.contains_key(model) {
                return Err(PricingError::InvalidLongContextThreshold(model.clone()));
            }
        }

        for (proxy, target) in &raw.proxies {
            if !raw.histories.contains_key(target) {
                return Err(PricingError::UnresolvedProxy {
                    proxy: proxy.clone(),
                    target: target.clone(),
                });
            }
        }
        for proxy in &raw.undated_proxies {
            if !raw.proxies.contains_key(proxy) {
                return Err(PricingError::InvalidUndatedProxy(proxy.clone()));
            }
        }

        Ok(Self {
            as_of: raw.as_of,
            source: raw.source,
            histories: raw.histories,
            long_context_thresholds: raw.long_context_thresholds,
            proxies: raw.proxies,
            undated_proxies: raw.undated_proxies,
        })
    }

    pub(crate) fn cost(
        &self,
        model: &str,
        at: Option<OffsetDateTime>,
        tier: &ServiceTier,
        usage: Usage,
    ) -> CostResult {
        if matches!(tier, ServiceTier::Fast)
            && at.is_some_and(|at| at < FIRST_PERSISTED_FAST_RELEASE)
        {
            return CostResult {
                known: 0.0,
                complete: None,
            };
        }
        let target = self.proxies.get(model).map_or(model, String::as_str);
        let Some(history) = self.histories.get(target) else {
            return CostResult {
                known: 0.0,
                complete: None,
            };
        };
        let rates = match at {
            None => history.last(),
            Some(_) if self.undated_proxies.contains(model) => history.last(),
            Some(at) => history
                .iter()
                .rev()
                .find(|point| point.effective_from <= at.date()),
        };
        let Some(price_point) = rates else {
            return CostResult {
                known: 0.0,
                complete: None,
            };
        };
        let long_context = self
            .long_context_thresholds
            .get(target)
            .is_some_and(|threshold| usage.input > *threshold);
        let Some(rates) = price_point.rates(long_context, tier) else {
            return CostResult {
                known: 0.0,
                complete: None,
            };
        };

        let uncached = usage
            .input
            .saturating_sub(usage.cached_input.saturating_add(usage.cache_write_input));
        let mut known = 0.0;
        let mut complete = true;
        for (tokens, rate) in [
            (uncached, rates.input),
            (usage.cached_input, rates.cached_input),
            (usage.cache_write_input, rates.cache_write_input),
            (usage.output, rates.output),
        ] {
            match rate {
                Some(rate) => known += tokens as f64 * rate / 1_000_000.0,
                None if tokens > 0 => complete = false,
                None => {}
            }
        }
        CostResult {
            known,
            complete: complete.then_some(known),
        }
    }

    pub(crate) fn as_of(&self) -> &str {
        &self.as_of
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn proxies(&self) -> &HashMap<String, String> {
        &self.proxies
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    #[test]
    fn selects_the_rate_effective_at_the_event_time() {
        let catalog = Catalog::embedded().unwrap();
        let before = datetime!(2026-07-29 12:00 UTC);
        let after = datetime!(2026-07-30 12:00 UTC);
        let usage = Usage {
            output: 1_000_000,
            ..Usage::default()
        };

        assert_eq!(
            catalog
                .cost("gpt-5.6-terra", Some(before), &ServiceTier::Standard, usage)
                .complete,
            Some(15.0)
        );
        assert_eq!(
            catalog
                .cost("gpt-5.6-terra", Some(after), &ServiceTier::Standard, usage)
                .complete,
            Some(12.0)
        );
    }

    #[test]
    fn selects_service_tier_context_and_effective_date() {
        let catalog = Catalog::embedded().unwrap();
        let before_fast_markers = datetime!(2026-07-09 16:47:11 UTC);
        let first_stable_fast_marker = datetime!(2026-07-09 16:47:12 UTC);
        let effective = datetime!(2026-08-22 12:00 UTC);
        let short = Usage {
            input: 272_000,
            ..Usage::default()
        };
        let long = Usage {
            input: 272_001,
            ..Usage::default()
        };

        let cases = [
            (ServiceTier::Standard, short, 1.088),
            (ServiceTier::Standard, long, 2.176008),
            (ServiceTier::Fast, short, 2.176),
            (ServiceTier::Fast, long, 4.352016),
        ];
        for (tier, usage, expected) in cases {
            let actual = catalog
                .cost("gpt-5.6-sol", Some(effective), &tier, usage)
                .complete
                .unwrap();
            assert!((actual - expected).abs() < 1e-12, "{tier:?}");
        }
        assert_eq!(
            catalog
                .cost(
                    "gpt-5.6-sol",
                    Some(before_fast_markers),
                    &ServiceTier::Fast,
                    short,
                )
                .complete,
            None
        );
        let historical_fast_cases = [
            ("gpt-5.6-sol", 2.72),
            ("gpt-5.6-terra", 1.36),
            ("gpt-5.5", 3.4),
        ];
        for (model, expected) in historical_fast_cases {
            let actual = catalog
                .cost(
                    model,
                    Some(first_stable_fast_marker),
                    &ServiceTier::Fast,
                    short,
                )
                .complete
                .unwrap();
            assert!((actual - expected).abs() < 1e-12, "{model}");
        }
    }

    #[test]
    fn unknown_or_unpublished_rates_preserve_only_known_cost() {
        let catalog = Catalog::embedded().unwrap();
        let input = Usage {
            input: 1_000_000,
            ..Usage::default()
        };
        let mixed = Usage {
            input: 1_000_000,
            cached_input: 1,
            ..Usage::default()
        };

        assert_eq!(
            catalog
                .cost("unknown", None, &ServiceTier::Standard, input)
                .complete,
            None
        );
        let result = catalog.cost("gpt-5.2-pro", None, &ServiceTier::Standard, mixed);
        assert_eq!(result.known, 20.999979);
        assert_eq!(result.complete, None);
    }

    #[test]
    fn resolves_a_proxy_to_its_target_history() {
        let catalog = Catalog::embedded().unwrap();
        let usage = Usage {
            output: 1_000_000,
            ..Usage::default()
        };

        assert_eq!(
            catalog
                .cost("gpt-5.6", None, &ServiceTier::Standard, usage)
                .complete,
            Some(20.0)
        );
    }

    #[test]
    fn uses_the_newest_rate_for_an_undated_proxy() {
        let catalog = Catalog::embedded().unwrap();
        let before = datetime!(2026-07-29 12:00 UTC);
        let usage = Usage {
            output: 1_000_000,
            ..Usage::default()
        };

        assert_eq!(
            catalog
                .cost(
                    "codex-auto-review",
                    Some(before),
                    &ServiceTier::Standard,
                    usage,
                )
                .complete,
            Some(1.2)
        );
    }

    #[test]
    fn rejects_invalid_catalogs() {
        enum ExpectedError {
            NegativeRate,
            NonIncreasingDate,
            UnresolvedProxy,
            InvalidUndatedProxy,
            InvalidLongContextThreshold,
        }

        let cases = [
            (
                "negative rate",
                r#"{"histories":{"model":[{"effective_from":"2026-01-01","input":-1.0,"cached_input":null,"cache_write_input":null,"output":null}]},"proxies":{},"undated_proxies":[]}"#,
                ExpectedError::NegativeRate,
            ),
            (
                "equal effective dates",
                r#"{"histories":{"model":[{"effective_from":"2026-01-01","input":1.0,"cached_input":null,"cache_write_input":null,"output":null},{"effective_from":"2026-01-01","input":2.0,"cached_input":null,"cache_write_input":null,"output":null}]},"proxies":{},"undated_proxies":[]}"#,
                ExpectedError::NonIncreasingDate,
            ),
            (
                "unresolved proxy",
                r#"{"histories":{},"proxies":{"alias":"missing"},"undated_proxies":[]}"#,
                ExpectedError::UnresolvedProxy,
            ),
            (
                "undated non-proxy",
                r#"{"histories":{},"proxies":{},"undated_proxies":["alias"]}"#,
                ExpectedError::InvalidUndatedProxy,
            ),
            (
                "threshold without history",
                r#"{"histories":{},"long_context_thresholds":{"missing":272000},"proxies":{},"undated_proxies":[]}"#,
                ExpectedError::InvalidLongContextThreshold,
            ),
        ];

        for (name, prices, expected) in cases {
            let error = Catalog::parse(prices).err().unwrap();
            assert!(
                matches!(
                    (expected, error),
                    (
                        ExpectedError::NegativeRate,
                        PricingError::NegativeRate { .. }
                    ) | (
                        ExpectedError::NonIncreasingDate,
                        PricingError::NonIncreasingEffectiveDate { .. }
                    ) | (
                        ExpectedError::UnresolvedProxy,
                        PricingError::UnresolvedProxy { .. }
                    ) | (
                        ExpectedError::InvalidUndatedProxy,
                        PricingError::InvalidUndatedProxy(_)
                    ) | (
                        ExpectedError::InvalidLongContextThreshold,
                        PricingError::InvalidLongContextThreshold(_)
                    )
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn subtracts_cached_and_cache_write_input_saturating_at_zero() {
        let catalog = Catalog::embedded().unwrap();
        let usage = Usage {
            input: 100,
            cached_input: 80,
            cache_write_input: 30,
            ..Usage::default()
        };

        let cost = catalog
            .cost("gpt-5.6-sol", None, &ServiceTier::Standard, usage)
            .complete
            .unwrap();
        assert!((cost - 0.000182).abs() < 1e-15);
    }
}
