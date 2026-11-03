//! Region weather: the weather a region (or a parcel) sets through its
//! environment, for the viewers that let it (Preferences, "Let regions set
//! the weather"; off by default). Aurora's own: the Legacy Viewer has no
//! weather.
//!
//! **How it reaches every viewer.** An EEP setting's description is a
//! property of its inventory item: only who holds the item sees it, it is
//! never in the asset nor in what the simulator sends (ExtEnvironment gives
//! the day cycle's LLSD: its frames and their names, the day's name). The
//! names are, so the weather rides at the end of one: a compact code, `~`
//! and ten letters (`Evening ~K7Q2M9XAPQ`), written by Aurora's environment
//! editor on save. Other viewers show it as part of the name.
//!
//! **How every viewer sees the same weather.** Nothing is sent while it
//! runs: the weather is a pure function of the code, a seed and the clock.
//! Time is cut into windows (5 minutes to a day); in each, a hash of the seed
//! and the window's number decides whether it rains (or snows) and how hard,
//! with the region's chance. The seed is the region's own (its id) unless
//! the code gives one, so that parcels can share a sky or differ. Clocks a
//! few seconds apart see the same weather a few seconds apart; the changes
//! between windows are eased over a few minutes, so that is invisible.

use super::{Precipitation, WeatherSettings};

/// The code's version (2 bits): the first, 1.
const VERSION: u64 = 1;
/// The windows a code can choose (minutes): its 4 bits index this.
pub const WINDOWS: [u16; 16] = [5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 360, 480, 720, 1080, 1440];
/// Letters of the code: Crockford's base 32 (no I, L, O, U: nothing to
/// misread), read in either case.
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// The code's length after its `~`: 50 bits, 41 of settings and a 9-bit
/// check.
pub const CODE_LEN: usize = 10;
const DATA_BITS: u32 = 41;
const CHECK_BITS: u32 = 9;

/// What a region's code sets. The amounts and winds are 0..1 like the
/// Weather section's, in steps of a seventh; the chance in fifteenths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionWeather {
    pub precipitation: Precipitation,
    /// The chance that it falls in a window (0 never, 1 always).
    pub chance: f32,
    /// Index into [`WINDOWS`].
    pub window: u8,
    /// How hard it falls when it does: between these.
    pub amount: (f32, f32),
    /// The wind's lulls and gusts.
    pub wind: (f32, f32),
    /// Where the wind comes from (degrees, a compass point of sixteen), or
    /// the region's own wind.
    pub wind_from: Option<f32>,
    /// Thunder and lightning when it rains hard enough.
    pub thunder: bool,
    /// The Strikes setting (0..1, in thirds).
    pub strikes: f32,
    /// 0: the region's own seed; 1..1023: this one, shared by every parcel
    /// or region that gives it.
    pub seed: u16,
}

impl Default for RegionWeather {
    fn default() -> Self {
        Self {
            precipitation: Precipitation::Rain,
            chance: 0.4,
            window: 4,
            amount: (2.0 / 7.0, 5.0 / 7.0),
            wind: (1.0 / 7.0, 3.0 / 7.0),
            wind_from: None,
            thunder: false,
            strikes: 1.0 / 3.0,
            seed: 0,
        }
    }
}

/// `v` (0..1) as a level of `steps` (0..=steps).
fn level(v: f32, steps: u64) -> u64 {
    (v.clamp(0.0, 1.0) * steps as f32).round() as u64
}

fn check(data: u64) -> u64 {
    data.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> (64 - CHECK_BITS)
}

/// A bit field writer and reader, low bits first.
struct Bits {
    v: u64,
    at: u32,
}

impl Bits {
    fn put(&mut self, value: u64, width: u32) {
        self.v |= (value & ((1 << width) - 1)) << self.at;
        self.at += width;
    }
    fn take(&mut self, width: u32) -> u64 {
        let out = (self.v >> self.at) & ((1 << width) - 1);
        self.at += width;
        out
    }
}

impl RegionWeather {
    /// The window's length (s).
    pub fn window_secs(&self) -> f64 {
        f64::from(WINDOWS[usize::from(self.window.min(15))]) * 60.0
    }

