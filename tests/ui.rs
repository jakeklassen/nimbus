//! Drives the real `WeatherView` in a headless GPUI window.
//!
//! Network and disk go through a fake `WeatherSource`, so every run paints the
//! same forecast. Controls are found by `ElementId` and driven with real
//! pointer and keyboard events; assertions read accessibility properties and
//! the view's own state.
//!
//!   cargo test --test ui

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{NaiveDate, NaiveDateTime};
use futures::{FutureExt as _, channel::oneshot, future::BoxFuture};
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, ElementId, Entity, ScrollDelta, TestAppContext, Window,
    component::Root,
    point, px, size,
    test::{TestAppContextExt as _, TestWindowExt as _},
};
use nimbus::{
    system::{Clock, SystemSettings},
    ui::{Load, Options, Refresh, WeatherView},
    weather::{Current, Day, Forecast, Hour, Place, Units, WeatherSource},
};

const WAIT: Duration = Duration::from_secs(2);

fn home() -> Place {
    Place {
        name: "Winnipeg".into(),
        region: Some("Manitoba".into()),
        latitude: 49.9,
        longitude: -97.1,
    }
}

fn paris() -> Place {
    Place {
        name: "Paris".into(),
        region: Some("Île-de-France, France".into()),
        latitude: 48.85,
        longitude: 2.35,
    }
}

fn canada() -> SystemSettings {
    SystemSettings {
        locale: "en-CA".into(),
        units: Units::Metric,
        clock: Clock::TwelveHour,
    }
}

/// 2026-09-16 is a Wednesday. The first hour is 7pm, so the row crosses midnight.
fn forecast(temperature: f64) -> Forecast {
    let at = |day: u32, hour: u32| -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, day)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap()
    };
    Forecast {
        current: Current {
            temperature,
            feels_like: 17.4,
            code: 3,
            is_day: false,
            wind: 20.2,
            humidity: 79.,
            precipitation: 1.,
        },
        hours: (0..24)
            .map(|ix| {
                let hour = (19 + ix) % 24;
                Hour {
                    time: at(if ix < 5 { 16 } else { 17 }, hour),
                    temperature: 19. - ix as f64 * 0.25,
                    code: if ix == 3 { 61 } else { 2 },
                    is_day: (7..20).contains(&hour),
                    precipitation: if ix == 3 { 60. } else { 5. },
                }
            })
            .collect(),
        days: (0..7)
            .map(|ix| Day {
                date: NaiveDate::from_ymd_opt(2026, 9, 16 + ix).unwrap(),
                min: 9. + ix as f64,
                max: 21. - ix as f64,
                code: if ix == 3 { 80 } else { 3 },
                precipitation: if ix == 3 { 77. } else { 0. },
            })
            .collect(),
    }
}

/// What the fake returns next, and what it was asked.
struct Script {
    temperature: f64,
    fail: bool,
    /// When set, the next forecast waits until the sender fires.
    hold: Option<oneshot::Receiver<()>>,
    forecasts: Vec<(Place, Units)>,
    remembered: Vec<Option<Place>>,
    searches: Vec<String>,
}

#[derive(Clone)]
struct Fake(Arc<Mutex<Script>>);

impl Fake {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(Script {
            temperature: 19.,
            fail: false,
            hold: None,
            forecasts: Vec::new(),
            remembered: Vec::new(),
            searches: Vec::new(),
        })))
    }

    fn script<R>(&self, f: impl FnOnce(&mut Script) -> R) -> R {
        f(&mut self.0.lock().unwrap())
    }

    /// Make the next forecast wait. Send on the result to let it through.
    fn hold(&self) -> oneshot::Sender<()> {
        let (release, hold) = oneshot::channel();
        self.script(|script| script.hold = Some(hold));
        release
    }
}

