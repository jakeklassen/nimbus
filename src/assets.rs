use std::borrow::Cow;

use gpui_kit::{AssetSource, Result, SharedString, assets::icon_assets};

// The Lucide icons Nimbus draws beyond GPUI Component's default set. Only
// these SVGs are embedded.
icon_assets!(
    WeatherIcons,
    [
        CloudDrizzle,
        CloudFog,
        CloudLightning,
        CloudMoon,
        CloudMoonRain,
        CloudRain,
        CloudSnow,
        CloudSun,
        CloudSunRain,
        Cloudy,
        Droplets,
        MapPin,
        Moon,
        RefreshCw,
        Sun,
        Umbrella,
        Wind,
    ]
);

/// The weather icons, then GPUI Component's own icons.
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if let Some(bytes) = WeatherIcons.load(path)? {
            return Ok(Some(bytes));
        }
        gpui_kit::assets::Assets.load(path)
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut paths = gpui_kit::assets::Assets.list(path)?;
        paths.extend(WeatherIcons.list(path)?);
        paths.sort();
        paths.dedup();
        Ok(paths)
    }
}
