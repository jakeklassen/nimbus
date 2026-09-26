//! Everything Nimbus knows about weather, with no UI in it.
//!
//! The app reaches the outside world only through [`WeatherSource`], so tests
//! swap in a fake and every run paints the same forecast.

mod conditions;
mod open_meteo;

use chrono::{NaiveDate, NaiveDateTime};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};

pub use conditions::{Condition, describe};
pub use open_meteo::OpenMeteo;

/// How many hours the hourly row shows, starting with the current hour.
pub const HOURS_SHOWN: usize = 24;
/// How many days the daily list shows, starting with today.
pub const DAYS_SHOWN: usize = 7;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Place {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
}

/// The unit system for temperatures and wind speeds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Units {
    /// Celsius and km/h.
    #[default]
    Metric,
    /// Fahrenheit and mph.
    Imperial,
}

impl Units {
    pub fn wind_label(self) -> &'static str {
        match self {
            Units::Metric => "km/h",
            Units::Imperial => "mph",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Current {
    pub temperature: f64,
    pub feels_like: f64,
    /// A WMO weather interpretation code.
    pub code: u8,
    pub is_day: bool,
    pub wind: f64,
    /// Relative humidity, 0 to 100.
    pub humidity: f64,
    /// Chance of precipitation, 0 to 100.
    pub precipitation: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hour {
    /// Local to the place, not to the machine.
    pub time: NaiveDateTime,
    pub temperature: f64,
    pub code: u8,
    pub is_day: bool,
    pub precipitation: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    /// Local to the place, not to the machine.
    pub date: NaiveDate,
    pub min: f64,
    pub max: f64,
    pub code: u8,
    pub precipitation: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Forecast {
    pub current: Current,
    pub hours: Vec<Hour>,
    pub days: Vec<Day>,
}

/// Precipitation chances under this are noise, so the slot stays blank.
pub const PRECIPITATION_SHOWN_FROM: f64 = 20.;

/// Whether a precipitation chance is worth showing.
pub fn notable_precipitation(chance: f64) -> bool {
    chance >= PRECIPITATION_SHOWN_FROM
}

/// Everything the app needs from the outside world.
///
/// Each call returns a `'static` future so the app can run it on GPUI's
/// background executor. [`OpenMeteo`]'s futures block on network and disk
/// while they run, which is fine there and nowhere else.
pub trait WeatherSource: Send + Sync + 'static {
    /// The remembered place, or a best guess from the IP address.
    fn locate(&self) -> BoxFuture<'static, anyhow::Result<Place>>;
    fn search(&self, query: String) -> BoxFuture<'static, anyhow::Result<Vec<Place>>>;
    fn forecast(&self, place: Place, units: Units) -> BoxFuture<'static, anyhow::Result<Forecast>>;
    /// Remember a searched place. `None` goes back to automatic location.
    fn remember(&self, place: Option<Place>) -> BoxFuture<'static, anyhow::Result<()>>;
}
