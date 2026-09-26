use gpui_kit::{
    App, InteractiveElement as _, IntoElement, ParentElement, RenderOnce, Role,
    StatefulInteractiveElement as _, Styled, TestSupportExt as _, Window,
    component::{h_flex, skeleton::Skeleton, v_flex},
    div,
    prelude::FluentBuilder as _,
    rems,
};

use crate::{
    ui::{
        card::Card,
        daily::{DAY_COLUMN, NUMBER_COLUMN},
    },
    weather::{DAYS_SHOWN, HOURS_SHOWN},
};

/// Stands in for the forecast while it loads.
///
/// Every bone sits in a box the height of the text it replaces, so nothing
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

/// A placeholder bar, centred in a line box of the text it stands for.
fn bone(width: f32, height: f32, line: f32) -> impl IntoElement {
    h_flex()
        .h(rems(line))
        .child(Skeleton::new().w(rems(width)).h(rems(height)).rounded_sm())
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
                                    .child(
                                        h_flex()
                                            .h(rems(5.75))
                                            .child(Skeleton::new().w(rems(7.)).h_16().rounded_xl()),
                                    )
                                    .child(bone(6., 0.85, 1.6))
                                    .child(bone(11., 0.65, 1.2)),
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
                            .child(bone(width, 0.65, 1.2))
                    }))),
            )
            .child(
                Card::new("hourly-skeleton")
                    .child(bone(5.5, 0.6, 1.))
                    .child(h_flex().mx_neg_1p5().pb_3().overflow_hidden().children(
                        (0..HOURS_SHOWN).map(|_| {
                            v_flex()
                                .w_12()
                                .flex_shrink_0()
                                .items_center()
                                .gap_1p5()
                                .py_0p5()
                                .child(bone(1.75, 0.6, 1.))
                                .child(Skeleton::new().size_5().rounded_md())
                                .child(bone(1.75, 0.75, 1.25))
                                .when(self.precipitation_line, |this| {
                                    this.child(div().h(rems(1.)))
                                })
                        }),
                    )),
            )
            .child(
                Card::new("daily-skeleton")
                    .tight()
                    .child(bone(3., 0.6, 1.))
                    .children((0..DAYS_SHOWN).map(|ix| {
                        h_flex()
                            .gap_3()
                            .h_10()
                            .child(div().w(rems(DAY_COLUMN)).child(bone(
                                if ix == 0 { 2.75 } else { 2. },
                                0.75,
                                1.25,
                            )))
                            .child(h_flex().w_6().child(Skeleton::new().size_5().rounded_md()))
                            .child(div().w(rems(NUMBER_COLUMN)))
                            .child(
                                h_flex()
                                    .w(rems(NUMBER_COLUMN))
                                    .justify_end()
                                    .child(bone(1.5, 0.75, 1.25)),
                            )
                            .child(Skeleton::new().flex_1().h_1().rounded_full())
                            .child(div().w(rems(NUMBER_COLUMN)).child(bone(1.5, 0.75, 1.25)))
                    })),
            )
            .id("skeleton")
            .role(Role::ProgressIndicator)
            .aria_label("Loading forecast")
            .test_support()
    }
}
