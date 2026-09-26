use std::time::Duration;

use gpui_kit::{
    AppContext as _, Context, CursorStyle, DragMoveEvent, EmptyView, InteractiveElement as _,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, Pixels, Render, ScrollDelta,
    ScrollHandle, ScrollWheelEvent, StatefulInteractiveElement as _, Styled, TestSupportExt as _,
    Window,
    base::{Spring, spring},
    component::{ActiveTheme as _, Icon, scroll::ScrollableElement as _, v_flex},
    div, linear_color_stop, linear_gradient, point,
    prelude::FluentBuilder as _,
    px,
};

use crate::{
    format::{chance_label, degrees, hour_label},
    system::Clock,
    ui::card::Card,
    weather::{Hour, describe, notable_precipitation},
};

/// How a notched wheel glides to where it points. Short enough to keep up
/// with a spun wheel, long enough to read as motion rather than a jump.
const WHEEL_RESPONSE: Duration = Duration::from_millis(220);
/// A glide closer than this to its target has arrived.
const WHEEL_SETTLED: f32 = 0.5;

/// The next 24 hours in a row wider than its card.
///
/// It scrolls four ways: a notched mouse wheel glides on a spring, a trackpad
/// and the Kit scrollbar move it directly, and so does dragging the row. The
/// scroll handle is the one source of truth, for the edge fades too.
pub struct Hourly {
    hours: Vec<Hour>,
    clock: Clock,
    scroll: ScrollHandle,
    /// Where the pointer went down, for dragging the row.
    press: Option<Press>,
    /// Where a wheel glide is headed, in pixels from the start.
    glide_to: Option<Pixels>,
}

#[derive(Clone, Copy)]
struct Press {
    x: Pixels,
    scrolled: Pixels,
}

/// The payload of a row drag. The row moves; nothing is dropped anywhere.
#[derive(Clone, Copy)]
struct RowDrag;

impl Hourly {
    pub fn new(clock: Clock) -> Self {
        Self {
            hours: Vec::new(),
            clock,
            scroll: ScrollHandle::new(),
            press: None,
            glide_to: None,
        }
    }

    /// Show new hours. A row that starts at a different hour, such as another
    /// place's, goes back to "Now".
    pub fn set_hours(&mut self, hours: Vec<Hour>, cx: &mut Context<Self>) {
        let first = |hours: &[Hour]| hours.first().map(|hour| hour.time);
        if first(&self.hours) != first(&hours) {
            self.glide_to = None;
            self.scroll.set_offset(point(px(0.), px(0.)));
        }
        self.hours = hours;
        cx.notify();
    }

    /// Pixels scrolled from the start. GPUI stores the same value negated.
    fn scrolled(&self) -> Pixels {
        -self.scroll.offset().x
    }

    fn max_scroll(&self) -> Pixels {
        self.scroll.max_offset().x.max(px(0.))
    }

    fn scroll_to(&self, x: Pixels) {
        self.scroll
            .set_offset(point(-x.clamp(px(0.), self.max_scroll()), px(0.)));
    }

    /// A notched wheel moves a few lines at a time. GPUI would jump the row
    /// there; Nimbus glides instead. Trackpads already send smooth pixels, so
    /// they fall through to GPUI's own scrolling.
    fn on_wheel(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ScrollDelta::Lines(lines) = event.delta else {
            return;
        };
        // The row only scrolls sideways, so a vertical wheel drives it.
        let lines = if lines.y.abs() > lines.x.abs() {
            lines.y
        } else {
            lines.x
        };
        let from = self.glide_to.unwrap_or_else(|| self.scrolled());
        let to = (from - window.line_height() * lines).clamp(px(0.), self.max_scroll());
        self.glide_to = Some(to);
        cx.stop_propagation();
        cx.notify();
    }

