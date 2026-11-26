//! Loading, caching, and drawing icons contributed by plugin activity bar containers.
//!
//! Extensions declare container icons via `contributes.viewsContainers.activitybar[].icon` (or
//! `secondarySidebar`), typically as a relative `.svg` or raster image path inside the extension
//! folder, or as a `$(codicon)` reference. Every file read is confined inside the extension root.

use graphite_render::{Image, Renderer};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const RASTER_SIZE: u32 = 64;
const MAX_ICON_BYTES: u64 = 1 << 20;

/// Disjoint image ID range for plugin activity bar icons (file icon themes start at `1 << 62`).
static NEXT_PLUGIN_ICON_IMAGE: AtomicU64 = AtomicU64::new(3 << 61);

/// Resolved icon ready to draw for a plugin activity bar or secondary sidebar container.
#[derive(Clone, Debug)]
pub(crate) enum ActivityBarIcon {
    /// Rasterized SVG or bitmap image. When `tinted` is true (monochrome SVG mask), the renderer
    /// colors its alpha mask with the active/inactive bar foreground color.
    Image { image: Arc<Image>, tinted: bool },
    /// Glyph from the embedded icon font (e.g., from a `$(codicon)` reference).
    Glyph(char),
}

/// Lazy, thread-safe cache of resolved plugin activity bar icons keyed by `(plugin_root, icon)`.
#[derive(Default)]
pub(crate) struct PluginIconCache {
    entries: Mutex<HashMap<(PathBuf, String), Option<ActivityBarIcon>>>,
}

impl PluginIconCache {
    /// Resolves `icon_spec` relative to `plugin_root`, caching both hits and misses so the UI
    /// thread never hits the disk or re-rasterizes on every frame.
    pub(crate) fn get_or_load(
        &self,
        plugin_id: &str,
        plugin_root: Option<&Path>,
        icon_spec: Option<&str>,
    ) -> Option<ActivityBarIcon> {
        let spec = icon_spec?.trim();
        if spec.is_empty() {
            return None;
        }
        if let Some(glyph) = codicon_glyph(spec) {
            return Some(ActivityBarIcon::Glyph(glyph));
        }
        let root = plugin_root?;
        let key = (root.to_path_buf(), spec.to_owned());
        if let Ok(guard) = self.entries.lock() {
            if let Some(cached) = guard.get(&key) {
                return cached.clone();
            }
        }
        let loaded = match load_plugin_icon_file(root, spec) {
            Ok(icon) => Some(icon),
            Err(reason) => {
                crate::app::error_log::record(
                    crate::app::error_log::ErrorSeverity::Low,
                    "activity_bar.plugin_icon",
                    format!("failed to load icon '{spec}' for plugin '{plugin_id}': {reason}"),
                );
                None
            }
        };
        if let Ok(mut guard) = self.entries.lock() {
            guard.insert(key, loaded.clone());
        }
        loaded
    }

    pub(crate) fn clear(&self) {
        if let Ok(mut guard) = self.entries.lock() {
            guard.clear();
        }
    }
}

/// Draws a plugin activity icon centered inside `(x, y, width, height)`, falling back to the
/// first letter of `title` when no icon is available.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_plugin_activity_icon(
    renderer: &mut Renderer,
    icon: Option<&ActivityBarIcon>,
    title: &str,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    icon_size: f32,
    color: [f32; 4],
) {
    let icon_x = x + (width - icon_size) * 0.5;
    let icon_y = y + (height - icon_size) * 0.5;
    match icon {
        Some(ActivityBarIcon::Image {
            image,
            tinted: true,
        }) => {
            renderer.draw_image_tinted(image, icon_x, icon_y, icon_size, icon_size, color);
        }
        Some(ActivityBarIcon::Image {
            image,
            tinted: false,
        }) => {
            renderer.draw_image(image, icon_x, icon_y, icon_size, icon_size);
        }
        Some(ActivityBarIcon::Glyph(glyph)) => {
            renderer.draw_icon(*glyph, icon_x, icon_y, icon_size, color);
        }
        None => {
            let initial = title.chars().next().unwrap_or('?').to_string();
            let tw = initial.chars().count() as f32 * renderer.char_width();
            renderer.draw_text(
                &initial,
                x + (width - tw) * 0.5,
                y + (height - renderer.line_height()) * 0.5,
                color,
            );
        }
    }
}

