use gpui_kit::{
    App, Div, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Role,
    StatefulInteractiveElement as _, Styled, TestSupportExt as _, Window,
    component::{h_flex, skeleton::Skeleton, v_flex},
    div,
    prelude::FluentBuilder as _,
    rems,
};

use crate::{
    ui::{
        card::Card,
        current::{TEMPERATURE_LINE, TEMPERATURE_SIZE},
        daily::{DAY_COLUMN, NUMBER_COLUMN},
    },
    weather::{DAYS_SHOWN, HOURS_SHOWN},
};

/// Stands in for the forecast while it loads.
///
/// Every bone sits in a line box styled like the text it replaces, so nothing
/// moves when the real screen lands. Kit's [`Skeleton`] brings the pulse.
#[derive(IntoElement)]
pub struct ForecastSkeleton {
    precipitation_line: bool,
}

impl ForecastSkeleton {
    /// `precipitation_line` reserves the hourly row's rain line, when the
    /// forecast being replaced had one.
    pub fn new(precipitation_line: bool) -> Self {
        Self { precipitation_line }
    }
}

/// A placeholder bar, centred in `line`: a box styled like the text it stands
/// for. The box holds a non-breaking space in that style, so it takes the
/// text's own height whatever the font's metrics.
fn bone(line: Div, width: f32, height: f32) -> impl IntoElement {
    line.relative().w(rems(width)).child("\u{a0}").child(
        h_flex()
            .absolute()
            .inset_0()
            .child(Skeleton::new().w_full().h(rems(height)).rounded_sm()),
    )
}

/// A line of nothing, as tall as a line of `line`'s text.
fn blank(line: Div) -> impl IntoElement {
    line.child("\u{a0}")
}

impl RenderOnce for ForecastSkeleton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        v_flex()
            .gap_3()
            .child(
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
                                    .child(bone(
                                        div()
                                            .text_size(rems(TEMPERATURE_SIZE))
                                            .line_height(rems(TEMPERATURE_LINE)),
                                        7.,
                                        4.,
                                    ))
                                    .child(bone(div().text_lg(), 6., 0.85))
                                    .child(bone(div().text_sm(), 11., 0.65)),
                            )
                            .child(
                                h_flex()
                                    .size_16()
                                    .justify_center()
                                    .child(Skeleton::new().size(rems(3.5)).rounded_2xl()),
                            ),
                    )
                    .child(h_flex().gap_5().children([3.5, 2.25, 1.75].map(|width| {
                        h_flex()
                            .gap_1p5()
                            .child(Skeleton::new().size_3p5().rounded_sm())
                            .child(bone(div().text_sm(), width, 0.65))
                    }))),
            )
            .child(
                Card::new("hourly-skeleton")
                    .child(bone(div().text_xs(), 5.5, 0.6))
                    .child(h_flex().mx_neg_1p5().pb_3().overflow_hidden().children(
                        (0..HOURS_SHOWN).map(|_| {
                            v_flex()
                                .w_12()
                                .flex_shrink_0()
                                .items_center()
                                .gap_1p5()
                                .py_0p5()
                                .child(bone(div().text_xs(), 1.75, 0.6))
                                .child(Skeleton::new().size_5().rounded_md())
                                .child(bone(div().text_sm(), 1.75, 0.75))
                                .when(self.precipitation_line, |this| {
                                    this.child(blank(div().text_xs()))
                                })
                        }),
                    )),
            )
            .child(
                Card::new("daily-skeleton")
                    .tight()
                    .child(bone(div().text_xs(), 3., 0.6))
                    .children((0..DAYS_SHOWN).map(|ix| {
                        h_flex()
                            .gap_3()
                            .h_10()
                            .child(div().w(rems(DAY_COLUMN)).child(bone(
                                div(),
                                if ix == 0 { 2.75 } else { 2. },
                                0.75,
                            )))
                            .child(h_flex().w_6().child(Skeleton::new().size_5().rounded_md()))
                            .child(div().w(rems(NUMBER_COLUMN)))
                            .child(h_flex().w(rems(NUMBER_COLUMN)).justify_end().child(bone(
                                div(),
                                1.5,
                                0.75,
                            )))
                            .child(Skeleton::new().flex_1().h_1().rounded_full())
                            .child(div().w(rems(NUMBER_COLUMN)).child(bone(div(), 1.5, 0.75)))
                    })),
            )
            .id("skeleton")
            .role(Role::ProgressIndicator)
            .aria_label("Loading forecast")
            .test_support()
    }
}
