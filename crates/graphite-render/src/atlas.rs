//! CPU-side shelf packer for the glyph atlas (single-channel coverage texture).

pub const ATLAS_SIZE: u32 = 2048;

#[derive(Debug, Clone, Copy)]
pub struct Slot {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug)]
pub struct ShelfPacker {
    cursor_x: u32,
    cursor_y: u32,
    row_h: u32,
}

impl ShelfPacker {
    /// Starts after a 2x2 reserved block at the origin (solid pixel used for rectangles).
    pub fn new() -> Self {
        Self {
            cursor_x: 2,
            cursor_y: 0,
            row_h: 2,
        }
    }

    pub fn allocate(&mut self, w: u32, h: u32) -> Option<Slot> {
        // 1px padding avoids bleeding between neighbouring glyphs.
        let (pw, ph) = (w + 1, h + 1);
        if pw > ATLAS_SIZE || ph > ATLAS_SIZE {
            return None;
        }
        if self.cursor_x + pw > ATLAS_SIZE {
            self.cursor_x = 0;
            self.cursor_y += self.row_h;
            self.row_h = 0;
        }
        if self.cursor_y + ph > ATLAS_SIZE {
            return None;
        }
        let slot = Slot {
            x: self.cursor_x,
            y: self.cursor_y,
            w,
            h,
        };
        self.cursor_x += pw;
        self.row_h = self.row_h.max(ph);
        Some(slot)
    }
}

impl Default for ShelfPacker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_to_next_row() {
        let mut p = ShelfPacker::new();
        let a = p.allocate(1500, 10).unwrap();
        let b = p.allocate(1000, 20).unwrap();
        assert_eq!(a.y, 0);
        assert_eq!(b.x, 0);
        assert_eq!(b.y, 11);
    }

    #[test]
    fn fails_when_full() {
        let mut p = ShelfPacker::new();
        assert!(p.allocate(ATLAS_SIZE, 10).is_none());
    }
}
