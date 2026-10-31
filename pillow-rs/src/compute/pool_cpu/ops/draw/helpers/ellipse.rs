use crate::draw::DrawCanvas;

#[derive(Clone)]
pub(super) struct QuarterState {
    cx: i32,
    cy: i32,
    ex: i32,
    ey: i32,
    a2: i128,
    b2: i128,
    a2b2: i128,
    finished: bool,
}

impl QuarterState {
    fn new(a: i32, b: i32) -> Self {
        if a < 0 || b < 0 {
            return Self {
                cx: 0,
                cy: 0,
                ex: 0,
                ey: 0,
                a2: 0,
                b2: 0,
                a2b2: 0,
                finished: true,
            };
        }
        let a2 = i128::from(a) * i128::from(a);
        let b2 = i128::from(b) * i128::from(b);
        Self {
            cx: a,
            cy: b % 2,
            ex: a % 2,
            ey: b,
            a2,
            b2,
            a2b2: a2 * b2,
            finished: false,
        }
    }

    fn delta(&self, x: i128, y: i128) -> i128 {
        (self.a2 * y * y + self.b2 * x * x - self.a2b2).abs()
    }

    fn next(&mut self) -> Option<(i32, i32)> {
        if self.finished {
            return None;
        }
        let point = (self.cx, self.cy);
        if self.cx == self.ex && self.cy == self.ey {
            self.finished = true;
            return Some(point);
        }

        let mut next_x = self.cx;
        let mut next_y = self.cy.saturating_add(2);
        let mut next_delta = self.delta(i128::from(next_x), i128::from(next_y));
        if next_x > 1 {
            let diagonal_delta = self.delta(
                i128::from(self.cx.saturating_sub(2)),
                i128::from(self.cy.saturating_add(2)),
            );
            if next_delta > diagonal_delta {
                next_x = self.cx.saturating_sub(2);
                next_y = self.cy.saturating_add(2);
                next_delta = diagonal_delta;
            }
            let horizontal_delta =
                self.delta(i128::from(self.cx.saturating_sub(2)), i128::from(self.cy));
            if next_delta > horizontal_delta {
                next_x = self.cx.saturating_sub(2);
                next_y = self.cy;
            }
        }
        self.cx = next_x;
        self.cy = next_y;
        Some(point)
    }
}

pub(super) struct EllipseState {
    outer: QuarterState,
    inner: QuarterState,
    previous_y: i32,
    previous_left: i32,
    previous_right: i32,
    buffer: Vec<(i32, i32, i32)>,
    finished: bool,
    leftmost: i32,
}

impl EllipseState {
    pub(super) fn new(a: i32, b: i32, width: i32) -> Self {
        let mut outer = QuarterState::new(a, b);
        let leftmost = a % 2;
        let first = if width < 1 { None } else { outer.next() };
        if let Some((previous_right, previous_y)) = first {
            let inset = width.saturating_sub(1).saturating_mul(2);
            Self {
                outer,
                inner: QuarterState::new(a.saturating_sub(inset), b.saturating_sub(inset)),
                previous_y,
                previous_left: leftmost,
                previous_right,
                buffer: Vec::with_capacity(4),
                finished: false,
                leftmost,
            }
        } else {
            Self {
                outer,
                inner: QuarterState::new(-1, -1),
                previous_y: 0,
                previous_left: 0,
                previous_right: 0,
                buffer: Vec::with_capacity(4),
                finished: true,
                leftmost,
            }
        }
    }

    pub(super) fn next(&mut self) -> Option<(i32, i32, i32)> {
        if self.buffer.is_empty() {
            if self.finished {
                return None;
            }
            let y = self.previous_y;
            let mut left = self.previous_left;
            let right = self.previous_right;

            loop {
                match self.outer.next() {
                    Some((_, next_y)) if next_y <= y => {}
                    Some((x, next_y)) => {
                        self.previous_right = x;
                        self.previous_y = next_y;
                        break;
                    }
                    None => {
                        self.finished = true;
                        break;
                    }
                }
            }

            let mut next_inner = None;
            loop {
                match self.inner.next() {
                    Some((x, next_y)) if next_y <= y => left = x,
                    Some((x, _)) => {
                        next_inner = Some(x);
                        break;
                    }
                    None => break,
                }
            }
            self.previous_left = next_inner.unwrap_or(self.leftmost);

            let has_right_segment = left > 0 || left < right;
            if has_right_segment && y > 0 {
                self.buffer
                    .push((if left == 0 { 2 } else { left }, y, right));
            }
            if y > 0 {
                self.buffer.push((-right, y, -left));
            }
            if has_right_segment {
                self.buffer
                    .push((if left == 0 { 2 } else { left }, -y, right));
            }
            self.buffer.push((-right, -y, -left));
        }
        self.buffer.pop()
    }
}

pub(super) fn draw_hline<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y: i32,
    x1: i32,
    color: (u8, u8, u8, u8),
) {
    let width = canvas.width();
    let height = canvas.height();
    if width == 0 || height == 0 || y < 0 || y >= height as i32 {
        return;
    }
    let left = x0.max(0);
    let right = x1.min(width as i32 - 1);
    if left > right {
        return;
    }
    for x in left..=right {
        canvas.put_rgba(x as u32, y as u32, [color.0, color.1, color.2, color.3]);
    }
}

/// Enumerate Pillow's ellipse scan-conversion spans without writing pixels.
///
/// The state machine is scalar geometry/control-plane work.  Backends may
/// consume the inclusive native-coordinate spans with their own data-plane
/// kernel; in particular, the SIMD adapter uses this helper only to produce
/// spans and performs every destination write itself.
pub(crate) fn for_each_ellipse_span<F>(x0: i32, y0: i32, x1: i32, y1: i32, width: i32, mut visit: F)
where
    F: FnMut(i32, i32, i32),
{
    let Some(a) = x1.checked_sub(x0) else {
        return;
    };
    let Some(b) = y1.checked_sub(y0) else {
        return;
    };
    if a < 0 || b < 0 {
        return;
    }
    let mut state = EllipseState::new(a, b, width);
    while let Some((segment_x0, segment_y, segment_x1)) = state.next() {
        let map_coordinate = |origin: i32, offset: i32, diameter: i32| {
            (i64::from(origin) + (i64::from(offset) + i64::from(diameter)) / 2)
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
        };
        visit(
            map_coordinate(x0, segment_x0, a),
            map_coordinate(y0, segment_y, b),
            map_coordinate(x0, segment_x1, a),
        );
    }
}

pub(super) fn draw_ellipse_segments<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    width: i32,
    color: (u8, u8, u8, u8),
) {
    for_each_ellipse_span(x0, y0, x1, y1, width, |span_x0, span_y, span_x1| {
        draw_hline(canvas, span_x0, span_y, span_x1, color);
    });
}

/// Draw an ellipse using Pillow 12.2's `ellipse_state` scan conversion.
pub(super) fn draw_ellipse_on_canvas<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
) {
    let Some(a) = x1.checked_sub(x0) else {
        return;
    };
    let Some(b) = y1.checked_sub(y0) else {
        return;
    };
    if let Some(fill_color) = fill {
        draw_ellipse_segments(canvas, x0, y0, x1, y1, a.saturating_add(b), fill_color);
    }
    if let Some(outline_color) = outline.filter(|color| Some(*color) != fill && width != 0) {
        draw_ellipse_segments(
            canvas,
            x0,
            y0,
            x1,
            y1,
            i32::try_from(width).unwrap_or(i32::MAX),
            outline_color,
        );
    }
}
