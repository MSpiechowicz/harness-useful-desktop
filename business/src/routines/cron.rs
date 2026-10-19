//! Five-field cron expressions (see "Cron" in `docs/routines-format.md`): minute, hour, day of
//! month, month, day of week, separated by exactly one space.
//!
//! - A field is items joined by `,`; an item is `*`, a value or a range `a-b` (start not after
//!   end), optionally followed by `/step` (1 up to the field's highest value, only after `*` or a
//!   range). Values are one or two decimal digits, or three-letter names (ASCII case-insensitive)
//!   in the month and day of week fields.
//! - Day of week runs 0 to 7, 0 and 7 are Sunday; `sun` or `0` at the end of a range counts as 7
//!   (`fri-sun`).
//! - Day of month and day of week combine with OR when both fields begin with something other than
//!   `*` (the Vixie rule), else with AND. Month, hour and minute always combine with AND.
//! - An expression that can never match the calendar is refused: when day of month is the only
//!   restricted day field, one of its days must exist in one of its months (February has 29).

/// A parsed expression: the set of each field as bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CronExpr {
    minutes: u64,
    hours: u32,
    days: u32,
    months: u16,
    weekdays: u8,
    day_or: bool,
}

/// What one field takes.
struct Field {
    label: &'static str,
    min: u8,
    max: u8,
    names: &'static [&'static str],
    /// The first name's value.
    first_name: u8,
    /// Day of week: `0` (Sunday) at the end of a range counts as 7.
    sunday_end: bool,
}

const MINUTE: Field = Field {
    label: "minute",
    min: 0,
    max: 59,
    names: &[],
    first_name: 0,
    sunday_end: false,
};

const HOUR: Field = Field {
    label: "hour",
    min: 0,
    max: 23,
    names: &[],
    first_name: 0,
    sunday_end: false,
};

const DAY: Field = Field {
    label: "day of month",
    min: 1,
    max: 31,
    names: &[],
    first_name: 0,
    sunday_end: false,
};

const MONTH: Field = Field {
    label: "month",
    min: 1,
    max: 12,
    names: &[
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ],
    first_name: 1,
    sunday_end: false,
};

const WEEKDAY: Field = Field {
    label: "day of week",
    min: 0,
    max: 7,
    names: &["sun", "mon", "tue", "wed", "thu", "fri", "sat"],
    first_name: 0,
    sunday_end: true,
};

/// Days in each month, February with 29.
const MONTH_DAYS: [u8; 12] = [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

impl CronExpr {
    /// The expression in `text`, or why it is invalid (it does not parse, or it can never match).
    pub fn parse(text: &str) -> Result<Self, String> {
        let fields: Vec<&str> = text.split(' ').collect();

        if fields.len() != 5 {
            return Err("a cron expression has five fields separated by one space".to_owned());
        }

        let expr = Self {
            minutes: parse_field(fields[0], &MINUTE)?,
            hours: parse_field(fields[1], &HOUR)? as u32,
            days: parse_field(fields[2], &DAY)? as u32,
            months: parse_field(fields[3], &MONTH)? as u16,
            weekdays: fold_sunday(parse_field(fields[4], &WEEKDAY)?),
            day_or: !fields[2].starts_with('*') && !fields[4].starts_with('*'),
        };

        if !expr.can_fire() {
            return Err("the cron expression never matches a day of the calendar".to_owned());
        }

        Ok(expr)
    }

    /// True when some day of the calendar matches: with OR every weekday is somewhere in every
    /// month; with AND every day of month that exists in a month falls on every weekday over the
    /// years, so one of its days must exist in one of its months.
    fn can_fire(&self) -> bool {
        if self.day_or {
            return true;
        }

        (1..=12u8).filter(|month| self.months & (1 << month) != 0).any(|month| {
            let last = MONTH_DAYS[usize::from(month - 1)];
            (1..=last).any(|day| self.days & (1 << day) != 0)
        })
    }

    /// True when the date matches: `month` 1 to 12, `day` 1 to 31, `weekday` 0 (Sunday) to 6.
    pub fn matches_date(&self, month: i8, day: i8, weekday: i8) -> bool {
        let has = |bits: u64, value: i8| u8::try_from(value).is_ok_and(|value| value < 64 && bits & (1 << value) != 0);

        if !has(u64::from(self.months), month) {
            return false;
        }

        let day_matches = has(u64::from(self.days), day);
        let weekday_matches = has(u64::from(self.weekdays), weekday);

        if self.day_or {
            day_matches || weekday_matches
        } else {
            day_matches && weekday_matches
        }
    }

    /// The `(hour, minute)` pairs that match, earliest first.
    pub fn times(&self) -> impl Iterator<Item = (i8, i8)> + '_ {
        (0..24i8)
            .filter(|hour| self.hours & (1 << hour) != 0)
            .flat_map(move |hour| {
                (0..60i8)
                    .filter(move |minute| self.minutes & (1 << minute) != 0)
                    .map(move |minute| (hour, minute))
            })
    }
}

