# Nimbus

A minimal weather app written in Rust with [GPUI Kit](https://gpui-kit.com):
current conditions, the next 24 hours and 7 days. It follows the system light or
dark setting while it runs.

It is a port of the GPUIX/React version in `../weather-app`, made to compare how
the two feel to build and use.

```bash
cargo run
```

Rust is pinned with [mise](https://mise.jdx.dev) in `mise.toml`. The first build
compiles GPUI and takes a couple of minutes; after that `cargo run` is quick.

## Commands

| Command | What it does |
|---|---|
| `mise run build` | Debug build into `target/debug/nimbus.exe`, with a console for logs |
| `mise run release` | Release build into `target/release/nimbus.exe`: about 15 MB, with its icon and no console window. Takes about 4 minutes |
| `cargo run` | Build and start the app. GPUI and the text stack are optimized even in debug builds |
| `cargo test` | Unit tests, then drive the real view in a headless window |
| `cargo clippy --all-targets` | Lint |

## Files

```
src/main.rs            opens the window: assets, theme, TitleBar options, Root
src/lib.rs             the crate's modules, so tests can build the real view
src/weather/           the data: model, the WeatherSource seam, WMO codes, Open-Meteo
src/system.rs          locale, units and 12- or 24-hour clock, from the OS
src/format.rs          degrees, percentages, hour and day labels
src/theme.rs           loads Nimbus Light and Dark and follows the OS between them
src/assets.rs          the Lucide weather icons, on top of GPUI Component's own
src/ui/weather_view.rs the screen and its load, refresh and error states
src/ui/hourly.rs       the 24-hour row: wheel glide, drag, scrollbar, edge fades
src/ui/daily.rs        the 7-day list and its range bars
src/ui/current.rs      the big temperature and quick stats
src/ui/search.rs       city search
src/ui/skeleton.rs     the loading placeholder, laid out like the forecast
src/ui/card.rs         the raised card with a title inside it
themes/nimbus.json     both palettes, as GPUI Component theme roles
tests/ui.rs            the app against a fake weather source
build.rs               embeds assets/nimbus.ico in the Windows exe
```

## Data

Unchanged from the GPUIX version:

- **Forecast and city search:** [Open-Meteo](https://open-meteo.com). No API key.
- **First location:** a guess from your IP address through [ipwho.is](https://ipwho.is).
  Click the place name to search for a city instead. The choice is saved to
  `%APPDATA%\Nimbus\place.json` on Windows, `~/Library/Application Support/Nimbus`
  on macOS and `~/.config/nimbus` on Linux, the same file the GPUIX version uses.
  **Use my location** clears it.
- **Units:** Celsius and km/h, or Fahrenheit and mph where the system is set to
  US units.
- The forecast refreshes every 15 minutes, or from the refresh button in the
  title bar. A refresh you ask for, a new city and the first load show a pulsing
  skeleton for at least half a second; the timer's refresh stays silent. A
  failed refresh keeps the last forecast on screen and says so.

## What GPUI Kit does that GPUIX left to the app

**Light and dark.** GPUI reports the window's appearance and notifies when it
changes. `theme.rs` maps it onto Nimbus Light or Nimbus Dark. No registry
polling.

**Window controls.** Kit's `TitleBar` draws minimize, maximize and close on
Windows and Linux and leaves room for the traffic lights on macOS. The place
picker and refresh button live in it.

**Window icon.** GPUI's Windows backend loads icon resource 1 from the exe, and
`build.rs` embeds `assets/nimbus.ico` there. No FFI.

**Locale.** `sys-locale` and the Windows registry give the real locale, units
and clock; chrono's locale data names the weekdays.

**Scrolling the hourly row.** GPUI already turns a vertical wheel into sideways
scrolling for a row that only scrolls sideways, and Kit supplies the scrollbar.
Nimbus adds the glide and the drag.

**Components.** Button (with tooltips and a loading spinner), Input, Icon,
Skeleton, GroupBox and the scrollbar all come from Kit, themed by
`themes/nimbus.json`.

## Things that are easy to miss

**An element scrolls before its own wheel listeners run.** GPUI runs bubble-phase listeners in
reverse paint order, and an element registers its own scrolling after any
listener set on it, so `on_scroll_wheel` on the row itself always loses to
GPUI's jump. The glide listens on a transparent layer painted over the row.

**Put a manual scrollbar on the wrapper.** `horizontal_scrollbar(&handle)` on
the scrolling element itself is laid out in its content and scrolls away with
it. Attach it to the non-scrolling parent.

**Buttons in a TitleBar need `.occlude()`.** On Windows, GPUI answers the
OS hit test with "caption" whenever the pointer is over the title bar's drag
area, even with a button on top, so a real click drags the window instead of
pressing the button. `.occlude()` stops the hit test at the button;
`block_mouse_except_scroll()` does not. Headless tests and posted messages skip
this hit test, so neither catches it.

**Kit's Button centres its children.** Custom content that should start at the
left needs `flex_1()` to fill the button.

**The theme's font size is the rem.** `font.size` in `themes/nimbus.json` is 15,
and every `text_sm()`, `gap_3()` and `rems(…)` scales from it.

**`.test_support()` goes last.** With the `test-support` feature it wraps the
element, and the wrapper takes no more builder calls. `role()` and
`aria_label()` need `StatefulInteractiveElement` in scope.

**Check focus in a fresh update.** In a test, focus moved by a click shows in
`ElementSnapshot::focused()` only after the window update that clicked has
ended.

## Tests

`tests/ui.rs` opens the real `WeatherView` in a headless window with a fake
`WeatherSource`, clicks and types through `gpui_kit::test`, and checks
accessibility labels, element bounds and the view's state. It covers the same
cases as the GPUIX suite plus Escape, the wheel glide, dragging the row, and a
check that the skeleton and the forecast have the same geometry.
