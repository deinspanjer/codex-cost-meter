//! Sparse user pricing data. Keys identify one whole set of component rates.
use super::{Catalog, PricingError, ProxyPoint, Rates, ServiceTier};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use time::Date;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Basis {
    Official,
    Observed,
    Researched,
    #[default]
    Custom,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Tier {
    Standard,
    Fast,
    Ultrafast,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Band {
    Short,
    Long,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub(crate) struct PointKey {
    pub model: String,
    pub tier: Tier,
    pub context: Band,
    pub effective_from: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Point {
    #[serde(flatten)]
    pub key: PointKey,
    pub rates: Rates,
    #[serde(default)]
    pub basis: Basis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Removal {
    #[serde(flatten)]
    pub key: PointKey,
    #[serde(default)]
    pub basis: Basis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ThresholdOverride {
    pub model: String,
    pub threshold: Option<u64>,
    #[serde(default)]
    pub basis: Basis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProxyOverride {
    pub proxy: String,
    pub points: Vec<ProxyEntry>,
    #[serde(default)]
    pub basis: Basis,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProxyEntry {
    #[serde(default)]
    pub effective_from: Option<String>,
    pub target: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Overlay {
    pub schema_version: u32,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    #[serde(default)]
    pub points: Vec<Point>,
    #[serde(default)]
    pub removals: Vec<Removal>,
    #[serde(default)]
    pub thresholds: Vec<ThresholdOverride>,
    #[serde(default)]
    pub proxies: Vec<ProxyOverride>,
}
impl Default for Overlay {
    fn default() -> Self {
        Self {
            schema_version: 1,
            version: "1".into(),
            label: None,
            base: None,
            points: vec![],
            removals: vec![],
            thresholds: vec![],
            proxies: vec![],
        }
    }
}
fn invalid(message: impl Into<String>) -> PricingError {
    PricingError::Overlay(message.into())
}
fn date(s: &str) -> Result<Date, PricingError> {
    let d = Date::parse(s, time::macros::format_description!("[year]-[month]-[day]"))
        .map_err(|_| invalid(format!("invalid date {s}")))?;
    if d.to_string() != s {
        return Err(invalid(format!("invalid date {s}")));
    }
    Ok(d)
}
impl Overlay {
    pub(crate) fn parse(s: &str) -> Result<Self, PricingError> {
        let o: Self = serde_json::from_str(s)?;
        o.validate()?;
        Ok(o)
    }
    fn validate(&self) -> Result<(), PricingError> {
        if self.schema_version != 1 {
            return Err(invalid(format!(
                "unsupported schema {}; supported schema is 1",
                self.schema_version
            )));
        }
        if self.base.as_deref().is_some_and(|b| b != "none") || self.version.trim().is_empty() {
            return Err(invalid("invalid base or empty version"));
        }
        let mut keys = BTreeSet::new();
        for p in &self.points {
            validate_key(&p.key)?;
            if !keys.insert(&p.key) {
                return Err(invalid("duplicate point or removal"));
            }
            if p.rates.values().any(|r| !r.is_finite() || r < 0.0) {
                return Err(invalid("rates must be finite and nonnegative"));
            }
        }
        for p in &self.removals {
            validate_key(&p.key)?;
            if !keys.insert(&p.key) {
                return Err(invalid("duplicate point or removal"));
            }
        }
        let mut names = BTreeSet::new();
        for t in &self.thresholds {
            if t.model.is_empty()
                || !names.insert(&t.model)
                || t.threshold == Some(0)
                || t.threshold == Some(u64::MAX)
            {
                return Err(invalid("invalid or duplicate threshold"));
            }
        }
        names.clear();
        for p in &self.proxies {
            if p.proxy.is_empty() || !names.insert(&p.proxy) {
                return Err(invalid("invalid or duplicate proxy"));
            }
            let mut prev = None;
            for (i, x) in p.points.iter().enumerate() {
                if x.target.is_empty() {
                    return Err(invalid("empty proxy target"));
                }
                match &x.effective_from {
                    None if i != 0 => return Err(invalid("undated proxy after first point")),
                    None => (),
                    Some(d) => {
                        let d = date(d)?;
                        if prev.is_some_and(|v| d <= v) {
                            return Err(invalid("proxy dates not increasing"));
                        }
                        prev = Some(d);
                    }
                }
            }
        }
        Ok(())
    }
    pub(crate) fn prune_generated(
        &mut self,
        embedded: &Catalog,
    ) -> Result<Vec<Point>, PricingError> {
        if self.base.as_deref() == Some("none") {
            return Ok(vec![]);
        }
        let mut removed = vec![];
        let mut i = 0;
        while i < self.points.len() {
            if self.points[i].basis == Basis::Custom {
                i += 1;
                continue;
            }
            let before = embedded.merge_overlay(self)?;
            let p = self.points.remove(i);
            let after = embedded.merge_overlay(self);
            if after
                .as_ref()
                .is_ok_and(|after| before.equivalent_history(after))
            {
                removed.push(p);
            } else {
                self.points.insert(i, p);
                i += 1;
            }
        }
        Ok(removed)
    }
}
fn validate_key(k: &PointKey) -> Result<(), PricingError> {
    if k.model.trim().is_empty() {
        return Err(invalid("empty model"));
    }
    date(&k.effective_from)?;
    Ok(())
}
impl Catalog {
    pub(crate) fn points(&self) -> &BTreeMap<PointKey, Rates> {
        &self.points
    }
    pub(crate) fn effective_rates(&self, key: &PointKey) -> Option<Rates> {
        self.points
            .iter()
            .rfind(|(k, _)| {
                k.model == key.model
                    && k.tier == key.tier
                    && k.context == key.context
                    && k.effective_from <= key.effective_from
            })
            .map(|(_, v)| *v)
    }
    pub(crate) fn merge_overlay(&self, overlay: &Overlay) -> Result<Self, PricingError> {
        overlay.validate()?;
        let standalone = overlay.base.as_deref() == Some("none");
        let mut c = self.clone();
        if standalone {
            c.points.clear();
            c.histories.clear();
            c.long_context_thresholds.clear();
            c.proxies.clear();
        }
        for p in &overlay.removals {
            c.points.remove(&p.key);
        }
        for p in &overlay.points {
            c.points.insert(p.key.clone(), p.rates);
        }
        for t in &overlay.thresholds {
            if let Some(v) = t.threshold {
                c.long_context_thresholds.insert(t.model.clone(), v);
            } else {
                c.long_context_thresholds.remove(&t.model);
            }
        }
        for p in &overlay.proxies {
            if p.points.is_empty() {
                c.proxies.remove(&p.proxy);
            } else {
                let points = p
                    .points
                    .iter()
                    .map(|x| {
                        Ok(ProxyPoint {
                            effective_from: x.effective_from.as_deref().map(date).transpose()?,
                            target: x.target.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, PricingError>>()?;
                c.proxies.insert(p.proxy.clone(), points);
            }
        }
        c.histories
            .retain(|m, _| c.points.keys().any(|k| &k.model == m));
        for k in c.points.keys() {
            c.histories.entry(k.model.clone()).or_default();
        }
        for (m, t) in &c.long_context_thresholds {
            if *t == 0 || *t == u64::MAX || !c.histories.contains_key(m) {
                return Err(invalid(format!("invalid threshold for {m}")));
            }
        }
        for k in c.points.keys() {
            if k.context == Band::Long && !c.long_context_thresholds.contains_key(&k.model) {
                return Err(invalid(format!(
                    "long-context rates for {} require a threshold",
                    k.model
                )));
            }
        }
        for (p, entries) in &c.proxies {
            for e in entries {
                if !c.histories.contains_key(&e.target) {
                    return Err(invalid(format!(
                        "proxy {p} has unavailable target {}",
                        e.target
                    )));
                }
            }
        }
        c.overlay = Some(overlay.clone());
        Ok(c)
    }
    fn equivalent_history(&self, other: &Self) -> bool {
        let keys: BTreeSet<_> = self
            .points
            .keys()
            .chain(other.points.keys())
            .cloned()
            .collect();
        keys.iter()
            .all(|k| self.effective_rates(k) == other.effective_rates(k))
    }
    pub(crate) fn select_rates(
        &self,
        model: &str,
        tier: &ServiceTier,
        long: bool,
        at: Option<Date>,
    ) -> Option<(String, Rates)> {
        let tier = match tier {
            ServiceTier::Standard | ServiceTier::AssumedStandard => Tier::Standard,
            ServiceTier::Fast => Tier::Fast,
            ServiceTier::Ultrafast => Tier::Ultrafast,
            ServiceTier::Unpriced(_) => return None,
        };
        let context = if long { Band::Long } else { Band::Short };
        let date = at.map(|d| d.to_string());
        self.points
            .iter()
            .rfind(|(k, _)| {
                k.model == model
                    && k.tier == tier
                    && k.context == context
                    && date.as_ref().is_none_or(|d| &k.effective_from <= d)
            })
            .map(|(k, r)| (k.effective_from.clone(), *r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pricing::Usage;
    use time::macros::datetime;
    fn key(date: &str) -> PointKey {
        PointKey {
            model: "gpt-6-sol".into(),
            tier: Tier::Standard,
            context: Band::Short,
            effective_from: date.into(),
        }
    }
    fn point(date: &str, input: f64, basis: Basis) -> Point {
        Point {
            key: key(date),
            rates: Rates {
                input: Some(input),
                ..Rates::default()
            },
            basis,
            source: None,
        }
    }
    #[test]
    fn sparse_override_preserves_other_bands_and_unknown_components() {
        let embedded = Catalog::embedded().unwrap();
        let overlay = Overlay {
            points: vec![point("2026-09-22", 0.01, Basis::Custom)],
            ..Overlay::default()
        };
        let c = embedded.merge_overlay(&overlay).unwrap();
        let input = Usage {
            input: 1_000_000,
            ..Usage::default()
        };
        // This model has a context threshold; choose a short input for direct point checks.
        assert_eq!(c.points()[&key("2026-09-22")].input, Some(0.01));
        assert_eq!(c.points()[&key("2026-09-22")].output, None);
        assert_eq!(
            c.cost(
                "gpt-6-sol",
                Some(datetime!(2026-09-22 0:00 UTC)),
                &ServiceTier::Fast,
                input
            ),
            embedded.cost(
                "gpt-6-sol",
                Some(datetime!(2026-09-22 0:00 UTC)),
                &ServiceTier::Fast,
                input
            )
        );
    }
    #[test]
    fn standalone_removals_and_pruning_preserve_effective_history() {
        let embedded = Catalog::embedded().unwrap();
        let original = embedded.points()[&key("2026-09-22")];
        let mut overlay = Overlay {
            points: vec![Point {
                key: key("2026-10-01"),
                rates: original,
                basis: Basis::Observed,
                source: None,
            }],
            ..Overlay::default()
        };
        assert_eq!(overlay.prune_generated(&embedded).unwrap().len(), 1);
        overlay.points = vec![
            point("2026-09-25", 4.0, Basis::Custom),
            Point {
                key: key("2026-10-01"),
                rates: original,
                basis: Basis::Observed,
                source: None,
            },
        ];
        assert!(overlay.prune_generated(&embedded).unwrap().is_empty());
        overlay.base = Some("none".into());
        assert!(overlay.prune_generated(&embedded).unwrap().is_empty());
        let c = embedded.merge_overlay(&overlay).unwrap();
        assert!(!c.points().keys().any(|k| k.model == "gpt-6-luna"));
        overlay.base = None;
        overlay.points.clear();
        overlay.removals.push(Removal {
            key: key("2026-09-22"),
            basis: Basis::Custom,
            source: None,
        });
        assert!(
            !embedded
                .merge_overlay(&overlay)
                .unwrap()
                .points()
                .contains_key(&key("2026-09-22"))
        );
    }
    #[test]
    fn pruning_multiple_generated_points_preserves_intermediate_changes() {
        let embedded = Catalog::embedded().unwrap();
        let base = embedded.points()[&key("2026-09-22")];
        let changed = Rates {
            input: Some(4.0),
            ..Rates::default()
        };
        let mut overlay = Overlay {
            points: vec![
                Point {
                    key: key("2026-09-25"),
                    rates: changed,
                    basis: Basis::Observed,
                    source: None,
                },
                Point {
                    key: key("2026-09-27"),
                    rates: changed,
                    basis: Basis::Researched,
                    source: None,
                },
                Point {
                    key: key("2026-10-01"),
                    rates: base,
                    basis: Basis::Official,
                    source: None,
                },
            ],
            ..Overlay::default()
        };
        let removed = overlay.prune_generated(&embedded).unwrap();
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0].key.effective_from, "2026-09-27");
        let snapshot = embedded.merge_overlay(&overlay).unwrap();
        assert_eq!(snapshot.effective_rates(&key("2026-09-26")), Some(changed));
        assert_eq!(snapshot.effective_rates(&key("2026-10-01")), Some(base));
    }

    #[test]
    fn parses_and_rejects_conflicting_and_invalid_overlays() {
        let json = r#"{"schema_version":1,"version":"test","points":[{"model":"x","tier":"standard","context":"short","effective_from":"2026-01-01","rates":{"input":0.01}}]}"#;
        let overlay = Overlay::parse(json).unwrap();
        assert!(
            Overlay::parse(&json.replace("\"rates\":", "\"unexpected\":1,\"rates\":")).is_err()
        );
        assert_eq!(overlay.points[0].basis, Basis::Custom);
        assert!(
            Overlay::parse(&json.replace("\"input\":0.01", "\"input\":0.01,\"unsupported\":1"))
                .is_err()
        );
        let mut no_threshold = overlay.clone();
        no_threshold.points[0].key.context = Band::Long;
        assert!(
            Catalog::embedded()
                .unwrap()
                .merge_overlay(&no_threshold)
                .is_err()
        );
        let mut duplicate = overlay.clone();
        duplicate.removals.push(Removal {
            key: duplicate.points[0].key.clone(),
            basis: Basis::Custom,
            source: None,
        });
        assert!(
            Catalog::embedded()
                .unwrap()
                .merge_overlay(&duplicate)
                .is_err()
        );
        let mut invalid = overlay;
        invalid.proxies.push(ProxyOverride {
            proxy: "alias".into(),
            points: vec![ProxyEntry {
                effective_from: None,
                target: "missing".into(),
            }],
            basis: Basis::Custom,
            source: None,
        });
        assert!(
            Catalog::embedded()
                .unwrap()
                .merge_overlay(&invalid)
                .is_err()
        );
    }
}
