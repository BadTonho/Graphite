use super::*;

impl Renderer {
    /// Draws a single line of monospace text with its top-left corner at (x, y).
    pub fn draw_text(&mut self, text: &str, x: f32, y: f32, color: Color) {
        if text.is_empty() {
            return;
        }
        let key = (text.to_owned(), self.metrics.font_size.to_bits());
        if self.line_cache.len() > 2000 {
            self.line_cache.clear();
        }
        let metrics = self.metrics;
        let buffer = self.line_cache.entry(key).or_insert_with(|| {
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            buffer.set_size(None, None);
            buffer.set_text(
                text,
                &Attrs::new().family(Family::Monospace),
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
            buffer
        });

        // Collect physical glyphs first to release the borrow on the cache.
        let mut physical = Vec::new();
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                physical.push((glyph.physical((0.0, 0.0), 1.0), run.line_y));
            }
        }
        for (pg, line_y) in physical {
            let Some(entry) = self.glyph_entry(pg.cache_key) else {
                continue;
            };
            let gx = x + pg.x as f32 + entry.left as f32;
            let gy = y + line_y + pg.y as f32 - entry.top as f32;
            let s = entry.slot;
            let a = ATLAS_SIZE as f32;
            self.push_quad(
                gx,
                gy,
                gx + s.w as f32,
                gy + s.h as f32,
                [s.x as f32 / a, s.y as f32 / a],
                [(s.x + s.w) as f32 / a, (s.y + s.h) as f32 / a],
                color,
            );
        }
    }

    /// Draws an icon glyph centered inside a `size` by `size` box.
    ///
    /// The glyph comes from the embedded file-icon font. Unlike [`Self::draw_text`],
    /// the visible ink is centered in the box, so icons with different bearings
    /// line up on the same baseline.
    pub fn draw_icon(&mut self, icon: char, x: f32, y: f32, size: f32, color: Color) {
        if !size.is_finite() || size <= 0.0 {
            return;
        }
        let key = (icon, size.to_bits());
        if self.icon_cache.len() > 512 {
            self.icon_cache.clear();
        }
        let metrics = Metrics::new(size, size);
        let buffer = self.icon_cache.entry(key).or_insert_with(|| {
            let mut buffer = Buffer::new(&mut self.font_system, metrics);
            buffer.set_size(None, None);
            buffer.set_text(
                &icon.to_string(),
                &Attrs::new().family(Family::Name(crate::icons::FAMILY)),
                Shaping::Advanced,
                None,
            );
            buffer.shape_until_scroll(&mut self.font_system, false);
            buffer
        });

        let physical = buffer
            .layout_runs()
            .flat_map(|run| {
                run.glyphs
                    .iter()
                    .map(move |glyph| (glyph.physical((0.0, 0.0), 1.0), run.line_y))
            })
            .collect::<Vec<_>>();
        let mut glyphs = Vec::with_capacity(physical.len());
        for (pg, line_y) in physical {
            if let Some(entry) = self.glyph_entry(pg.cache_key) {
                glyphs.push((pg, line_y, entry));
            }
        }
        let Some((first, first_line_y, first_entry)) = glyphs.first() else {
            return;
        };

        // Center the union of the rasterized ink boxes, not the font bearings.
        let first_x = first.x as f32 + first_entry.left as f32;
        let first_y = *first_line_y + first.y as f32 - first_entry.top as f32;
        let mut min_x = first_x;
        let mut min_y = first_y;
        let mut max_x = first_x + first_entry.slot.w as f32;
        let mut max_y = first_y + first_entry.slot.h as f32;
        for (pg, line_y, entry) in &glyphs {
            let gx = pg.x as f32 + entry.left as f32;
            let gy = *line_y + pg.y as f32 - entry.top as f32;
            min_x = min_x.min(gx);
            min_y = min_y.min(gy);
            max_x = max_x.max(gx + entry.slot.w as f32);
            max_y = max_y.max(gy + entry.slot.h as f32);
        }
        let dx = x + (size - (max_x - min_x)) * 0.5 - min_x;
        let dy = y + (size - (max_y - min_y)) * 0.5 - min_y;

        for (pg, line_y, entry) in glyphs {
            let gx = dx + pg.x as f32 + entry.left as f32;
            let gy = dy + line_y + pg.y as f32 - entry.top as f32;
            let slot = entry.slot;
            let atlas = ATLAS_SIZE as f32;
            self.push_quad(
                gx,
                gy,
                gx + slot.w as f32,
                gy + slot.h as f32,
                [slot.x as f32 / atlas, slot.y as f32 / atlas],
                [
                    (slot.x + slot.w) as f32 / atlas,
                    (slot.y + slot.h) as f32 / atlas,
                ],
                color,
            );
        }
    }

    fn glyph_entry(&mut self, key: CacheKey) -> Option<GlyphEntry> {
        if let Some(entry) = self.glyphs.get(&key) {
            return *entry;
        }
        let entry = self.rasterize(key);
        self.glyphs.insert(key, entry);
        entry
    }

    fn rasterize(&mut self, key: CacheKey) -> Option<GlyphEntry> {
        let image = self.swash.get_image_uncached(&mut self.font_system, key)?;
        let (w, h) = (image.placement.width, image.placement.height);
        if w == 0 || h == 0 {
            return None;
        }
        let coverage: Vec<u8> = match image.content {
            SwashContent::Mask => image.data.clone(),
            // Color glyphs (emoji): use the alpha channel only in v0.1.
            SwashContent::Color => image.data.chunks_exact(4).map(|p| p[3]).collect(),
            SwashContent::SubpixelMask => image.data.chunks_exact(4).map(|p| p[1]).collect(),
        };
        let slot = self.packer.allocate(w, h)?;
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_tex,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: slot.x,
                    y: slot.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &coverage,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        Some(GlyphEntry {
            slot,
            left: image.placement.left,
            top: image.placement.top,
        })
    }
}

pub(super) fn measure_char_width(font_system: &mut FontSystem, metrics: Metrics) -> f32 {
    let mut buffer = Buffer::new(font_system, metrics);
    buffer.set_size(None, None);
    buffer.set_text(
        "M",
        &Attrs::new().family(Family::Monospace),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(font_system, false);
    buffer
        .layout_runs()
        .next()
        .and_then(|r| r.glyphs.first().map(|g| g.w))
        .unwrap_or(metrics.font_size * 0.6)
}
