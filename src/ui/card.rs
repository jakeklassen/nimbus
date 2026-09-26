use gpui_kit::{
    AnyElement, App, ElementId, IntoElement, ParentElement, RenderOnce, SharedString,
    StyleRefinement, Styled, Window,
    component::{
        ActiveTheme as _,
        group_box::{GroupBox, GroupBoxVariants as _},
    },
    div,
};

/// A raised surface with a quiet title inside it, like "7 days".
///
/// A filled [`GroupBox`] owns the surface. Nimbus keeps the title inside the
/// card rather than above it, so the title is the first child.
#[derive(IntoElement)]
pub struct Card {
    id: ElementId,
    title: Option<SharedString>,
    tight: bool,
    children: Vec<AnyElement>,
}

impl Card {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            tight: false,
            children: Vec::new(),
        }
    }

    pub fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Pack rows closely, for a list whose rows carry their own height.
    pub fn tight(mut self) -> Self {
        self.tight = true;
        self
    }
}

impl ParentElement for Card {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl RenderOnce for Card {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let mut surface = StyleRefinement::default()
            .border_1()
            .border_color(theme.border)
            .rounded(theme.radius_lg)
            .pt_3p5()
            .pb_3()
            .px_4();
        surface = if self.tight {
            surface.gap_0p5()
        } else {
            surface.gap_2p5()
        };

        GroupBox::new()
            .fill()
            .id(self.id)
            .content_style(surface)
            .children(self.title.map(|title| card_title(title, cx)))
            .children(self.children)
    }
}

pub fn card_title(title: impl Into<SharedString>, cx: &App) -> impl IntoElement {
    div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(title.into())
}
