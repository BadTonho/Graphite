//! Turning the outputs of a cell into what is drawn: lines of text and images, within limits.

use super::output::{strip_ansi, text_of};
use base64::Engine;
use graphite_render::Image;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

/// How much of an output is kept and how large an image may be.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    /// Lines shown for the outputs of one cell.
    pub max_lines: usize,
    /// Characters kept of one line.
    pub max_line_chars: usize,
    /// Size of the base64 text of one image.
    pub max_image_bytes: usize,
    /// Pixels of one image before it is refused.
    pub max_pixels: u64,
    /// Longest side of the pixels kept; larger images are scaled down.
    pub max_texture_side: u32,
    /// The box an image is displayed in, in pixels.
    pub max_display: (f32, f32),
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_lines: 5000,
            max_line_chars: 10_000,
            max_image_bytes: 25 * 1024 * 1024,
            max_pixels: 40_000_000,
            // The image atlas is 2048 pixels wide; leave room for the padding.
            max_texture_side: 2000,
            max_display: (900.0, 480.0),
        }
    }
}

/// A decoded image and the size it is shown at.
#[derive(Clone, Debug)]
pub(crate) struct RichImage {
    pub image: Image,
    pub width: f32,
    pub height: f32,
}

/// One piece of the outputs of a cell, in the order they came.
#[derive(Clone, Debug)]
pub(crate) enum Block {
    Lines(Vec<String>),
    Image(RichImage),
}

impl Block {
    /// Lines of height `line_height` this block takes.
    pub(crate) fn rows(&self, line_height: f32) -> usize {
        match self {
            Block::Lines(lines) => lines.len(),
            Block::Image(image) => (image.height / line_height).ceil().max(1.0) as usize,
        }
    }
}

static NEXT_IMAGE: AtomicU64 = AtomicU64::new(1);

fn data_text(data: &Value, mime: &str) -> Option<String> {
    data.get(mime).and_then(text_of)
}

/// Replaces the HTML entities that matter for reading.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| {
                    entity
                        .strip_prefix('#')
                        .and_then(|number| number.parse().ok())
                })
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Pads the cells of a table into aligned columns.
fn format_table(rows: &[Vec<String>]) -> Vec<String> {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0usize; columns];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.chars().count());
        }
    }
    rows.iter()
        .map(|row| {
            let line: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(index, cell)| format!("{cell:<width$}", width = widths[index]))
                .collect();
            line.join("  ").trim_end().to_owned()
        })
        .collect()
}

