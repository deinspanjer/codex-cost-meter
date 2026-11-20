use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    str::FromStr,
};

use serde::Serialize;

use crate::{
    pricing::{Catalog, ServiceTier},
    quota::{Snapshot, Window},
    rollout::analysis::UsageEvent,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
pub(crate) enum Metric {
    #[serde(rename = "api-usd")]
    ApiUsd,
    #[serde(rename = "credits")]
    Credits,
    #[serde(rename = "quota-pp")]
    Quota,
}

impl Metric {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::ApiUsd => "api-usd",
            Self::Credits => "credits",
            Self::Quota => "quota-pp",
        }
    }
    pub(crate) fn unit(self) -> &'static str {
        match self {
            Self::ApiUsd => "usd",
            Self::Credits => "credits",
            Self::Quota => "percentage_points",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Selection(pub(crate) Vec<Metric>);

impl Default for Selection {
    fn default() -> Self {
        Self(vec![Metric::ApiUsd])
    }
}

impl FromStr for Selection {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        if value == "all" {
            return Ok(Self(vec![Metric::ApiUsd, Metric::Credits, Metric::Quota]));
        }
        let mut result = Vec::new();
        for value in value.split(',') {
            let metric = match value {
                "api-usd" => Metric::ApiUsd,
                "credits" => Metric::Credits,
                "quota-pp" => Metric::Quota,
                _ => {
                    return Err(format!(
                        "invalid cost metric {value:?}; use api-usd,credits,quota-pp or standalone all"
                    ));
                }
            };
            if result.contains(&metric) {
                return Err("duplicate cost metric".into());
            }
            result.push(metric);
        }
        Ok(Self(result))
    }
}

