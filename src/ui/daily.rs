use gpui_kit::{
    App, ColorSpace, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Styled,
    TestSupportExt as _, Window,
    component::{ActiveTheme as _, Icon, h_flex},
    div, linear_color_stop, linear_gradient, relative, rems,
};

use crate::{
    format::{chance_label, day_label, degrees},
    ui::card::Card,
    weather::{Day, describe},
};

/// Width of the weekday column, sized for "Today".
pub(crate) const DAY_COLUMN: f32 = 3.5;
/// Width of each number column: a chance of rain, a low, a high.
pub(crate) const NUMBER_COLUMN: f32 = 2.25;

/// The week, one row per day, with every range on the same scale.
#[derive(IntoElement)]
pub struct Daily {
    days: Vec<Day>,
    locale: String,
}

impl Daily {
    pub fn new(days: Vec<Day>, locale: impl Into<String>) -> Self {
        Self {
            days,
            locale: locale.into(),
        }
    }
}

impl RenderOnce for Daily {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let week_min = self
            .days
            .iter()
            .map(|day| day.min)
            .fold(f64::INFINITY, f64::min);
        let week_max = self
            .days
            .iter()
            .map(|day| day.max)
            .fold(f64::NEG_INFINITY, f64::max);

        Card::new("daily")
            .title("7 days")
            .tight()
            .children(self.days.iter().enumerate().map(|(ix, day)| {
                let condition = describe(day.code, true);
                let icon_color = if condition.is_sunny() {
                    theme.primary
                } else {
                    theme.secondary_foreground
                };
                let chance = chance_label(day.precipitation);

                h_flex()
                    .gap_3()
                    .h_10()
                    .child(
                        div()
                            .w(rems(DAY_COLUMN))
                            .child(day_label(day.date, ix, &self.locale)),
                    )
                    .child(
                        h_flex()
                            .w_6()
                            .child(Icon::new(condition.icon).size_5().text_color(icon_color)),
                    )
                    .child(
                        div()
                            .w(rems(NUMBER_COLUMN))
                            .text_xs()
                            .text_color(theme.info)
                            .child(chance),
                    )
                    .child(
                        div()
                            .w(rems(NUMBER_COLUMN))
                            .text_right()
                            .text_color(theme.muted_foreground)
                            .child(degrees(day.min)),
                    )
                    .child(RangeBar::new(day.min, day.max, week_min, week_max))
                    .child(div().w(rems(NUMBER_COLUMN)).child(degrees(day.max)))
                    .id(("day", ix))
                    .test_support()
            }))
    }
}

/// A day's low-to-high span on a track that runs from the week's lowest low
/// to its highest high.
#[derive(IntoElement)]
struct RangeBar {
    /// Where the span starts, as a fraction of the track.
    start: f32,
    /// How much of the track the span covers.
    span: f32,
}

impl RangeBar {
    fn new(min: f64, max: f64, week_min: f64, week_max: f64) -> Self {
        let week = (week_max - week_min).max(1.);
        // A day with no swing still shows a sliver.
        let span = ((max - min) / week).max(0.04);
        let start = ((min - week_min) / week).clamp(0., 1. - span);
        Self {
            start: start as f32,
            span: span as f32,
        }
    }
}

impl RenderOnce for RangeBar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .relative()
            .flex_1()
            .h_1()
            .rounded_full()
            .bg(theme.slider_bar)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(relative(self.start))
                    .w(relative(self.span))
                    .rounded_full()
                    .bg(linear_gradient(
                        90.,
                        linear_color_stop(theme.info, 0.),
                        linear_color_stop(theme.primary, 1.),
                    )
                    .color_space(ColorSpace::Oklab)),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::RangeBar;

    #[test]
    fn ranges_share_the_week_scale() {
        let bar = RangeBar::new(10., 20., 10., 30.);
        assert_eq!(bar.start, 0.);
        assert_eq!(bar.span, 0.5);

        let bar = RangeBar::new(25., 30., 10., 30.);
        assert_eq!(bar.start, 0.75);
        assert_eq!(bar.span, 0.25);
    }

    #[test]
    fn a_flat_day_keeps_a_sliver_inside_the_track() {
        let bar = RangeBar::new(30., 30., 10., 30.);
        assert!(bar.span > 0.);
        assert!(bar.start + bar.span <= 1.);
    }
}