/// A readable text version of HTML: tags dropped, block elements on their own lines, lists
/// bulleted, tables aligned in columns, scripts and styles removed.
pub(crate) fn html_text(html: &str) -> String {
    let mut out = String::new();
    let mut text = String::new();
    // Rows of the table being read, and the cell being filled.
    let mut table: Option<Vec<Vec<String>>> = None;
    let mut cell: Option<String> = None;
    let mut skip_until: Option<&'static str> = None;
    let mut preformatted = false;
    let chars: Vec<char> = html.chars().collect();
    let mut index = 0;

    let flush_text = |text: &mut String, out: &mut String, cell: &mut Option<String>, pre: bool| {
        if text.is_empty() {
            return;
        }
        let decoded = unescape(text);
        let piece = if pre {
            decoded
        } else {
            let mut collapsed = String::new();
            let mut space = false;
            for c in decoded.chars() {
                if c.is_whitespace() {
                    space = true;
                } else {
                    if space && !collapsed.is_empty() {
                        collapsed.push(' ');
                    }
                    space = false;
                    collapsed.push(c);
                }
            }
            if space && !collapsed.is_empty() {
                collapsed.push(' ');
            }
            collapsed
        };
        match cell {
            Some(cell) => cell.push_str(&piece),
            None => out.push_str(&piece),
        }
        text.clear();
    };
    let newline = |out: &mut String| {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
    };

    while index < chars.len() {
        if chars[index] != '<' {
            if skip_until.is_none() {
                text.push(chars[index]);
            }
            index += 1;
            continue;
        }
        if chars[index..].starts_with(&['<', '!', '-', '-']) {
            // A comment runs to its own end, which may contain `>`.
            let rest: String = chars[index..].iter().collect();
            index += rest
                .find("-->")
                .map_or(chars.len() - index, |end| rest[..end + 3].chars().count());
            continue;
        }
        let Some(close) = chars[index..].iter().position(|&c| c == '>') else {
            text.extend(&chars[index..]);
            break;
        };
        let tag: String = chars[index + 1..index + close].iter().collect();
        index += close + 1;
        let lower = tag.to_ascii_lowercase();
        let closing = lower.starts_with('/');
        let name: String = lower
            .trim_start_matches('/')
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric())
            .collect();
        if let Some(until) = skip_until {
            if closing && name == until {
                skip_until = None;
            }
            continue;
        }
        flush_text(&mut text, &mut out, &mut cell, preformatted);
        match (name.as_str(), closing) {
            ("script", false) => skip_until = Some("script"),
            ("style", false) => skip_until = Some("style"),
            ("br", _) => match cell.as_mut() {
                Some(cell) => cell.push(' '),
                None => out.push('\n'),
            },
            ("table", false) => {
                newline(&mut out);
                table = Some(Vec::new());
            }
            ("tr", false) => {
                if let Some(rows) = table.as_mut() {
                    rows.push(Vec::new());
                }
            }
            ("td" | "th", false) => cell = Some(String::new()),
            ("td" | "th", true) => {
                if let (Some(rows), Some(done)) = (table.as_mut(), cell.take()) {
                    if rows.is_empty() {
                        rows.push(Vec::new());
                    }
                    rows.last_mut()
                        .expect("a row exists")
                        .push(done.trim().to_owned());
                }
            }
            ("table", true) => {
                if let Some(rows) = table.take() {
                    for line in format_table(&rows) {
                        out.push_str(&line);
                        out.push('\n');
                    }
                }
            }
            ("pre", false) => {
                newline(&mut out);
                preformatted = true;
            }
            ("pre", true) => {
                preformatted = false;
                newline(&mut out);
            }
            ("li", false) => {
                newline(&mut out);
                out.push_str("• ");
            }
            (
                "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "ul" | "ol" | "li"
                | "blockquote",
                _,
            ) => {
                newline(&mut out);
            }
            _ => {}
        }
    }
    flush_text(&mut text, &mut out, &mut cell, preformatted);
    out.trim_matches('\n').to_owned()
}

/// Decodes a base64 image, scales it for the texture atlas and works out the size to show it at.
fn decode_image(
    encoded: &str,
    hint: Option<(f32, f32)>,
    limits: &Limits,
) -> Result<RichImage, String> {
    if encoded.len() > limits.max_image_bytes {
        return Err(format!(
            "image too large ({} MB)",
            encoded.len() / (1024 * 1024)
        ));
    }
    let compact: String = encoded.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(compact)
        .map_err(|_| "image data is not valid base64".to_owned())?;
    let reader = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .map_err(|error| error.to_string())?;
    let (width, height) = reader
        .into_dimensions()
        .map_err(|error| error.to_string())?;
    if u64::from(width) * u64::from(height) > limits.max_pixels {
        return Err(format!("image too large ({width}x{height})"));
    }
    let decoded = image::load_from_memory(&bytes).map_err(|error| error.to_string())?;
    let mut rgba = decoded.to_rgba8();
    let longest = rgba.width().max(rgba.height());
    if longest > limits.max_texture_side {
        let scale = limits.max_texture_side as f32 / longest as f32;
        let new_width = ((rgba.width() as f32 * scale).round() as u32).max(1);
        let new_height = ((rgba.height() as f32 * scale).round() as u32).max(1);
        rgba = image::imageops::resize(
            &rgba,
            new_width,
            new_height,
            image::imageops::FilterType::Triangle,
        );
    }
    // The size to show: the notebook's own hint when it has one, else the pixels of the image
    // as it was (not as it was scaled for the texture), fitted into the display box.
    let (natural_width, natural_height) = hint.unwrap_or((width as f32, height as f32));
    let fit = (limits.max_display.0 / natural_width)
        .min(limits.max_display.1 / natural_height)
        .min(1.0);
    let id = NEXT_IMAGE.fetch_add(1, Ordering::Relaxed);
    let (pixel_width, pixel_height) = (rgba.width(), rgba.height());
    let image = Image::new(id, pixel_width, pixel_height, rgba.into_raw().into())
        .ok_or_else(|| "image has no pixels".to_owned())?;
    Ok(RichImage {
        image,
        width: (natural_width * fit).max(1.0),
        height: (natural_height * fit).max(1.0),
    })
}