    /// The ten letters of the code (without its `~`).
    pub fn code(&self) -> String {
        let mut b = Bits { v: 0, at: 0 };
        b.put(VERSION, 2);
        b.put(u64::from(self.precipitation == Precipitation::Snow), 1);
        b.put(level(self.chance, 15), 4);
        b.put(u64::from(self.window.min(15)), 4);
        let (a0, a1) = (self.amount.0.min(self.amount.1), self.amount.0.max(self.amount.1));
        b.put(level(a0, 7), 3);
        b.put(level(a1, 7), 3);
        let (w0, w1) = (self.wind.0.min(self.wind.1), self.wind.0.max(self.wind.1));
        b.put(level(w0, 7), 3);
        b.put(level(w1, 7), 3);
        let from = self.wind_from.map_or(0, |d| 1 + ((d.rem_euclid(360.0) / 22.5).round() as u64 % 16));
        b.put(from, 5);
        b.put(u64::from(self.thunder), 1);
        b.put(level(self.strikes, 3), 2);
        b.put(u64::from(self.seed.min(1023)), 10);
        debug_assert_eq!(b.at, DATA_BITS);
        let data = b.v;
        let all = data | (check(data) << DATA_BITS);
        (0..CODE_LEN)
            .rev()
            .map(|i| ALPHABET[((all >> (i * 5)) & 31) as usize] as char)
            .collect()
    }

    /// The settings of ten letters, if they are a code of a version this
    /// viewer reads and their check holds (a typo or a name that only looks
    /// like one is refused).
    pub fn from_code(letters: &str) -> Option<RegionWeather> {
        if letters.len() != CODE_LEN {
            return None;
        }
        let mut all = 0u64;
        for c in letters.bytes() {
            let c = c.to_ascii_uppercase();
            let v = ALPHABET.iter().position(|&a| a == c)? as u64;
            all = (all << 5) | v;
        }
        let data = all & ((1 << DATA_BITS) - 1);
        if all >> DATA_BITS != check(data) {
            return None;
        }
        let mut b = Bits { v: data, at: 0 };
        if b.take(2) != VERSION {
            return None;
        }
        let snow = b.take(1) == 1;
        let chance = b.take(4) as f32 / 15.0;
        let window = b.take(4) as u8;
        let amount = (b.take(3) as f32 / 7.0, b.take(3) as f32 / 7.0);
        let wind = (b.take(3) as f32 / 7.0, b.take(3) as f32 / 7.0);
        let from = b.take(5);
        let thunder = b.take(1) == 1;
        let strikes = b.take(2) as f32 / 3.0;
        let seed = b.take(10) as u16;
        Some(RegionWeather {
            precipitation: if snow { Precipitation::Snow } else { Precipitation::Rain },
            chance,
            window,
            amount,
            wind,
            wind_from: (1..=16).contains(&from).then(|| (from - 1) as f32 * 22.5),
            thunder,
            strikes,
            seed,
        })
    }

    /// The code at the end of an environment's name (`… ~XXXXXXXXXX`), if
    /// it has a valid one.
    pub fn from_name(name: &str) -> Option<RegionWeather> {
        let trimmed = name.trim_end();
        let at = trimmed.rfind('~')?;
        Self::from_code(&trimmed[at + 1..])
    }

    /// `name` with this code at its end, in place of any code it had.
    pub fn name_with(&self, name: &str) -> String {
        format!("{} ~{}", strip_code(name), self.code())
    }

    /// The seed this weather runs on in a region of id `region`.
    pub fn seed_for(&self, region: uuid::Uuid) -> u64 {
        if self.seed == 0 {
            let (a, b) = region.as_u64_pair();
            mix(a ^ b.rotate_left(29))
        } else {
            mix(u64::from(self.seed).wrapping_mul(0xD1B5_4A32_D192_ED03))
        }
    }

