//! Forecasts and city search from [Open-Meteo](https://open-meteo.com), which
//! needs no API key, and a first location from the IP address through
//! [ipwho.is](https://ipwho.is).
//!
//! With `timezone=auto` every time is local to the place, as a bare
//! `YYYY-MM-DDTHH:MM` string, so nothing here converts time zones.

use std::{path::PathBuf, time::Duration};

use anyhow::{Context as _, anyhow};
use chrono::{NaiveDate, NaiveDateTime};
use futures::{FutureExt as _, future::BoxFuture};
use serde::{Deserialize, de::DeserializeOwned};

use super::{Current, DAYS_SHOWN, Day, Forecast, HOURS_SHOWN, Hour, Place, Units, WeatherSource};

const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
const GEOCODE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const IP_LOCATE_URL: &str = "https://ipwho.is/";

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const SEARCH_RESULTS: usize = 6;

/// The live weather source. Its futures block, so poll them on a background
/// executor.
pub struct OpenMeteo {
    agent: ureq::Agent,
    place_file: Option<PathBuf>,
}

impl OpenMeteo {
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(REQUEST_TIMEOUT))
            .build()
            .into();
        Self {
            agent,
            place_file: place_file(),
        }
    }

    fn get<T: DeserializeOwned>(&self, url: &str, query: &[(&str, String)]) -> anyhow::Result<T> {
        let mut request = self.agent.get(url);
        for (key, value) in query {
            request = request.query(*key, value);
        }
        let mut response = request.call().with_context(|| format!("GET {url}"))?;
        Ok(response.body_mut().read_json::<T>()?)
    }

    fn load_place(&self) -> Option<Place> {
        let text = std::fs::read_to_string(self.place_file.as_ref()?).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn locate_blocking(&self) -> anyhow::Result<Place> {
        if let Some(place) = self.load_place() {
            return Ok(place);
        }
        #[derive(Deserialize)]
        struct IpLocation {
            success: bool,
            city: String,
            region: Option<String>,
            latitude: f64,
            longitude: f64,
        }
        let ip: IpLocation = self.get(IP_LOCATE_URL, &[])?;
        if !ip.success {
            return Err(anyhow!("could not find your location"));
        }
        Ok(Place {
            name: ip.city,
            region: ip.region.filter(|region| !region.is_empty()),
            latitude: ip.latitude,
            longitude: ip.longitude,
        })
    }

    fn search_blocking(&self, query: &str) -> anyhow::Result<Vec<Place>> {
        #[derive(Deserialize)]
        struct Results {
            #[serde(default)]
            results: Vec<Result>,
        }
        #[derive(Deserialize)]
        struct Result {
            name: String,
            admin1: Option<String>,
            country: Option<String>,
            latitude: f64,
            longitude: f64,
        }
        let data: Results = self.get(
            GEOCODE_URL,
            &[
                ("name", query.to_string()),
                ("count", SEARCH_RESULTS.to_string()),
                ("format", "json".into()),
            ],
        )?;
        Ok(data
            .results
            .into_iter()
            .map(|result| {
                let region = [result.admin1, result.country]
                    .into_iter()
                    .flatten()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(", ");
                Place {
                    name: result.name,
                    region: (!region.is_empty()).then_some(region),
                    latitude: result.latitude,
                    longitude: result.longitude,
                }
            })
            .collect())
    }

    fn forecast_blocking(&self, place: &Place, units: Units) -> anyhow::Result<Forecast> {
        let imperial = units == Units::Imperial;
        let response: ForecastResponse = self.get(
            FORECAST_URL,
            &[
                ("latitude", place.latitude.to_string()),
                ("longitude", place.longitude.to_string()),
                (
                    "current",
                    "temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,\
                     is_day,wind_speed_10m,precipitation_probability"
                        .into(),
                ),
                (
                    "hourly",
                    "temperature_2m,weather_code,is_day,precipitation_probability".into(),
                ),
                (
                    "daily",
                    "weather_code,temperature_2m_max,temperature_2m_min,\
                     precipitation_probability_max"
                        .into(),
                ),
                ("timezone", "auto".into()),
                // One extra day so late evening still has 24 hours ahead.
                ("forecast_days", (DAYS_SHOWN + 1).to_string()),
                (
                    "temperature_unit",
                    if imperial { "fahrenheit" } else { "celsius" }.into(),
                ),
                (
                    "wind_speed_unit",
                    if imperial { "mph" } else { "kmh" }.into(),
                ),
            ],
        )?;
        response.into_forecast()
    }

    fn remember_blocking(&self, place: Option<&Place>) -> anyhow::Result<()> {
        let Some(file) = &self.place_file else {
            return Ok(());
        };
        match place {
            Some(place) => {
                if let Some(dir) = file.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(file, serde_json::to_string(place)?)?;
            }
            None => match std::fs::remove_file(file) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    return Err(error.into());
                }
                _ => {}
            },
        }
        Ok(())
    }
}

impl Default for OpenMeteo {
    fn default() -> Self {
        Self::new()
    }
}

// Each future owns its own handle. Clones of the agent share one connection pool.
impl WeatherSource for OpenMeteo {
    fn locate(&self) -> BoxFuture<'static, anyhow::Result<Place>> {
        let this = self.detach();
        async move { this.locate_blocking() }.boxed()
    }

    fn search(&self, query: String) -> BoxFuture<'static, anyhow::Result<Vec<Place>>> {
        let this = self.detach();
        async move { this.search_blocking(&query) }.boxed()
    }

    fn forecast(&self, place: Place, units: Units) -> BoxFuture<'static, anyhow::Result<Forecast>> {
        let this = self.detach();
        async move { this.forecast_blocking(&place, units) }.boxed()
    }

    fn remember(&self, place: Option<Place>) -> BoxFuture<'static, anyhow::Result<()>> {
        let this = self.detach();
        async move { this.remember_blocking(place.as_ref()) }.boxed()
    }
}

