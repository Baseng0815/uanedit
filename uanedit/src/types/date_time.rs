use core::cmp::Ordering;
use core::fmt;
use core::hash::{
    Hash,
    Hasher,
};
use core::str::FromStr;

use serde::{
    Deserialize,
    Serialize,
};

use crate::error::ParseError;

const TICKS_PER_SECOND: i64 = 10_000_000;
const TICKS_PER_MINUTE: i64 = 60 * TICKS_PER_SECOND;
const TICKS_PER_HOUR: i64 = 60 * TICKS_PER_MINUTE;
const TICKS_PER_DAY: i64 = 24 * TICKS_PER_HOUR;
/// Days from the DateTime epoch, 1601-01-01, to the Unix epoch.
const EPOCH_OFFSET_DAYS: i64 = 134_774;

/// A UTC instant as 100-nanosecond ticks since 1601-01-01.
///
/// `xs:dateTime` admits several spellings of one instant, so the form that was read is kept and
/// replayed on write; it takes no part in equality or ordering.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DateTime {
    ticks: i64,
    lexical: Option<Box<str>>,
}

impl DateTime {
    /// The zero of the tick scale, 1601-01-01T00:00:00Z, which is also the null DateTime.
    ///
    /// XML spells the null value `0001-01-01T00:00:00Z` rather than encoding it literally, so a
    /// codec has to map that sentinel here rather than parsing it.
    pub const EPOCH: Self = Self {
        ticks: 0,
        lexical: None,
    };

    /// The sentinel XML uses for the null DateTime (OPC 10000-6 §5.3.1.6).
    pub const NULL_LEXICAL: &'static str = "0001-01-01T00:00:00Z";

    /// The sentinel XML uses for the latest representable DateTime (OPC 10000-6 §5.3.1.6).
    pub const MAX_LEXICAL: &'static str = "9999-12-31T23:59:59Z";

    pub fn from_ticks(ticks: i64) -> Self {
        Self { ticks, lexical: None }
    }

    /// From seconds since the Unix epoch, which is what a caller with a clock has.
    pub fn from_unix_seconds(seconds: i64) -> Self {
        Self::from_ticks(
            seconds
                .saturating_add(EPOCH_OFFSET_DAYS * 86_400)
                .saturating_mul(TICKS_PER_SECOND),
        )
    }

    pub fn ticks(&self) -> i64 {
        self.ticks
    }

    /// The spelling this value was parsed from, if it was parsed rather than constructed.
    pub fn lexical(&self) -> Option<&str> {
        self.lexical.as_deref()
    }

    pub fn is_epoch(&self) -> bool {
        self.ticks == 0
    }

    /// Drops the remembered spelling, so writing this value produces the canonical form.
    pub fn canonicalized(self) -> Self {
        Self {
            ticks: self.ticks,
            lexical: None,
        }
    }

    fn parts(&self) -> (i64, i64, i64, i64, i64, i64, i64) {
        let days = self.ticks.div_euclid(TICKS_PER_DAY);
        let mut rest = self.ticks.rem_euclid(TICKS_PER_DAY);
        let (year, month, day) = civil_from_days(days - EPOCH_OFFSET_DAYS);
        let hour = rest / TICKS_PER_HOUR;
        rest %= TICKS_PER_HOUR;
        let minute = rest / TICKS_PER_MINUTE;
        rest %= TICKS_PER_MINUTE;
        let second = rest / TICKS_PER_SECOND;
        (year, month, day, hour, minute, second, rest % TICKS_PER_SECOND)
    }
}

impl PartialEq for DateTime {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        self.ticks == other.ticks
    }
}

impl Eq for DateTime {}

impl Hash for DateTime {
    fn hash<H: Hasher>(
        &self,
        state: &mut H,
    ) {
        self.ticks.hash(state);
    }
}

impl Ord for DateTime {
    fn cmp(
        &self,
        other: &Self,
    ) -> Ordering {
        self.ticks.cmp(&other.ticks)
    }
}

