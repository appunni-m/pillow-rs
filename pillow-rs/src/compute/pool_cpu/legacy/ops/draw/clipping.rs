use super::ellipse::EllipseState;

#[derive(Clone)]
pub(super) enum ClipNode {
    Clip { a: f64, b: f64, c: f64 },
    And(Box<ClipNode>, Box<ClipNode>),
    Or(Box<ClipNode>, Box<ClipNode>),
}

impl ClipNode {
    fn transpose(&mut self) {
        match self {
            Self::Clip { a, b, .. } => std::mem::swap(a, b),
            Self::And(left, right) | Self::Or(left, right) => {
                left.transpose();
                right.transpose();
            }
        }
    }
}

pub(super) fn union_intervals(mut intervals: Vec<(i32, i32)>) -> Vec<(i32, i32)> {
    intervals.sort_unstable();
    let mut merged: Vec<(i32, i32)> = Vec::with_capacity(intervals.len());
    for (left, right) in intervals {
        if let Some(last) = merged.last_mut()
            && left <= last.1.saturating_add(1)
        {
            last.1 = last.1.max(right);
        } else {
            merged.push((left, right));
        }
    }
    merged
}

pub(super) fn intersect_intervals(left: &[(i32, i32)], right: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut intersections = Vec::new();
    for &(left_start, left_end) in left {
        for &(right_start, right_end) in right {
            let start = left_start.max(right_start);
            let end = left_end.min(right_end);
            if start <= end {
                intersections.push((start, end));
            }
        }
    }
    union_intervals(intersections)
}

pub(super) fn clip_intervals(
    node: Option<&ClipNode>,
    mut x0: i32,
    y: i32,
    mut x1: i32,
) -> Vec<(i32, i32)> {
    let Some(node) = node else {
        return vec![(x0, x1)];
    };
    match node {
        ClipNode::Clip { a, b, c } => {
            const EPSILON: f64 = 1e-9;
            let y = f64::from(y);
            if a.abs() < EPSILON {
                if b * y + c < -EPSILON {
                    return Vec::new();
                }
            } else {
                let intersection_x = -(b * y + c) / a;
                if a * f64::from(x0) + b * y + c < EPSILON {
                    x0 = f64::from(x0).max(intersection_x).round() as i32;
                }
                if a * f64::from(x1) + b * y + c < EPSILON {
                    x1 = f64::from(x1).min(intersection_x).round() as i32;
                }
            }
            (x0 <= x1).then_some((x0, x1)).into_iter().collect()
        }
        ClipNode::And(left, right) => {
            let left = clip_intervals(Some(left), x0, y, x1);
            let right = clip_intervals(Some(right), x0, y, x1);
            intersect_intervals(&left, &right)
        }
        ClipNode::Or(left, right) => {
            let mut intervals = clip_intervals(Some(left), x0, y, x1);
            intervals.extend(clip_intervals(Some(right), x0, y, x1));
            union_intervals(intervals)
        }
    }
}

pub(crate) struct ClipEllipseState {
    ellipse: EllipseState,
    root: Option<ClipNode>,
    buffer: std::collections::VecDeque<(i32, i32, i32)>,
}

impl ClipEllipseState {
    fn new(ellipse: EllipseState, root: Option<ClipNode>) -> Self {
        Self {
            ellipse,
            root,
            buffer: std::collections::VecDeque::new(),
        }
    }

    fn next(&mut self) -> Option<(i32, i32, i32)> {
        while self.buffer.is_empty() {
            let (x0, y, x1) = self.ellipse.next()?;
            for (left, right) in clip_intervals(self.root.as_ref(), x0, y, x1) {
                self.buffer.push_back((left, y, right));
            }
        }
        self.buffer.pop_front()
    }
}

pub(crate) fn normalize_angles(mut start: f32, mut end: f32) -> (f32, f32) {
    if end - start >= 360.0 {
        return (0.0, 360.0);
    }
    start = if start < 0.0 {
        360.0 - ((-start) % 360.0)
    } else {
        start
    } % 360.0;
    end = start
        + if end < start {
            360.0 - ((start - end) % 360.0)
        } else {
            (end - start) % 360.0
        };
    (start, end)
}

pub(super) fn clip(a: f64, b: f64, c: f64) -> ClipNode {
    ClipNode::Clip { a, b, c }
}

pub(super) fn and(left: ClipNode, right: ClipNode) -> ClipNode {
    ClipNode::And(Box::new(left), Box::new(right))
}

pub(super) fn or(left: ClipNode, right: ClipNode) -> ClipNode {
    ClipNode::Or(Box::new(left), Box::new(right))
}

