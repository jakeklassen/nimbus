//! Nimbus Light and Nimbus Dark, and following the OS between them.
//!
//! Nimbus's palette lives in `themes/nimbus.json` and maps onto GPUI
//! Component's semantic roles:
//!
//! | Role | Nimbus use |
//! | --- | --- |
//! | `background` | the window canvas |
//! | `group_box` | the raised cards |
//! | `foreground` | primary text |
//! | `secondary_foreground` | supporting text, such as "Feels 17°" |
//! | `muted_foreground` | labels and icons |
//! | `primary` | the accent: a clear sky, the warm end of a range, the caret |
//! | `info` | precipitation and the cold end of a range |

use std::rc::Rc;

use gpui_kit::{
    App, Subscription, Window,
    component::{Theme, ThemeSet},
};

const THEMES: &str = include_str!("../themes/nimbus.json");

/// Install Nimbus Light and Nimbus Dark as the theme's light and dark halves.
///
/// Call after `gpui_kit::init`, before the first window.
pub fn init(cx: &mut App) {
    let set: ThemeSet = serde_json::from_str(THEMES).expect("themes/nimbus.json is valid");
    let theme = Theme::global_mut(cx);
    for config in set.themes {
        if config.mode.is_dark() {
            theme.dark_theme = Rc::new(config);
        } else {
            theme.light_theme = Rc::new(config);
        }
    }
    let appearance = cx.window_appearance();
    Theme::change(appearance, None, cx);
}

/// Match the window's light or dark appearance now and whenever the OS changes it.
pub fn follow_system(window: &mut Window, cx: &mut App) -> Subscription {
    Theme::sync_system_appearance(Some(window), cx);
    window.observe_window_appearance(|window, cx| {
        Theme::sync_system_appearance(Some(window), cx);
    })
}