/// The display size a notebook asks for in the metadata of an image output.
fn size_hint(output: &Value, mime: &str) -> Option<(f32, f32)> {
    let entry = output.get("metadata")?.get(mime)?;
    let width = entry.get("width")?.as_f64()? as f32;
    let height = entry.get("height")?.as_f64()? as f32;
    (width > 0.0 && height > 0.0).then_some((width, height))
}

/// What to show for the data of a display or result output.
fn data_blocks(output: &Value, limits: &Limits) -> Vec<Block> {
    let Some(data) = output.get("data").filter(|data| data.is_object()) else {
        return Vec::new();
    };
    for mime in ["image/png", "image/jpeg"] {
        if let Some(encoded) = data_text(data, mime) {
            return match decode_image(&encoded, size_hint(output, mime), limits) {
                Ok(image) => vec![Block::Image(image)],
                Err(reason) => vec![Block::Lines(vec![format!("[{mime}: {reason}]")])],
            };
        }
    }
    let text = data_text(data, "text/plain")
        .or_else(|| data_text(data, "text/html").map(|html| html_text(&html)))
        .or_else(|| data_text(data, "text/markdown"))
        .or_else(|| data_text(data, "text/latex"))
        .or_else(|| {
            data.get("application/json")
                .and_then(|json| serde_json::to_string_pretty(json).ok())
        });
    match text {
        Some(text) => vec![Block::Lines(split_lines(&strip_ansi(&text)))],
        None => {
            // Something this view cannot draw: say what it was.
            let kinds: Vec<&str> = data
                .as_object()
                .map(|object| object.keys().map(String::as_str).collect())
                .unwrap_or_default();
            if kinds.is_empty() {
                Vec::new()
            } else {
                vec![Block::Lines(vec![format!(
                    "[{} output not shown]",
                    kinds.join(", ")
                )])]
            }
        }
    }
}

fn split_lines(text: &str) -> Vec<String> {
    let text = text.replace("\r\n", "\n");
    text.trim_end_matches('\n')
        .split('\n')
        // A carriage return rewrites the line (progress bars): the last write wins.
        .map(|line| line.rsplit('\r').next().unwrap_or("").to_owned())
        .collect()
}

/// Everything a cell shows below its input: its outputs as blocks, within `limits`.
pub(crate) fn blocks(outputs: &[Value], limits: &Limits) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    for output in outputs {
        match output.get("output_type").and_then(Value::as_str) {
            Some("execute_result") | Some("display_data") => {
                blocks.extend(data_blocks(output, limits))
            }
            _ => {
                let lines = super::output::output_lines(std::slice::from_ref(output));
                if !lines.is_empty() {
                    blocks.push(Block::Lines(lines));
                }
            }
        }
    }
    limit(blocks, limits)
}

/// Cuts the lines past the limits, saying how many were left out.
fn limit(blocks: Vec<Block>, limits: &Limits) -> Vec<Block> {
    let mut remaining = limits.max_lines;
    let mut dropped = 0usize;
    let mut kept = Vec::new();
    for block in blocks {
        match block {
            Block::Lines(lines) => {
                let take = lines.len().min(remaining);
                dropped += lines.len() - take;
                remaining -= take;
                if take > 0 {
                    kept.push(Block::Lines(
                        lines
                            .into_iter()
                            .take(take)
                            .map(|line| shorten(line, limits.max_line_chars))
                            .collect(),
                    ));
                }
            }
            image @ Block::Image(_) => kept.push(image),
        }
    }
    if dropped > 0 {
        kept.push(Block::Lines(vec![format!(
            "… {dropped} more line(s) not shown"
        )]));
    }
    kept
}