impl Selection {
    pub(crate) fn text(&self) -> String {
        self.0
            .iter()
            .map(|m| m.name())
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[derive(Clone, Default)]
pub(crate) struct Context {
    pub(crate) selection: Selection,
    pub(crate) quota: Option<Rc<Snapshot>>,
    pub(crate) window: Window,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Amount {
    pub(crate) unit: &'static str,
    pub(crate) estimated: Option<f64>,
    pub(crate) known_minimum: f64,
    pub(crate) supported_events: u64,
    pub(crate) unsupported_events: BTreeMap<String, u64>,
    pub(crate) assumed_events: BTreeMap<String, u64>,
    pub(crate) extrapolated_events: u64,
    pub(crate) period_contributions: BTreeMap<String, f64>,
    pub(crate) provenance: serde_json::Value,
    #[serde(skip)]
    positive: bool,
}

impl Amount {
    fn new(metric: Metric, context: &Context) -> Self {
        Self {
            unit: metric.unit(),
            estimated: if metric == Metric::Quota
                && !context.quota.as_ref().is_some_and(|quota| {
                    quota
                        .config
                        .periods
                        .iter()
                        .any(|period| period.windows.contains_key(&context.window))
                }) {
                None
            } else {
                Some(0.0)
            },
            known_minimum: 0.0,
            supported_events: 0,
            unsupported_events: BTreeMap::new(),
            assumed_events: BTreeMap::new(),
            extrapolated_events: 0,
            period_contributions: BTreeMap::new(),
            provenance: match metric {
                Metric::ApiUsd => serde_json::json!({"basis":"API-equivalent token list cost"}),
                Metric::Credits => serde_json::json!({
                    "basis":"estimated token credits, not verified deductions; legacy/USD Enterprise agreements excluded",
                    "history":"observation boundaries, not established billing cutovers",
                    "scope":"token counters only; observable non-token billing is not included",
                    "source":"docs/research/credit-rate-evidence.md"
                }),
                Metric::Quota => context
                    .quota
                    .as_ref()
                    .map_or(serde_json::Value::Null, |q| q.provenance(context.window)),
            },
            positive: false,
        }
    }
    fn unsupported(&mut self, reason: &str) {
        *self.unsupported_events.entry(reason.into()).or_default() += 1;
        self.estimated = None;
    }
    fn assume(&mut self, reason: &str) {
        *self.assumed_events.entry(reason.into()).or_default() += 1;
    }
    fn add(&mut self, value: f64) {
        let next = self.known_minimum + value;
        if !next.is_finite() {
            self.unsupported("arithmetic_overflow");
            return;
        }
        self.known_minimum = next;
        self.positive |= value > 0.0;
        self.supported_events += 1;
        if self.estimated.is_some() {
            self.estimated = Some(next);
        }
    }

    fn reject_account_mix(&mut self) {
        self.unsupported_events
            .entry("multiple_accounts: separate the input and use one policy per account".into())
            .or_insert(1);
        self.estimated = None;
        self.known_minimum = 0.0;
        self.period_contributions.clear();
        self.supported_events = 0;
        self.positive = false;
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub(crate) struct Bundle {
    pub(crate) cost_metrics: Vec<Metric>,
    pub(crate) costs: BTreeMap<Metric, Amount>,
    #[serde(skip)]
    accounts: BTreeSet<String>,
}

#[derive(Default)]
pub(crate) struct Ledger {
    entries: BTreeMap<Metric, Amount>,
    accounts: BTreeSet<String>,
}

impl Ledger {
    pub(crate) fn add(&mut self, event: &UsageEvent, catalog: &Catalog) {
        let context = &catalog.cost_context;
        if let Some(account) = &event.account_id {
            self.accounts.insert(account.clone());
        }
        for metric in &context.selection.0 {
            let amount = self
                .entries
                .entry(*metric)
                .or_insert_with(|| Amount::new(*metric, context));
            if matches!(event.service_tier, ServiceTier::AssumedStandard) {
                amount.assume("assumed_standard");
            }
            match metric {
                Metric::ApiUsd => {
                    let cost =
                        catalog.cost(&event.model, event.at, &event.service_tier, event.usage);
                    if cost.complete.is_some() {
                        amount.supported_events += 1;
                    } else {
                        amount.unsupported("api_rate_or_tier_unavailable");
                    }
                    if event.timestamp_source != "event" {
                        amount.assume("session_or_latest_known_timestamp");
                    }
                    amount.assume("applied_tier_not_verified_served");
                    amount.provenance["catalog"] = serde_json::json!(catalog.metadata());
                }
                Metric::Credits => {
                    amount.assume("observed_card_carry_forward_not_verified_cutover");
                    if event.guardian_review {
                        amount
                            .unsupported("credit_workload_or_authentication_evidence_unavailable");
                        continue;
                    }
                    if event.timestamp_source != "event" {
                        amount.assume("session_or_latest_known_timestamp");
                    }
                    amount.assume("applied_tier_not_verified_served");
                    let estimate = crate::credits::cost(
                        &event.model,
                        event.at,
                        &event.service_tier,
                        event.usage,
                    );
                    if let Some(reason) = estimate.reason {
                        amount.unsupported(reason);
                    }
                    if let Some(source) = estimate.source {
                        let observations = amount
                            .provenance
                            .as_object_mut()
                            .expect("credit provenance object")
                            .entry("observations")
                            .or_insert_with(|| serde_json::json!({}));
                        observations[source] = serde_json::json!(estimate.observed_at);
                    }
                    if estimate.amount.complete.is_some() {
                        amount.add(estimate.amount.known);
                    }
                }
                Metric::Quota => {
                    if event.account_id.is_none() {
                        amount.assume("account_identity_missing");
                    }
                    if event.account_source == "creator_account_assumption" {
                        amount.assume("creator_account_not_verified_on_resume");
                    }
                    if event.timestamp_source == "session" {
                        amount.assume("session_timestamp_fallback");
                    }
                    amount.assume("configured_context_and_workload_policy");
                    let guardian = event.guardian_review || event.model == "codex-auto-review";
                    let result = context
                        .quota
                        .as_ref()
                        .ok_or("quota_config_unavailable")
                        .and_then(|q| {
                            q.estimate(
                                &event.model,
                                event.at,
                                &event.service_tier,
                                event.usage,
                                guardian,
                                context.window,
                            )
                        });
                    match result {
                        Ok(contribution) => {
                            amount.add(contribution.amount);
                            *amount
                                .period_contributions
                                .entry(contribution.period_id)
                                .or_default() += contribution.amount;
                            amount.extrapolated_events += u64::from(contribution.extrapolated);
                        }
                        Err(reason) => amount.unsupported(reason),
                    }
                }
            }
        }
    }

    pub(crate) fn finish(
        &self,
        catalog: &Catalog,
        usd: Option<f64>,
        known_usd: f64,
        incomplete_usage: bool,
    ) -> Bundle {
        let context = &catalog.cost_context;
        let mut costs = BTreeMap::new();
        for metric in &context.selection.0 {
            let mut amount = self
                .entries
                .get(metric)
                .cloned()
                .unwrap_or_else(|| Amount::new(*metric, context));
            if *metric == Metric::ApiUsd {
                amount.estimated = usd;
                amount.known_minimum = known_usd;
                if usd.is_none() {
                    amount.unsupported("api_coverage_or_input_incomplete");
                }
            } else if incomplete_usage {
                amount.unsupported("input_incomplete");
            }
            if *metric == Metric::Quota && self.accounts.len() > 1 {
                amount.reject_account_mix();
            }
            amount.known_minimum = round(amount.known_minimum);
            amount.estimated = amount.estimated.map(round);
            for value in amount.period_contributions.values_mut() {
                *value = round(*value);
            }
            costs.insert(*metric, amount);
        }
        Bundle {
            cost_metrics: context.selection.0.clone(),
            costs,
            accounts: self.accounts.clone(),
        }
    }
}

fn round(value: f64) -> f64 {
    if value.abs() < f64::MAX / 1e8 {
        (value * 1e8).round() / 1e8
    } else {
        value
    }
}

pub(crate) fn display(metric: Metric, amount: &Amount, window: Window) -> String {
    let plus = if amount.estimated.is_none() { "+" } else { "" };
    let known = amount.known_minimum;
    match metric {
        Metric::ApiUsd => format!("${}{plus}", decimal(known)),
        Metric::Credits => {
            if (known > 0.0 || amount.positive) && known < 0.01 {
                format!("<¤0.01{plus}")
            } else {
                format!("¤{}{plus}", decimal(known))
            }
        }
        Metric::Quota => {
            if amount.estimated.is_none() && amount.supported_events == 0 {
                format!("~?pp/{}", window.suffix())
            } else if (known > 0.0 || amount.positive) && known < 0.01 {
                format!("~<0.01pp/{}{plus}", window.suffix())
            } else {
                format!("~{}pp/{}{plus}", decimal(known), window.suffix())
            }
        }
    }
}

fn decimal(value: f64) -> String {
    let value = if value < f64::MAX / 100.0 {
        (value * 100.0).round() / 100.0
    } else {
        value
    };
    let number = format!("{value:.2}");
    let (whole, fraction) = number.split_once('.').unwrap_or((&number, "00"));
    let mut output = String::new();
    for (i, digit) in whole.chars().enumerate() {
        if i > 0 && (whole.len() - i).is_multiple_of(3) {
            output.push(',');
        }
        output.push(digit);
    }
    format!("{output}.{fraction}")
}

impl Bundle {
    pub(crate) fn merge(&mut self, other: &Self) {
        if self.cost_metrics.is_empty() {
            *self = other.clone();
            return;
        }
        self.accounts.extend(other.accounts.iter().cloned());
        for (metric, value) in &other.costs {
            let Some(total) = self.costs.get_mut(metric) else {
                self.costs.insert(*metric, value.clone());
                continue;
            };
            total.known_minimum += value.known_minimum;
            total.estimated = total.estimated.zip(value.estimated).map(|(a, b)| a + b);
            total.supported_events += value.supported_events;
            total.extrapolated_events += value.extrapolated_events;
            total.positive |= value.positive;
            for (reason, count) in &value.unsupported_events {
                *total.unsupported_events.entry(reason.clone()).or_default() += count;
            }
            for (reason, count) in &value.assumed_events {
                *total.assumed_events.entry(reason.clone()).or_default() += count;
            }
            for (period, amount) in &value.period_contributions {
                *total
                    .period_contributions
                    .entry(period.clone())
                    .or_default() += amount;
            }
        }
        if self.accounts.len() > 1
            && let Some(quota) = self.costs.get_mut(&Metric::Quota)
        {
            quota.reject_account_mix();
        }
    }

    pub(crate) fn human(&self, default_estimated: Option<f64>, default_known: f64) -> String {
        if self.cost_metrics.is_empty() || self.cost_metrics == [Metric::ApiUsd] {
            return crate::output::human_cost(default_estimated, default_known);
        }
        self.cost_metrics
            .iter()
            .map(|metric| {
                let amount = &self.costs[metric];
                let window = if amount.provenance["window"] == "five-hour" {
                    Window::FiveHour
                } else {
                    Window::Weekly
                };
                format!("{} {}", metric.name(), display(*metric, amount, window))
            })
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::Usage;
    use time::macros::datetime;

    fn event(account: &str) -> UsageEvent {
        UsageEvent {
            model: "gpt-6.1-sol".into(),
            at: Some(datetime!(2026-11-02 00:00 UTC)),
            service_tier: ServiceTier::Standard,
            usage: Usage {
                input: 100_000,
                cached_input: 80_000,
                cache_write_input: 5_000,
                output: 10_000,
            },
            reasoning_output: 9_000,
            account_id: Some(account.into()),
            account_source: "response_metadata".into(),
            guardian_review: false,
            timestamp_source: "event".into(),
        }
    }

    fn catalog() -> Catalog {
        let mut catalog = Catalog::embedded().unwrap();
        let config = crate::quota::Config::parse(crate::quota::PRESET).unwrap();
        catalog.cost_context = Context {
            selection: "all".parse().unwrap(),
            window: Window::Weekly,
            quota: Some(Rc::new(Snapshot {
                path: "/synthetic/quota.toml".into(),
                fingerprint: "test".into(),
                config,
            })),
        };
        catalog
    }

    #[test]
    fn multiple_accounts_refuse_quota_mixing_without_suppressing_credits() {
        let catalog = catalog();
        let mut ledger = Ledger::default();
        ledger.add(&event("one"), &catalog);
        ledger.add(&event("two"), &catalog);
        let bundle = ledger.finish(&catalog, Some(1.0), 1.0, false);
        assert!(bundle.costs[&Metric::Quota].estimated.is_none());
        assert_eq!(bundle.costs[&Metric::Quota].known_minimum, 0.0);
        assert_eq!(bundle.costs[&Metric::Credits].estimated, Some(7.4));
        assert_eq!(bundle.costs[&Metric::ApiUsd].estimated, Some(1.0));
    }

    #[test]
    fn human_tier_merging_cannot_reintroduce_mixed_account_quota() {
        let catalog = catalog();
        let mut standard = Ledger::default();
        standard.add(&event("account-for-merge-a"), &catalog);
        let mut assumed = Ledger::default();
        let mut other = event("account-for-merge-b");
        other.service_tier = ServiceTier::AssumedStandard;
        assumed.add(&other, &catalog);
        let mut total = standard.finish(&catalog, Some(1.0), 1.0, false);
        total.merge(&assumed.finish(&catalog, Some(1.0), 1.0, false));
        assert!(total.costs[&Metric::Quota].estimated.is_none());
        assert_eq!(total.costs[&Metric::Quota].known_minimum, 0.0);
        assert_eq!(total.costs[&Metric::Credits].estimated, Some(7.4));
        assert!(
            !serde_json::to_string(&total)
                .unwrap()
                .contains("account-for-merge")
        );
    }

    #[test]
    fn zero_guardian_is_supported_but_not_inferred_from_ordinary_review_usage() {
        let catalog = catalog();
        let mut review = event("one");
        let mut ledger = Ledger::default();
        ledger.add(&review, &catalog);
        assert!(
            ledger.finish(&catalog, Some(1.0), 1.0, false).costs[&Metric::Quota].known_minimum
                > 0.0
        );
        review.guardian_review = true;
        review.model = "unknown-guardian-model".into();
        let mut ledger = Ledger::default();
        ledger.add(&review, &catalog);
        let bundle = ledger.finish(&catalog, Some(1.0), 1.0, false);
        assert_eq!(bundle.costs[&Metric::Quota].estimated, Some(0.0));
        assert!(bundle.costs[&Metric::Credits].estimated.is_none());
    }

    #[test]
    fn fractional_values_aggregate_before_display_rounding() {
        let catalog = catalog();
        let mut tiny = event("one");
        tiny.usage = Usage {
            input: 1,
            ..Usage::default()
        };
        let mut ledger = Ledger::default();
        for _ in 0..300 {
            ledger.add(&tiny, &catalog);
        }
        let bundle = ledger.finish(&catalog, Some(0.0), 0.0, false);
        assert_eq!(bundle.costs[&Metric::Credits].estimated, Some(0.015));
        assert_eq!(
            display(
                Metric::Credits,
                &bundle.costs[&Metric::Credits],
                Window::Weekly
            ),
            "¤0.02"
        );
        let mut ledger = Ledger::default();
        ledger.add(&tiny, &catalog);
        let bundle = ledger.finish(&catalog, Some(0.0), 0.0, false);
        assert_eq!(
            display(
                Metric::Credits,
                &bundle.costs[&Metric::Credits],
                Window::Weekly
            ),
            "<¤0.01"
        );
        assert_eq!(
            display(Metric::Quota, &bundle.costs[&Metric::Quota], Window::Weekly),
            "~<0.01pp/wk"
        );
    }
}
