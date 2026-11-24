//! Shared geometry for the chat controls and connection form.

#[derive(Clone, Copy, Debug)]
pub(super) struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }

    pub fn row(self, y: f32, h: f32) -> Self {
        Self::new(self.x, y, self.w, h)
    }

    pub fn columns<const N: usize>(self, gap: f32) -> [Self; N] {
        let w = ((self.w - gap * (N - 1) as f32) / N as f32).max(0.0);
        std::array::from_fn(|i| Self::new(self.x + (w + gap) * i as f32, self.y, w, self.h))
    }

    pub fn bottom(self) -> f32 {
        self.y + self.h
    }
}

pub(super) const INSET: f32 = 8.0;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ComposerMenu {
    Mode,
    Context,
}

pub(super) struct PanelLayout {
    pub bounds: Rect,
    pub toolbar: [Rect; 2],
    pub connection: Rect,
    pub history: Vec<Rect>,
    pub edits: Option<[Rect; 3]>,
    pub body: Rect,
    pub setup_heading: Rect,
    pub setup: Rect,
    pub prompt: Rect,
    pub send: Rect,
    pub composer: Rect,
    pub mode: Rect,
    pub context: Rect,
    pub attachments: Vec<(usize, Rect)>,
    pub menu: Option<[Rect; 3]>,
    pub approval: Option<(Rect, [Rect; 3])>,
}

impl PanelLayout {
    pub fn new(
        bounds: Rect,
        lh: f32,
        history_count: usize,
        has_edits: bool,
        approval: bool,
        attached: [bool; 3],
        menu: Option<ComposerMenu>,
    ) -> Self {
        let gap = lh * 0.25;
        let control_h = lh * 1.45;
        let inner = Rect::new(
            bounds.x + INSET,
            bounds.y + gap,
            (bounds.w - INSET * 2.0).max(0.0),
            control_h,
        );
        // The threshold grows with the font so full Portuguese labels fit at high zoom.
        let narrow = inner.w < lh * 17.0;
        let toolbar = if narrow {
            [inner, inner.row(inner.bottom() + gap, control_h)]
        } else {
            inner.columns(gap)
        };
        let connection = inner.row(toolbar[1].bottom() + gap, control_h);
        let prompt_y = bounds.bottom() - gap - lh * 2.45;
        let send_w = (lh * 4.0).min(inner.w * 0.42);
        let prompt = Rect::new(inner.x, prompt_y, inner.w - send_w - gap, lh * 2.45);
        let send = Rect::new(prompt.x + prompt.w + gap, prompt_y, send_w, prompt.h);
        let context_row = inner.row(prompt_y - gap - control_h, control_h);
        let (mode, context) = if narrow {
            (
                context_row.row(context_row.y - control_h - gap, control_h),
                context_row,
            )
        } else {
            let [mode, context] = context_row.columns(gap);
            (mode, context)
        };
        let active: Vec<_> = attached
            .into_iter()
            .enumerate()
            .filter_map(|(i, on)| on.then_some(i))
            .collect();
        let attachments: Vec<_> = if inner.w < lh * 20.0 {
            active
                .iter()
                .enumerate()
                .map(|(row, &id)| {
                    let y = mode.y - (active.len() - row) as f32 * (lh + gap);
                    (id, inner.row(y, lh))
                })
                .collect()
        } else if active.is_empty() {
            Vec::new()
        } else {
            let row = inner.row(mode.y - gap - lh, lh);
            let slots = row.columns::<3>(gap);
            active
                .iter()
                .enumerate()
                .map(|(i, &id)| (id, slots[i]))
                .collect()
        };
        let composer_top = attachments.first().map_or(mode.y, |(_, r)| r.y);
        let composer = inner.row(composer_top, prompt.bottom() - composer_top);
        let menu = menu.map(|menu| {
            let anchor = match menu {
                ComposerMenu::Mode => mode,
                ComposerMenu::Context => context,
            };
            let width = anchor.w.max(lh * 10.0).min(inner.w);
            let x = (anchor.x + anchor.w - width).max(inner.x);
            std::array::from_fn(|i| {
                Rect::new(
                    x,
                    composer_top - gap - control_h * (3 - i) as f32,
                    width,
                    control_h,
                )
            })
        });
        let approval = approval.then(|| {
            let y = composer_top
                - gap
                - if narrow {
                    control_h * 4.0
                } else {
                    control_h * 2.0
                };
            let title = inner.row(y, control_h);
            let row = inner.row(title.bottom(), control_h);
            let buttons = if narrow {
                std::array::from_fn(|i| row.row(row.y + i as f32 * control_h, control_h))
            } else {
                row.columns(gap)
            };
            (title, buttons)
        });
        let body_bottom = approval.map_or(composer_top, |(title, _)| title.y) - gap;
        let mut y = connection.bottom() + gap;
        let edit_h = if has_edits {
            control_h * 2.0 + gap
        } else {
            0.0
        };
        let rows = ((body_bottom - y - edit_h - lh * 2.0) / control_h)
            .floor()
            .max(0.0) as usize;
        let history = (0..history_count.min(6).min(rows))
            .map(|_| {
                let row = inner.row(y, control_h);
                y = row.bottom();
                row
            })
            .collect();
        let edits = has_edits.then(|| {
            let title = inner.row(y, lh);
            let [accept, discard] = inner.row(title.bottom(), control_h).columns(gap);
            y = discard.bottom() + gap;
            [title, accept, discard]
        });
        let body = inner.row(y, (body_bottom - y).max(0.0));
        let setup_height = lh * 3.0 + gap + control_h;
        let setup_heading = body.row(body.y + (body.h - setup_height).max(0.0) / 2.0, lh * 3.0);
        let setup_w = inner.w.min(lh * 16.0);
        let setup = Rect::new(
            inner.x + (inner.w - setup_w) / 2.0,
            setup_heading.bottom() + gap,
            setup_w,
            control_h,
        );
        Self {
            bounds,
            toolbar,
            connection,
            history,
            edits,
            body,
            setup_heading,
            setup,
            prompt,
            send,
            composer,
            mode,
            context,
            attachments,
            menu,
            approval,
        }
    }
}
