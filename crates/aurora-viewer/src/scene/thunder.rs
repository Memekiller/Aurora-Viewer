//! Weather: thunder and lightning in heavy rain, for this viewer only.
//!
//! Strikes come at random, the more often the harder it rains, each at a
//! random distance (300 m to 6 km, the far ones more often) and direction.
//! Its flash is at once: two to four strokes in a fraction of a second
//! (`flash`, which the app turns into light). Its thunder comes as late as
//! sound takes to cover the distance (3 s a kilometre), from its side: a
//! near strike cracks, a far one rolls, quieter and duller (the air takes
//! the highs out first); under a roof all of it is softer and muffled.

use super::sound_reverb::Sky;
use aurora_audio::{AudioEngine, Channel, SoundClip};
use glam::{Vec2, Vec3};
use std::sync::{Once, OnceLock};
use std::time::{Duration, Instant};

const CRACK_1: &[u8] = include_bytes!("../../../../assets/rain/thunder-crack-1.ogg");
const CRACK_2: &[u8] = include_bytes!("../../../../assets/rain/thunder-crack-2.ogg");
const DOUBLE: &[u8] = include_bytes!("../../../../assets/rain/thunder-double.ogg");
const ROLL: &[u8] = include_bytes!("../../../../assets/rain/thunder-roll.ogg");
const FAR: &[u8] = include_bytes!("../../../../assets/rain/thunder-far.ogg");

/// Voice ids, apart from the rain's and the world sounds'.
const VOICE_BASE: u64 = 0x7468_756E_0000_0000;
/// Overall level of the thunder before the Ambient channel's volume.
const LEVEL: f32 = 0.9;
/// Speed of sound (m/s).
const SOUND_SPEED: f32 = 343.0;
/// No thunder below this much rain.
const MIN_RAIN: f32 = 0.4;
/// Strikes' distances (m).
const NEAREST: f32 = 300.0;
const FARTHEST: f32 = 6000.0;

/// Which recording: the near ones crack, the far ones roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Clip {
    Crack1,
    Crack2,
    Double,
    Roll,
    Far,
}

struct Clips([SoundClip; 5]);

impl Clips {
    fn get(&self, c: Clip) -> &SoundClip {
        &self.0[c as usize]
    }
}

fn clips() -> Option<&'static Clips> {
    static CLIPS: OnceLock<Option<Clips>> = OnceLock::new();
    static STARTED: Once = Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            let decode = |name: &str, bytes: &[u8]| SoundClip::decode(bytes).map_err(|e| log::warn!("thunder {name}: {e}")).ok();
            let all = (|| {
                Some(Clips([
                    decode("crack 1", CRACK_1)?,
                    decode("crack 2", CRACK_2)?,
                    decode("double", DOUBLE)?,
                    decode("roll", ROLL)?,
                    decode("far", FAR)?,
                ]))
            })();
            let _ = CLIPS.set(all);
        });
    });
    CLIPS.get().and_then(Option::as_ref)
}

/// What the thunder depends on, this frame.
#[derive(Debug, Clone, Copy)]
pub struct ThunderInput {
    /// Thunder and lightning chosen in the weather settings.
    pub enabled: bool,
    /// How hard it rains (0..1).
    pub rain: f32,
    /// Where the listener looks (for the panning).
    pub forward: Vec3,
    /// The sky around the listener, when measured (softer under a roof).
    pub sky: Option<Sky>,
    /// Under the water.
    pub underwater: bool,
    /// In the world, not a silent demo.
    pub audible: bool,
}

/// One strike's flash: its strokes (when after the strike, how bright).
#[derive(Debug, Clone)]
struct Flash {
    at: Instant,
    strokes: Vec<(f32, f32)>,
}

/// One strike's thunder, on its way.
#[derive(Debug, Clone, Copy)]
struct Clap {
    at: Instant,
    clip: Clip,
    gain: f32,
    muffle: f32,
    /// Where it comes from (world, unit).
    from: Vec2,
}

#[derive(Debug)]
pub struct Thunder {
    seed: u64,
    next: Option<Instant>,
    flashes: Vec<Flash>,
    claps: Vec<Clap>,
    played: u64,
    /// How bright the lightning is now (0: none; about 1 for a strike a
    /// kilometre away).
    pub flash: f32,
}

impl Default for Thunder {
    fn default() -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9e37_79b9, |d| d.as_nanos() as u64)
            | 1;
        Self {
            seed,
            next: None,
            flashes: Vec::new(),
            claps: Vec::new(),
            played: 0,
            flash: 0.0,
        }
    }
}

