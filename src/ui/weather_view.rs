use std::{sync::Arc, time::Duration};

use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement, Render, Role, SharedString, StatefulInteractiveElement as _, Styled,
    Subscription, Task, TestSupportExt as _, Window,
    assets::IconName,
    component::{
        ActiveTheme as _, Disableable as _, Icon, Sizable as _, TitleBar,
        button::{Button, ButtonVariants as _},
        h_flex, v_flex,
    },
    div,
    prelude::FluentBuilder as _,
};

use crate::{
    system::SystemSettings,
    theme,
    ui::{
        current::CurrentConditions,
        daily::Daily,
        hourly::Hourly,
        search::{PlaceSearch, PlaceSearchEvent},
        skeleton::ForecastSkeleton,
    },
    update::Updater,
    weather::{Forecast, Place, WeatherSource, notable_precipitation},
};

/// Timing the tests shorten.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// A fast response would flash the skeleton for a frame, so a load the
    /// user can see holds it at least this long.
    pub min_skeleton: Duration,
    /// How often the forecast refreshes on its own, silently.
    pub refresh_every: Duration,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            min_skeleton: Duration::from_millis(500),
            refresh_every: Duration::from_secs(15 * 60),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Load {
    Loading,
    Ready {
        place: Place,
        forecast: Forecast,
    },
    Failed {
        message: SharedString,
        place: Option<Place>,
    },
}

/// Only describes refreshes over a forecast already on screen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refresh {
    Idle,
    Busy,
    Failed,
}

enum LoadError {
    /// No place was asked for, and none could be found.
    Locate,
    Forecast(Place),
}

/// The whole screen: a title bar with the place, refresh and any waiting
/// update, then either the
/// forecast, its skeleton, an error, or the city search.
pub struct WeatherView {
    source: Arc<dyn WeatherSource>,
    system: SystemSettings,
    options: Options,
    load: Load,
    refresh: Refresh,
    skeleton: bool,
    /// The place asked for. `None` locates automatically on every load.
    requested: Option<Place>,
    hourly: Entity<Hourly>,
    search: Option<Entity<PlaceSearch>>,
    updater: Entity<Updater>,
    // Replacing a task drops the one before it, so a stale load can never land.
    _load: Task<()>,
    _remember: Task<()>,
    _refresh_timer: Task<()>,
    _search_events: Option<Subscription>,
    _appearance: Subscription,
    _updates: Subscription,
}

impl WeatherView {
    pub fn new(
        source: Arc<dyn WeatherSource>,
        system: SystemSettings,
        options: Options,
        updater: Entity<Updater>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let hourly = cx.new(|_| Hourly::new(system.clock));
        let appearance = theme::follow_system(window, cx);
        let updates = cx.observe(&updater, |_, _, cx| cx.notify());
        let mut this = Self {
            source,
            system,
            options,
            load: Load::Loading,
            refresh: Refresh::Idle,
            skeleton: true,
            requested: None,
            hourly,
            search: None,
            updater,
            _load: Task::ready(()),
            _remember: Task::ready(()),
            _refresh_timer: Task::ready(()),
            _search_events: None,
            _appearance: appearance,
            _updates: updates,
        };
        this.reload(true, cx);
        this
    }

    pub fn load(&self) -> &Load {
        &self.load
    }

    pub fn refresh(&self) -> Refresh {
        self.refresh
    }

    pub fn is_searching(&self) -> bool {
        self.search.is_some()
    }

