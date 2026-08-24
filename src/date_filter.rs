use std::str::FromStr;

use jiff::{ToSpan, civil::Date, tz::TimeZone};
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupDimension {
    Day,
    Week,
    Month,
    RolloutType,
    Model,
}

impl FromStr for GroupDimension {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "day" => Ok(Self::Day),
            "week" => Ok(Self::Week),
            "month" => Ok(Self::Month),
            "rollout-type" => Ok(Self::RolloutType),
            "model" => Ok(Self::Model),
            _ => Err("must be day, week, month, rollout-type, or model".into()),
        }
    }
}

impl GroupDimension {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::RolloutType => "rollout-type",
            Self::Model => "model",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Options {
    pub(crate) since: Option<Date>,
    pub(crate) through: Option<Date>,
    pub(crate) group_by: Vec<GroupDimension>,
    pub(crate) include_empty: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Filter {
    pub(crate) since: Option<Date>,
    pub(crate) through: Option<Date>,
    pub(crate) group_by: Vec<GroupDimension>,
    pub(crate) include_empty: bool,
    pub(crate) timezone: Option<TimeZone>,
}

impl Filter {
    pub(crate) fn is_filtered(&self) -> bool {
        self.since.is_some() || self.through.is_some()
    }

    pub(crate) fn time_dimension(&self) -> Option<GroupDimension> {
        self.group_by.iter().copied().find(|dimension| {
            matches!(
                dimension,
                GroupDimension::Day | GroupDimension::Week | GroupDimension::Month
            )
        })
    }

    pub(crate) fn groups_by(&self, dimension: GroupDimension) -> bool {
        self.group_by.contains(&dimension)
    }

    pub(crate) fn local_date(&self, at: Option<OffsetDateTime>) -> Result<Option<Date>, Error> {
        let Some(at) = at else { return Ok(None) };
        let Some(timezone) = &self.timezone else {
            return Ok(None);
        };
        let timestamp =
            jiff::Timestamp::from_second(at.unix_timestamp()).map_err(Error::Timezone)?;
        Ok(Some(timestamp.to_zoned(timezone.clone()).date()))
    }

    pub(crate) fn includes(&self, at: Option<OffsetDateTime>) -> Result<bool, Error> {
        if !self.is_filtered() {
            return Ok(true);
        }
        let Some(date) = self.local_date(at)? else {
            return Ok(false);
        };
        Ok(self.since.is_none_or(|since| date >= since)
            && self.through.is_none_or(|through| date <= through))
    }

    pub(crate) fn retains_rollout(
        &self,
        created_at: Option<OffsetDateTime>,
        updated_at: Option<OffsetDateTime>,
    ) -> bool {
        let created = self.local_date(created_at).ok().flatten();
        let updated = self.local_date(updated_at).ok().flatten();
        self.since
            .is_none_or(|since| updated.is_none_or(|updated| updated >= since))
            && self
                .through
                .is_none_or(|through| created.is_none_or(|created| created <= through))
    }

    pub(crate) fn bucket_start(&self, date: Date) -> Result<Date, Error> {
        match self.time_dimension() {
            Some(GroupDimension::Day) => Ok(date),
            Some(GroupDimension::Week) => date
                .checked_sub(date.weekday().to_monday_zero_offset().days())
                .map_err(Error::Timezone),
            Some(GroupDimension::Month) => {
                Date::new(date.year(), date.month(), 1).map_err(Error::Timezone)
            }
            _ => Ok(date),
        }
    }

    pub(crate) fn empty_bucket_starts(&self) -> Result<Vec<Date>, Error> {
        if !self.include_empty {
            return Ok(Vec::new());
        }
        let (Some(since), Some(through), Some(dimension)) =
            (self.since, self.through, self.time_dimension())
        else {
            return Ok(Vec::new());
        };
        let mut current = self.bucket_start(since)?;
        let end = self.bucket_start(through)?;
        let mut dates = Vec::new();
        while current <= end {
            dates.push(current);
            current = match dimension {
                GroupDimension::Day => current.tomorrow(),
                GroupDimension::Week => current.checked_add(7.days()),
                GroupDimension::Month => current.checked_add(1.month()),
                _ => unreachable!(),
            }
            .map_err(Error::Timezone)?;
        }
        Ok(dates)
    }
}

#[derive(Debug, Error)]
pub(crate) enum Error {
    #[error("--through must not be earlier than --since")]
    InvertedRange,
    #[error("--include-empty requires both --since and --through")]
    EmptyRange,
    #[error("--group-by requires exactly one of day, week, or month")]
    TimeDimension,
    #[error("--group-by cannot repeat a dimension")]
    DuplicateDimension,
    #[error("date and grouping options are only available for project or --all reports")]
    ExactThread,
    #[error("could not resolve the host local timezone: {0}")]
    Timezone(#[source] jiff::Error),
}

impl TryFrom<Options> for Filter {
    type Error = Error;

    fn try_from(options: Options) -> Result<Self, Self::Error> {
        if options
            .since
            .zip(options.through)
            .is_some_and(|(since, through)| through < since)
        {
            return Err(Error::InvertedRange);
        }
        if options.include_empty && (options.since.is_none() || options.through.is_none()) {
            return Err(Error::EmptyRange);
        }
        let mut unique = options.group_by.clone();
        unique.sort_by_key(|dimension| *dimension as u8);
        if unique.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(Error::DuplicateDimension);
        }
        let time_dimensions = options
            .group_by
            .iter()
            .filter(|dimension| {
                matches!(
                    dimension,
                    GroupDimension::Day | GroupDimension::Week | GroupDimension::Month
                )
            })
            .count();
        if !options.group_by.is_empty() && time_dimensions != 1 {
            return Err(Error::TimeDimension);
        }
        let needs_timezone =
            options.since.is_some() || options.through.is_some() || !options.group_by.is_empty();
        let timezone = needs_timezone
            .then(TimeZone::try_system)
            .transpose()
            .map_err(Error::Timezone)?;
        Ok(Self {
            since: options.since,
            through: options.through,
            group_by: options.group_by,
            include_empty: options.include_empty,
            timezone,
        })
    }
}

pub(crate) fn parse_date(value: &str) -> Result<Date, String> {
    value
        .parse()
        .map_err(|_| "must be an ISO date (YYYY-MM-DD)".into())
}

#[cfg(test)]
mod tests {
    use jiff::tz::TimeZone;

    use super::{Error, Filter, GroupDimension, Options};

    fn date(value: &str) -> jiff::civil::Date {
        value.parse().unwrap()
    }

    #[test]
    fn rejects_invalid_group_shapes_and_unbounded_empty_rows() {
        assert!(super::parse_date("2026-13-01").is_err());
        assert!(matches!(
            Filter::try_from(Options {
                since: None,
                through: None,
                group_by: vec![GroupDimension::Model],
                include_empty: false
            }),
            Err(Error::TimeDimension)
        ));
        assert!(matches!(
            Filter::try_from(Options {
                since: None,
                through: None,
                group_by: vec![GroupDimension::Day, GroupDimension::Week],
                include_empty: false
            }),
            Err(Error::TimeDimension)
        ));
        assert!(matches!(
            Filter::try_from(Options {
                since: None,
                through: None,
                group_by: vec![GroupDimension::Day, GroupDimension::Day],
                include_empty: false
            }),
            Err(Error::DuplicateDimension)
        ));
        assert!(matches!(
            Filter::try_from(Options {
                since: Some(date("2026-08-01")),
                through: None,
                group_by: vec![GroupDimension::Day],
                include_empty: true
            }),
            Err(Error::EmptyRange)
        ));
    }

    #[test]
    fn rejects_inverted_dates() {
        assert!(matches!(
            Filter::try_from(Options {
                since: Some(date("2026-08-02")),
                through: Some(date("2026-08-01")),
                group_by: vec![],
                include_empty: false
            }),
            Err(Error::InvertedRange)
        ));
    }

    #[test]
    fn local_midnights_follow_daylight_saving_transitions() {
        let timezone = TimeZone::get("America/New_York").unwrap();
        let before = date("2026-03-08")
            .to_zoned(timezone.clone())
            .unwrap()
            .timestamp();
        let after = date("2026-03-09").to_zoned(timezone).unwrap().timestamp();
        assert_eq!(after.as_second() - before.as_second(), 23 * 60 * 60);
    }

    #[test]
    fn calendar_buckets_use_monday_weeks_and_local_months() {
        let mut filter = Filter::try_from(Options {
            since: Some(date("2026-08-01")),
            through: Some(date("2026-08-10")),
            group_by: vec![GroupDimension::Week],
            include_empty: true,
        })
        .unwrap();
        assert_eq!(
            filter.bucket_start(date("2026-08-05")).unwrap(),
            date("2026-08-03")
        );
        assert_eq!(
            filter.empty_bucket_starts().unwrap(),
            vec![date("2026-07-27"), date("2026-08-03"), date("2026-08-10")]
        );
        filter.group_by = vec![GroupDimension::Month];
        assert_eq!(
            filter.bucket_start(date("2026-08-05")).unwrap(),
            date("2026-08-01")
        );
    }
}