impl WeatherSource for Fake {
    fn locate(&self) -> BoxFuture<'static, anyhow::Result<Place>> {
        async { Ok(home()) }.boxed()
    }

    fn search(&self, query: String) -> BoxFuture<'static, anyhow::Result<Vec<Place>>> {
        self.script(|script| script.searches.push(query));
        async { Ok(vec![paris()]) }.boxed()
    }

    fn forecast(&self, place: Place, units: Units) -> BoxFuture<'static, anyhow::Result<Forecast>> {
        let (hold, fail, temperature) = self.script(|script| {
            script.forecasts.push((place.clone(), units));
            let temperature = if place == paris() {
                24.
            } else {
                script.temperature
            };
            (script.hold.take(), script.fail, temperature)
        });
        async move {
            if let Some(hold) = hold {
                hold.await.ok();
            }
            if fail {
                anyhow::bail!("offline");
            }
            Ok(forecast(temperature))
        }
        .boxed()
    }

    fn remember(&self, place: Option<Place>) -> BoxFuture<'static, anyhow::Result<()>> {
        self.script(|script| script.remembered.push(place));
        async { Ok(()) }.boxed()
    }
}

/// No minimum skeleton time, so tests wait on data rather than on a timer.
fn instant() -> Options {
    Options {
        min_skeleton: Duration::ZERO,
        ..Options::default()
    }
}

fn open(
    cx: &mut TestAppContext,
    source: &Fake,
    system: SystemSettings,
    options: Options,
) -> (AnyWindowHandle, Entity<WeatherView>) {
    cx.update(gpui_kit::init);
    cx.update(nimbus::theme::init);
    let source: Arc<dyn WeatherSource> = Arc::new(source.clone());
    let mut view = None;
    let handle = cx.open_window(size(px(440.), px(820.)), |window, cx| {
        let weather = cx.new(|cx| WeatherView::new(source, system, options, window, cx));
        view = Some(weather.clone());
        Root::new(weather, window, cx)
    });
    (handle.into(), view.unwrap())
}

fn label(window: &Window, id: impl Into<ElementId>) -> Option<String> {
    window
        .try_find(id)
        .filter(|element| element.visible())
        .and_then(|element| element.label().map(str::to_string))
}

fn shows(window: &Window, id: impl Into<ElementId>) -> bool {
    window.try_find(id).is_some_and(|element| element.visible())
}

/// Wait until the current conditions read `expected`.
async fn wait_for_current(cx: &mut TestAppContext, handle: AnyWindowHandle, expected: &str) {
    cx.wait_for(handle, WAIT, |window, _| {
        label(window, "current").as_deref() == Some(expected)
    })
    .await;
}

fn update(cx: &mut TestAppContext, handle: AnyWindowHandle, f: impl FnOnce(&mut Window, &mut App)) {
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        f(window, cx);
    })
    .unwrap();
}

