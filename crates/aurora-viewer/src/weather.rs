//! The weather of this viewer: what the Environment window's "Weather"
//! section sets, kept in the settings. It is drawn for your view only.

use serde::{Deserialize, Serialize};

/// What falls from the sky.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Precipitation {
    #[default]
    Rain,
    Snow,
}

/// How much of the weather's effects are drawn (their cost).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum WeatherQuality {
    Low,
    Medium,
    #[default]
    High,
}

/// Puddle view distance, metres: the range of the slider and its default.
pub const PUDDLE_DISTANCE: std::ops::RangeInclusive<f32> = 10.0..=60.0;
pub const PUDDLE_DISTANCE_DEFAULT: f32 = 38.0;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WeatherSettings {
    pub precipitation: Precipitation,
    /// How much falls, 0 (none) to 1 (a downpour).
    pub amount: f32,
    /// How hard the wind blows, 0 (calm) to 1 (a gale). Not used yet.
    pub wind: f32,
    /// Puddles gather on level ground (rain only).
    pub puddles: bool,
    /// How far the wet ground and its puddles are drawn (m).
    pub puddle_distance: f32,
    /// Thunder and lightning in heavy rain. Not used yet.
    pub thunder: bool,
    /// Let regions set the weather through their environment. Not used yet.
    pub region_weather: bool,
    pub quality: WeatherQuality,
    /// The menu was used this session: `AURORA_RAIN` no longer applies.
    #[serde(skip)]
    pub touched: bool,
}

impl Default for WeatherSettings {
    fn default() -> Self {
        Self {
            precipitation: Precipitation::Rain,
            amount: 0.0,
            wind: 0.0,
            puddles: true,
            puddle_distance: PUDDLE_DISTANCE_DEFAULT,
            thunder: false,
            region_weather: false,
            quality: WeatherQuality::High,
            touched: false,
        }
    }
}

impl WeatherSettings {
    /// How hard it rains, 0 to 1: the menu's, or `AURORA_RAIN` until the
    /// menu is used (demo captures).
    pub fn rain(&self) -> f32 {
        if !self.touched
            && let Some(v) = env_rain()
        {
            return v;
        }
        match self.precipitation {
            Precipitation::Rain => self.amount.clamp(0.0, 1.0),
            // snow is not drawn yet
            Precipitation::Snow => 0.0,
        }
    }

    /// The amount the slider shows: `AURORA_RAIN` until the menu is used.
    pub fn rain_or_amount(&self) -> f32 {
        if !self.touched
            && let Some(v) = env_rain()
        {
            return v;
        }
        self.amount
    }

    /// What the renderer needs besides the rain's strength.
    pub fn rain_look(&self) -> aurora_render::RainLook {
        aurora_render::RainLook {
            puddles: self.puddles,
            distance: self.puddle_distance.clamp(*PUDDLE_DISTANCE.start(), *PUDDLE_DISTANCE.end()),
        }
    }
}

fn env_rain() -> Option<f32> {
    static RAIN: std::sync::OnceLock<Option<f32>> = std::sync::OnceLock::new();
    *RAIN.get_or_init(|| {
        std::env::var("AURORA_RAIN")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .map(|v| v.clamp(0.0, 1.0))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snow_draws_no_rain_and_old_settings_files_get_the_defaults() {
        let mut w = WeatherSettings {
            amount: 0.7,
            touched: true,
            ..Default::default()
        };
        assert_eq!(w.rain(), 0.7);
        w.precipitation = Precipitation::Snow;
        assert_eq!(w.rain(), 0.0);
        let read: WeatherSettings = serde_json::from_str("{\"amount\": 0.3}").expect("parse");
        assert!(read.puddles);
        assert_eq!(read.puddle_distance, PUDDLE_DISTANCE_DEFAULT);
    }
}