    /// What falls in window `w` (`fall` 0..1, 0: dry) on `seed`.
    fn window_fall(&self, seed: u64, w: i64) -> f32 {
        let h = mix(seed ^ (w as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        if unit(h) >= self.chance {
            return 0.0;
        }
        let (lo, hi) = (self.amount.0.min(self.amount.1), self.amount.0.max(self.amount.1));
        // a little for the weakest, never nothing once it falls
        (lo + (hi - lo) * unit(mix(h ^ 0xA5A5))).max(0.05)
    }

    /// How hard it falls at `secs` (Unix time) on `seed`: the window's
    /// amount, eased into the next over a fifth of a window (at most five
    /// minutes) around their boundary.
    pub fn fall_at(&self, seed: u64, secs: f64) -> f32 {
        let window = self.window_secs();
        let ramp = (window * 0.2).min(300.0);
        let w = (secs / window).floor();
        let into = secs - w * window;
        let w = w as i64;
        if into < ramp * 0.5 {
            let k = smoothstep((into + ramp * 0.5) / ramp);
            lerp(self.window_fall(seed, w - 1), self.window_fall(seed, w), k)
        } else if window - into < ramp * 0.5 {
            let k = smoothstep((into - (window - ramp * 0.5)) / ramp);
            lerp(self.window_fall(seed, w), self.window_fall(seed, w + 1), k)
        } else {
            self.window_fall(seed, w)
        }
    }

    /// The viewer's weather while this code runs at `secs` on `seed`: what
    /// it sets, over the viewer's own settings (the quality, the puddles and
    /// how far they are drawn stay the viewer's).
    pub fn apply(&self, own: &WeatherSettings, seed: u64, secs: f64) -> WeatherSettings {
        WeatherSettings {
            precipitation: self.precipitation,
            amount: self.fall_at(seed, secs),
            wind_min: self.wind.0.min(self.wind.1),
            wind_max: self.wind.0.max(self.wind.1),
            wind_follow_region: self.wind_from.is_none(),
            wind_from: self.wind_from.unwrap_or(own.wind_from),
            thunder: self.thunder,
            thunder_frequency: self.strikes,
            // what falls is the region's, not AURORA_RAIN's
            touched: true,
            ..own.clone()
        }
    }
}

/// The code an environment's day cycle (its LLSD, as ExtEnvironment sends
/// it) gives for sky track `track` (0-based, the ground first): the first
/// valid one in the names of that track's skies (their frame names, then
/// their settings' names), then the ground's, then the day's own name. So a
/// region can give each altitude its own weather, or one for all.
pub fn code_in_day(day: &aurora_llsd::Llsd, track: usize) -> Option<RegionWeather> {
    let frames = &day["frames"];
    let in_track = |t: usize| {
        // tracks: 0 the water, 1 the ground, 2.. the skies above
        day["tracks"].at(t + 1).as_array().iter().find_map(|key| {
            let name = key["key_name"].as_str();
            RegionWeather::from_name(name).or_else(|| RegionWeather::from_name(frames[name]["name"].as_str()))
        })
    };
    in_track(track)
        .or_else(|| if track > 0 { in_track(0) } else { None })
        .or_else(|| RegionWeather::from_name(day["name"].as_str()))
}

/// `name` without a weather code at its end.
pub fn strip_code(name: &str) -> &str {
    let trimmed = name.trim_end();
    match trimmed.rfind('~') {
        Some(at) if RegionWeather::from_code(&trimmed[at + 1..]).is_some() => trimmed[..at].trim_end(),
        _ => trimmed,
    }
}

/// SplitMix64's finaliser: a good spread of every input bit.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 0..1 from a hash.
fn unit(h: u64) -> f32 {
    (h >> 40) as f32 / (1u64 << 24) as f32
}

fn smoothstep(t: f64) -> f32 {
    let t = t.clamp(0.0, 1.0) as f32;
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f32, b: f32, k: f32) -> f32 {
    a + (b - a) * k
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RegionWeather {
        RegionWeather {
            precipitation: Precipitation::Snow,
            chance: 9.0 / 15.0,
            window: 6,
            amount: (1.0 / 7.0, 6.0 / 7.0),
            wind: (2.0 / 7.0, 5.0 / 7.0),
            wind_from: Some(292.5),
            thunder: true,
            strikes: 2.0 / 3.0,
            seed: 777,
        }
    }

    #[test]
    fn a_code_is_ten_plain_letters_and_reads_back_the_same() {
        for w in [RegionWeather::default(), sample()] {
            let code = w.code();
            assert_eq!(code.len(), CODE_LEN);
            assert!(code.bytes().all(|c| ALPHABET.contains(&c)), "{code}");
            assert_eq!(RegionWeather::from_code(&code), Some(w), "{code}");
            // either case
            assert_eq!(RegionWeather::from_code(&code.to_lowercase()), Some(w));
        }
    }

    #[test]
    fn a_typo_or_a_name_that_only_looks_like_a_code_is_refused() {
        let code = sample().code();
        let mut refused = 0;
        for i in 0..CODE_LEN {
            for c in ALPHABET.iter().map(|&c| c as char) {
                if code.as_bytes()[i] as char == c {
                    continue;
                }
                let mut typo: Vec<char> = code.chars().collect();
                typo[i] = c;
                let typo: String = typo.into_iter().collect();
                if RegionWeather::from_code(&typo).is_none() {
                    refused += 1;
                }
            }
        }
        // a 9-bit check: all but about one typo in 512 is caught
        assert!(refused >= 10 * 31 - 2, "{refused} of 310 typos refused");
        assert!(RegionWeather::from_code("HELLOWORLD").is_none());
        assert!(RegionWeather::from_code("ABC").is_none());
        assert!(RegionWeather::from_name("Sunset ~ by the sea").is_none());
    }

    #[test]
    fn the_code_rides_at_the_end_of_a_name_and_replaces_the_one_before() {
        let w = sample();
        let named = w.name_with("Evening");
        assert!(
            named.starts_with("Evening ~") && named.len() == "Evening ~".len() + CODE_LEN,
            "{named}"
        );
        assert_eq!(RegionWeather::from_name(&named), Some(w));
        assert_eq!(RegionWeather::from_name(&format!("{named}  ")), Some(w));
        let other = RegionWeather::default().name_with(&named);
        assert_eq!(strip_code(&other), "Evening");
        assert_eq!(RegionWeather::from_name(&other), Some(RegionWeather::default()));
        // a tilde that is not a code stays in the name
        assert_eq!(strip_code("Sky ~ blue"), "Sky ~ blue");
        assert!(RegionWeather::from_name("Evening").is_none());
    }

    #[test]
    fn every_viewer_sees_the_same_weather_from_the_same_code_seed_and_time() {
        let w = sample();
        let region = uuid::Uuid::from_u128(0x1234_5678_9abc_def0_1122_3344_5566_7788);
        let seed = w.seed_for(region);
        let t0 = 1_800_000_000.0;
        for k in 0..500 {
            let t = t0 + f64::from(k) * 37.0;
            assert_eq!(w.fall_at(seed, t), w.fall_at(w.seed_for(region), t));
        }
        // a seed of its own is the same in every region; the region's own differs
        let other = uuid::Uuid::from_u128(42);
        assert_eq!(w.seed_for(region), w.seed_for(other));
        let own = RegionWeather { seed: 0, ..w };
        assert_ne!(own.seed_for(region), own.seed_for(other));
    }

    #[test]
    fn it_falls_about_as_often_as_the_chance_and_eases_between_windows() {
        let w = RegionWeather {
            chance: 0.4,
            window: 0,
            ..RegionWeather::default()
        };
        let seed = w.seed_for(uuid::Uuid::from_u128(7));
        let window = w.window_secs();
        let t0 = 1_800_000_000.0;
        let wet = (0..4000)
            .filter(|i| w.fall_at(seed, t0 + (f64::from(*i) + 0.5) * window) > 0.0)
            .count();
        assert!((1400..=1800).contains(&wet), "{wet} wet windows of 4000 at 40%");
        // never a jump: a second apart, at most a little change
        let mut last = w.fall_at(seed, t0);
        for s in 1..20_000 {
            let now = w.fall_at(seed, t0 + f64::from(s));
            assert!((now - last).abs() < 0.02, "{last} -> {now} at {s} s");
            last = now;
        }
        // within the amounts when it falls
        for i in 0..200 {
            let f = w.fall_at(seed, t0 + (f64::from(i) + 0.5) * window);
            assert!(f == 0.0 || (w.amount.0 - 1e-4..=w.amount.1 + 1e-4).contains(&f) || f == 0.05, "{f}");
        }
        let never = RegionWeather { chance: 0.0, ..w };
        assert!((0..100).all(|i| never.fall_at(seed, t0 + f64::from(i) * 100.0) == 0.0));
    }

    #[test]
    fn the_code_is_read_from_the_track_s_skies_then_the_ground_then_the_day() {
        use aurora_llsd::{Llsd, llsd_map};
        let ground = RegionWeather::default();
        let high = sample();
        let day = llsd_map! {
            "name" => "Island day",
            "frames" => llsd_map! {
                "noon" => llsd_map! {"type" => "sky", "name" => ground.name_with("Noon")},
                "high" => llsd_map! {"type" => "sky", "name" => "High sky"},
            },
            "tracks" => Llsd::Array(vec![
                Llsd::Array(vec![]),
                Llsd::Array(vec![llsd_map! {"key_keyframe" => 0.0, "key_name" => "noon"}]),
                Llsd::Array(vec![llsd_map! {"key_keyframe" => 0.0, "key_name" => high.name_with("high")}]),
            ]),
        };
        assert_eq!(code_in_day(&day, 0), Some(ground));
        // the frame key of track 2 carries its own
        assert_eq!(code_in_day(&day, 1), Some(high));
        // an empty track above falls back to the ground's
        assert_eq!(code_in_day(&day, 3), Some(ground));
        let plain = llsd_map! {"name" => sample().name_with("Day"), "frames" => llsd_map! {}, "tracks" => Llsd::Array(vec![])};
        assert_eq!(code_in_day(&plain, 0), Some(sample()));
        let none = llsd_map! {"name" => "Day", "frames" => llsd_map! {}, "tracks" => Llsd::Array(vec![])};
        assert_eq!(code_in_day(&none, 0), None);
    }

    #[test]
    fn the_region_sets_what_falls_and_the_wind_and_the_viewer_keeps_its_quality() {
        let own = WeatherSettings {
            quality: super::super::WeatherQuality::Low,
            puddle_distance: 20.0,
            ..WeatherSettings::default()
        };
        let w = sample();
        let now = w.apply(&own, w.seed_for(uuid::Uuid::nil()), 1_800_000_000.0);
        assert_eq!(now.precipitation, Precipitation::Snow);
        assert_eq!((now.wind_min, now.wind_max), w.wind);
        assert!(!now.wind_follow_region && now.wind_from == 292.5);
        assert!(now.thunder && now.thunder_frequency == w.strikes);
        assert_eq!(now.quality, super::super::WeatherQuality::Low);
        assert_eq!(now.puddle_distance, 20.0);
    }
}
