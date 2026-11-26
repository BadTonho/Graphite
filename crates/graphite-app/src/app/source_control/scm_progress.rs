//! Visible feedback for source control work: an animated spinner on the primary button while
//! an operation runs, and the count of changed files on the activity bar icon.
use super::*;
use std::time::{Duration, Instant};

/// How long a busy mark is trusted; a lost completion event must not spin forever.
const BUSY_LIMIT: Duration = Duration::from_secs(10 * 60);
/// Time one spinner step lasts.
pub(crate) const SPINNER_STEP: Duration = Duration::from_millis(90);
const SPINNER_DOTS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ScmBusyKind {
    Sync,
    Publish,
    Commit,
    Other,
}

impl ScmBusyKind {
    fn label_key(self) -> &'static str {
        match self {
            Self::Sync => "scm.sidebar.syncing",
            Self::Publish => "scm.sidebar.publishing",
            Self::Commit => "scm.sidebar.committing",
            Self::Other => "scm.sidebar.working",
        }
    }
}

/// Text of the badge for a number of changed files.
pub(crate) fn badge_text(count: usize) -> String {
    if count > 99 {
        "99+".to_owned()
    } else {
        count.to_string()
    }
}

/// Which dot of the spinner is the brightest after `elapsed`.
pub(crate) fn spinner_head(elapsed: Duration) -> usize {
    (elapsed.as_millis() / SPINNER_STEP.as_millis()) as usize % SPINNER_DOTS
}

/// Mixes two `0xRRGGBB` colors; `t` is the share of `from` (0.0 to 1.0).
fn blend(from: u32, to: u32, t: f32) -> u32 {
    let channel = |shift: u32| {
        let a = ((from >> shift) & 0xff) as f32;
        let b = ((to >> shift) & 0xff) as f32;
        ((a * t + b * (1.0 - t)).round() as u32).min(255) << shift
    };
    channel(16) | channel(8) | channel(0)
}

/// Draws a ring of dots whose brightness trails behind the head dot.
pub(crate) fn draw_spinner(
    renderer: &mut Renderer,
    x: f32,
    y: f32,
    size: f32,
    foreground: u32,
    background: u32,
    head: usize,
) {
    let center = (x + size * 0.5, y + size * 0.5);
    let radius = size * 0.34;
    let dot = (size * 0.2).max(2.0);
    for index in 0..SPINNER_DOTS {
        let angle = index as f32 / SPINNER_DOTS as f32 * std::f32::consts::TAU;
        let age = (head + SPINNER_DOTS - index) % SPINNER_DOTS;
        let strength = 1.0 - age as f32 / SPINNER_DOTS as f32 * 0.85;
        renderer.draw_rect(
            center.0 + radius * angle.sin() - dot * 0.5,
            center.1 - radius * angle.cos() - dot * 0.5,
            dot,
            dot,
            rgb(blend(foreground, background, strength)),
        );
    }
}

impl App {
    pub(crate) fn begin_scm_busy(&mut self, kind: ScmBusyKind) {
        self.scm.busy = Some((kind, Instant::now()));
    }

    pub(crate) fn end_scm_busy(&mut self) {
        self.scm.busy = None;
    }

    /// The running operation, if one is in progress.
    pub(crate) fn scm_busy_state(&self) -> Option<(ScmBusyKind, Duration)> {
        self.scm
            .busy
            .map(|(kind, since)| (kind, since.elapsed()))
            .filter(|(_, elapsed)| *elapsed < BUSY_LIMIT)
    }

    pub(crate) fn scm_busy_label(&self, kind: ScmBusyKind) -> &str {
        self.t(kind.label_key())
    }

    /// Number shown on the Source Control icon: files with pending changes.
    pub(crate) fn scm_badge_count(&self) -> usize {
        if self.scm.snapshot.error.is_some() {
            0
        } else {
            self.scm.snapshot.files.len()
        }
    }

