//! The weather of this viewer: what the Environment window's "Weather"
//! section sets, kept in the settings. It is drawn for your view only.

use glam::Vec2;
use serde::{Deserialize, Serialize};
use std::time::Instant;

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
    /// How hard the wind blows, 0 (calm) to 1 (a gale).
    pub wind: f32,
    /// Puddles gather on level ground (rain only).
    pub puddles: bool,
    /// How far the wet ground and its puddles are drawn (m).
    pub puddle_distance: f32,
    /// Thunder and lightning in heavy rain.
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
    pub fn rain_look(&self, wind: &Wind) -> aurora_render::RainLook {
        aurora_render::RainLook {
            puddles: self.puddles,
            distance: self.puddle_distance.clamp(*PUDDLE_DISTANCE.start(), *PUDDLE_DISTANCE.end()),
            wind: wind.now.to_array(),
            drift: wind.drift.to_array(),
        }
    }
}

/// The wind of the Wind slider at its strongest (m/s, a gale).
const WIND_MAX: f32 = 18.0;

/// The weather's wind: the slider's strength in gusts, blowing the way the
/// region's wind does. `drift` is how far it has carried the drops so far
/// (the sum of the wind over time: a gust bends the rain, the drops never
/// jump).
#[derive(Debug, Clone)]
pub struct Wind {
    last: Option<Instant>,
    clock: f32,
    /// The direction it blows (unit), turning slowly to the region's.
    heading: Vec2,
    /// The wind now (m/s).
    pub now: Vec2,
    /// How far it has carried a drop so far (m).
    pub drift: Vec2,
}

impl Default for Wind {
    fn default() -> Self {
        Self {
            last: None,
            clock: 0.0,
            heading: Vec2::new(0.94, 0.34),
            now: Vec2::ZERO,
            drift: Vec2::ZERO,
        }
    }
}

impl Wind {
    /// Move on to `at`: `strength` the slider's (0..1), `region` the
    /// region's wind at the camera (m/s; its direction only).
    pub fn advance(&mut self, at: Instant, strength: f32, region: Vec2) {
        let dt = self.last.map_or(0.0, |t| at.saturating_duration_since(t).as_secs_f32().min(0.25));
        self.last = Some(at);
        self.clock += dt;
        if region.length_squared() > 0.09 {
            let to = region.normalize();
            let k = 1.0 - (-dt / 20.0).exp();
            self.heading = (self.heading + (to - self.heading) * k).normalize_or(to);
        }
        let base = WIND_MAX * strength.clamp(0.0, 1.0).powf(1.5);
        let t = self.clock;
        // gusts: a few slow waves of no common period, from about half the
        // wind to half again as much; the direction swings with them
        let g = 0.5 * (t * 0.37).sin() + 0.3 * (t * 0.91 + 1.3).sin() + 0.2 * (t * 2.3 + 4.0).sin();
        let swing = 0.12 * (t * 0.23 + 0.7).sin() + 0.06 * (t * 0.71).sin();
        let dir = Vec2::from_angle(swing).rotate(self.heading);
        self.now = dir * base * (1.0 + 0.45 * g);
        self.drift += self.now * dt;
    }

    /// The gusts' strength now relative to the slider's (about 0.55..1.45).
    pub fn speed(&self) -> f32 {
        self.now.length()
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

    #[test]
    fn the_wind_gusts_around_the_slider_and_carries_the_drops_smoothly() {
        let start = Instant::now();
        let mut w = Wind::default();
        let mut slowest = f32::MAX;
        let mut fastest = 0.0f32;
        let mut last_drift = Vec2::ZERO;
        for i in 0..6000 {
            let at = start + std::time::Duration::from_millis(16 * i);
            w.advance(at, 1.0, Vec2::new(0.0, 5.0));
            if i > 0 {
                slowest = slowest.min(w.speed());
                fastest = fastest.max(w.speed());
                // no jump: at most the strongest gust over one frame
                assert!((w.drift - last_drift).length() < WIND_MAX * 1.5 * 0.017, "{i}");
            }
            last_drift = w.drift;
        }
        assert!(slowest > WIND_MAX * 0.4 && fastest < WIND_MAX * 1.5, "{slowest} {fastest}");
        assert!(fastest - slowest > WIND_MAX * 0.4, "gusts: {slowest} {fastest}");
        // turned to the region wind (north) in the 96 s
        assert!(w.heading.y > 0.95, "{:?}", w.heading);
        let mut calm = Wind::default();
        calm.advance(start, 0.0, Vec2::ZERO);
        calm.advance(start + std::time::Duration::from_secs(1), 0.0, Vec2::ZERO);
        assert_eq!(calm.now, Vec2::ZERO);
    }
}