pub(crate) fn arc_clip_state(a: i32, b: i32, width: i32, start: f32, end: f32) -> ClipEllipseState {
    if a < b {
        let mut state = arc_clip_state(b, a, width, 90.0 - end, 90.0 - start);
        state.ellipse = EllipseState::new(a, b, width);
        if let Some(root) = state.root.as_mut() {
            root.transpose();
        }
        return state;
    }

    let (start, end) = normalize_angles(start, end);
    let root = if end == start + 360.0 {
        None
    } else {
        let start_radians = f64::from(start).to_radians();
        let end_radians = f64::from(end).to_radians();
        let axis_delta = (i64::from(a) * i64::from(a) - i64::from(b) * i64::from(b)) as f64;
        let left = clip(
            -f64::from(a) * start_radians.sin(),
            f64::from(b) * start_radians.cos(),
            axis_delta * (f64::from(start) * std::f64::consts::PI / 90.0).sin() / 2.0,
        );
        let right = clip(
            f64::from(a) * end_radians.sin(),
            -f64::from(b) * end_radians.cos(),
            -axis_delta * (f64::from(end) * std::f64::consts::PI / 90.0).sin() / 2.0,
        );
        if start % 180.0 == 0.0 || end % 180.0 == 0.0 {
            Some(if end - start < 180.0 {
                and(left, right)
            } else {
                or(left, right)
            })
        } else if (((start / 180.0) as i32 + (end / 180.0) as i32) % 2) == 1 {
            let start_half_plane = clip(
                0.0,
                if ((start / 180.0) as i32) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                },
                0.0,
            );
            let end_half_plane = clip(
                0.0,
                if ((end / 180.0) as i32) % 2 == 0 {
                    1.0
                } else {
                    -1.0
                },
                0.0,
            );
            Some(or(and(start_half_plane, left), and(end_half_plane, right)))
        } else {
            let combine = |left, right| {
                if end - start < 180.0 {
                    and(left, right)
                } else {
                    or(left, right)
                }
            };
            Some(combine(
                combine(left, right),
                clip(
                    0.0,
                    if !(180.0..=540.0).contains(&end) {
                        1.0
                    } else {
                        -1.0
                    },
                    0.0,
                ),
            ))
        }
    };
    ClipEllipseState::new(EllipseState::new(a, b, width), root)
}

pub(crate) fn chord_clip_state(
    a: i32,
    b: i32,
    width: i32,
    start: f32,
    end: f32,
) -> ClipEllipseState {
    let start = f64::from(start).to_radians();
    let end = f64::from(end).to_radians();
    let start_x = f64::from(a) * start.cos();
    let end_x = f64::from(a) * end.cos();
    let start_y = f64::from(b) * start.sin();
    let end_y = f64::from(b) * end.sin();
    let line_a = end_y - start_y;
    let line_b = start_x - end_x;
    let line_c = -(line_a * start_x + line_b * start_y);
    ClipEllipseState::new(
        EllipseState::new(a, b, width),
        Some(clip(line_a, line_b, line_c)),
    )
}

pub(crate) fn chord_line_clip_state(
    a: i32,
    b: i32,
    width: i32,
    start: f32,
    end: f32,
) -> ClipEllipseState {
    let start = f64::from(start).to_radians();
    let end = f64::from(end).to_radians();
    let start_x = f64::from(a) * start.cos();
    let end_x = f64::from(a) * end.cos();
    let start_y = f64::from(b) * start.sin();
    let end_y = f64::from(b) * end.sin();
    let line_a = end_y - start_y;
    let line_b = start_x - end_x;
    let line_c = -(line_a * start_x + line_b * start_y);
    let opposite_c = 2.0 * f64::from(width) * (line_a * line_a + line_b * line_b).sqrt() - line_c;
    ClipEllipseState::new(
        EllipseState::new(a, b, a.saturating_add(b).saturating_add(1)),
        Some(and(
            clip(line_a, line_b, line_c),
            clip(-line_a, -line_b, opposite_c),
        )),
    )
}

pub(crate) fn pie_side_clip_state(a: i32, b: i32, width: i32, angle: f32) -> ClipEllipseState {
    let angle = f64::from(angle).to_radians();
    let x = f64::from(a) * angle.cos();
    let y = f64::from(b) * angle.sin();
    let line_a = -y;
    let line_b = x;
    let line_c = f64::from(width) * (line_a * line_a + line_b * line_b).sqrt();
    ClipEllipseState::new(
        EllipseState::new(a, b, a.saturating_add(b).saturating_add(1)),
        Some(and(
            and(clip(line_a, line_b, line_c), clip(-line_a, -line_b, line_c)),
            clip(line_b, -line_a, 0.0),
        )),
    )
}

pub(crate) fn pie_clip_state(a: i32, b: i32, width: i32, start: f32, end: f32) -> ClipEllipseState {
    let start_radians = f64::from(start).to_radians();
    let end_radians = f64::from(end).to_radians();
    let start_x = f64::from(a) * start_radians.cos();
    let end_x = f64::from(a) * end_radians.cos();
    let start_y = f64::from(b) * start_radians.sin();
    let end_y = f64::from(b) * end_radians.sin();
    let left = clip(-start_y, start_x, 0.0);
    let right = clip(end_y, -end_x, 0.0);
    let mut root = if end - start < 180.0 {
        and(left, right)
    } else {
        or(left, right)
    };
    if end - start < 90.0 {
        root = and(
            root,
            clip((start_x + end_x) / 2.0, (start_y + end_y) / 2.0, 0.0),
        );
    }
    ClipEllipseState::new(EllipseState::new(a, b, width), Some(root))
}

pub(crate) fn for_each_clipped_ellipse_span<F>(
    x0: i32,
    y0: i32,
    a: i32,
    b: i32,
    mut state: ClipEllipseState,
    mut visit: F,
) where
    F: FnMut(i32, i32, i32),
{
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