/// The bits of `text` in `field`.
fn parse_field(text: &str, field: &Field) -> Result<u64, String> {
    let mut bits = 0u64;

    for item in text.split(',') {
        bits |= parse_item(item, field)?;
    }

    Ok(bits)
}

fn parse_item(item: &str, field: &Field) -> Result<u64, String> {
    let invalid = || format!("the cron {} field has an invalid item {item:?}", field.label);

    if item.is_empty() {
        return Err(format!("the cron {} field has an empty item", field.label));
    }

    let (base, step) = match item.split_once('/') {
        Some((base, step)) => (base, Some(step)),
        None => (item, None),
    };

    let (start, end, spans) = if base == "*" {
        (field.min, field.max, true)
    } else if let Some((first, last)) = base.split_once('-') {
        let start = parse_value(first, field, false).ok_or_else(invalid)?;
        let end = parse_value(last, field, start > 0).ok_or_else(invalid)?;

        if start > end {
            return Err(format!("the cron {} range {base:?} runs backwards", field.label));
        }

        (start, end, true)
    } else {
        let value = parse_value(base, field, false).ok_or_else(invalid)?;
        (value, value, false)
    };

    let step = match step {
        None => 1,
        Some(_) if !spans => return Err(format!("a cron step needs * or a range, not {item:?}")),
        Some(step) => parse_number(step)
            .filter(|step| (1..=field.max).contains(step))
            .ok_or_else(|| format!("the cron {} step in {item:?} is out of range", field.label))?,
    };

    let mut bits = 0u64;
    for value in (start..=end).step_by(usize::from(step)) {
        bits |= 1 << value;
    }

    Ok(bits)
}

/// A value of `field`: one or two digits in range, or one of its names. Sunday at the end of a day
/// of week range that starts after Sunday (`end`) counts as 7, so `fri-sun` runs Friday to Sunday
/// and `sun-sun` is Sunday only.
fn parse_value(text: &str, field: &Field, end: bool) -> Option<u8> {
    let value = match parse_number(text) {
        Some(number) => number,
        None => {
            let index = field
                .names
                .iter()
                .position(|name| text.len() == 3 && name.eq_ignore_ascii_case(text))?;
            field.first_name + u8::try_from(index).ok()?
        }
    };

    if !(field.min..=field.max).contains(&value) {
        return None;
    }

    if field.sunday_end && end && value == 0 {
        return Some(7);
    }

    Some(value)
}

/// One or two ASCII digits.
fn parse_number(text: &str) -> Option<u8> {
    let plain = (1..=2).contains(&text.len()) && text.bytes().all(|byte| byte.is_ascii_digit());

    plain.then(|| text.parse().ok())?
}

/// Day of week bits with 7 (Sunday) folded onto 0.
fn fold_sunday(bits: u64) -> u8 {
    let sunday = u8::from(bits & (1 << 7) != 0);

    (bits & 0x7f) as u8 | sunday
}
