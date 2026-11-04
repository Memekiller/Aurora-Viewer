//! Rain prototype: the sound of the rain (Ambient channel). Not a sound at a
//! place: a bed around the listener that leans toward the open sky.
//!
//! Four "speakers" stand in fixed directions around the listener (east,
//! north, west, south), each playing the light and the heavy loops
//! (`assets/rain`) from a different point, so that they sound like four
//! patches of rain rather than one. The rain slider crossfades the loops at
//! equal power (a drizzle's patter to a downpour's hiss) and sets the level.
//! Each speaker is as loud as the sky is open on its side (the reverb's
//! rays, `sound_reverb.rs` `Sky`): in a field all four, in a doorway the
//! door's side. Enclosed, the speakers go through the mixer's occlusion (a
//! dull rumble, as from inside a house), and a roof close above adds the
//! drumming of rain on it. Changes glide over about half a second; the
//! loops decode once, on a thread, the first time it rains.

use super::sound_reverb::Sky;
use aurora_audio::{AudioEngine, Channel, SoundClip};
use glam::{Vec2, Vec3};
use std::sync::{Once, OnceLock};

const LIGHT: &[u8] = include_bytes!("../../../../assets/rain/rain-light.ogg");
const HEAVY: &[u8] = include_bytes!("../../../../assets/rain/rain-heavy.ogg");
const ROOF: &[u8] = include_bytes!("../../../../assets/rain/rain-roof.ogg");

/// What the light and heavy loops lack of the roof's level (dB): they could
/// not be raised that far without clipping (assets/rain/README.md).
const LIGHT_MAKEUP_DB: f32 = 0.0;
const HEAVY_MAKEUP_DB: f32 = 6.2;
/// Overall level of the rain before the Ambient channel's volume.
const LEVEL: f32 = 0.55;
/// The speakers' directions (world: east, north, west, south).
const SPEAKERS: [Vec2; 4] = [Vec2::X, Vec2::Y, Vec2::NEG_X, Vec2::NEG_Y];
/// Voice ids, far from the world sounds' (counted from 1).
const VOICE_BASE: u64 = 0x7261_696E_0000_0000;
/// How muffled the light loop is as a drizzle on a roof (the mixer's
/// occlusion: 0.5 one wall, 1 two).
const LIGHT_ROOF_OCCLUSION: f32 = 0.8;
/// Voices: 4 light, 4 heavy, 2 roof (heavy), 2 roof (light).
const VOICES: u64 = 12;
/// Changes glide over about this long (s).
const GLIDE_SECS: f32 = 0.5;

struct Clips {
    light: [SoundClip; 4],
    heavy: [SoundClip; 4],
    roof: [SoundClip; 2],
    /// The light loop again, for a drizzle on a roof (muffled).
    light_roof: [SoundClip; 2],
}

fn load() -> Option<Clips> {
    let decode = |name: &str, bytes: &[u8]| SoundClip::decode(bytes).map_err(|e| log::warn!("rain sound {name}: {e}")).ok();
    let light = decode("light", LIGHT)?;
    let heavy = decode("heavy", HEAVY)?;
    let roof = decode("roof", ROOF)?;
    Some(Clips {
        light: [0.0, 0.25, 0.5, 0.75].map(|f| light.rotated(f)),
        heavy: [0.13, 0.38, 0.63, 0.88].map(|f| heavy.rotated(f)),
        roof: [0.0, 0.5].map(|f| roof.rotated(f)),
        light_roof: [0.4, 0.9].map(|f| light.rotated(f)),
    })
}

/// The loops, once decoded (none until then, or if they cannot be).
fn clips() -> Option<&'static Clips> {
    static CLIPS: OnceLock<Option<Clips>> = OnceLock::new();
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            let _ = CLIPS.set(load());
        });
    });
    CLIPS.get().and_then(Option::as_ref)
}

/// What the rain sound depends on, this frame.
#[derive(Debug, Clone, Copy)]
pub struct RainSoundInput {
    /// How hard it rains (0..1).
    pub rain: f32,
    /// The sky around the listener, when measured.
    pub sky: Option<Sky>,
    /// Where the listener looks (for the panning).
    pub forward: Vec3,
    /// How deep the listener is under the water (m; 0 above it).
    pub underwater: f32,
    /// In the world, not a silent demo.
    pub audible: bool,
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Default)]
pub struct RainSound {
    playing: bool,
    /// The glided sky: openness per speaker, roof, enclosure.
    open: [f32; 4],
    roof: f32,
    roof_distance: f32,
    enclosure: f32,
    exposure: f32,
    surface: f32,
    sent_occlusion: f32,
}

