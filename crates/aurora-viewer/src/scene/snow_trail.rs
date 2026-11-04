//! Aurora Weather: footprints in the lying snow. The feet of the avatars
//! near the camera (their L_FOOT / R_FOOT collision volumes, as the water's
//! contacts read them) are watched; a foot that lifts and is set down again
//! presses its print, a rounded sole along its heading, into the renderer's
//! footprints map (`RainLook::snow_stamps`, env.wgsl `fs_snow_trail_update`).

use super::{Scene, anchors, anim};
use crate::world::World;
use aurora_render::SNOW_STAMPS;
use glam::{Mat3, Mat4, Vec3};
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

/// Only the feet this near the camera leave prints (m): the map covers 32 m.
const REACH: f32 = 15.0;
const FEET: [&str; 2] = ["L_FOOT", "R_FOOT"];

#[derive(Clone, Copy, Debug)]
struct Foot {
    at: Vec3,
    time: f32,
    /// The lowest the foot has been lately (its sole on the ground).
    low: f32,
    /// It has lifted since its last print.
    lifted: bool,
}

/// One foot's step: where it is and how fast, what is decided.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Step {
    /// Still in the air or sliding.
    None,
    /// Set down: a print here.
    Down,
}

impl Foot {
    /// The foot at `at` at `time` (s): a print when it comes to rest low
    /// after having lifted (a foot that only slides, or an avatar standing,
    /// presses none more).
    fn advance(&mut self, at: Vec3, time: f32) -> Step {
        let dt = time - self.time;
        if !(0.001..0.5).contains(&dt) {
            *self = Foot {
                at,
                time,
                low: at.z,
                ..*self
            };
            return Step::None;
        }
        let speed = (at - self.at).truncate().length() / dt;
        // the ground's height follows the lowest the foot reaches, rising
        // slowly (a step up a stair is soon the new ground)
        self.low = (self.low + dt * 0.25).min(at.z);
        if speed > 0.45 || at.z > self.low + 0.05 {
            self.lifted = true;
        }
        let step = if self.lifted && speed < 0.2 && at.z < self.low + 0.03 {
            self.lifted = false;
            Step::Down
        } else {
            Step::None
        };
        self.at = at;
        self.time = time;
        step
    }
}

#[derive(Default)]
pub struct SnowTrails {
    feet: HashMap<(Uuid, u8), Foot>,
}

impl SnowTrails {
    /// The feet set down this frame near `eye`, as the renderer's stamps.
    pub fn stamps(scene: &mut Scene, world: &World, eye: Vec3, time: f32) -> ([[f32; 4]; SNOW_STAMPS], u32) {
        let mut out = [[0.0f32; 4]; SNOW_STAMPS];
        let mut count = 0usize;
        let now = Instant::now();
        let mut seen = Vec::new();
        for (idx, o) in world.objects.avatars() {
            if count >= SNOW_STAMPS {
                break;
            }
            let Some((p, r, _)) = Scene::object_transform(world, idx, now, 0) else {
                continue;
            };
            if (p - eye).truncate().length() > REACH {
                continue;
            }
            let Some(slot) = scene.palette_slots.get(&o.full_id) else {
                continue;
            };
            let base = *slot as usize * anim::PALETTE_JOINTS;
            let model = anchors::skeleton_model(world, &scene.meshes, idx, p, r, 1.0);
            for (k, name) in FEET.iter().enumerate() {
                let Some(&j) = scene.avatar_lib.rig.names.get(*name) else {
                    continue;
                };
                let Some(joint) = scene.palettes.get(base + j) else {
                    continue;
                };
                let m: Mat4 = model * Mat4::from_cols_array_2d(joint);
                if !m.is_finite() {
                    continue;
                }
                // the volume's centre, its sole below it, its heading along
                // its long axis
                let axes = Mat3::from_mat4(m);
                let half_height = (axes.transpose() * Vec3::Z).length();
                let at = m.w_axis.truncate() - Vec3::Z * half_height;
                let key = (o.full_id, k as u8);
                seen.push(key);
                let foot = scene.snow_trails.feet.entry(key).or_insert(Foot {
                    at,
                    time,
                    low: at.z,
                    lifted: false,
                });
                if foot.advance(at, time) == Step::Down && count < SNOW_STAMPS {
                    let forward = axes.x_axis.truncate();
                    let half_length = (forward.length() * 1.1).clamp(0.08, 0.2);
                    out[count] = [at.x, at.y, forward.y.atan2(forward.x), half_length];
                    count += 1;
                }
            }
        }
        scene.snow_trails.feet.retain(|k, _| seen.contains(k));
        (out, count as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_foot_prints_once_each_time_it_is_set_down_and_not_while_it_stands() {
        let mut foot = Foot {
            at: Vec3::ZERO,
            time: 0.0,
            low: 0.0,
            lifted: false,
        };
        let mut prints = 0;
        let mut t = 0.0;
        // standing: none
        for _ in 0..60 {
            t += 0.016;
            if foot.advance(Vec3::ZERO, t) == Step::Down {
                prints += 1;
            }
        }
        assert_eq!(prints, 0);
        // three steps: lift and swing forward 0.7 m in 0.4 s, then rest
        let mut x = 0.0;
        for _ in 0..3 {
            for i in 0..25 {
                t += 0.016;
                x += 0.028;
                let lift = (i as f32 / 25.0 * std::f32::consts::PI).sin() * 0.1;
                foot.advance(Vec3::new(x, 0.0, lift), t);
            }
            for _ in 0..30 {
                t += 0.016;
                if foot.advance(Vec3::new(x, 0.0, 0.0), t) == Step::Down {
                    prints += 1;
                }
            }
        }
        assert_eq!(prints, 3);
    }
}
