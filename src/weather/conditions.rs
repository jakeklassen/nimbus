use gpui_kit::assets::IconName;

/// What a WMO weather code looks like on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Condition {
    pub label: &'static str,
    pub icon: IconName,
}

impl Condition {
    /// A clear sky gets the accent. Every other condition stays quiet.
    pub fn is_sunny(&self) -> bool {
        self.icon == IconName::Sun
    }
}

/// WMO weather interpretation codes, as Open-Meteo reports them.
pub fn describe(code: u8, is_day: bool) -> Condition {
    let clear = if is_day {
        IconName::Sun
    } else {
        IconName::Moon
    };
    let partly = if is_day {
        IconName::CloudSun
    } else {
        IconName::CloudMoon
    };
    let showers = if is_day {
        IconName::CloudSunRain
    } else {
        IconName::CloudMoonRain
    };

    let (label, icon) = match code {
        0 => ("Clear", clear),
        1 => ("Mostly clear", clear),
        2 => ("Partly cloudy", partly),
        3 => ("Cloudy", IconName::Cloudy),
        45 | 48 => ("Fog", IconName::CloudFog),
        51..=55 => ("Drizzle", IconName::CloudDrizzle),
        56 | 57 => ("Freezing drizzle", IconName::CloudDrizzle),
        61 => ("Light rain", IconName::CloudRain),
        63 => ("Rain", IconName::CloudRain),
        65 => ("Heavy rain", IconName::CloudRain),
        66 | 67 => ("Freezing rain", IconName::CloudRain),
        71 => ("Light snow", IconName::CloudSnow),
        73 | 77 => ("Snow", IconName::CloudSnow),
        75 => ("Heavy snow", IconName::CloudSnow),
        80..=82 => ("Showers", showers),
        85 | 86 => ("Snow showers", IconName::CloudSnow),
        95.. => ("Thunderstorms", IconName::CloudLightning),
        _ => ("Unknown", IconName::Cloudy),
    };
    Condition { label, icon }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_swaps_the_sun_for_the_moon() {
        assert_eq!(describe(0, true).icon, IconName::Sun);
        assert_eq!(describe(0, false).icon, IconName::Moon);
        assert_eq!(describe(81, false).icon, IconName::CloudMoonRain);
    }

    #[test]
    fn only_a_clear_day_is_sunny() {
        assert!(describe(1, true).is_sunny());
        assert!(!describe(1, false).is_sunny());
        assert!(!describe(2, true).is_sunny());
    }

    #[test]
    fn unknown_codes_fall_back_to_cloudy() {
        assert_eq!(describe(42, true).label, "Unknown");
        assert_eq!(describe(99, true).label, "Thunderstorms");
    }
}
