//! Build-time conversion of the original artwork into native icon formats.
use image::{
    codecs::ico::{IcoEncoder, IcoFrame},
    imageops::FilterType,
    ExtendedColorType, ImageFormat,
};
pub const WINDOW_SIZE: u32 = 256;
pub const ICON_SIZES: &[u32] = &[16, 24, 32, 48, 64, 128, 256];
pub struct IconAssets {
    pub rgba: Vec<u8>,
    pub ico: Vec<u8>,
}
pub fn generate(source: &[u8]) -> image::ImageResult<IconAssets> {
    let image = image::load_from_memory_with_format(source, ImageFormat::Png)?;
    // Fit the artwork without cropping or stretching, including future nonsquare replacements.
    let square = |size| {
        let resized = image.resize(size, size, FilterType::Lanczos3).to_rgba8();
        let mut canvas = image::RgbaImage::new(size, size);
        let x = (size - resized.width()) / 2;
        let y = (size - resized.height()) / 2;
        image::imageops::overlay(&mut canvas, &resized, i64::from(x), i64::from(y));
        canvas
    };
    let rgba = square(WINDOW_SIZE).into_raw();
    let mut frames = Vec::new();
    for &size in ICON_SIZES {
        frames.push(IcoFrame::as_png(
            square(size).as_raw(),
            size,
            size,
            ExtendedColorType::Rgba8,
        )?);
    }
    let mut ico = Vec::new();
    IcoEncoder::new(&mut ico).encode_images(&frames)?;
    Ok(IconAssets { rgba, ico })
}