impl PartialOrd for DateTime {
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for DateTime {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if let Some(lexical) = &self.lexical {
            return f.write_str(lexical);
        }
        let (year, month, day, hour, minute, second, fraction) = self.parts();
        if year < 0 {
            f.write_str("-")?;
        }
        let year = year.unsigned_abs();
        write!(f, "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}")?;
        if fraction != 0 {
            write!(f, ".{fraction:07}")?;
        }
        f.write_str("Z")
    }
}

impl FromStr for DateTime {
    type Err = ParseError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || ParseError::DateTime(text.to_owned());
        let (date, rest) = text.split_once('T').ok_or_else(invalid)?;

        let negative = date.starts_with('-');
        let digits = if negative { &date[1..] } else { date };
        let mut fields = digits.split('-');
        let number = |field: Option<&str>| -> Result<i64, ParseError> {
            field
                .filter(|f| !f.is_empty())
                .ok_or_else(invalid)?
                .parse()
                .map_err(|_| invalid())
        };
        let year = number(fields.next())?;
        let year = if negative { -year } else { year };
        let month = number(fields.next())?;
        let day = number(fields.next())?;
        if fields.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return Err(invalid());
        }

        let (time, offset) = split_offset(rest).ok_or_else(invalid)?;
        let mut fields = time.split(':');
        let hour = number(fields.next())?;
        let minute = number(fields.next())?;
        let seconds = fields
            .next()
            .filter(|f| !f.is_empty())
            .ok_or_else(invalid)?;
        if fields.next().is_some() {
            return Err(invalid());
        }
        let (second, fraction) = match seconds.split_once('.') {
            Some((second, digits)) => (second, parse_fraction(digits).ok_or_else(invalid)?),
            None => (seconds, 0),
        };
        let second: i64 = second.parse().map_err(|_| invalid())?;
        if hour > 24 || minute > 59 || second > 60 {
            return Err(invalid());
        }

        let days = days_from_civil(year.into(), month.into(), day.into()) + i128::from(EPOCH_OFFSET_DAYS);
        let ticks = days * i128::from(TICKS_PER_DAY)
            + i128::from(hour) * i128::from(TICKS_PER_HOUR)
            + i128::from(minute) * i128::from(TICKS_PER_MINUTE)
            + i128::from(second) * i128::from(TICKS_PER_SECOND)
            + i128::from(fraction)
            - offset;
        Ok(Self {
            ticks: i64::try_from(ticks).map_err(|_| invalid())?,
            lexical: Some(text.into()),
        })
    }
}

/// Splits a time from its trailing zone designator, returning the offset in ticks.
fn split_offset(rest: &str) -> Option<(&str, i128)> {
    if let Some(time) = rest.strip_suffix('Z') {
        return Some((time, 0));
    }
    let split = rest.rfind(['+', '-'])?;
    let (time, zone) = rest.split_at(split);
    let (sign, zone) = zone.split_at(1);
    let (hour, minute) = zone.split_once(':')?;
    let offset = i128::from(hour.parse::<i64>().ok()?) * i128::from(TICKS_PER_HOUR)
        + i128::from(minute.parse::<i64>().ok()?) * i128::from(TICKS_PER_MINUTE);
    Some((time, if sign == "-" { -offset } else { offset }))
}

/// Scales fractional-second digits to ticks, rejecting anything finer than 100 nanoseconds.
fn parse_fraction(digits: &str) -> Option<i64> {
    if digits.is_empty() || !digits.bytes().all(|digit| digit.is_ascii_digit()) {
        return None;
    }
    let (digits, trailing) = digits.split_at(digits.len().min(7));
    if trailing.bytes().any(|digit| digit != b'0') {
        return None;
    }
    let mut ticks: i64 = digits.parse().ok()?;
    for _ in digits.len()..7 {
        ticks *= 10;
    }
    Some(ticks)
}

/// Wider than the tick scale it feeds, so a year no `DateTime` can hold is an error rather than an
/// overflow.
fn days_from_civil(
    year: i128,
    month: i128,
    day: i128,
) -> i128 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let months = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * months + 2) / 5 + 1;
    let month = if months < 10 { months + 3 } else { months - 9 };
    let year = year_of_era + era * 400;
    (if month <= 2 { year + 1 } else { year }, month, day)
}
