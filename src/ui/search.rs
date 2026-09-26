use std::sync::Arc;

use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, InteractiveElement as _, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Task, Window,
    assets::IconName,
    component::{
        ActiveTheme as _, Icon, Sizable as _,
        button::{Button, ButtonVariants as _},
        h_flex,
        input::{Escape, Input, InputEvent, InputState},
        v_flex,
    },
    div,
    prelude::FluentBuilder as _,
};

use crate::weather::{Place, WeatherSource};

/// What the user decided in the search panel.
pub enum PlaceSearchEvent {
    /// A place to show, or `None` for automatic location.
    Picked(Option<Place>),
    Cancelled,
}

/// Find a city by name. Enter searches, Escape or Cancel goes back.
pub struct PlaceSearch {
    source: Arc<dyn WeatherSource>,
    query: Entity<InputState>,
    results: Vec<Place>,
    status: Option<SharedString>,
    _search: Task<()>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<PlaceSearchEvent> for PlaceSearch {}

impl PlaceSearch {
    pub fn new(
        source: Arc<dyn WeatherSource>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("Search a city"));
        let subscription = cx.subscribe_in(&query, window, |this, _, event, _, cx| {
            if let InputEvent::PressEnter { .. } = event {
                this.submit(cx);
            }
        });
        query.update(cx, |query, cx| query.focus(window, cx));
        Self {
            source,
            query,
            results: Vec::new(),
            status: None,
            _search: Task::ready(()),
            _subscriptions: vec![subscription],
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let query = self.query.read(cx).value().trim().to_string();
        if query.is_empty() {
            return;
        }
        self.status = Some("Searching…".into());
        cx.notify();

        let search = self.source.search(query);
        // Replacing the task drops a search still in flight.
        self._search = cx.spawn(async move |this, cx| {
            let found = cx.background_spawn(search).await;
            this.update(cx, |this, cx| {
                match found {
                    Ok(places) => {
                        this.status = places.is_empty().then(|| "No places found".into());
                        this.results = places;
                    }
                    Err(_) => {
                        this.status = Some("Search failed. Check your connection.".into());
                    }
                }
                cx.notify();
            })
            .ok();
        });
    }

    fn pick(&mut self, place: Option<Place>, cx: &mut Context<Self>) {
        cx.emit(PlaceSearchEvent::Picked(place));
    }

    fn cancel(&mut self, _: &Escape, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(PlaceSearchEvent::Cancelled);
    }

    fn render_row(
        &self,
        id: impl Into<gpui_kit::ElementId>,
        icon: IconName,
        title: impl Into<SharedString>,
        detail: Option<SharedString>,
        place: Option<Place>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let title = title.into();
        let theme = cx.theme();
        Button::new(id)
            .ghost()
            .w_full()
            .h_11()
            .px_2p5()
            .justify_start()
            .accessibility_label(title.clone())
            .child(
                h_flex()
                    .gap_3()
                    .flex_1()
                    .min_w_0()
                    .child(Icon::new(icon).small().text_color(theme.muted_foreground))
                    .child(div().text_color(theme.foreground).child(title))
                    .when_some(detail, |this, detail| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(theme.muted_foreground)
                                .truncate()
                                .child(detail),
                        )
                    }),
            )
            .on_click(cx.listener(move |this, _, _, cx| this.pick(place.clone(), cx)))
    }
}

impl Render for PlaceSearch {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let muted = cx.theme().muted_foreground;
        let results = self
            .results
            .iter()
            .enumerate()
            .map(|(ix, place)| {
                self.render_row(
                    ("result", ix),
                    IconName::Search,
                    place.name.clone(),
                    place.region.clone().map(SharedString::from),
                    Some(place.clone()),
                    cx,
                )
                .into_any_element()
            })
            .collect::<Vec<_>>();

        v_flex()
            .id("place-search")
            .key_context("PlaceSearch")
            .on_action(cx.listener(Self::cancel))
            .flex_1()
            .min_h_0()
            .gap_2p5()
            .pt_1()
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        Input::new(&self.query)
                            .id("search")
                            .large()
                            .flex_1()
                            .prefix(Icon::new(IconName::Search).small())
                            .cleanable(true),
                    )
                    .child(
                        Button::new("search-cancel")
                            .ghost()
                            .label("Cancel")
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(PlaceSearchEvent::Cancelled);
                            })),
                    ),
            )
            .child(
                v_flex()
                    .child(self.render_row(
                        "use-my-location",
                        IconName::MapPin,
                        "Use my location",
                        None,
                        None,
                        cx,
                    ))
                    .children(results),
            )
            .when_some(self.status.clone(), |this, status| {
                this.child(div().pl_2p5().text_sm().text_color(muted).child(status))
            })
    }
}