    /// Fetch the forecast again. A `visible` load swaps in the skeleton; the
    /// timer's loads keep the forecast on screen until the new one lands.
    fn reload(&mut self, visible: bool, cx: &mut Context<Self>) {
        let visible = visible || !matches!(self.load, Load::Ready { .. });
        self.skeleton = visible;
        self.refresh = Refresh::Busy;

        let source = self.source.clone();
        let requested = self.requested.clone();
        let units = self.system.units;
        let fetch = async move {
            let place = match requested {
                Some(place) => place,
                None => source.locate().await.map_err(|_| LoadError::Locate)?,
            };
            match source.forecast(place.clone(), units).await {
                Ok(forecast) => Ok((place, forecast)),
                Err(_) => Err(LoadError::Forecast(place)),
            }
        };

        let executor = cx.background_executor().clone();
        let started = executor.now();
        let hold = if visible {
            self.options.min_skeleton
        } else {
            Duration::ZERO
        };
        self._load = cx.spawn(async move |this, cx| {
            let result = cx.background_spawn(fetch).await;
            let shown = executor.now().saturating_duration_since(started);
            if shown < hold {
                executor.timer(hold - shown).await;
            }
            this.update(cx, |this, cx| this.finish_load(result, cx))
                .ok();
        });

        let every = self.options.refresh_every;
        self._refresh_timer = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(every).await;
            this.update(cx, |this, cx| this.reload(false, cx)).ok();
        });
        cx.notify();
    }

    fn finish_load(
        &mut self,
        result: Result<(Place, Forecast), LoadError>,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok((place, forecast)) => {
                self.hourly.update(cx, |hourly, cx| {
                    hourly.set_hours(forecast.hours.clone(), cx)
                });
                self.load = Load::Ready { place, forecast };
                self.refresh = Refresh::Idle;
            }
            // Keep the last good forecast on screen when a refresh fails, and say so.
            Err(_) if matches!(self.load, Load::Ready { .. }) => {
                self.refresh = Refresh::Failed;
            }
            Err(error) => {
                self.refresh = Refresh::Idle;
                self.load = match error {
                    LoadError::Locate => Load::Failed {
                        message: "Could not find your location".into(),
                        place: None,
                    },
                    LoadError::Forecast(place) => Load::Failed {
                        message: "Could not load the forecast".into(),
                        place: Some(place),
                    },
                };
            }
        }
        self.skeleton = false;
        cx.notify();
    }

    fn retry(&mut self, cx: &mut Context<Self>) {
        if let Load::Failed {
            place: Some(place), ..
        } = &self.load
        {
            self.requested = Some(place.clone());
        }
        self.load = Load::Loading;
        self.reload(true, cx);
    }

    /// Show a place, or go back to automatic location with `None`.
    fn pick(&mut self, place: Option<Place>, cx: &mut Context<Self>) {
        self.close_search(cx);
        self.load = Load::Loading;
        self.skeleton = true;
        self.requested = place.clone();
        // Remember first: automatic location reads the remembered place.
        let remember = self.source.remember(place);
        self._remember = cx.spawn(async move |this, cx| {
            cx.background_spawn(remember).await.ok();
            this.update(cx, |this, cx| this.reload(true, cx)).ok();
        });
        cx.notify();
    }

    fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.source.clone();
        let search = cx.new(|cx| PlaceSearch::new(source, window, cx));
        self._search_events =
            Some(
                cx.subscribe_in(&search, window, |this, _, event, _, cx| match event {
                    PlaceSearchEvent::Picked(place) => this.pick(place.clone(), cx),
                    PlaceSearchEvent::Cancelled => this.close_search(cx),
                }),
            );
        self.search = Some(search);
        cx.notify();
    }

    fn close_search(&mut self, cx: &mut Context<Self>) {
        self.search = None;
        self._search_events = None;
        cx.notify();
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let place_name: SharedString = match &self.load {
            Load::Loading => "Locating…".into(),
            Load::Ready { place, .. } => place.name.clone().into(),
            Load::Failed { place, .. } => place
                .as_ref()
                .map_or("Choose a city".into(), |place| place.name.clone().into()),
        };
        let ready = matches!(self.load, Load::Ready { .. });
        let update = self.updater.read(cx).ready().cloned();

        TitleBar::new().child(h_flex().flex_1().pr_1().justify_between().when(
            self.search.is_none(),
            |this| {
                this.child(
                    // Title bar buttons block the drag area behind them. Otherwise
                    // Windows hit-tests the press as a caption drag, and the button
                    // never sees it.
                    Button::new("place")
                        .occlude()
                        .ghost()
                        .small()
                        .tooltip("Choose a city")
                        .accessibility_label(place_name.clone())
                        .child(
                            h_flex()
                                .gap_1p5()
                                .child(
                                    Icon::new(IconName::MapPin)
                                        .small()
                                        .text_color(theme.muted_foreground),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(theme.secondary_foreground)
                                        .child(place_name),
                                ),
                        )
                        .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx))),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .when_some(update, |this, version| {
                            this.child(
                                Button::new("update")
                                    .occlude()
                                    .primary()
                                    .xsmall()
                                    .label("Restart to update")
                                    .tooltip(SharedString::from(format!(
                                        "Nimbus {version} is ready"
                                    )))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.updater.update(cx, |updater, _| updater.restart());
                                    })),
                            )
                        })
                        .when(ready, |this| {
                            this.when(self.refresh == Refresh::Failed, |this| {
                                this.child(
                                    div()
                                        .text_xs()
                                        .text_color(theme.muted_foreground)
                                        .child("Couldn’t refresh")
                                        .id("refresh-status")
                                        .role(Role::Status)
                                        .aria_label("Couldn’t refresh")
                                        .test_support(),
                                )
                            })
                            .child(
                                Button::new("refresh")
                                    .occlude()
                                    .ghost()
                                    .small()
                                    .icon(
                                        Icon::new(IconName::RefreshCw)
                                            .text_color(theme.muted_foreground),
                                    )
                                    .tooltip("Refresh forecast")
                                    .accessibility_label("Refresh forecast")
                                    .loading(self.refresh == Refresh::Busy)
                                    .disabled(self.refresh == Refresh::Busy)
                                    .on_click(cx.listener(|this, _, _, cx| this.reload(true, cx))),
                            )
                        }),
                )
            },
        ))
    }

    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.skeleton || matches!(self.load, Load::Loading) {
            // Keep the rain line if the forecast being replaced had one, so the
            // hourly card does not change height when it comes back.
            let precipitation_line = matches!(
                &self.load,
                Load::Ready { forecast, .. }
                    if forecast.hours.iter().any(|hour| notable_precipitation(hour.precipitation))
            );
            return ForecastSkeleton::new(precipitation_line).into_any_element();
        }

        match &self.load {
            Load::Loading => unreachable!("handled by the skeleton above"),
            Load::Failed { message, .. } => v_flex()
                .flex_1()
                .items_center()
                .justify_center()
                .gap_3p5()
                .child(
                    div()
                        .text_color(cx.theme().secondary_foreground)
                        .child(message.clone())
                        .id("error")
                        .role(Role::Alert)
                        .aria_label(message.clone())
                        .test_support(),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("retry")
                                .primary()
                                .label("Try again")
                                .on_click(cx.listener(|this, _, _, cx| this.retry(cx))),
                        )
                        .child(
                            Button::new("choose-city")
                                .ghost()
                                .label("Choose a city")
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.open_search(window, cx)),
                                ),
                        ),
                )
                .into_any_element(),
            Load::Ready { forecast, .. } => v_flex()
                .gap_3()
                .child(CurrentConditions::new(
                    forecast.current.clone(),
                    forecast.days.first().cloned(),
                    self.system.units,
                ))
                .child(self.hourly.clone())
                .child(Daily::new(
                    forecast.days.clone(),
                    self.system.locale.clone(),
                ))
                .into_any_element(),
        }
    }
}

impl Render for WeatherView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (background, foreground) = (cx.theme().background, cx.theme().foreground);
        let body = match &self.search {
            Some(search) => search.clone().into_any_element(),
            None => self.render_body(cx),
        };

        v_flex()
            .size_full()
            .bg(background)
            .text_color(foreground)
            .child(self.render_title_bar(cx))
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .px_5()
                    .pb_5()
                    .gap_3()
                    .child(body),
            )
    }
}
