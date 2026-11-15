//! Small icons for the buttons of the source control panel.
//!
//! The embedded icon font does not carry these glyphs, so each icon is built on a 16x16 grid
//! and drawn as rectangles, the same way as the settings gear of the Activity Bar.

use super::scm_ui::ScmRect;
use super::*;

const GRID: usize = 16;
type Bitmap = [[bool; GRID]; GRID];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScmIcon {
    Sync,
    Refresh,
    PullRequest,
    More,
    Plus,
    Minus,
    History,
    Stash,
    Check,
    Fetch,
    Pull,
    Push,
    Discard,
}

fn plot(map: &mut Bitmap, x: f32, y: f32) {
    let (x, y) = (x.floor() as i32, y.floor() as i32);
    if (0..GRID as i32).contains(&x) && (0..GRID as i32).contains(&y) {
        map[y as usize][x as usize] = true;
    }
}

fn fill(map: &mut Bitmap, x: usize, y: usize, width: usize, height: usize) {
    for row in y..(y + height).min(GRID) {
        for column in x..(x + width).min(GRID) {
            map[row][column] = true;
        }
    }
}

/// A one pixel outline.
fn frame(map: &mut Bitmap, x: usize, y: usize, width: usize, height: usize) {
    fill(map, x, y, width, 1);
    fill(map, x, y + height - 1, width, 1);
    fill(map, x, y, 1, height);
    fill(map, x + width - 1, y, 1, height);
}

fn line(map: &mut Bitmap, from: (f32, f32), to: (f32, f32)) {
    let length = (to.0 - from.0).abs().max((to.1 - from.1).abs());
    let steps = ((length * 2.0).ceil() as usize).max(1);
    for step in 0..=steps {
        let t = step as f32 / steps as f32;
        plot(
            map,
            from.0 + (to.0 - from.0) * t,
            from.1 + (to.1 - from.1) * t,
        );
    }
}

/// A two pixel wide arc around the center of the grid. Angles are in degrees, measured from
/// the right and increasing clockwise (the screen's y axis points down).
fn arc(map: &mut Bitmap, start: f32, sweep: f32) {
    let steps = (sweep * 2.0) as usize;
    for step in 0..=steps {
        let angle = (start + step as f32 / 2.0).to_radians();
        for radius in [4.0_f32, 5.0] {
            plot(
                map,
                8.0 + radius * angle.cos(),
                8.0 + radius * angle.sin(),
            );
        }
    }
}

fn point_on_arc(angle: f32) -> (f32, f32) {
    let angle = angle.to_radians();
    (8.0 + 4.5 * angle.cos(), 8.0 + 4.5 * angle.sin())
}

fn triangle(map: &mut Bitmap, a: (f32, f32), b: (f32, f32), c: (f32, f32)) {
    let side = |p: (f32, f32), q: (f32, f32), r: (f32, f32)| {
        (p.0 - r.0) * (q.1 - r.1) - (q.0 - r.0) * (p.1 - r.1)
    };
    for y in 0..GRID {
        for x in 0..GRID {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            let (d1, d2, d3) = (side(p, a, b), side(p, b, c), side(p, c, a));
            let negative = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
            let positive = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
            if !(negative && positive) {
                map[y][x] = true;
            }
        }
    }
}

/// An arrow head whose base is centered on `at` and which points along `direction`.
fn arrow_head(map: &mut Bitmap, at: (f32, f32), direction: (f32, f32)) {
    let normal = (-direction.1, direction.0);
    let tip = (at.0 + direction.0 * 2.8, at.1 + direction.1 * 2.8);
    let one = (at.0 + normal.0 * 2.6, at.1 + normal.1 * 2.6);
    let other = (at.0 - normal.0 * 2.6, at.1 - normal.1 * 2.6);
    triangle(map, tip, one, other);
}

/// The direction an arc travelling clockwise points in at `angle` degrees.
fn clockwise(angle: f32) -> (f32, f32) {
    let angle = angle.to_radians();
    (-angle.sin(), angle.cos())
}

fn counter_clockwise(angle: f32) -> (f32, f32) {
    let (x, y) = clockwise(angle);
    (-x, -y)
}

