//! Loading the files an icon theme points at: SVG and raster images become RGBA pixels, fonts
//! are read as bytes. Every file must stay inside the extension that contributes the theme.

use graphite_render::Image;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Pixel size icons are rasterized at; the renderer scales them down to the row height.
pub(super) const RASTER_SIZE: u32 = 64;
const MAX_IMAGE_BYTES: u64 = 1 << 20;
const MAX_FONT_BYTES: u64 = 8 << 20;

/// Image ids for icon pixels, kept far from the ids other features hand out.
static NEXT_IMAGE: AtomicU64 = AtomicU64::new(1 << 62);

/// Font ids for icon fonts; 0 is the embedded font.
static NEXT_FONT: AtomicU64 = AtomicU64::new(1);

pub(super) fn next_font_id() -> u64 {
    NEXT_FONT.fetch_add(1, Ordering::Relaxed)
}

/// `path` with links resolved, when it exists and lies inside `root` (already canonical).
pub(super) fn confined(path: &Path, root: &Path) -> Option<PathBuf> {
    let path = path.canonicalize().ok()?;
    path.starts_with(root).then_some(path)
}

fn read_limited(path: &Path, root: &Path, limit: u64) -> Option<Vec<u8>> {
    let path = confined(path, root)?;
    let metadata = std::fs::metadata(&path).ok()?;
    (metadata.is_file() && metadata.len() <= limit)
        .then(|| std::fs::read(&path).ok())
        .flatten()
}

/// Reads a font file of the theme.
pub(super) fn read_font(path: &Path, root: &Path) -> Option<Vec<u8>> {
    read_limited(path, root, MAX_FONT_BYTES)
}

/// Loads an SVG, PNG or JPEG icon as a square image of [`RASTER_SIZE`] pixels.
pub(super) fn load_image(path: &Path, root: &Path) -> Option<Image> {
    let data = read_limited(path, root, MAX_IMAGE_BYTES)?;
    let is_svg = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"));
    let rgba = if is_svg {
        rasterize_svg(&data)?
    } else {
        rasterize_bitmap(&data)?
    };
    let id = NEXT_IMAGE.fetch_add(1, Ordering::Relaxed);
    Image::new(id, RASTER_SIZE, RASTER_SIZE, rgba.into())
}

/// Renders an SVG document centered in a [`RASTER_SIZE`] square, keeping its aspect ratio.
pub(super) fn rasterize_svg(data: &[u8]) -> Option<Vec<u8>> {
    use resvg::{tiny_skia, usvg};
    let tree = usvg::Tree::from_data(data, &usvg::Options::default()).ok()?;
    let size = tree.size();
    let side = RASTER_SIZE as f32;
    let scale = (side / size.width()).min(side / size.height());
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let offset_x = (side - size.width() * scale) * 0.5;
    let offset_y = (side - size.height() * scale) * 0.5;
    let mut pixmap = tiny_skia::Pixmap::new(RASTER_SIZE, RASTER_SIZE)?;
    let transform =
        tiny_skia::Transform::from_scale(scale, scale).post_translate(offset_x, offset_y);
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Some(
        pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let color = pixel.demultiply();
                [color.red(), color.green(), color.blue(), color.alpha()]
            })
            .collect(),
    )
}

/// Decodes a PNG or JPEG and fits it into a [`RASTER_SIZE`] square.
fn rasterize_bitmap(data: &[u8]) -> Option<Vec<u8>> {
    let decoded = image::load_from_memory(data).ok()?.to_rgba8();
    let (width, height) = decoded.dimensions();
    if width == 0 || height == 0 {
        return None;
    }
    let scale = (RASTER_SIZE as f32 / width as f32).min(RASTER_SIZE as f32 / height as f32);
    let fitted_width = ((width as f32 * scale).round() as u32).clamp(1, RASTER_SIZE);
    let fitted_height = ((height as f32 * scale).round() as u32).clamp(1, RASTER_SIZE);
    let fitted = image::imageops::resize(
        &decoded,
        fitted_width,
        fitted_height,
        image::imageops::FilterType::Triangle,
    );
    let mut square = image::RgbaImage::new(RASTER_SIZE, RASTER_SIZE);
    image::imageops::overlay(
        &mut square,
        &fitted,
        ((RASTER_SIZE - fitted_width) / 2) as i64,
        ((RASTER_SIZE - fitted_height) / 2) as i64,
    );
    Some(square.into_raw())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="8">
        <rect width="16" height="8" fill="#ff0000"/></svg>"##;

    fn pixel(rgba: &[u8], x: u32, y: u32) -> [u8; 4] {
        let at = ((y * RASTER_SIZE + x) * 4) as usize;
        rgba[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn svg_icons_are_centered_and_keep_their_aspect_ratio() {
        let rgba = rasterize_svg(SVG.as_bytes()).unwrap();
        assert_eq!(rgba.len(), (RASTER_SIZE * RASTER_SIZE * 4) as usize);
        // A 2:1 rectangle fills the middle half of the square, top and bottom stay clear.
        assert_eq!(pixel(&rgba, 32, 32), [255, 0, 0, 255]);
        assert_eq!(pixel(&rgba, 32, 4)[3], 0);
        assert_eq!(pixel(&rgba, 32, 60)[3], 0);
        assert!(rasterize_svg(b"<not svg").is_none());
    }

    #[test]
    fn png_icons_are_fitted_into_the_square() {
        let mut png = Vec::new();
        let source = image::RgbaImage::from_pixel(4, 8, image::Rgba([0, 0, 255, 255]));
        source
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let rgba = rasterize_bitmap(&png).unwrap();
        assert_eq!(pixel(&rgba, 32, 32), [0, 0, 255, 255]);
        assert_eq!(pixel(&rgba, 2, 32)[3], 0);
        assert!(rasterize_bitmap(b"not an image").is_none());
    }

    #[test]
    fn files_outside_the_extension_are_not_read() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path().join("ext");
        std::fs::create_dir_all(root.join("icons")).unwrap();
        std::fs::write(root.join("icons/a.svg"), SVG).unwrap();
        std::fs::write(dir.path().join("outside.svg"), SVG).unwrap();
        let root = root.canonicalize().unwrap();
        assert!(load_image(&root.join("icons/a.svg"), &root).is_some());
        assert!(load_image(&root.join("icons/../../outside.svg"), &root).is_none());
        assert!(load_image(&root.join("icons/missing.svg"), &root).is_none());
        assert!(read_font(&root.join("icons/a.svg"), &root).is_some());
    }
}
