//! User-owned, event-time subscription policy; independent of both price cards.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::pricing::{ServiceTier, Usage};

pub(crate) const PRESET: &str = include_str!("../data/quota-pro-20x.toml");

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd, ValueEnum,
)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Window {
    #[default]
    Weekly,
    FiveHour,
}

impl Window {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::FiveHour => "five-hour",
        }
    }
    pub(crate) fn suffix(self) -> &'static str {
        match self {
            Self::Weekly => "wk",
            Self::FiveHour => "5h",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    schema_version: u32,
    pub(crate) periods: Vec<Period>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Period {
    pub(crate) id: String,
    pub(crate) subscription_plan: String,
    pub(crate) subscription_variant: String,
    #[serde(with = "time::serde::rfc3339")]
    pub(crate) effective_from: OffsetDateTime,
    #[serde(default, with = "time::serde::rfc3339::option")]
    pub(crate) effective_until: Option<OffsetDateTime>,
    pub(crate) calibration_id: String,
    #[serde(with = "time::serde::rfc3339")]
    pub(crate) calibrated_from: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub(crate) calibrated_through: OffsetDateTime,
    pub(crate) notes: String,
    pub(crate) windows: BTreeMap<Window, Coefficient>,
    pub(crate) defaults: Factors,
    pub(crate) workload_multipliers: Workloads,
    pub(crate) models: BTreeMap<String, Weights>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Coefficient {
    pub(crate) points_per_reference_unit: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Speeds {
    pub(crate) standard: Option<f64>,
    pub(crate) fast: Option<f64>,
    pub(crate) ultrafast: Option<f64>,
}

impl Speeds {
    fn get(&self, tier: &ServiceTier) -> Option<f64> {
        match tier {
            ServiceTier::Standard | ServiceTier::AssumedStandard => self.standard,
            ServiceTier::Fast => self.fast,
            ServiceTier::Ultrafast => self.ultrafast,
            ServiceTier::Unpriced(_) => None,
        }
    }
    fn validate(&self) -> Result<(), String> {
        for n in [self.standard, self.fast, self.ultrafast]
            .into_iter()
            .flatten()
        {
            number(n, false)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ContextBand {
    pub(crate) min_input_tokens: u64,
    pub(crate) max_input_tokens: Option<u64>,
    pub(crate) multiplier: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Factors {
    #[serde(default)]
    pub(crate) speed_multipliers: Speeds,
    #[serde(default)]
    pub(crate) context_bands: Vec<ContextBand>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Workloads {
    pub(crate) ordinary: Option<f64>,
    pub(crate) guardian_review: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Weights {
    pub(crate) input: f64,
    pub(crate) cached_input: f64,
    pub(crate) output: f64,
    pub(crate) cache_write_input: Option<f64>,
    #[serde(default)]
    pub(crate) speed_multipliers: Speeds,
    pub(crate) context_bands: Option<Vec<ContextBand>>,
}

#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    pub(crate) path: PathBuf,
    pub(crate) fingerprint: String,
    pub(crate) config: Config,
}

fn number(n: f64, positive: bool) -> Result<(), String> {
    if !n.is_finite() || n < 0.0 || (positive && n == 0.0) {
        Err(if positive {
            "expected a finite positive number"
        } else {
            "expected a finite nonnegative number"
        }
        .into())
    } else {
        Ok(())
    }
}

fn bands(bands: &[ContextBand]) -> Result<(), String> {
    let mut sorted = bands.iter().collect::<Vec<_>>();
    sorted.sort_by_key(|band| band.min_input_tokens);
    for (i, band) in sorted.iter().enumerate() {
        number(band.multiplier, false)?;
        if band
            .max_input_tokens
            .is_some_and(|max| max < band.min_input_tokens)
        {
            return Err("inverted context band".into());
        }
        if i > 0
            && sorted[i - 1]
                .max_input_tokens
                .is_none_or(|max| max >= band.min_input_tokens)
        {
            return Err("overlapping context bands".into());
        }
    }
    Ok(())
}

impl Config {
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let config: Self = toml::from_str(text).map_err(|error| error.to_string())?;
        if config.schema_version != 1 {
            return Err("schema_version: supported version is 1".into());
        }
        let mut ids = BTreeSet::new();
        for period in &config.periods {
            let validate = || -> Result<(), String> {
                if !ids.insert(&period.id) {
                    return Err("duplicate period id".into());
                }
                for label in [
                    &period.id,
                    &period.subscription_plan,
                    &period.subscription_variant,
                    &period.calibration_id,
                    &period.notes,
                ] {
                    if label.trim().is_empty() {
                        return Err(
                            "plan, variant, id, calibration_id and notes must be nonempty".into(),
                        );
                    }
                }
                if period
                    .effective_until
                    .is_some_and(|end| end <= period.effective_from)
                {
                    return Err("inverted effective period".into());
                }
                if period.calibrated_through < period.calibrated_from {
                    return Err("inverted calibration bounds".into());
                }
                for coefficient in period.windows.values() {
                    number(coefficient.points_per_reference_unit, true)?;
                }
                period.defaults.speed_multipliers.validate()?;
                bands(&period.defaults.context_bands)?;
                for n in [
                    period.workload_multipliers.ordinary,
                    period.workload_multipliers.guardian_review,
                ]
                .into_iter()
                .flatten()
                {
                    number(n, false)?;
                }
                for (model, weights) in &period.models {
                    let validate = || -> Result<(), String> {
                        if model.trim().is_empty() {
                            return Err("empty exact model id".into());
                        }
                        for n in [
                            Some(weights.input),
                            Some(weights.cached_input),
                            Some(weights.output),
                            weights.cache_write_input,
                        ]
                        .into_iter()
                        .flatten()
                        {
                            number(n, false)?;
                        }
                        weights.speed_multipliers.validate()?;
                        if let Some(context) = &weights.context_bands {
                            bands(context)?;
                        }
                        Ok(())
                    };
                    validate().map_err(|error| format!("models.{model}: {error}"))?;
                }
                Ok(())
            };
            // Keep period/key context in semantic errors as well as TOML decoding errors.
            let mut validate = validate;
            validate().map_err(|error| format!("periods.{}: {error}", period.id))?;
        }
        let mut sorted = config.periods.iter().collect::<Vec<_>>();
        sorted.sort_by_key(|period| period.effective_from);
        for pair in sorted.windows(2) {
            if pair[0]
                .effective_until
                .is_none_or(|end| end > pair[1].effective_from)
            {
                return Err(format!("periods.{} overlaps {}", pair[0].id, pair[1].id));
            }
        }
        Ok(config)
    }
}

pub(crate) fn resolve(explicit: Option<&Path>) -> Result<PathBuf, String> {
    let path = match explicit {
        Some(path) => path.to_owned(),
        None => match std::env::var_os("CODEX_HOME").filter(|value| !value.is_empty()) {
            Some(home) => PathBuf::from(home).join("codex-cost-meter/quota.toml"),
            None => crate::cli::user_home()
                .map_err(|error| error.to_string())?
                .join(".codex/codex-cost-meter/quota.toml"),
        },
    };
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| error.to_string())
    }
}

impl Snapshot {
    pub(crate) fn load(path: PathBuf) -> Result<Self, String> {
        let text = fs::read_to_string(&path).map_err(|error| {
            format!(
                "{}: {error}; use quota-config init to create a preset",
                path.display()
            )
        })?;
        let config =
            Config::parse(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        // Stable content identity, not a signature or security boundary.
        let hash = text.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
        Ok(Self {
            path,
            config,
            fingerprint: format!("fnv1a64:{hash:016x}"),
        })
    }

    pub(crate) fn estimate(
        &self,
        model: &str,
        at: Option<OffsetDateTime>,
        tier: &ServiceTier,
        usage: Usage,
        guardian: bool,
        window: Window,
    ) -> Result<Contribution, &'static str> {
        let at = at.ok_or("quota_timestamp_unavailable")?;
        let period = self
            .config
            .periods
            .iter()
            .find(|p| at >= p.effective_from && p.effective_until.is_none_or(|end| at < end))
            .ok_or("quota_period_unavailable")?;
        let coefficient = period
            .windows
            .get(&window)
            .ok_or("quota_window_uncalibrated")?
            .points_per_reference_unit;
        let partition = usage
            .cached_input
            .checked_add(usage.cache_write_input)
            .filter(|n| *n <= usage.input)
            .ok_or("invalid_token_partition")?;
        let workload = if guardian {
            period.workload_multipliers.guardian_review
        } else {
            period.workload_multipliers.ordinary
        }
        .ok_or("quota_workload_unavailable")?;
        let contribution = |amount| Contribution {
            amount,
            period_id: period.id.clone(),
            extrapolated: at > period.calibrated_through,
        };
        if workload == 0.0 {
            return Ok(contribution(0.0));
        }
        let weights = period.models.get(model).ok_or("quota_model_unavailable")?;
        let speed = weights
            .speed_multipliers
            .get(tier)
            .or_else(|| period.defaults.speed_multipliers.get(tier))
            .ok_or("quota_speed_unavailable")?;
        let context_bands = weights
            .context_bands
            .as_ref()
            .unwrap_or(&period.defaults.context_bands);
        let context = context_bands
            .iter()
            .find(|band| {
                usage.input >= band.min_input_tokens
                    && band.max_input_tokens.is_none_or(|max| usage.input <= max)
            })
            .ok_or("quota_context_unavailable")?
            .multiplier;
        let reference = ((usage.input - partition) as f64 * weights.input
            + usage.cached_input as f64 * weights.cached_input
            + usage.cache_write_input as f64 * weights.cache_write_input.unwrap_or(weights.input)
            + usage.output as f64 * weights.output)
            / 1_000_000.0;
        let amount = reference * coefficient * speed * context * workload;
        if !amount.is_finite() {
            return Err("quota_arithmetic_overflow");
        }
        Ok(contribution(amount))
    }

    pub(crate) fn provenance(&self, window: Window) -> serde_json::Value {
        serde_json::json!({
            "window": window.name(), "config_path": self.path,
            "window_configured": self.config.periods.iter().any(|period| period.windows.contains_key(&window)),
            "schema_version": 1, "config_fingerprint": self.fingerprint,
            "periods": self.config.periods,
            "basis": "approximate modeled allowance points; supported subtotals are not statistical lower bounds",
            "assumptions": ["single-account policy; missing account identity is assumed", "applied tier is not verified served tier", "configured context and workload factors are policy assumptions"]
        })
    }
}

pub(crate) struct Contribution {
    pub(crate) amount: f64,
    pub(crate) period_id: String,
    pub(crate) extrapolated: bool,
}

pub(crate) fn init(path: &Path) -> Result<(), String> {
    if fs::symlink_metadata(path).is_ok() {
        return Err(format!(
            "{}: already exists; refusing to overwrite",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("{}: {error}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    file.write_all(PRESET.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn snapshot(text: &str) -> Snapshot {
        Snapshot {
            path: PathBuf::from("/synthetic/quota.toml"),
            fingerprint: "test".into(),
            config: Config::parse(text).unwrap(),
        }
    }

    fn two_periods(end: &str) -> String {
        let first = PRESET.replace(
            "effective_from = \"2026-10-16T00:00:00Z\"",
            &format!("effective_from = \"2026-10-16T00:00:00Z\"\neffective_until = \"{end}\""),
        );
        let second = PRESET
            .split_once("[[periods]]")
            .unwrap()
            .1
            .replace("owner-pro-20x-2026-10", "second")
            .replace(
                "effective_from = \"2026-10-16T00:00:00Z\"",
                "effective_from = \"2026-11-01T00:00:00Z\"",
            )
            .replace("0.08236824622767631", "1.0");
        format!("{first}\n[[periods]]{second}")
    }

    #[test]
    fn event_periods_use_exclusive_ends_without_backfilling_gaps() {
        let s = snapshot(&two_periods("2026-11-01T00:00:00Z"));
        let usage = Usage {
            input: 1_000_000,
            ..Usage::default()
        };
        let estimate = |at| {
            s.estimate(
                "gpt-6.1-sol",
                Some(at),
                &ServiceTier::Standard,
                usage,
                false,
                Window::Weekly,
            )
        };
        assert_eq!(
            estimate(datetime!(2026-10-31 23:59:59 UTC))
                .unwrap()
                .period_id,
            "owner-pro-20x-2026-10"
        );
        let next = estimate(datetime!(2026-11-01 00:00 UTC)).unwrap();
        assert_eq!(next.period_id, "second");
        assert_eq!(next.amount, 2.0);
        assert!(next.extrapolated);
        assert!(estimate(datetime!(2026-10-15 23:59:59 UTC)).is_err());
        assert!(Config::parse(&two_periods("2026-11-01T00:00:01Z")).is_err());
        assert!(Config::parse(&two_periods("2026-10-15T00:00:00Z")).is_err());
        assert!(
            Config::parse(
                &two_periods("2026-11-01T00:00:00Z")
                    .replace("id = \"second\"", "id = \"owner-pro-20x-2026-10\"")
            )
            .is_err()
        );
    }

    #[test]
    fn model_overrides_replace_defaults_once_per_dimension() {
        let text = PRESET.replace("standard = 1.0", "standard = 1.0\nfast = 3.0").replace(
            "\"gpt-6.1-sol\" = { input = 2.0, cached_input = 0.1, output = 10.0 }",
            "\"gpt-6.1-sol\" = { input = 2.0, cached_input = 0.1, output = 10.0, speed_multipliers = { fast = 2.0 }, context_bands = [{ min_input_tokens = 0, max_input_tokens = 272000, multiplier = 4.0 }, { min_input_tokens = 272001, multiplier = 5.0 }] }",
        );
        let s = snapshot(&text);
        for (input, context) in [(272_000, 4.0), (272_001, 5.0)] {
            let usage = Usage {
                input,
                ..Usage::default()
            };
            let amount = s
                .estimate(
                    "gpt-6.1-sol",
                    Some(datetime!(2026-10-20 00:00 UTC)),
                    &ServiceTier::Fast,
                    usage,
                    false,
                    Window::Weekly,
                )
                .unwrap()
                .amount;
            assert!(
                (amount - input as f64 * 2.0 / 1e6 * 0.08236824622767631 * 2.0 * context).abs()
                    < 1e-14
            );
        }
        assert_eq!(
            s.estimate(
                "unknown-review-agent",
                Some(datetime!(2026-10-20 00:00 UTC)),
                &ServiceTier::Ultrafast,
                Usage::default(),
                true,
                Window::Weekly
            )
            .unwrap()
            .amount,
            0.0
        );
        assert!(
            s.estimate(
                "unknown-review-agent",
                Some(datetime!(2026-10-20 00:00 UTC)),
                &ServiceTier::Standard,
                Usage::default(),
                false,
                Window::Weekly
            )
            .is_err()
        );
    }

    #[test]
    fn preset_arithmetic_preserves_inclusive_components() {
        let snapshot = snapshot(PRESET);
        let usage = Usage {
            input: 1_000_000,
            cached_input: 800_000,
            cache_write_input: 50_000,
            output: 100_000,
        };
        let amount = snapshot
            .estimate(
                "gpt-6.1-sol",
                Some(datetime!(2026-10-20 00:00 UTC)),
                &ServiceTier::Standard,
                usage,
                false,
                Window::Weekly,
            )
            .unwrap()
            .amount;
        assert!((amount - 0.12190500441696094).abs() < 1e-14);
        assert!(
            snapshot
                .estimate(
                    "gpt-6.1-sol",
                    Some(datetime!(2026-10-20 00:00 UTC)),
                    &ServiceTier::Fast,
                    usage,
                    false,
                    Window::Weekly
                )
                .is_err()
        );
        assert!(
            snapshot
                .estimate(
                    "gpt-6.1-sol",
                    Some(datetime!(2026-10-20 00:00 UTC)),
                    &ServiceTier::Standard,
                    usage,
                    false,
                    Window::FiveHour
                )
                .is_err()
        );
    }

    #[test]
    fn invalid_policy_cannot_silently_default() {
        for text in [
            PRESET.replace("schema_version = 1", "schema_version = 2"),
            PRESET.replace("schema_version = 1", "schema_version = 1\nunknown_field = true"),
            format!("{PRESET}\nunknown = 1"),
            PRESET.replace("0.08236824622767631", "-1"),
            PRESET.replace("0.08236824622767631", "nan"),
            PRESET.replace("0.08236824622767631", "inf"),
            PRESET.replace("min_input_tokens = 0, multiplier = 1.0", "min_input_tokens = 10, max_input_tokens = 9, multiplier = 1.0"),
            PRESET.replace("context_bands = [{ min_input_tokens = 0, multiplier = 1.0 }]", "context_bands = [{ min_input_tokens = 0, max_input_tokens = 10, multiplier = 1.0 }, { min_input_tokens = 10, multiplier = 1.0 }]"),
            format!("{PRESET}\n[[periods]]\nid = 'broken'"),
            PRESET.replace("effective_from = \"2026-10-16T00:00:00Z\"", "effective_from = \"not-a-date\""),
        ] {
            assert!(Config::parse(&text).is_err(), "{text}");
        }
    }

    #[test]
    fn init_is_create_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/quota.toml");
        init(&path).unwrap();
        Snapshot::load(path.clone()).unwrap();
        fs::write(&path, "user data").unwrap();
        assert!(init(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "user data");
        #[cfg(unix)]
        {
            let link = dir.path().join("dangling.toml");
            std::os::unix::fs::symlink(dir.path().join("absent"), &link).unwrap();
            assert!(init(&link).is_err());
            assert!(fs::symlink_metadata(link).unwrap().file_type().is_symlink());
        }
    }
}