fn bitmap(icon: ScmIcon) -> Bitmap {
    let mut map = [[false; GRID]; GRID];
    match icon {
        ScmIcon::Plus => {
            fill(&mut map, 7, 3, 2, 10);
            fill(&mut map, 3, 7, 10, 2);
        }
        ScmIcon::Minus => fill(&mut map, 3, 7, 10, 2),
        ScmIcon::More => {
            for x in [2, 7, 12] {
                fill(&mut map, x, 7, 2, 2);
            }
        }
        ScmIcon::Check => {
            line(&mut map, (2.5, 8.5), (6.5, 12.5));
            line(&mut map, (2.5, 7.5), (6.5, 11.5));
            line(&mut map, (6.5, 12.5), (13.5, 5.5));
            line(&mut map, (6.5, 11.5), (13.5, 4.5));
        }
        ScmIcon::Refresh => {
            arc(&mut map, 20.0, 260.0);
            arrow_head(&mut map, point_on_arc(280.0), clockwise(280.0));
        }
        ScmIcon::Sync => {
            arc(&mut map, 200.0, 140.0);
            arrow_head(&mut map, point_on_arc(340.0), clockwise(340.0));
            arc(&mut map, 20.0, 140.0);
            arrow_head(&mut map, point_on_arc(160.0), clockwise(160.0));
        }
        ScmIcon::History => {
            arc(&mut map, 220.0, 300.0);
            arrow_head(&mut map, point_on_arc(220.0), counter_clockwise(220.0));
            line(&mut map, (8.0, 8.0), (8.0, 4.5));
            line(&mut map, (8.0, 8.0), (10.5, 9.5));
        }
        ScmIcon::Stash => {
            frame(&mut map, 2, 3, 12, 3);
            frame(&mut map, 3, 6, 10, 7);
            fill(&mut map, 6, 8, 4, 1);
        }
        ScmIcon::PullRequest => {
            frame(&mut map, 2, 2, 4, 4);
            frame(&mut map, 2, 10, 4, 4);
            frame(&mut map, 10, 10, 4, 4);
            fill(&mut map, 3, 6, 2, 4);
            fill(&mut map, 11, 5, 2, 5);
            fill(&mut map, 7, 4, 5, 1);
            fill(&mut map, 6, 3, 1, 3);
        }
        ScmIcon::Fetch => {
            // Dashed down arrow to tray.
            fill(&mut map, 7, 2, 2, 2);
            fill(&mut map, 7, 5, 2, 5);
            line(&mut map, (4.5, 7.5), (7.5, 10.5));
            line(&mut map, (11.5, 7.5), (8.5, 10.5));
            fill(&mut map, 4, 12, 8, 2);
        }
        ScmIcon::Pull => {
            // Down arrow to base line.
            fill(&mut map, 7, 3, 2, 7);
            line(&mut map, (4.5, 7.5), (7.5, 10.5));
            line(&mut map, (11.5, 7.5), (8.5, 10.5));
            fill(&mut map, 4, 12, 8, 2);
        }
        ScmIcon::Discard => {
            // An arrow curving back, like undo.
            arc(&mut map, 170.0, 220.0);
            arrow_head(&mut map, point_on_arc(170.0), counter_clockwise(170.0));
        }
        ScmIcon::Push => {
            // Up arrow from base line.
            fill(&mut map, 7, 4, 2, 7);
            line(&mut map, (4.5, 6.5), (7.5, 3.5));
            line(&mut map, (11.5, 6.5), (8.5, 3.5));
            fill(&mut map, 4, 12, 8, 2);
        }
    }
    map
}

/// Draws `icon` centered in `bounds`.
pub(super) fn draw_scm_icon(
    renderer: &mut Renderer,
    icon: ScmIcon,
    bounds: ScmRect,
    color: graphite_render::Color,
) {
    let map = bitmap(icon);
    let size = (bounds.height * 0.72).round().max(12.0);
    let scale = size / GRID as f32;
    let left = bounds.x + (bounds.width - size) * 0.5;
    let top = bounds.y + (bounds.height - size) * 0.5;
    for (row, cells) in map.iter().enumerate() {
        let mut column = 0;
        while column < GRID {
            if !cells[column] {
                column += 1;
                continue;
            }
            let start = column;
            while column < GRID && cells[column] {
                column += 1;
            }
            let x0 = (left + start as f32 * scale).round();
            let x1 = (left + column as f32 * scale).round();
            let y0 = (top + row as f32 * scale).round();
            let y1 = (top + (row + 1) as f32 * scale).round();
            renderer.draw_rect(x0, y0, (x1 - x0).max(1.0), (y1 - y0).max(1.0), color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICONS: [ScmIcon; 13] = [
        ScmIcon::Sync,
        ScmIcon::Refresh,
        ScmIcon::PullRequest,
        ScmIcon::More,
        ScmIcon::Plus,
        ScmIcon::Minus,
        ScmIcon::History,
        ScmIcon::Stash,
        ScmIcon::Check,
        ScmIcon::Fetch,
        ScmIcon::Pull,
        ScmIcon::Push,
        ScmIcon::Discard,
    ];

    fn count(map: &Bitmap) -> usize {
        map.iter().flatten().filter(|cell| **cell).count()
    }

    #[test]
    fn every_icon_draws_something_and_leaves_a_margin_for_the_hover_background() {
        for icon in ICONS {
            let map = bitmap(icon);
            assert!(count(&map) >= 8, "{icon:?} is nearly empty");
            assert!(count(&map) < GRID * GRID / 2, "{icon:?} fills the grid");
            for edge in [0, GRID - 1] {
                assert!(
                    map[edge].iter().all(|cell| !cell),
                    "{icon:?} touches a horizontal edge"
                );
                assert!(
                    map.iter().all(|row| !row[edge]),
                    "{icon:?} touches a vertical edge"
                );
            }
        }
    }

    #[test]
    fn plus_is_a_symmetric_cross_and_minus_is_its_horizontal_bar() {
        let plus = bitmap(ScmIcon::Plus);
        for y in 0..GRID {
            for x in 0..GRID {
                assert_eq!(plus[y][x], plus[x][y], "plus is symmetric at {x},{y}");
            }
        }
        let minus = bitmap(ScmIcon::Minus);
        assert!(minus[7][3..13].iter().all(|cell| *cell));
        assert_eq!(count(&minus), 20);
        assert!(count(&plus) > count(&minus));
    }

    #[test]
    fn sync_has_an_arrow_in_each_half_and_refresh_only_one_gap() {
        let sync = bitmap(ScmIcon::Sync);
        let top: usize = sync[..GRID / 2].iter().flatten().filter(|c| **c).count();
        let bottom: usize = sync[GRID / 2..].iter().flatten().filter(|c| **c).count();
        assert!(top > 10 && bottom > 10);
        // The refresh ring is open at the top right, so that corner is emptier than the left.
        let refresh = bitmap(ScmIcon::Refresh);
        let left: usize = refresh.iter().flat_map(|row| &row[..4]).filter(|c| **c).count();
        assert!(left > 4);
    }
}