    fn render_hour(
        &self,
        ix: usize,
        hour: &Hour,
        show_precipitation: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let theme = cx.theme();
        let first = ix == 0;
        let condition = describe(hour.code, hour.is_day);
        let label = if first {
            "Now".to_string()
        } else {
            hour_label(hour.time, self.clock)
        };
        let chance = chance_label(hour.precipitation);

        v_flex()
            .w_12()
            .flex_shrink_0()
            .items_center()
            .gap_1p5()
            .py_0p5()
            .child(
                div()
                    .text_xs()
                    .text_color(if first {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .child(label),
            )
            .child(
                Icon::new(condition.icon)
                    .size_5()
                    .text_color(if condition.is_sunny() {
                        theme.primary
                    } else {
                        theme.secondary_foreground
                    }),
            )
            .child(div().text_sm().child(degrees(hour.temperature)))
            .when(show_precipitation, |this| {
                this.child(div().text_xs().text_color(theme.info).child(chance))
            })
            .id(("hour", ix))
            .test_support()
    }
}

impl Render for Hourly {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The spring rests wherever the row is, so a glide always starts from
        // the row's real position, even after a drag or a trackpad swipe.
        let resting = self.glide_to.is_none();
        let target = self.glide_to.unwrap_or_else(|| self.scrolled());
        let glided = spring(
            "hourly-glide",
            target,
            Spring::new(WHEEL_RESPONSE)
                .with_epsilon(WHEEL_SETTLED)
                .with_travel(!resting),
            window,
            cx,
        );
        if !resting {
            self.scroll_to(glided);
            if glided == target {
                self.glide_to = None;
            }
        }

        let theme = cx.theme();
        let surface = theme.group_box;
        let scrolled = self.scrolled();
        let max = self.max_scroll();
        let scrollable = max > px(0.);
        // Reserve the precipitation line only when some hour will use it.
        let show_precipitation = self
            .hours
            .iter()
            .any(|hour| notable_precipitation(hour.precipitation));

        // Each side fades out when there are more hours beyond it.
        let fade = |angle: f32| {
            div()
                .absolute()
                .top_0()
                .bottom_3()
                .w_7()
                .bg(linear_gradient(
                    angle,
                    linear_color_stop(surface.opacity(0.), 0.),
                    linear_color_stop(surface, 1.),
                ))
        };

        let columns = self
            .hours
            .iter()
            .enumerate()
            .map(|(ix, hour)| {
                self.render_hour(ix, hour, show_precipitation, cx)
                    .into_any_element()
            })
            .collect::<Vec<_>>();

        Card::new("hourly").title("Next 24 hours").child(
            div()
                .relative()
                // The columns centre their text, so let the first and last
                // reach into the card's padding.
                .mx_neg_1p5()
                .child(
                    div()
                        .id("hourly-row")
                        .flex()
                        .flex_row()
                        .overflow_x_scroll()
                        .track_scroll(&self.scroll)
                        .pb_3()
                        .when(scrollable, |this| this.cursor(CursorStyle::OpenHand))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, _| {
                                this.glide_to = None;
                                this.press = Some(Press {
                                    x: event.position.x,
                                    scrolled: this.scrolled(),
                                });
                            }),
                        )
                        .on_drag(RowDrag, |_, _, _, cx| cx.new(|_| EmptyView))
                        .on_drag_move(cx.listener(|this, event: &DragMoveEvent<RowDrag>, _, cx| {
                            if let Some(press) = this.press {
                                this.scroll_to(press.scrolled - (event.event.position.x - press.x));
                                cx.notify();
                            }
                        }))
                        .children(columns)
                        .horizontal_scrollbar(&self.scroll),
                )
                .when(scrollable && scrolled > px(1.), |this| {
                    this.child(fade(270.).left_0())
                })
                .when(scrollable && scrolled < max - px(1.), |this| {
                    this.child(fade(90.).right_0())
                })
                // GPUI hands the wheel to the last-painted listener first, and
                // the row registers its own scrolling after any listener set on
                // it. This layer over the hours (not the scrollbar) gets the
                // wheel before the row does.
                .child(
                    div()
                        .id("hourly-wheel")
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_3()
                        .on_scroll_wheel(cx.listener(Self::on_wheel))
                        .test_support(),
                ),
        )
    }
}
