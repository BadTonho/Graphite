/// Clip geometry and interpolate texture coordinates so partial glyphs render correctly.
pub(super) fn clip_quad(
    rect: [f32; 4],
    uv0: [f32; 2],
    uv1: [f32; 2],
    clip: Option<[f32; 4]>,
) -> Option<([f32; 4], [f32; 2], [f32; 2])> {
    let [x0, y0, x1, y1] = rect;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let Some([left, top, right, bottom]) = clip else {
        return Some((rect, uv0, uv1));
    };
    let clipped = [x0.max(left), y0.max(top), x1.min(right), y1.min(bottom)];
    if clipped[2] <= clipped[0] || clipped[3] <= clipped[1] {
        return None;
    }
    let u = |x: f32| uv0[0] + (uv1[0] - uv0[0]) * (x - x0) / (x1 - x0);
    let v = |y: f32| uv0[1] + (uv1[1] - uv0[1]) * (y - y0) / (y1 - y0);
    Some((
        clipped,
        [u(clipped[0]), v(clipped[1])],
        [u(clipped[2]), v(clipped[3])],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clips_both_geometry_and_texture_coordinates() {
        let (rect, uv0, uv1) = clip_quad(
            [0., 0., 100., 100.],
            [0., 0.],
            [1., 1.],
            Some([25., 10., 75., 90.]),
        )
        .unwrap();
        assert_eq!(rect, [25., 10., 75., 90.]);
        assert_eq!(uv0, [0.25, 0.1]);
        assert_eq!(uv1, [0.75, 0.9]);
        assert!(clip_quad(
            [0., 0., 5., 5.],
            [0., 0.],
            [1., 1.],
            Some([6., 0., 10., 10.])
        )
        .is_none());
    }
}
