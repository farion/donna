use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CronDateTime {
    pub minute: u8,
    pub hour: u8,
    pub day_of_month: u8,
    pub month: u8,
    pub day_of_week: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronSchedule {
    source: String,
    minutes: CronField,
    hours: CronField,
    days_of_month: CronField,
    months: CronField,
    days_of_week: CronField,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CronField {
    values: Vec<bool>,
    min: u8,
    wildcard: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CronParseError {
    message: String,
}

impl CronSchedule {
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn matches(&self, time: CronDateTime) -> bool {
        let dow = if time.day_of_week == 7 {
            0
        } else {
            time.day_of_week
        };
        let dom_matches = self.days_of_month.contains(time.day_of_month);
        let dow_matches = self.days_of_week.contains(dow);
        let day_matches = if !self.days_of_month.wildcard && !self.days_of_week.wildcard {
            dom_matches || dow_matches
        } else {
            dom_matches && dow_matches
        };

        self.minutes.contains(time.minute)
            && self.hours.contains(time.hour)
            && day_matches
            && self.months.contains(time.month)
    }
}

impl FromStr for CronSchedule {
    type Err = CronParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let fields = value.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 5 {
            return Err(CronParseError::new(
                "cron expressions must have five fields: minute hour day month weekday",
            ));
        }

        Ok(Self {
            source: value.to_owned(),
            minutes: CronField::parse(fields[0], 0, 59, false)?,
            hours: CronField::parse(fields[1], 0, 23, false)?,
            days_of_month: CronField::parse(fields[2], 1, 31, false)?,
            months: CronField::parse(fields[3], 1, 12, false)?,
            days_of_week: CronField::parse(fields[4], 0, 6, true)?,
        })
    }
}

impl CronField {
    fn parse(raw: &str, min: u8, max: u8, seven_is_zero: bool) -> Result<Self, CronParseError> {
        let mut field = Self {
            values: vec![false; (max - min + 1) as usize],
            min,
            wildcard: raw == "*",
        };

        for part in raw.split(',') {
            field.apply_part(part.trim(), min, max, seven_is_zero)?;
        }

        Ok(field)
    }

    fn apply_part(
        &mut self,
        raw: &str,
        min: u8,
        max: u8,
        seven_is_zero: bool,
    ) -> Result<(), CronParseError> {
        if raw.is_empty() {
            return Err(CronParseError::new("empty cron field segment"));
        }

        let (range_raw, step) = match raw.split_once('/') {
            Some((range, step)) => {
                let step = parse_number(step, "step")?;
                if step == 0 {
                    return Err(CronParseError::new("cron step must be greater than zero"));
                }
                (range, step)
            }
            None => (raw, 1),
        };

        let (start, end) = if range_raw == "*" {
            self.wildcard = true;
            (min, max)
        } else if let Some((start, end)) = range_raw.split_once('-') {
            (
                normalize(parse_number(start, "range start")?, seven_is_zero)?,
                normalize(parse_number(end, "range end")?, seven_is_zero)?,
            )
        } else {
            let value = normalize(parse_number(range_raw, "value")?, seven_is_zero)?;
            (value, value)
        };

        if start < min || end > max || start > end {
            return Err(CronParseError::new(format!(
                "cron value {start}-{end} is outside allowed range {min}-{max}"
            )));
        }

        let mut value = start;
        while value <= end {
            self.set(value);
            match value.checked_add(step) {
                Some(next) => value = next,
                None => break,
            }
        }

        Ok(())
    }

    fn set(&mut self, value: u8) {
        self.values[(value - self.min) as usize] = true;
    }

    fn contains(&self, value: u8) -> bool {
        if value < self.min {
            return false;
        }
        self.values
            .get((value - self.min) as usize)
            .copied()
            .unwrap_or(false)
    }
}

impl CronParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

fn parse_number(raw: &str, label: &str) -> Result<u8, CronParseError> {
    raw.parse::<u8>()
        .map_err(|_| CronParseError::new(format!("invalid cron {label}: {raw}")))
}

fn normalize(value: u8, seven_is_zero: bool) -> Result<u8, CronParseError> {
    if seven_is_zero && value == 7 {
        Ok(0)
    } else {
        Ok(value)
    }
}

impl Display for CronParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid cron expression: {}", self.message)
    }
}

impl Error for CronParseError {}

#[cfg(test)]
mod tests {
    use super::{CronDateTime, CronSchedule};
    use std::str::FromStr;

    #[test]
    fn parses_step_cron_expression() {
        let schedule = CronSchedule::from_str("*/15 9-17 * * 1-5").expect("cron");

        assert!(schedule.matches(CronDateTime {
            minute: 30,
            hour: 10,
            day_of_month: 12,
            month: 6,
            day_of_week: 2,
        }));
        assert!(!schedule.matches(CronDateTime {
            minute: 31,
            hour: 10,
            day_of_month: 12,
            month: 6,
            day_of_week: 2,
        }));
    }

    #[test]
    fn rejects_invalid_cron_expression() {
        let error = CronSchedule::from_str("60 * * * *").expect_err("invalid");

        assert!(error.to_string().contains("outside allowed range"));
    }
}
