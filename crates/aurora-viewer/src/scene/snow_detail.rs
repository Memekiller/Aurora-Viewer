//! Aurora Weather: the detail of the snow lying on the ground, a normal and
//! height map ("Snow 004", ambientCG, CC0; assets/snow/README.md) uploaded
//! once as a bindless texture with its mips, its slot given to the wet
//! ground pass (`RainLook::snow_detail`).

use aurora_render::{MipLevel, Renderer};

const PNG: &[u8] = include_bytes!("../../../../assets/snow/snow-normal-height.png");

/// The texture's slot, made on first use (None if it could not be).
#[derive(Default)]
pub struct SnowDetail {
    slot: Option<u32>,
    tried: bool,
}

impl SnowDetail {
    pub fn slot(&mut self, renderer: &mut Renderer) -> Option<u32> {
        if !self.tried {
            self.tried = true;
            self.slot = upload(renderer);
        }
        self.slot
    }
}

fn upload(renderer: &mut Renderer) -> Option<u32> {
    let image = match image::load_from_memory_with_format(PNG, image::ImageFormat::Png) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            log::warn!("snow detail texture: {e}");
            return None;
        }
    };
    let mut levels = vec![image];
    while levels.last().is_some_and(|l| l.width() > 1) {
        let last = levels.last().expect("a level");
        let next = image::imageops::resize(
            last,
            (last.width() / 2).max(1),
            (last.height() / 2).max(1),
            image::imageops::FilterType::Triangle,
        );
        levels.push(next);
    }
    let mips: Vec<MipLevel> = levels
        .iter()
        .map(|l| MipLevel {
            width: l.width(),
            height: l.height(),
            data: l.as_raw(),
        })
        .collect();
    let slot = renderer.create_texture(&mips);
    if slot.is_none() {
        log::warn!("snow detail texture: no slot");
    }
    slot
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_snow_detail_is_a_square_rgba_png() {
        let image = image::load_from_memory_with_format(super::PNG, image::ImageFormat::Png).expect("decodes");
        assert_eq!((image.width(), image.height()), (512, 512));
        assert_eq!(image.color(), image::ColorType::Rgba8);
    }
}
