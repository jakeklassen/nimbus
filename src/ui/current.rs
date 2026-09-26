use gpui_kit::{
    App, FontWeight, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Role,
    SharedString, StatefulInteractiveElement as _, Styled, TestSupportExt as _, Window,
    assets::IconName,
    component::{ActiveTheme as _, Icon, Sizable as _, h_flex, v_flex},
    div, rems,
};

use crate::{
    format::{degrees, percent},
    weather::{Current, Day, Units, describe},
};

/// The big temperature's type size and line height, in rems.
pub(crate) const TEMPERATURE_SIZE: f32 = 5.;
pub(crate) const TEMPERATURE_LINE: f32 = 5.75;

/// The big temperature, what the sky is doing, and three quick stats.
#[derive(IntoElement)]
pub struct CurrentConditions {
    current: Current,
    today: Option<Day>,
    units: Units,
}

impl CurrentConditions {
    pub fn new(current: Current, today: Option<Day>, units: Units) -> Self {
        Self {
            current,
            today,
            units,
        }
    }
}

impl RenderOnce for CurrentConditions {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let condition = describe(self.current.code, self.current.is_day);
        let temperature = degrees(self.current.temperature);
        let feels = degrees(self.current.feels_like);
        let summary = match &self.today {
            Some(today) => format!(
                "Feels {feels}  ·  H {}  L {}",
                degrees(today.max),
                degrees(today.min)
            ),
            None => format!("Feels {feels}"),
        };
        let icon_color = if condition.is_sunny() {
            theme.primary
        } else {
            theme.secondary_foreground
        };

        v_flex()
            .gap_4()
            .pt_2()
            .pb_5()
            .child(
                h_flex()
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_size(rems(TEMPERATURE_SIZE))
                                    .line_height(rems(TEMPERATURE_LINE))
                                    .font_weight(FontWeight::LIGHT)
                                    .child(temperature.clone()),
                            )
                            .child(div().text_lg().child(condition.label))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(theme.secondary_foreground)
                                    .child(summary),
                            ),
                    )
                    .child(Icon::new(condition.icon).size_16().text_color(icon_color)),
            )
            .child(
                h_flex()
                    .gap_5()
                    .child(stat(
                        IconName::Wind,
                        format!(
                            "{} {}",
                            self.current.wind.round() as i64,
                            self.units.wind_label()
                        ),
                        cx,
                    ))
                    .child(stat(IconName::Droplets, percent(self.current.humidity), cx))
                    .child(stat(
                        IconName::Umbrella,
                        percent(self.current.precipitation),
                        cx,
                    )),
            )
            // Screen readers get the headline in one phrase.
            .id("current")
            .role(Role::Group)
            .aria_label(SharedString::from(format!(
                "{temperature}, {}",
                condition.label
            )))
            .test_support()
    }
}

fn stat(icon: IconName, value: String, cx: &App) -> impl IntoElement {
    h_flex()
        .gap_1p5()
        .child(
            Icon::new(icon)
                .small()
                .text_color(cx.theme().muted_foreground),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().secondary_foreground)
                .child(value),
        )
}