    /// Draws the count over the bottom-right corner of the activity bar item.
    pub(crate) fn draw_scm_activity_badge(
        &self,
        renderer: &mut Renderer,
        item: (f32, f32, f32, f32),
    ) {
        let count = self.scm_badge_count();
        if count == 0 {
            return;
        }
        let text = badge_text(count);
        let cw = renderer.char_width();
        let lh = renderer.line_height();
        let height = (lh * 0.78).round().max(12.0);
        let width = (text.chars().count() as f32 * cw + 6.0).max(height);
        let right = item.0 + item.2 - 3.0;
        let x = (right - width).max(item.0);
        let y = item.1 + item.3 - height - 5.0;
        renderer.draw_rect(
            x,
            y,
            width,
            height,
            rgb(Theme::color(Theme::STATUS, 0x007acc)),
        );
        renderer.draw_text(
            &text,
            x + (width - text.chars().count() as f32 * cw) * 0.5,
            y + (height - lh) * 0.5,
            rgb(0xffffff),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scm::FileStatus;
    use crate::test_support::TempDir;

    fn app() -> (TempDir, App) {
        let dir = TempDir::new();
        let app = App::new_with_options(dir.path().to_path_buf(), None, true);
        (dir, app)
    }

    #[test]
    fn the_badge_caps_large_counts() {
        assert_eq!(badge_text(1), "1");
        assert_eq!(badge_text(99), "99");
        assert_eq!(badge_text(100), "99+");
    }

    #[test]
    fn the_badge_counts_changed_files_and_hides_at_zero_or_on_error() {
        let (_dir, mut app) = app();
        assert_eq!(app.scm_badge_count(), 0);
        for name in ["a.txt", "b.txt"] {
            app.scm.snapshot.files.push(FileStatus {
                path: name.into(),
                unstaged: true,
                ..Default::default()
            });
        }
        assert_eq!(app.scm_badge_count(), 2);
        app.scm.snapshot.error = Some("boom".into());
        assert_eq!(app.scm_badge_count(), 0);
    }

    #[test]
    fn the_spinner_head_advances_and_wraps() {
        assert_eq!(spinner_head(Duration::ZERO), 0);
        assert_eq!(spinner_head(SPINNER_STEP), 1);
        assert_eq!(spinner_head(SPINNER_STEP * SPINNER_DOTS as u32), 0);
    }

    #[test]
    fn blending_goes_from_background_to_foreground() {
        assert_eq!(blend(0xffffff, 0x000000, 1.0), 0xffffff);
        assert_eq!(blend(0xffffff, 0x000000, 0.0), 0x000000);
        assert_eq!(blend(0xff0000, 0x0000ff, 0.5), 0x800080);
    }

    #[test]
    fn busy_state_begins_and_ends() {
        let (_dir, mut app) = app();
        assert!(app.scm_busy_state().is_none());
        app.begin_scm_busy(ScmBusyKind::Sync);
        assert_eq!(
            app.scm_busy_state().map(|(kind, _)| kind),
            Some(ScmBusyKind::Sync)
        );
        app.end_scm_busy();
        assert!(app.scm_busy_state().is_none());
    }

    #[test]
    fn a_stale_busy_mark_expires() {
        let (_dir, mut app) = app();
        app.scm.busy = Some((
            ScmBusyKind::Other,
            Instant::now() - BUSY_LIMIT - Duration::from_secs(1),
        ));
        assert!(app.scm_busy_state().is_none());
    }

    #[test]
    fn every_busy_kind_has_a_translated_label() {
        let (_dir, app) = app();
        for kind in [
            ScmBusyKind::Sync,
            ScmBusyKind::Publish,
            ScmBusyKind::Commit,
            ScmBusyKind::Other,
        ] {
            let label = app.scm_busy_label(kind);
            assert!(
                !label.starts_with("scm.sidebar."),
                "missing text for {kind:?}"
            );
        }
    }
}
