//! Icon fonts supplied at run time (for example by an icon theme). The bytes may be TrueType,
//! OpenType, WOFF or WOFF2; compressed formats are unpacked before the font is registered.

use super::Renderer;
use cosmic_text::fontdb;
use std::sync::Arc;

/// Font bytes to draw icons from. `id` names the bytes: the font is registered once per id, so
/// give different fonts different ids. Id 0 is reserved for the embedded icon font.
#[derive(Clone, Debug)]
pub struct IconFont {
    pub id: u64,
    pub data: Arc<[u8]>,
}

/// Returns the sfnt bytes of a TrueType, OpenType, WOFF or WOFF2 font, or `None` when a
/// compressed font cannot be unpacked.
pub(crate) fn decode_font(data: &[u8]) -> Option<Vec<u8>> {
    match data.get(..4) {
        Some(b"wOFF") => wuff::decompress_woff1(data).ok(),
        Some(b"wOF2") => wuff::decompress_woff2(data).ok(),
        _ => Some(data.to_vec()),
    }
}

/// Registers `data` in `db` and returns the family name of its first face.
pub(crate) fn register(db: &mut fontdb::Database, data: &[u8]) -> Option<String> {
    let sfnt = decode_font(data)?;
    let ids = db.load_font_source(fontdb::Source::Binary(Arc::new(sfnt)));
    ids.iter()
        .find_map(|id| db.face(*id))
        .and_then(|face| face.families.first())
        .map(|(family, _)| family.clone())
}

impl Renderer {
    /// The family name `font` was registered under, loading it on first use.
    pub(super) fn icon_font_family(&mut self, font: &IconFont) -> Option<String> {
        if let Some(family) = self.icon_fonts.get(&font.id) {
            return family.clone();
        }
        let family = register(self.font_system.db_mut(), &font.data);
        self.icon_fonts.insert(font.id, family.clone());
        family
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_u16(data: &[u8], at: usize) -> u16 {
        u16::from_be_bytes([data[at], data[at + 1]])
    }

    fn read_u32(data: &[u8], at: usize) -> u32 {
        u32::from_be_bytes(data[at..at + 4].try_into().unwrap())
    }

    /// Wraps an sfnt font in a WOFF 1 container with uncompressed tables.
    fn to_woff1(sfnt: &[u8]) -> Vec<u8> {
        let count = read_u16(sfnt, 4) as usize;
        let tables: Vec<_> = (0..count)
            .map(|index| {
                let record = 12 + index * 16;
                let tag = &sfnt[record..record + 4];
                let checksum = read_u32(sfnt, record + 4);
                let offset = read_u32(sfnt, record + 8) as usize;
                let length = read_u32(sfnt, record + 12) as usize;
                (
                    tag.to_vec(),
                    checksum,
                    sfnt[offset..offset + length].to_vec(),
                )
            })
            .collect();
        let padded = |len: usize| (len + 3) & !3;
        let header = 44 + 20 * count;
        let total_sfnt = 12 + 16 * count + tables.iter().map(|t| padded(t.2.len())).sum::<usize>();
        let mut body = Vec::new();
        let mut directory = Vec::new();
        for (tag, checksum, data) in &tables {
            directory.extend_from_slice(tag);
            directory.extend_from_slice(&((header + body.len()) as u32).to_be_bytes());
            directory.extend_from_slice(&(data.len() as u32).to_be_bytes());
            directory.extend_from_slice(&(data.len() as u32).to_be_bytes());
            directory.extend_from_slice(&checksum.to_be_bytes());
            body.extend_from_slice(data);
            body.resize(padded(body.len()), 0);
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"wOFF");
        out.extend_from_slice(&sfnt[0..4]);
        out.extend_from_slice(&((header + body.len()) as u32).to_be_bytes());
        out.extend_from_slice(&(count as u16).to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&(total_sfnt as u32).to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&0u16.to_be_bytes());
        out.extend_from_slice(&[0; 20]);
        out.extend_from_slice(&directory);
        out.extend_from_slice(&body);
        out
    }

    #[test]
    fn plain_fonts_register_under_their_family_name() {
        let mut db = fontdb::Database::new();
        assert_eq!(
            register(&mut db, crate::icons::FONT).as_deref(),
            Some(crate::icons::FAMILY)
        );
    }

    #[test]
    fn woff_fonts_are_unpacked_before_registering() {
        let woff = to_woff1(crate::icons::FONT);
        assert_eq!(&woff[..4], b"wOFF");
        let mut db = fontdb::Database::new();
        assert_eq!(
            register(&mut db, &woff).as_deref(),
            Some(crate::icons::FAMILY)
        );
    }

    #[test]
    fn damaged_compressed_fonts_are_refused() {
        assert!(decode_font(b"wOFFbroken").is_none());
        assert!(decode_font(b"wOF2broken").is_none());
        let mut db = fontdb::Database::new();
        assert!(register(&mut db, b"not a font").is_none());
    }
}
