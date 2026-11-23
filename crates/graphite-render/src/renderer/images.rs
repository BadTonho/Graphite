//! Drawing RGBA images: a second atlas, packed on demand, sampled with linear filtering.

use super::Renderer;
use crate::atlas::{ShelfPacker, Slot, ATLAS_SIZE};
use std::sync::Arc;

/// A decoded image to draw. `id` names the pixels: draw the same `id` again and they are not
/// uploaded twice, so give different pixels different ids.
#[derive(Clone, Debug)]
pub struct Image {
    pub id: u64,
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes of straight (not premultiplied) RGBA.
    pub rgba: Arc<[u8]>,
}

impl Image {
    pub fn new(id: u64, width: u32, height: u32, rgba: Arc<[u8]>) -> Option<Self> {
        (rgba.len() == (width as usize) * (height as usize) * 4 && width > 0 && height > 0)
            .then_some(Self {
                id,
                width,
                height,
                rgba,
            })
    }
}

/// Texture coordinates of the pixels of `slot`, inset by half a texel so linear filtering does
/// not blend in the neighbouring image.
pub(super) fn slot_uvs(slot: Slot) -> ([f32; 2], [f32; 2]) {
    let size = ATLAS_SIZE as f32;
    (
        [(slot.x as f32 + 0.5) / size, (slot.y as f32 + 0.5) / size],
        [
            ((slot.x + slot.w) as f32 - 0.5) / size,
            ((slot.y + slot.h) as f32 - 0.5) / size,
        ],
    )
}

pub(super) struct ImageAtlas {
    pub packer: ShelfPacker,
    pub slots: std::collections::HashMap<u64, Slot>,
    /// The atlas filled up during a frame; it is emptied when the frame ends, so the images
    /// already drawn in this frame keep their pixels until then.
    pub reset_pending: bool,
}

impl ImageAtlas {
    pub(super) fn new() -> Self {
        Self {
            packer: ShelfPacker::empty(),
            slots: Default::default(),
            reset_pending: false,
        }
    }

    pub(super) fn reset(&mut self) {
        self.packer = ShelfPacker::empty();
        self.slots.clear();
        self.reset_pending = false;
    }
}

impl Renderer {
    /// Draws `image` scaled into the rectangle at (`x`, `y`) of size `w` by `h`. An image that
    /// does not fit the atlas is skipped; one that arrives when the atlas is full is skipped for
    /// this frame and the atlas is emptied for the next.
    pub fn draw_image(&mut self, image: &Image, x: f32, y: f32, w: f32, h: f32) {
        self.draw_image_mode(image, x, y, w, h, [1.0; 4], 1.0);
    }

    /// Draws `image` as an alpha mask tinted with `color` (useful for monochrome SVG icons that
    /// adapt to active/inactive and dark/light theme colors).
    pub fn draw_image_tinted(
        &mut self,
        image: &Image,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    ) {
        self.draw_image_mode(image, x, y, w, h, color, 2.0);
    }

    fn draw_image_mode(
        &mut self,
        image: &Image,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        mode: f32,
    ) {
        let slot = match self.images.slots.get(&image.id).copied() {
            Some(slot) => slot,
            None => {
                let Some(slot) = self.images.packer.allocate(image.width, image.height) else {
                    if image.width < ATLAS_SIZE && image.height < ATLAS_SIZE {
                        self.images.reset_pending = true;
                    }
                    return;
                };
                self.queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.image_tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: slot.x,
                            y: slot.y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &image.rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(image.width * 4),
                        rows_per_image: Some(image.height),
                    },
                    wgpu::Extent3d {
                        width: image.width,
                        height: image.height,
                        depth_or_array_layers: 1,
                    },
                );
                self.images.slots.insert(image.id, slot);
                slot
            }
        };
        let (uv0, uv1) = slot_uvs(slot);
        let (tx, ty) = self.translation;
        self.push_quad_mode(
            x + tx,
            y + ty,
            x + w + tx,
            y + h + ty,
            uv0,
            uv1,
            color,
            mode,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_needs_matching_pixels() {
        assert!(Image::new(1, 2, 2, vec![0; 16].into()).is_some());
        assert!(Image::new(1, 2, 2, vec![0; 15].into()).is_none());
        assert!(Image::new(1, 0, 2, Vec::new().into()).is_none());
    }

    #[test]
    fn texture_coordinates_stay_half_a_texel_inside_the_slot() {
        let (uv0, uv1) = slot_uvs(Slot {
            x: 10,
            y: 20,
            w: 4,
            h: 6,
        });
        let size = ATLAS_SIZE as f32;
        assert_eq!(uv0, [10.5 / size, 20.5 / size]);
        assert_eq!(uv1, [13.5 / size, 25.5 / size]);
    }

    #[test]
    fn the_image_atlas_starts_empty_and_can_be_emptied_again() {
        let mut atlas = ImageAtlas::new();
        let first = atlas.packer.allocate(100, 50).unwrap();
        // Unlike the glyph atlas there is no reserved block at the origin.
        assert_eq!((first.x, first.y), (0, 0));
        atlas.slots.insert(1, first);
        atlas.reset_pending = true;
        atlas.reset();
        assert!(atlas.slots.is_empty() && !atlas.reset_pending);
        assert_eq!(atlas.packer.allocate(10, 10).unwrap().x, 0);
    }

    #[test]
    fn an_image_larger_than_the_atlas_cannot_be_placed() {
        let mut atlas = ImageAtlas::new();
        assert!(atlas.packer.allocate(ATLAS_SIZE, 10).is_none());
        assert!(atlas.packer.allocate(10, ATLAS_SIZE).is_none());
        // Filling the atlas row by row eventually refuses more.
        let mut placed = 0;
        while atlas.packer.allocate(1000, 1000).is_some() {
            placed += 1;
            assert!(placed < 10);
        }
        assert_eq!(placed, 4);
    }
}
