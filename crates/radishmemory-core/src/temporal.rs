use std::cmp::Ordering;

use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::{CoreError, InvalidTimeReason};

/// Precision explicitly present in an RFC 3339 timestamp representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimestampPrecision {
    fractional_second_digits: usize,
}

impl TimestampPrecision {
    /// Number of fractional-second digits present in the external text.
    #[must_use]
    pub const fn fractional_second_digits(self) -> usize {
        self.fractional_second_digits
    }
}

/// A parsed absolute instant that retains its original RFC 3339 representation.
#[derive(Clone, Debug)]
pub struct Timestamp {
    original: Box<str>,
    utc_second_key: i128,
    fractional_seconds: Box<str>,
    offset_seconds: i32,
    precision: TimestampPrecision,
}

impl Timestamp {
    /// Parses an RFC 3339 timestamp without using locale or the system timezone.
    pub fn parse(input: &str) -> Result<Self, CoreError> {
        let instant = OffsetDateTime::parse(input, &Rfc3339)
            .map_err(|source| CoreError::invalid_time(InvalidTimeReason::Parse, Some(source)))?;
        let fractional_seconds = fractional_seconds(input);
        let leap_second = input.as_bytes().get(17..19) == Some(b"60");

        Ok(Self {
            original: input.into(),
            utc_second_key: i128::from(instant.unix_timestamp()) * 2 + i128::from(leap_second),
            precision: TimestampPrecision {
                fractional_second_digits: fractional_seconds.len(),
            },
            fractional_seconds: fractional_seconds.into(),
            offset_seconds: instant.offset().whole_seconds(),
        })
    }

    /// Returns the exact external representation supplied by the caller.
    #[must_use]
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Returns precision facts derived from the external representation.
    #[must_use]
    pub const fn precision(&self) -> TimestampPrecision {
        self.precision
    }

    /// Returns the numeric offset that appeared in the parsed representation.
    #[must_use]
    pub const fn offset_seconds(&self) -> i32 {
        self.offset_seconds
    }
}

impl PartialEq for Timestamp {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Timestamp {}

impl PartialOrd for Timestamp {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Timestamp {
    fn cmp(&self, other: &Self) -> Ordering {
        self.utc_second_key
            .cmp(&other.utc_second_key)
            .then_with(|| compare_fraction(&self.fractional_seconds, &other.fractional_seconds))
    }
}

fn fractional_seconds(input: &str) -> &str {
    if input.as_bytes().get(19) != Some(&b'.') {
        return "";
    }
    let digits = input[20..].bytes().take_while(u8::is_ascii_digit).count();
    &input[20..20 + digits]
}

fn compare_fraction(left: &str, right: &str) -> Ordering {
    let width = left.len().max(right.len());
    for index in 0..width {
        let left_digit = left.as_bytes().get(index).copied().unwrap_or(b'0');
        let right_digit = right.as_bytes().get(index).copied().unwrap_or(b'0');
        match left_digit.cmp(&right_digit) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

/// Frozen M0 `ValidTime` modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValidTimeMode {
    Unknown,
    Instant,
    Interval,
    OpenEnded,
}

/// Frozen M0 precision labels for `ValidTime`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimePrecision {
    Exact,
    Day,
    Month,
    Year,
    Unknown,
}

/// A validated M0 valid-time value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidTime {
    mode: ValidTimeMode,
    start_at: Option<Timestamp>,
    end_at: Option<Timestamp>,
    precision: TimePrecision,
}

impl ValidTime {
    /// Validates the conditional boundary fields and interval ordering.
    pub fn new(
        mode: ValidTimeMode,
        start_at: Option<Timestamp>,
        end_at: Option<Timestamp>,
        precision: TimePrecision,
    ) -> Result<Self, CoreError> {
        let boundaries_match = match mode {
            ValidTimeMode::Unknown => start_at.is_none() && end_at.is_none(),
            ValidTimeMode::Instant | ValidTimeMode::OpenEnded => {
                start_at.is_some() && end_at.is_none()
            }
            ValidTimeMode::Interval => start_at.is_some() && end_at.is_some(),
        };
        if !boundaries_match {
            return Err(CoreError::invalid_time(
                InvalidTimeReason::BoundaryCombination,
                None,
            ));
        }

        if let (ValidTimeMode::Interval, Some(start), Some(end)) =
            (mode, start_at.as_ref(), end_at.as_ref())
            && start >= end
        {
            return Err(CoreError::invalid_time(
                InvalidTimeReason::IntervalOrder,
                None,
            ));
        }

        Ok(Self {
            mode,
            start_at,
            end_at,
            precision,
        })
    }

    #[must_use]
    pub const fn mode(&self) -> ValidTimeMode {
        self.mode
    }

    #[must_use]
    pub fn start_at(&self) -> Option<&Timestamp> {
        self.start_at.as_ref()
    }

    #[must_use]
    pub fn end_at(&self) -> Option<&Timestamp> {
        self.end_at.as_ref()
    }

    #[must_use]
    pub const fn precision(&self) -> TimePrecision {
        self.precision
    }

    /// Applies M0 point-in-time membership, including `[start, end)` intervals.
    #[must_use]
    pub fn contains(&self, candidate: &Timestamp) -> bool {
        match self.mode {
            ValidTimeMode::Unknown => false,
            ValidTimeMode::Instant => self.start_at.as_ref() == Some(candidate),
            ValidTimeMode::Interval => {
                self.start_at
                    .as_ref()
                    .is_some_and(|start| start <= candidate)
                    && self.end_at.as_ref().is_some_and(|end| candidate < end)
            }
            ValidTimeMode::OpenEnded => self
                .start_at
                .as_ref()
                .is_some_and(|start| start <= candidate),
        }
    }
}