fn codicon_glyph(spec: &str) -> Option<char> {
    let inner = spec.strip_prefix("$(")?.strip_suffix(')')?.trim();
    if inner.is_empty() {
        return None;
    }
    let normalized = inner.to_ascii_lowercase().replace('_', "-");
    let glyph = match normalized.as_str() {
        "files" => '\u{eaf0}',
        "file" => '\u{ea7b}',
        "file-text" => '\u{ec5e}',
        "file-binary" => '\u{eae8}',
        "folder" => '\u{e5ff}',
        "folder-opened" => '\u{e5fe}',
        "search" => '\u{ea6d}',
        "source-control" => '\u{ea68}',
        "extensions" => '\u{eae6}',
        "gear" | "settings-gear" | "settings" => '\u{e690}',
        "database" => '\u{e706}',
        "lock" => '\u{e672}',
        "git-branch" | "git-commit" => '\u{e702}',
        "terminal" => '\u{e760}',
        _ => return None,
    };
    graphite_render::icons::AVAILABLE_GLYPHS
        .contains(&glyph)
        .then_some(glyph)
}

fn load_plugin_icon_file(
    plugin_root: &Path,
    relative_spec: &str,
) -> Result<ActivityBarIcon, String> {
    let rel = Path::new(relative_spec);
    if rel.is_absolute() {
        return Err("absolute icon paths are not permitted".into());
    }
    let canonical_root = plugin_root
        .canonicalize()
        .map_err(|err| format!("plugin root unavailable: {err}"))?;
    let candidate = canonical_root.join(rel);
    let canonical_file = candidate
        .canonicalize()
        .map_err(|err| format!("icon file unavailable: {err}"))?;
    if !canonical_file.starts_with(&canonical_root) {
        return Err("icon path escapes extension directory".into());
    }
    let metadata = std::fs::metadata(&canonical_file)
        .map_err(|err| format!("failed to stat icon file: {err}"))?;
    if !metadata.is_file() {
        return Err("icon path is not a file".into());
    }
    if metadata.len() > MAX_ICON_BYTES {
        return Err("icon file exceeds size limit".into());
    }
    let data =
        std::fs::read(&canonical_file).map_err(|err| format!("failed to read icon file: {err}"))?;

    let is_svg = canonical_file
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"));

    let (rgba, tinted) = if is_svg {
        let rgba = rasterize_svg(&data).ok_or_else(|| "invalid SVG icon".to_owned())?;
        let tinted = is_monochrome_rgba(&rgba);
        (rgba, tinted)
    } else {
        let rgba = rasterize_bitmap(&data).ok_or_else(|| "invalid bitmap icon".to_owned())?;
        (rgba, false)
    };

    let id = NEXT_PLUGIN_ICON_IMAGE.fetch_add(1, Ordering::Relaxed);
    let image = Image::new(id, RASTER_SIZE, RASTER_SIZE, rgba.into())
        .ok_or_else(|| "decoded icon buffer size mismatch".to_owned())?;
    Ok(ActivityBarIcon::Image {
        image: Arc::new(image),
        tinted,
    })
}

