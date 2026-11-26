//! GPU-free caches for the renderer: where glyphs live in the atlas, and shaped text.

use crate::atlas::{ShelfPacker, Slot};
use std::collections::HashMap;
use std::hash::Hash;

/// Glyph positions in the atlas. A glyph that does not fit is not remembered as missing:
/// the atlas is marked full, the glyph is skipped for the current frame, and
/// [`Self::reset_if_full`] empties the atlas once that frame is presented, so every glyph the
/// next frame needs is placed again. Entries handed out during a frame stay valid until then.
pub struct GlyphCache<K, V> {
    packer: ShelfPacker,
    entries: HashMap<K, Option<V>>,
    full: bool,
}

impl<K: Hash + Eq, V: Copy> GlyphCache<K, V> {
    pub fn new() -> Self {
        Self {
            packer: ShelfPacker::new(),
            entries: HashMap::new(),
            full: false,
        }
    }

    /// `Some(entry)` when `key` was seen: the placed glyph, or `None` for a glyph with no ink.
    pub fn get(&self, key: &K) -> Option<Option<V>> {
        self.entries.get(key).copied()
    }

    /// Room for a `w` by `h` glyph, or `None` when the atlas is full.
    pub fn allocate(&mut self, w: u32, h: u32) -> Option<Slot> {
        let slot = self.packer.allocate(w, h);
        if slot.is_none() {
            self.full = true;
        }
        slot
    }

    /// Remembers a placed glyph, or `None` for one that draws nothing.
    pub fn insert(&mut self, key: K, entry: Option<V>) {
        self.entries.insert(key, entry);
    }

    /// Empties the atlas if a glyph did not fit. Returns whether it did, so the caller draws
    /// another frame with the missing glyphs.
    pub fn reset_if_full(&mut self) -> bool {
        if !self.full {
            return false;
        }
        self.packer = ShelfPacker::new();
        self.entries.clear();
        self.full = false;
        true
    }
}

impl<K: Hash + Eq, V: Copy> Default for GlyphCache<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

/// Shaped text by font size and string, in two generations: when the current one reaches its
/// limit it becomes the previous one, and entries still in use move back as they are drawn.
/// Lines that stay on screen are never shaped again, unlike clearing everything at the limit.
pub struct TextCache<V> {
    current: HashMap<u32, HashMap<String, V>>,
    previous: HashMap<u32, HashMap<String, V>>,
    current_len: usize,
    limit: usize,
}

impl<V> TextCache<V> {
    pub fn new(limit: usize) -> Self {
        Self {
            current: HashMap::new(),
            previous: HashMap::new(),
            current_len: 0,
            limit: limit.max(1),
        }
    }

    /// The cached value for `text` at `size`, made with `make` when neither generation has it.
    /// A hit allocates nothing.
    pub fn get_or_insert_with(
        &mut self,
        size: u32,
        text: &str,
        make: impl FnOnce() -> V,
    ) -> &mut V {
        let present = self
            .current
            .get(&size)
            .is_some_and(|texts| texts.contains_key(text));
        if !present {
            let value = self
                .previous
                .get_mut(&size)
                .and_then(|texts| texts.remove(text))
                .unwrap_or_else(make);
            if self.current_len >= self.limit {
                self.previous = std::mem::take(&mut self.current);
                self.current_len = 0;
            }
            self.current
                .entry(size)
                .or_default()
                .insert(text.to_owned(), value);
            self.current_len += 1;
        }
        self.current
            .get_mut(&size)
            .and_then(|texts| texts.get_mut(text))
            .expect("the entry was just found or inserted")
    }

    pub fn clear(&mut self) {
        self.current.clear();
        self.previous.clear();
        self.current_len = 0;
    }

    /// How many values are held, in both generations.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.current_len + self.previous.values().map(HashMap::len).sum::<usize>()
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::atlas::ATLAS_SIZE;

    #[test]
    fn a_full_atlas_is_emptied_after_the_frame_instead_of_hiding_glyphs_for_good() {
        let mut cache: GlyphCache<u32, Slot> = GlyphCache::new();
        let big = ATLAS_SIZE / 2;
        let mut placed = 0;
        let mut key = 0;
        let missed = loop {
            match cache.allocate(big, big) {
                Some(slot) => {
                    cache.insert(key, Some(slot));
                    placed += 1;
                    key += 1;
                }
                None => break key,
            }
        };
        assert!(placed > 0);
        // The glyph that did not fit is not remembered as missing.
        assert!(cache.get(&missed).is_none());

        assert!(cache.reset_if_full());

        assert!(cache.get(&0).is_none(), "old placements are forgotten");
        let slot = cache
            .allocate(big, big)
            .expect("room again after the reset");
        cache.insert(missed, Some(slot));
        assert!(cache.get(&missed).is_some_and(|entry| entry.is_some()));
        assert!(
            !cache.reset_if_full(),
            "nothing to reset when everything fits"
        );
    }

    #[test]
    fn glyphs_without_ink_are_remembered_as_empty() {
        let mut cache: GlyphCache<u32, Slot> = GlyphCache::new();
        cache.insert(7, None);
        assert!(matches!(cache.get(&7), Some(None)));
        assert!(!cache.reset_if_full());
    }

    #[test]
    fn text_in_use_survives_reaching_the_limit() {
        let mut cache = TextCache::new(3);
        let mut made = 0;
        let mut get = |cache: &mut TextCache<usize>, text: &str| {
            *cache.get_or_insert_with(14, text, || {
                made += 1;
                made
            })
        };
        let a = get(&mut cache, "visible");
        get(&mut cache, "b");
        get(&mut cache, "c");
        // The limit is reached: the next new line starts a generation, and "visible", still
        // drawn every frame, moves into it without being shaped again.
        get(&mut cache, "d");
        assert_eq!(get(&mut cache, "visible"), a);
        get(&mut cache, "e");
        assert_eq!(get(&mut cache, "visible"), a);
        assert_eq!(made, 5, "only new text is shaped");
    }

    #[test]
    fn text_no_longer_drawn_is_eventually_dropped() {
        let mut cache = TextCache::new(2);
        for text in ["a", "b", "c", "d", "e", "f"] {
            cache.get_or_insert_with(14, text, || ());
        }
        assert!(cache.len() <= 4, "at most two generations of two");
    }

    #[test]
    fn sizes_are_cached_separately() {
        let mut cache = TextCache::new(8);
        *cache.get_or_insert_with(14, "x", || 1) += 0;
        assert_eq!(*cache.get_or_insert_with(20, "x", || 2), 2);
        assert_eq!(*cache.get_or_insert_with(14, "x", || 3), 1);
        cache.clear();
        assert!(cache.is_empty());
    }
}