impl OpenMeteo {
    fn detach(&self) -> Self {
        Self {
            agent: self.agent.clone(),
            place_file: self.place_file.clone(),
        }
    }
}

/// Each platform's per-user settings folder: `%APPDATA%\Nimbus` on Windows,
/// `~/Library/Application Support/Nimbus` on macOS and `~/.config/nimbus` on
/// Linux. The GPUIX version of Nimbus saves to the same file.
fn place_file() -> Option<PathBuf> {
    let folder = if cfg!(target_os = "linux") {
        "nimbus"
    } else {
        "Nimbus"
    };
    Some(dirs::config_dir()?.join(folder).join("place.json"))
}

#[derive(Deserialize)]
struct ForecastResponse {
    current: CurrentResponse,
    hourly: HourlyResponse,
    daily: DailyResponse,
}

#[derive(Deserialize)]
struct CurrentResponse {
    time: String,
    temperature_2m: f64,
    apparent_temperature: f64,
    relative_humidity_2m: f64,
    weather_code: u8,
    is_day: u8,
    wind_speed_10m: f64,
    precipitation_probability: Option<f64>,
}

#[derive(Deserialize)]
struct HourlyResponse {
    time: Vec<String>,
    temperature_2m: Vec<f64>,
    weather_code: Vec<u8>,
    is_day: Vec<u8>,
    precipitation_probability: Vec<Option<f64>>,
}

#[derive(Deserialize)]
struct DailyResponse {
    time: Vec<String>,
    temperature_2m_max: Vec<f64>,
    temperature_2m_min: Vec<f64>,
    weather_code: Vec<u8>,
    precipitation_probability_max: Vec<Option<f64>>,
}

fn parse_time(time: &str) -> anyhow::Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(time, "%Y-%m-%dT%H:%M")
        .with_context(|| format!("unexpected time {time:?}"))
}

impl ForecastResponse {
    fn into_forecast(self) -> anyhow::Result<Forecast> {
        let Self {
            current: c,
            hourly: h,
            daily: d,
        } = self;

        // Start the hourly row at the current hour.
        let this_hour = c.time.get(..13).unwrap_or(&c.time);
        let start = h
            .time
            .iter()
            .position(|time| time.starts_with(this_hour))
            .unwrap_or(0);
        let chance = |ix: usize| h.precipitation_probability.get(ix).copied().flatten();

        let hours = (start..h.time.len().min(start + HOURS_SHOWN))
            .map(|ix| {
                Ok(Hour {
                    time: parse_time(&h.time[ix])?,
                    temperature: h.temperature_2m[ix],
                    code: h.weather_code[ix],
                    is_day: h.is_day[ix] == 1,
                    precipitation: chance(ix).unwrap_or(0.),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let days = d
            .time
            .iter()
            .take(DAYS_SHOWN)
            .enumerate()
            .map(|(ix, date)| {
                Ok(Day {
                    date: NaiveDate::parse_from_str(date, "%Y-%m-%d")
                        .with_context(|| format!("unexpected date {date:?}"))?,
                    min: d.temperature_2m_min[ix],
                    max: d.temperature_2m_max[ix],
                    code: d.weather_code[ix],
                    precipitation: d.precipitation_probability_max[ix].unwrap_or(0.),
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        Ok(Forecast {
            current: Current {
                temperature: c.temperature_2m,
                feels_like: c.apparent_temperature,
                code: c.weather_code,
                is_day: c.is_day == 1,
                wind: c.wind_speed_10m,
                humidity: c.relative_humidity_2m,
                precipitation: c
                    .precipitation_probability
                    .or_else(|| chance(start))
                    .unwrap_or(0.),
            },
            hours,
            days,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hourly_row_starts_at_the_current_hour() {
        let json = serde_json::json!({
            "current": {
                "time": "2026-09-16T21:15",
                "temperature_2m": 18.2,
                "apparent_temperature": 17.1,
                "relative_humidity_2m": 81,
                "weather_code": 3,
                "is_day": 0,
                "wind_speed_10m": 14.0,
                "precipitation_probability": null
            },
            "hourly": {
                "time": ["2026-09-16T20:00", "2026-09-16T21:00", "2026-09-16T22:00"],
                "temperature_2m": [19.0, 18.0, 17.0],
                "weather_code": [2, 3, 61],
                "is_day": [1, 0, 0],
                "precipitation_probability": [0, 35, null]
            },
            "daily": {
                "time": ["2026-09-16", "2026-09-17"],
                "temperature_2m_max": [21.0, 22.0],
                "temperature_2m_min": [17.0, 16.0],
                "weather_code": [3, 61],
                "precipitation_probability_max": [10, null]
            }
        });
        let response: ForecastResponse = serde_json::from_value(json).unwrap();
        let forecast = response.into_forecast().unwrap();

        assert_eq!(forecast.hours.len(), 2);
        assert_eq!(
            forecast.hours[0].time,
            parse_time("2026-09-16T21:00").unwrap()
        );
        assert_eq!(forecast.hours[1].precipitation, 0.);
        // The current chance falls back to the current hour's.
        assert_eq!(forecast.current.precipitation, 35.);
        assert!(!forecast.current.is_day);
        assert_eq!(forecast.days[1].precipitation, 0.);
    }
}