fn shorten(line: String, max: usize) -> String {
    if line.chars().count() <= max {
        return line;
    }
    let mut short: String = line.chars().take(max).collect();
    short.push('…');
    short
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn png(width: u32, height: u32) -> String {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([200, 30, 30, 255]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgba8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn jpeg(width: u32, height: u32) -> String {
        let image = image::RgbImage::from_pixel(width, height, image::Rgb([20, 120, 220]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    fn display(data: Value) -> Value {
        json!({"output_type": "display_data", "data": data, "metadata": {}})
    }

    fn lines(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .flat_map(|block| match block {
                Block::Lines(lines) => lines.clone(),
                Block::Image(_) => vec!["<image>".to_owned()],
            })
            .collect()
    }

    #[test]
    fn plain_text_streams_and_errors_stay_lines() {
        let out = blocks(
            &[
                json!({"output_type": "stream", "name": "stdout", "text": "a\nb\n"}),
                json!({"output_type": "error", "ename": "E", "evalue": "v", "traceback": []}),
            ],
            &Limits::default(),
        );
        assert_eq!(lines(&out), ["a", "b", "E: v"]);
    }

    #[test]
    fn a_png_becomes_an_image_block_scaled_to_the_display_box() {
        let out = blocks(
            &[display(
                json!({"image/png": png(1800, 600), "text/plain": "<Figure>"}),
            )],
            &Limits::default(),
        );
        let [Block::Image(image)] = out.as_slice() else {
            panic!("expected one image, got {out:?}");
        };
        assert_eq!((image.image.width, image.image.height), (1800, 600));
        // Fitted to 900 wide, keeping the proportions.
        assert_eq!((image.width, image.height), (900.0, 300.0));
        assert_eq!(image.image.rgba.len(), 1800 * 600 * 4);
        assert_eq!(&image.image.rgba[..4], &[200, 30, 30, 255]);
    }

    #[test]
    fn a_small_image_is_not_enlarged_and_a_tall_one_fits_the_height() {
        let out = blocks(
            &[display(json!({"image/png": png(40, 20)}))],
            &Limits::default(),
        );
        let Block::Image(small) = &out[0] else {
            panic!("image")
        };
        assert_eq!((small.width, small.height), (40.0, 20.0));
        let out = blocks(
            &[display(json!({"image/png": png(100, 960)}))],
            &Limits::default(),
        );
        let Block::Image(tall) = &out[0] else {
            panic!("image")
        };
        assert_eq!(tall.height, 480.0);
        assert_eq!(tall.width, 50.0);
    }

    #[test]
    fn jpeg_images_work_too_and_base64_may_contain_line_breaks() {
        let encoded = jpeg(30, 30);
        let broken: String = encoded
            .as_bytes()
            .chunks(20)
            .map(|c| String::from_utf8_lossy(c).into_owned() + "\n")
            .collect();
        let out = blocks(
            &[display(json!({"image/jpeg": broken}))],
            &Limits::default(),
        );
        assert!(matches!(out.as_slice(), [Block::Image(_)]));
    }

    #[test]
    fn the_metadata_of_an_output_can_set_the_size() {
        let output = json!({
            "output_type": "display_data",
            "data": {"image/png": png(800, 400)},
            "metadata": {"image/png": {"width": 200, "height": 100}},
        });
        let out = blocks(&[output], &Limits::default());
        let Block::Image(image) = &out[0] else {
            panic!("image")
        };
        assert_eq!((image.width, image.height), (200.0, 100.0));
        // The pixels are kept at their own size.
        assert_eq!(image.image.width, 800);
    }

    #[test]
    fn huge_images_are_scaled_for_the_texture_and_refused_past_the_limits() {
        let limits = Limits {
            max_texture_side: 100,
            ..Limits::default()
        };
        let out = blocks(&[display(json!({"image/png": png(400, 200)}))], &limits);
        let Block::Image(image) = &out[0] else {
            panic!("image")
        };
        assert_eq!((image.image.width, image.image.height), (100, 50));
        // The display size still follows the original.
        assert_eq!((image.width, image.height), (400.0, 200.0));

        let limits = Limits {
            max_pixels: 1000,
            ..Limits::default()
        };
        let refused = blocks(&[display(json!({"image/png": png(100, 100)}))], &limits);
        assert!(lines(&refused)[0].contains("too large"));
        let limits = Limits {
            max_image_bytes: 10,
            ..Limits::default()
        };
        let refused = blocks(&[display(json!({"image/png": png(10, 10)}))], &limits);
        assert!(lines(&refused)[0].contains("too large"));
    }

    #[test]
    fn broken_image_data_is_reported_not_fatal() {
        let out = blocks(
            &[display(json!({"image/png": "!!!not base64!!!"}))],
            &Limits::default(),
        );
        assert!(lines(&out)[0].contains("base64"));
        let out = blocks(
            &[display(
                json!({"image/png": base64::engine::general_purpose::STANDARD.encode(b"not an image")}),
            )],
            &Limits::default(),
        );
        assert!(lines(&out)[0].starts_with("[image/png:"));
    }

    #[test]
    fn plain_text_wins_over_html_and_html_is_read_as_text_when_alone() {
        let out = blocks(
            &[display(
                json!({"text/plain": "plain", "text/html": "<b>html</b>"}),
            )],
            &Limits::default(),
        );
        assert_eq!(lines(&out), ["plain"]);
        let out = blocks(
            &[display(json!({"text/html": "<p>Hello <b>there</b></p>"}))],
            &Limits::default(),
        );
        assert_eq!(lines(&out), ["Hello there"]);
        let out = blocks(
            &[display(json!({"text/markdown": "# Title"}))],
            &Limits::default(),
        );
        assert_eq!(lines(&out), ["# Title"]);
        let out = blocks(
            &[display(json!({"application/json": {"a": 1}}))],
            &Limits::default(),
        );
        assert!(lines(&out).join("\n").contains("\"a\": 1"));
    }

    #[test]
    fn data_that_cannot_be_shown_is_named() {
        let out = blocks(
            &[display(json!({"image/svg+xml": "<svg/>"}))],
            &Limits::default(),
        );
        assert_eq!(lines(&out), ["[image/svg+xml output not shown]"]);
        assert!(blocks(&[display(json!({}))], &Limits::default()).is_empty());
        assert!(blocks(&[display(json!(null))], &Limits::default()).is_empty());
    }

    #[test]
    fn html_tables_become_aligned_columns() {
        let html = "<table><thead><tr><th></th><th>name</th><th>n</th></tr></thead><tbody>\
            <tr><th>0</th><td>alpha</td><td>1</td></tr><tr><th>1</th><td>b</td><td>22</td></tr></tbody></table>";
        let text = html_text(html);
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            ["   name   n", "0  alpha  1", "1  b      22"]
        );
    }

    #[test]
    fn html_blocks_lists_entities_and_hidden_parts() {
        let html = "<style>p{color:red}</style><h1>Title</h1><p>a &amp; b &lt; c &#65; &#x42;</p><ul><li>one</li><li>two</li></ul>\
            <script>alert(1)</script>line<br>next<!-- hidden <b> --><pre>  keep\n  spaces</pre>";
        let text = html_text(html);
        assert_eq!(
            text,
            "Title\na & b < c A B\n• one\n• two\nline\nnext\n  keep\n  spaces"
        );
    }

    #[test]
    fn broken_html_does_not_panic() {
        for html in [
            "<",
            "<b",
            "&",
            "&amp",
            "<table><tr><td>x",
            "</td></tr>",
            "<<<>>>",
            "&#xZZ; &#99999999999;",
        ] {
            let _ = html_text(html);
        }
        assert_eq!(html_text("a &unknown; b"), "a &unknown; b");
    }

    #[test]
    fn long_outputs_are_cut_with_a_note_and_long_lines_are_shortened() {
        let limits = Limits {
            max_lines: 3,
            max_line_chars: 5,
            ..Limits::default()
        };
        let out = blocks(
            &[json!({"output_type": "stream", "name": "stdout", "text": "1234567\n2\n3\n4\n5\n"})],
            &limits,
        );
        assert_eq!(
            lines(&out),
            ["12345…", "2", "3", "… 2 more line(s) not shown"]
        );
        // The limit counts all outputs of the cell together; images are not lines.
        let out = blocks(
            &[
                json!({"output_type": "stream", "name": "stdout", "text": "a\nb\n"}),
                display(json!({"image/png": png(4, 4)})),
                json!({"output_type": "stream", "name": "stderr", "text": "c\nd\n"}),
            ],
            &limits,
        );
        assert_eq!(
            lines(&out),
            ["a", "b", "<image>", "c", "… 1 more line(s) not shown"]
        );
    }

    #[test]
    fn rows_count_lines_and_round_images_up() {
        let text = Block::Lines(vec!["a".into(), "b".into()]);
        assert_eq!(text.rows(20.0), 2);
        let out = blocks(
            &[display(json!({"image/png": png(50, 45)}))],
            &Limits::default(),
        );
        assert_eq!(out[0].rows(20.0), 3);
        assert_eq!(out[0].rows(100.0), 1);
    }

    #[test]
    fn every_decoded_image_gets_its_own_id() {
        let one = blocks(
            &[display(json!({"image/png": png(2, 2)}))],
            &Limits::default(),
        );
        let two = blocks(
            &[display(json!({"image/png": png(2, 2)}))],
            &Limits::default(),
        );
        let (Block::Image(a), Block::Image(b)) = (&one[0], &two[0]) else {
            panic!("images")
        };
        assert_ne!(a.image.id, b.image.id);
    }
}