impl Thunder {
    /// 0..1, uniform (xorshift).
    fn rand(&mut self) -> f32 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        (self.seed >> 40) as f32 / (1u64 << 24) as f32
    }

    /// The mean time between strikes for a rain of `rain` (s): a minute
    /// and more in moderate rain, about ten seconds in a downpour.
    fn mean_interval(rain: f32) -> f32 {
        let t = ((rain - MIN_RAIN) / (1.0 - MIN_RAIN)).clamp(0.0, 1.0);
        70.0 * (10.0f32 / 70.0).powf(t)
    }

    /// A strike now, at `distance` m (random when None).
    pub fn strike(&mut self, now: Instant, distance: Option<f32>) {
        let d = distance.unwrap_or_else(|| NEAREST * (FARTHEST / NEAREST).powf(self.rand()));
        let angle = self.rand() * std::f32::consts::TAU;
        let from = Vec2::from_angle(angle);
        // the flash: brighter the nearer, two to four strokes
        let bright = (1400.0 / (400.0 + d)).clamp(0.15, 1.6);
        let n = 2 + (self.rand() * 2.99) as usize;
        let mut strokes = Vec::with_capacity(n);
        let mut t = 0.0;
        for i in 0..n {
            let amp = if i == 0 { 1.0 } else { 0.4 + self.rand() * 0.6 };
            strokes.push((t, amp * bright));
            t += 0.04 + self.rand() * 0.14;
        }
        self.flashes.push(Flash { at: now, strokes });
        // the thunder: a crack near, a roll farther, the far roll far off;
        // quieter and duller with the distance
        let r = self.rand();
        let clip = if d < 1200.0 {
            [Clip::Crack1, Clip::Crack2, Clip::Double][((r * 2.99) as usize).min(2)]
        } else if d < 3000.0 {
            if r < 0.5 { Clip::Roll } else { Clip::Crack2 }
        } else if r < 0.6 {
            Clip::Far
        } else {
            Clip::Roll
        };
        let gain = (1500.0 / (500.0 + d)).clamp(0.12, 1.0);
        let muffle = ((d - 800.0) / 5000.0).clamp(0.0, 0.7);
        self.claps.push(Clap {
            at: now + Duration::from_secs_f32(d / SOUND_SPEED),
            clip,
            gain,
            muffle,
            from,
        });
    }

    /// One frame: maybe a strike, the thunder due, and the flash now.
    pub fn update(&mut self, engine: Option<&AudioEngine>, input: &ThunderInput, now: Instant) {
        let active = input.enabled && input.rain >= MIN_RAIN;
        if active {
            // the clips start decoding with the first storm
            let _ = clips();
            match self.next {
                None => {
                    let wait = -self.rand().max(1e-4).ln() * Self::mean_interval(input.rain);
                    self.next = Some(now + Duration::from_secs_f32(wait.clamp(4.0, 240.0)));
                }
                Some(t) if now >= t => {
                    self.next = None;
                    self.strike(now, None);
                }
                Some(_) => {}
            }
        } else {
            self.next = None;
        }

        // the flash: each stroke lit at once, fading in about a tenth of a
        // second, and a dimmer glow of the clouds after
        let mut flash = 0.0;
        self.flashes.retain(|f| {
            let t = now.saturating_duration_since(f.at).as_secs_f32();
            if t > 1.5 {
                return false;
            }
            for &(at, amp) in &f.strokes {
                if t >= at {
                    let k = t - at;
                    flash += amp * ((-k / 0.06).exp() + 0.25 * (-k / 0.4).exp());
                }
            }
            true
        });
        self.flash = flash;

        // the thunder due
        let (due, later): (Vec<Clap>, Vec<Clap>) = self.claps.iter().partition(|c| now >= c.at);
        self.claps = later;
        let Some(engine) = engine else {
            return;
        };
        if !input.audible {
            return;
        }
        let Some(clips) = clips() else {
            return;
        };
        let open = input.sky.map_or(1.0, |s| s.exposure.clamp(0.0, 1.0));
        let right = input.forward.cross(Vec3::Z).truncate().normalize_or(Vec2::X);
        for c in due {
            // from its side, but broadly (thunder fills the sky)
            let pan = (c.from.dot(right) * 0.6).clamp(-1.0, 1.0);
            let (l, r) = (((1.0 - pan) * 0.5).sqrt(), ((1.0 + pan) * 0.5).sqrt());
            let mut gain = c.gain * LEVEL * (0.45 + 0.55 * open);
            let mut muffle = c.muffle.max((1.0 - open) * 0.6);
            if input.underwater {
                gain *= 0.5;
                muffle = 1.0;
            }
            let id = VOICE_BASE + self.played;
            self.played += 1;
            engine.play_world(id, Channel::Ambient, clips.get(c.clip), false, (gain * l, gain * r));
            if muffle > 0.01 {
                engine.world_occlusion(id, muffle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(rain: f32) -> ThunderInput {
        ThunderInput {
            enabled: true,
            rain,
            forward: Vec3::X,
            sky: None,
            underwater: false,
            audible: false,
        }
    }

    #[test]
    fn a_strike_flashes_at_once_and_thunders_after_the_sound_has_come() {
        let t0 = Instant::now();
        let mut th = Thunder::default();
        th.strike(t0, Some(1715.0));
        th.update(None, &input(0.0), t0 + Duration::from_millis(1));
        assert!(th.flash > 0.5, "{}", th.flash);
        assert_eq!(th.claps.len(), 1);
        // 5 s for 1.7 km: still on its way at 4.9 s, gone at 5.1 s
        th.update(None, &input(0.0), t0 + Duration::from_millis(4900));
        assert_eq!(th.claps.len(), 1);
        assert!(th.flash < 0.01);
        th.update(None, &input(0.0), t0 + Duration::from_millis(5100));
        assert!(th.claps.is_empty());
    }

    #[test]
    fn storms_strike_more_often_in_heavier_rain_and_never_when_off() {
        assert!(Thunder::mean_interval(1.0) < 15.0);
        assert!(Thunder::mean_interval(MIN_RAIN) > 60.0);
        let t0 = Instant::now();
        let mut th = Thunder::default();
        let mut off = input(1.0);
        off.enabled = false;
        for s in 0..600 {
            th.update(None, &off, t0 + Duration::from_secs(s));
        }
        assert!(th.flashes.is_empty() && th.claps.is_empty());
        let mut strikes = 0;
        for s in 0..600 {
            th.update(None, &input(1.0), t0 + Duration::from_secs(s));
            if th.flashes.iter().any(|f| f.at == t0 + Duration::from_secs(s)) {
                strikes += 1;
            }
        }
        assert!((20..=120).contains(&strikes), "{strikes} in 10 minutes");
    }
}
