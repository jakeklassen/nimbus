use chrono::{Datelike as _, NaiveDate, NaiveDateTime, Timelike as _};

use crate::{
    system::{Clock, chrono_locale},
    weather::notable_precipitation,
};

/// `18.6` becomes `19°`, and a rounded `-0` stays `0°`.
pub fn degrees(value: f64) -> String {
    format!("{}°", value.round() as i64)
}

pub fn percent(value: f64) -> String {
    format!("{}%", value.round() as i64)
}

/// A chance of precipitation worth reading, or a blank that keeps its line.
pub fn chance_label(chance: f64) -> String {
    if notable_precipitation(chance) {
        percent(chance)
    } else {
        " ".into()
    }
}

/// `8pm` on a 12-hour clock, `20` on a 24-hour one.
pub fn hour_label(time: NaiveDateTime, clock: Clock) -> String {
    let hour = time.hour();
    match clock {
        Clock::TwentyFourHour => format!("{hour:02}"),
        Clock::TwelveHour => {
            let suffix = if hour < 12 { "am" } else { "pm" };
            let twelve = match hour % 12 {
                0 => 12,
                hour => hour,
            };
            format!("{twelve}{suffix}")
        }
    }
}

/// `Today` for the first day, then the locale's short weekday name.
pub fn day_label(date: NaiveDate, ix: usize, locale: &str) -> String {
    if ix == 0 {
        return "Today".into();
    }
    let weekday = date
        .format_localized("%a", chrono_locale(locale))
        .to_string();
    // Some locales end the abbreviation with a period, which reads as noise
    // in a column of days.
    let weekday = weekday.trim_end_matches('.');
    let mut chars = weekday.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => date.weekday().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 16)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
    }

    #[test]
    fn rounds_degrees_without_negative_zero() {
        assert_eq!(degrees(18.6), "19°");
        assert_eq!(degrees(-0.4), "0°");
        assert_eq!(degrees(-3.5), "-4°");
    }

    #[test]
    fn labels_hours_on_either_clock() {
        assert_eq!(hour_label(at(0), Clock::TwelveHour), "12am");
        assert_eq!(hour_label(at(12), Clock::TwelveHour), "12pm");
        assert_eq!(hour_label(at(20), Clock::TwelveHour), "8pm");
        assert_eq!(hour_label(at(8), Clock::TwentyFourHour), "08");
        assert_eq!(hour_label(at(20), Clock::TwentyFourHour), "20");
    }

    #[test]
    fn labels_days_in_the_locale() {
        // 2026-09-17 is a Thursday.
        let thursday = NaiveDate::from_ymd_opt(2026, 9, 17).unwrap();
        assert_eq!(day_label(thursday, 0, "en-CA"), "Today");
        assert_eq!(day_label(thursday, 1, "en-CA"), "Thu");
        assert_eq!(day_label(thursday, 1, "de-DE"), "Do");
        assert_eq!(day_label(thursday, 1, "fr-FR"), "Jeu");
    }
}