fn rasterize_svg(data: &[u8]) -> Option<Vec<u8>> {
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

/// Returns `true` when every visible pixel in an SVG is neutral grayscale or shares a single
/// flat color (like standard VS Code activity bar SVGs that use `#000`, `#fff`, `#c5c5c5`, or
/// `currentColor`), so it should be rendered as a theme-tinted mask.
fn is_monochrome_rgba(rgba: &[u8]) -> bool {
    let mut reference: Option<[u8; 3]> = None;
    let mut all_neutral = true;
    let mut single_color = true;
    let mut saw_visible = false;

    for px in rgba.chunks_exact(4) {
        if px[3] < 16 {
            continue;
        }
        saw_visible = true;
        let max_c = px[0].max(px[1]).max(px[2]);
        let min_c = px[0].min(px[1]).min(px[2]);
        if max_c - min_c > 24 {
            all_neutral = false;
        }
        match reference {
            None => reference = Some([px[0], px[1], px[2]]),
            Some(first) => {
                if px[0].abs_diff(first[0]) > 24
                    || px[1].abs_diff(first[1]) > 24
                    || px[2].abs_diff(first[2]) > 24
                {
                    single_color = false;
                }
            }
        }
        if !all_neutral && !single_color {
            return false;
        }
    }
    saw_visible && (all_neutral || single_color)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONO_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
        <path fill="currentColor" d="M4 4h16v16H4z"/>
    </svg>"##;

    const COLOR_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
        <rect x="0" y="0" width="12" height="24" fill="#2563eb"/>
        <rect x="12" y="0" width="12" height="24" fill="#f59e0b"/>
    </svg>"##;

    #[test]
    fn monochrome_svg_loads_as_tinted_mask_image() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path().join("ext");
        std::fs::create_dir_all(root.join("media")).unwrap();
        std::fs::write(root.join("media/icon.svg"), MONO_SVG).unwrap();

        let cache = PluginIconCache::default();
        let icon = cache
            .get_or_load("sample.plugin", Some(&root), Some("media/icon.svg"))
            .expect("monochrome SVG should load");
        match icon {
            ActivityBarIcon::Image { image, tinted } => {
                assert!(tinted);
                assert_eq!(image.width, RASTER_SIZE);
                assert_eq!(image.height, RASTER_SIZE);
            }
            other => panic!("expected tinted image, got {other:?}"),
        }
    }

    #[test]
    fn multicolor_svg_preserves_original_colors() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path().join("ext");
        std::fs::create_dir_all(root.join("media")).unwrap();
        std::fs::write(root.join("media/brand.svg"), COLOR_SVG).unwrap();

        let cache = PluginIconCache::default();
        let icon = cache
            .get_or_load("sample.plugin", Some(&root), Some("media/brand.svg"))
            .expect("multicolor SVG should load");
        assert!(matches!(icon, ActivityBarIcon::Image { tinted: false, .. }));
    }

    #[test]
    fn png_icon_loads_untinted_and_is_cached() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path().join("ext");
        std::fs::create_dir_all(root.join("assets")).unwrap();
        let mut png = Vec::new();
        let source = image::RgbaImage::from_pixel(16, 16, image::Rgba([0, 128, 255, 255]));
        source
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let icon_path = root.join("assets/icon.png");
        std::fs::write(&icon_path, png).unwrap();

        let cache = PluginIconCache::default();
        let first = cache
            .get_or_load("sample.plugin", Some(&root), Some("assets/icon.png"))
            .expect("PNG icon should load");
        let first_id = match first {
            ActivityBarIcon::Image { image, tinted } => {
                assert!(!tinted);
                image.id
            }
            other => panic!("expected image, got {other:?}"),
        };

        // Remove the file on disk: a second lookup must hit the cache.
        std::fs::remove_file(&icon_path).unwrap();
        let second = cache
            .get_or_load("sample.plugin", Some(&root), Some("assets/icon.png"))
            .expect("cached PNG icon should still be returned");
        match second {
            ActivityBarIcon::Image { image, .. } => assert_eq!(image.id, first_id),
            other => panic!("expected cached image, got {other:?}"),
        }

        // Clearing the cache forces a fresh lookup, which now returns None.
        cache.clear();
        assert!(cache
            .get_or_load("sample.plugin", Some(&root), Some("assets/icon.png"))
            .is_none());
    }

    #[test]
    fn codicon_syntax_resolves_embedded_glyphs() {
        let cache = PluginIconCache::default();
        let icon = cache
            .get_or_load("sample.plugin", None, Some("$(search)"))
            .expect("codicon should resolve");
        match icon {
            ActivityBarIcon::Glyph(glyph) => {
                assert!(graphite_render::icons::AVAILABLE_GLYPHS.contains(&glyph));
            }
            other => panic!("expected glyph, got {other:?}"),
        }
        assert!(cache
            .get_or_load("sample.plugin", None, Some("$(nonexistent-codicon)"))
            .is_none());
    }

    #[test]
    fn path_traversal_and_invalid_files_return_none() {
        let dir = crate::test_support::TempDir::new();
        let root = dir.path().join("ext");
        std::fs::create_dir_all(root.join("media")).unwrap();
        std::fs::write(dir.path().join("outside.svg"), MONO_SVG).unwrap();
        std::fs::write(root.join("media/bad.svg"), b"<not-valid-svg").unwrap();

        let cache = PluginIconCache::default();
        assert!(cache
            .get_or_load("sample.plugin", Some(&root), Some("../outside.svg"))
            .is_none());
        assert!(cache
            .get_or_load("sample.plugin", Some(&root), Some("media/missing.svg"))
            .is_none());
        assert!(cache
            .get_or_load("sample.plugin", Some(&root), Some("media/bad.svg"))
            .is_none());
    }
}
