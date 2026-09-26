//! The user's locale, unit system and clock, read from the OS.
//!
//! Light and dark come from GPUI itself (`Window::appearance`), so only what
//! GPUI does not know lives here.

use crate::weather::Units;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clock {
    TwelveHour,
    TwentyFourHour,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SystemSettings {
    /// A BCP 47 tag such as `en-CA`.
    pub locale: String,
    pub units: Units,
    pub clock: Clock,
}

// Only these regions still use Fahrenheit and miles for the weather.
const IMPERIAL_REGIONS: [&str; 3] = ["US", "LR", "MM"];

impl SystemSettings {
    /// Read the settings from the OS, filling gaps from the locale.
    pub fn read() -> Self {
        let os = platform::read();
        let locale = os
            .locale
            .or_else(sys_locale::get_locale)
            .and_then(|tag| canonical_locale(&tag))
            .unwrap_or_else(|| "en-US".into());
        let defaults = Self::for_locale(&locale);
        Self {
            units: os.units.unwrap_or(defaults.units),
            clock: os.clock.unwrap_or(defaults.clock),
            locale,
        }
    }

    /// The conventions of a locale, without asking the OS for overrides.
    pub fn for_locale(locale: &str) -> Self {
        let region = region_of(locale);
        let units = if region.is_some_and(|region| IMPERIAL_REGIONS.contains(&region.as_str())) {
            Units::Imperial
        } else {
            Units::Metric
        };
        Self {
            locale: locale.into(),
            units,
            clock: clock_for_locale(locale),
        }
    }
}

/// `en_CA.UTF-8`, `en-ca` and `en-CA` all become `en-CA`.
fn canonical_locale(raw: &str) -> Option<String> {
    let tag = raw.trim().split(['.', '@']).next()?.replace('_', "-");
    let mut parts = tag.split('-').filter(|part| !part.is_empty());
    let language = parts.next()?.to_ascii_lowercase();
    if language.is_empty() || language == "c" || language == "posix" {
        return None;
    }
    let rest = parts.map(|part| match part.len() {
        2 => part.to_ascii_uppercase(),
        4 => {
            let mut script = part.to_ascii_lowercase();
            script[..1].make_ascii_uppercase();
            script
        }
        _ => part.to_string(),
    });
    Some(
        std::iter::once(language)
            .chain(rest)
            .collect::<Vec<_>>()
            .join("-"),
    )
}

/// The two-letter region of a tag, if it names one.
fn region_of(locale: &str) -> Option<String> {
    locale
        .split('-')
        .skip(1)
        .find(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|part| part.to_ascii_uppercase())
}

/// The chrono locale for a tag, or US English when chrono has none for it.
pub fn chrono_locale(locale: &str) -> chrono::Locale {
    let posix = locale.replace('-', "_");
    chrono::Locale::try_from(posix.as_str())
        .or_else(|_| {
            // `sr-Latn-RS` has no chrono entry, but its language often does.
            let language = posix.split('_').next().unwrap_or_default();
            chrono::Locale::try_from(language)
        })
        .unwrap_or(chrono::Locale::en_US)
}

/// Whether the locale's own time format has an AM/PM marker.
fn clock_for_locale(locale: &str) -> Clock {
    use chrono::TimeZone as _;
    let one_pm = chrono::Utc.with_ymd_and_hms(2026, 1, 1, 13, 0, 0).unwrap();
    let formatted = one_pm
        .format_localized("%X", chrono_locale(locale))
        .to_string();
    if formatted.contains("13") {
        Clock::TwentyFourHour
    } else {
        Clock::TwelveHour
    }
}

/// What the OS says explicitly. `None` means ask the locale.
#[derive(Default)]
struct OsSettings {
    locale: Option<String>,
    units: Option<Units>,
    clock: Option<Clock>,
}

#[cfg(windows)]
mod platform {
    use super::*;

    pub(super) fn read() -> OsSettings {
        let Ok(key) = windows_registry::CURRENT_USER.open(r"Control Panel\International") else {
            return OsSettings::default();
        };
        let value = |name: &str| key.get_string(name).ok();
        OsSettings {
            locale: value("LocaleName"),
            // 0 is metric, 1 is US.
            units: value("iMeasure").map(|measure| {
                if measure.trim() == "1" {
                    Units::Imperial
                } else {
                    Units::Metric
                }
            }),
            // `h` is a 12-hour hour and `H` a 24-hour one, as in `h:mm tt`.
            clock: value("sShortTime").map(|format| {
                if format.contains('h') {
                    Clock::TwelveHour
                } else {
                    Clock::TwentyFourHour
                }
            }),
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::process::Command;

    fn defaults(key: &str) -> Option<String> {
        let output = Command::new("defaults")
            .args(["read", "-g", key])
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub(super) fn read() -> OsSettings {
        let clock = if defaults("AppleICUForce24HourTime").as_deref() == Some("1") {
            Some(Clock::TwentyFourHour)
        } else if defaults("AppleICUForce12HourTime").as_deref() == Some("1") {
            Some(Clock::TwelveHour)
        } else {
            None
        };
        OsSettings {
            locale: defaults("AppleLocale"),
            units: defaults("AppleTemperatureUnit").map(|unit| {
                if unit.contains("Fahrenheit") {
                    Units::Imperial
                } else {
                    Units::Metric
                }
            }),
            clock,
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::*;

    pub(super) fn read() -> OsSettings {
        let env = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
        let measurement = env("LC_MEASUREMENT")
            .or_else(|| env("LC_ALL"))
            .or_else(|| env("LANG"));
        OsSettings {
            // The measurement locale decides units; the display locale stays
            // whatever `sys-locale` reports.
            units: measurement
                .and_then(|tag| canonical_locale(&tag))
                .map(|tag| SystemSettings::for_locale(&tag).units),
            clock: env("LC_TIME")
                .and_then(|tag| canonical_locale(&tag))
                .map(|tag| clock_for_locale(&tag)),
            locale: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_posix_and_bcp47_tags() {
        assert_eq!(canonical_locale("en_CA.UTF-8").as_deref(), Some("en-CA"));
        assert_eq!(canonical_locale("en-gb").as_deref(), Some("en-GB"));
        assert_eq!(
            canonical_locale("sr-latn-rs").as_deref(),
            Some("sr-Latn-RS")
        );
        assert_eq!(canonical_locale("C"), None);
    }

    #[test]
    fn the_locale_decides_units_and_clock() {
        let canada = SystemSettings::for_locale("en-CA");
        assert_eq!(canada.units, Units::Metric);
        assert_eq!(canada.clock, Clock::TwelveHour);

        let us = SystemSettings::for_locale("en-US");
        assert_eq!(us.units, Units::Imperial);
        assert_eq!(us.clock, Clock::TwelveHour);

        let uk = SystemSettings::for_locale("en-GB");
        assert_eq!(uk.units, Units::Metric);
        assert_eq!(uk.clock, Clock::TwentyFourHour);

        assert_eq!(
            SystemSettings::for_locale("de-DE").clock,
            Clock::TwentyFourHour
        );
    }
}