impl RainSound {
    pub fn update(&mut self, engine: Option<&AudioEngine>, input: &RainSoundInput, dt: f32) {
        let Some(engine) = engine else {
            return;
        };
        let rain = input.rain.clamp(0.0, 1.0);
        let clips = if input.audible && rain > 0.0 { clips() } else { None };
        let Some(clips) = clips else {
            if self.playing {
                for i in 0..VOICES {
                    engine.stop_world(VOICE_BASE + i);
                }
                self.playing = false;
            }
            return;
        };
        if !self.playing {
            for (i, c) in clips
                .light
                .iter()
                .chain(&clips.heavy)
                .chain(&clips.roof)
                .chain(&clips.light_roof)
                .enumerate()
            {
                engine.play_world(VOICE_BASE + i as u64, Channel::Ambient, c, true, (0.0, 0.0));
            }
            for i in 10..VOICES {
                engine.world_occlusion(VOICE_BASE + i, LIGHT_ROOF_OCCLUSION);
            }
            self.playing = true;
            self.sent_occlusion = -1.0;
            self.open = [1.0; 4];
            self.roof = 0.0;
            self.roof_distance = 15.0;
            self.enclosure = 0.0;
            self.exposure = 1.0;
            self.surface = 0.0;
        }

        // the sky glides to the last measure (open all round until one)
        let sky = input.sky.unwrap_or(Sky::OPEN);
        let k = 1.0 - (-dt.max(0.0) / GLIDE_SECS).exp();
        for (o, t) in self.open.iter_mut().zip(sky.open) {
            *o += (t - *o) * k;
        }
        self.roof += (sky.roof - self.roof) * k;
        self.roof_distance += (sky.roof_distance - self.roof_distance) * k;
        self.enclosure += (sky.enclosure - self.enclosure) * k;
        self.exposure += (sky.exposure - self.exposure) * k;
        self.surface += (sky.surface - self.surface) * k;
        // high above what the rain lands on (flying): only a faint, soft
        // hiss from far below; the loops' mix leans to the light one
        let aloft = if input.underwater > 0.0 {
            0.0
        } else {
            smoothstep(6.0, 38.0, self.surface)
        };
        // under the water: the rain on the surface, muffled, fading with
        // the depth; no roof
        let under = input.underwater > 0.0;
        let depth_fade = if under { (-input.underwater / 3.0).exp() * 0.7 } else { 1.0 };

        // the loops' mix: equal power, more of it the harder it rains
        let level = LEVEL * rain.sqrt() * depth_fade * (1.0 - 0.75 * aloft);
        let angle = rain * (1.0 - 0.6 * aloft) * std::f32::consts::FRAC_PI_2;
        let light = angle.cos() * 10f32.powf(LIGHT_MAKEUP_DB / 20.0) * level;
        let heavy = angle.sin() * 10f32.powf(HEAVY_MAKEUP_DB / 20.0) * level;

        // each speaker panned by its side of the listener, as loud as the
        // sky is open there (a little of it even when closed: the rest is
        // the occlusion's)
        let right = input.forward.cross(Vec3::Z).truncate().normalize_or(Vec2::X);
        for (q, dir) in SPEAKERS.iter().enumerate() {
            let pan = dir.dot(right).clamp(-1.0, 1.0);
            let side = ((1.0 - pan) * 0.5).sqrt();
            let other = ((1.0 + pan) * 0.5).sqrt();
            let open = if under { 1.0 } else { self.open[q] };
            let g = 0.5 * (0.15 + 0.85 * open);
            engine.world_gain(VOICE_BASE + q as u64, (light * g * side, light * g * other));
            engine.world_gain(VOICE_BASE + 4 + q as u64, (heavy * g * side, heavy * g * other));
        }
        // a roof above: its drumming, wide (two copies apart), when it
        // covers much of the sky (an arch overhead is a strip of it), the
        // fainter the higher it is
        let cover = smoothstep(0.35, 0.85, self.roof);
        let near = (3.0 / self.roof_distance.max(0.5)).clamp(0.2, 1.0);
        // (a drizzle: the light loop muffled; harder: the cabin's drumming)
        let drum = if under { 0.0 } else { LEVEL * rain.sqrt() * cover * near };
        let (patter, pour) = (drum * angle.cos() * 1.6, drum * angle.sin() * 0.8);
        engine.world_gain(VOICE_BASE + 8, (pour * 0.85, pour * 0.5));
        engine.world_gain(VOICE_BASE + 9, (pour * 0.5, pour * 0.85));
        engine.world_gain(VOICE_BASE + 10, (patter * 0.85, patter * 0.5));
        engine.world_gain(VOICE_BASE + 11, (patter * 0.5, patter * 0.85));

        // no falling rain in sight: the open-air speakers muffled as
        // through walls (under an arch or an awning, or in a doorway, rain
        // is in sight and they stay clear; each speaker's own side is
        // already as loud as rain is heard there); under the water, as
        // through two walls
        let occlusion = if under {
            1.0
        } else {
            (1.0 - smoothstep(0.03, 0.3, self.exposure)).max(0.5 * aloft)
        };
        if (occlusion - self.sent_occlusion).abs() > 0.02 {
            for i in 0..8 {
                engine.world_occlusion(VOICE_BASE + i, occlusion);
            }
            self.sent_occlusion = occlusion;
        }
    }
}
