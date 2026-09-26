// Release builds are a windowed app on Windows, with no console behind them.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use gpui_kit::{
    AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions,
    component::{Root, TitleBar},
    px, size,
};
use nimbus::{
    system::SystemSettings,
    ui::{Options, WeatherView},
    update::{UpdateBackend, Updater, Velopack},
    weather::OpenMeteo,
};

fn main() {
    // First, before anything else: Velopack may run an install or update hook
    // here and exit, or apply a downloaded update and restart.
    velopack::VelopackApp::build().run();

    let system = SystemSettings::read();

    gpui_kit::application()
        .with_assets(nimbus::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            nimbus::theme::init(cx);
            let backend =
                Velopack::new().map(|velopack| Arc::new(velopack) as Arc<dyn UpdateBackend>);
            let updater = cx.new(|cx| Updater::new(backend, cx));
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(440.), px(820.)),
                    cx,
                ))),
                window_min_size: Some(size(px(400.), px(760.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Nimbus".into()),
                    ..TitleBar::title_bar_options()
                }),
                app_id: Some("nimbus".into()),
                ..TitleBar::window_options()
            };

            cx.spawn(async move |cx| {
                cx.open_window(options, |window, cx| {
                    let view = cx.new(|cx| {
                        WeatherView::new(
                            Arc::new(OpenMeteo::new()),
                            system,
                            Options::default(),
                            updater,
                            window,
                            cx,
                        )
                    });
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("failed to open the Nimbus window");
            })
            .detach();
        });
}