#[gpui_kit::test]
async fn paints_current_conditions_the_next_hours_and_seven_days(cx: &mut TestAppContext) {
    let source = Fake::new();
    let (handle, view) = open(cx, &source, canada(), instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    update(cx, handle, |window, _| {
        assert_eq!(label(window, "place").as_deref(), Some("Winnipeg"));
        assert!(!shows(window, "skeleton"));
        for ix in 0..24 {
            assert!(
                window.try_find(hour(ix)).is_some(),
                "hour {ix} is in the row"
            );
        }
        for ix in 0..7 {
            assert!(window.find(day(ix)).visible(), "day {ix} is on screen");
        }
        // The row is wider than the card, so the last hours start out of view.
        assert!(window.find(hour(0)).visible());
        assert!(!window.find(hour(23)).visible());
    });
    cx.update(|cx| {
        let Load::Ready { place, forecast } = view.read(cx).load() else {
            panic!("the forecast loaded");
        };
        assert_eq!(place, &home());
        assert_eq!(forecast.days.len(), 7);
    });
}

#[gpui_kit::test]
async fn asks_for_the_units_the_system_uses(cx: &mut TestAppContext) {
    let source = Fake::new();
    let system = SystemSettings {
        locale: "en-GB".into(),
        units: Units::Imperial,
        clock: Clock::TwentyFourHour,
    };
    let (handle, _) = open(cx, &source, system, instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    source.script(|script| {
        assert_eq!(script.forecasts, vec![(home(), Units::Imperial)]);
    });
}

#[gpui_kit::test]
async fn switches_to_a_searched_city_and_remembers_it(cx: &mut TestAppContext) {
    let source = Fake::new();
    let (handle, view) = open(cx, &source, canada(), instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    update(cx, handle, |window, cx| {
        window.click("place", cx);
    });
    update(cx, handle, |window, cx| {
        // The search field takes focus as it opens.
        assert_eq!(window.find("search").focused(), Some(true));
        window.input("Paris", cx);
        window.press("enter", cx);
    });
    cx.wait_for(handle, WAIT, |window, _| shows(window, result(0)))
        .await;
    update(cx, handle, |window, cx| {
        assert_eq!(window.find(result(0)).label(), Some("Paris"));
        window.click(result(0), cx);
    });
    wait_for_current(cx, handle, "24°, Cloudy").await;

    update(cx, handle, |window, _| {
        assert_eq!(label(window, "place").as_deref(), Some("Paris"));
    });
    source.script(|script| {
        assert_eq!(script.searches, ["Paris"]);
        assert_eq!(script.remembered, [Some(paris())]);
        assert_eq!(
            script.forecasts.last().map(|(place, _)| place),
            Some(&paris())
        );
    });
    cx.update(|cx| assert!(!view.read(cx).is_searching()));
}

#[gpui_kit::test]
async fn goes_back_to_automatic_location(cx: &mut TestAppContext) {
    let source = Fake::new();
    let (handle, _) = open(cx, &source, canada(), instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    update(cx, handle, |window, cx| {
        window.click("place", cx);
        window.click("use-my-location", cx);
    });
    wait_for_current(cx, handle, "19°, Cloudy").await;

    source.script(|script| assert_eq!(script.remembered, [None]));
}

#[gpui_kit::test]
async fn escape_closes_the_search(cx: &mut TestAppContext) {
    let source = Fake::new();
    let (handle, view) = open(cx, &source, canada(), instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    update(cx, handle, |window, cx| {
        window.click("place", cx);
        assert!(shows(window, "search"));
        window.press("escape", cx);
    });
    cx.wait_for(handle, WAIT, |window, _| shows(window, "current"))
        .await;

    cx.update(|cx| assert!(!view.read(cx).is_searching()));
    source.script(|script| {
        assert!(script.remembered.is_empty());
        assert_eq!(script.forecasts.len(), 1);
    });
}

#[gpui_kit::test]
async fn refreshes_the_forecast_from_the_title_bar(cx: &mut TestAppContext) {
    let source = Fake::new();
    let (handle, view) = open(cx, &source, canada(), instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    source.script(|script| script.temperature = 23.);
    update(cx, handle, |window, cx| window.click("refresh", cx));
    wait_for_current(cx, handle, "23°, Cloudy").await;

    source.script(|script| assert_eq!(script.forecasts.len(), 2));
    update(cx, handle, |window, _| {
        assert!(!shows(window, "refresh-status"))
    });
    cx.update(|cx| assert_eq!(view.read(cx).refresh(), Refresh::Idle));
}

#[gpui_kit::test]
async fn swaps_in_the_skeleton_while_a_refresh_loads(cx: &mut TestAppContext) {
    let source = Fake::new();
    let (handle, view) = open(cx, &source, canada(), instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    let release = source.hold();
    source.script(|script| script.temperature = 23.);
    update(cx, handle, |window, cx| window.click("refresh", cx));
    cx.wait_for(handle, WAIT, |window, _| shows(window, "skeleton"))
        .await;

    // The place stays in the title bar; the forecast itself is gone until data lands.
    let mut placeholders = Vec::new();
    update(cx, handle, |window, _| {
        assert_eq!(label(window, "place").as_deref(), Some("Winnipeg"));
        assert!(!shows(window, "current"));
        placeholders = vec![
            window.find("hourly-skeleton").bounds(),
            window.find("daily-skeleton").bounds(),
        ];
    });
    cx.update(|cx| assert_eq!(view.read(cx).refresh(), Refresh::Busy));

    release.send(()).unwrap();
    wait_for_current(cx, handle, "23°, Cloudy").await;
    update(cx, handle, |window, _| {
        assert!(!shows(window, "skeleton"));
        // The skeleton is laid out like the forecast, so nothing moves when it lands.
        let cards = [
            window.find("hourly").bounds(),
            window.find("daily").bounds(),
        ];
        for (placeholder, card) in placeholders.iter().zip(cards) {
            assert!(
                (placeholder.top() - card.top()).abs() < px(0.5)
                    && (placeholder.size.height - card.size.height).abs() < px(0.5),
                "skeleton card {placeholder:?} became {card:?}"
            );
        }
    });
}

#[gpui_kit::test]
async fn holds_the_skeleton_long_enough_to_see(cx: &mut TestAppContext) {
    let source = Fake::new();
    let options = Options {
        min_skeleton: Duration::from_millis(300),
        ..Options::default()
    };
    let (handle, _) = open(cx, &source, canada(), options);
    wait_for_current(cx, handle, "19°, Cloudy").await;

    let clicked = cx.executor().now();
    update(cx, handle, |window, cx| window.click("refresh", cx));
    cx.wait_for(handle, WAIT, |window, _| shows(window, "skeleton"))
        .await;
    wait_for_current(cx, handle, "19°, Cloudy").await;

    let shown = cx.executor().now() - clicked;
    assert!(
        shown >= Duration::from_millis(300),
        "skeleton showed for {shown:?}"
    );
}

#[gpui_kit::test]
async fn keeps_the_forecast_when_a_refresh_fails(cx: &mut TestAppContext) {
    let source = Fake::new();
    let (handle, _) = open(cx, &source, canada(), instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    source.script(|script| script.fail = true);
    update(cx, handle, |window, cx| window.click("refresh", cx));
    cx.wait_for(handle, WAIT, |window, _| shows(window, "refresh-status"))
        .await;
    update(cx, handle, |window, _| {
        assert_eq!(
            label(window, "refresh-status").as_deref(),
            Some("Couldn’t refresh")
        );
        assert_eq!(label(window, "current").as_deref(), Some("19°, Cloudy"));
    });

    // A later refresh that works clears the message.
    source.script(|script| script.fail = false);
    update(cx, handle, |window, cx| window.click("refresh", cx));
    cx.wait_for(handle, WAIT, |window, _| {
        shows(window, "current") && !shows(window, "refresh-status")
    })
    .await;
}

#[gpui_kit::test]
async fn shows_an_error_and_recovers_on_retry(cx: &mut TestAppContext) {
    let source = Fake::new();
    source.script(|script| script.fail = true);
    let (handle, _) = open(cx, &source, canada(), instant());

    cx.wait_for(handle, WAIT, |window, _| {
        label(window, "error").as_deref() == Some("Could not load the forecast")
    })
    .await;

    source.script(|script| script.fail = false);
    update(cx, handle, |window, cx| window.click("retry", cx));
    wait_for_current(cx, handle, "19°, Cloudy").await;
}

#[gpui_kit::test]
async fn the_wheel_glides_the_hourly_row(cx: &mut TestAppContext) {
    let source = Fake::new();
    let (handle, _) = open(cx, &source, canada(), instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    let mut start = px(0.);
    let mut line = px(0.);
    update(cx, handle, |window, cx| {
        start = window.find(hour(0)).bounds().left();
        line = window.line_height();
        // One notch down: three lines, towards later hours.
        window.scroll("hourly-wheel", ScrollDelta::Lines(point(0., -3.)), cx);
        // The first frame after the notch has barely begun to move.
        let moved = start - window.find(hour(0)).bounds().left();
        assert!(moved < line * 3. * 0.5, "the row jumped {moved:?} at once");
    });

    cx.wait_for(handle, WAIT, |window, _| {
        let moved = start - window.find(hour(0)).bounds().left();
        (moved - line * 3.).abs() < px(1.)
    })
    .await;
}

#[gpui_kit::test]
async fn dragging_the_row_scrolls_it(cx: &mut TestAppContext) {
    let source = Fake::new();
    let (handle, _) = open(cx, &source, canada(), instant());
    wait_for_current(cx, handle, "19°, Cloudy").await;

    update(cx, handle, |window, cx| {
        let before = window.find(hour(0)).bounds().left();
        let from = window.find(hour(4)).bounds().center();
        window.drag(from, from - point(px(120.), px(0.)), cx);
        window.render_frame(cx);
        let moved = before - window.find(hour(0)).bounds().left();
        assert!(moved > px(60.), "the row moved {moved:?}");
    });
}

fn hour(ix: usize) -> ElementId {
    ("hour", ix).into()
}

fn day(ix: usize) -> ElementId {
    ("day", ix).into()
}

fn result(ix: usize) -> ElementId {
    ("result", ix).into()
}
