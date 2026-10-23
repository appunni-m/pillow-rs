use crate::raster::{Rgba, RgbaImage};

// ── Drawing primitives ──────────────────────────────────────────────

/// Minimal native-pixel canvas used by the shared rasterizers.
///
/// Implementations retain their original storage layout; drawing code never
/// needs to convert an `L`, `LA`, `RGB`, or indexed buffer through `RGBA`.
pub(crate) trait DrawCanvas {
    /// Canvas width in pixels.
    fn width(&self) -> u32;
    /// Canvas height in pixels.
    fn height(&self) -> u32;
    /// Writes one normalized RGBA color using the canvas's native channels.
    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]);
}

impl DrawCanvas for RgbaImage {
    fn width(&self) -> u32 {
        self.width()
    }

    fn height(&self) -> u32 {
        self.height()
    }

    fn put_rgba(&mut self, x: u32, y: u32, color: [u8; 4]) {
        self.put_pixel(x, y, Rgba(color));
    }
}

/// Bresenham's line algorithm with clamping.
pub(crate) fn bresenham_line<C: DrawCanvas>(
    canvas: &mut C,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: (u8, u8, u8, u8),
    w: u32,
    h: u32,
    raw: bool,
) {
    for_each_bresenham_point(x0, y0, x1, y1, |x, y| {
        plot(canvas, x, y, color, w, h, raw);
    });
}

/// Enumerate the complete Pillow Bresenham raster for one line segment.
///
/// Geometry consumers such as the SIMD drawing adapter use this callback to
/// keep the integer control path shared while writing the destination through
/// their own native data-plane kernel.
pub(crate) fn for_each_bresenham_point<F>(x0: i32, y0: i32, x1: i32, y1: i32, mut visit: F)
where
    F: FnMut(i32, i32),
{
    // Match Pillow src/libImaging/Draw.c::{line8,line32,line32rgba}.
    // The C primitive omits its final endpoint because draw_lines adds it
    // once after the segment chain; this helper represents one complete
    // high-level segment, so it appends that endpoint below.
    let mut x = i64::from(x0);
    let mut y = i64::from(y0);
    let target_x = i64::from(x1);
    let target_y = i64::from(y1);
    let mut dx = target_x - x;
    let step_x = if dx < 0 {
        dx = -dx;
        -1
    } else {
        1
    };
    let mut dy = target_y - y;
    let step_y = if dy < 0 {
        dy = -dy;
        -1
    } else {
        1
    };

    if dx == 0 {
        for _ in 0..dy {
            visit(x as i32, y as i32);
            y += step_y;
        }
    } else if dy == 0 {
        for _ in 0..dx {
            visit(x as i32, y as i32);
            x += step_x;
        }
    } else if dx > dy {
        let steps = dx;
        dy += dy;
        let mut error = dy - dx;
        dx += dx;
        for _ in 0..steps {
            visit(x as i32, y as i32);
            if error >= 0 {
                y += step_y;
                error -= dx;
            }
            error += dy;
            x += step_x;
        }
    } else {
        let steps = dy;
        dx += dx;
        let mut error = dx - dy;
        dy += dy;
        for _ in 0..steps {
            visit(x as i32, y as i32);
            if error >= 0 {
                x += step_x;
                error -= dy;
            }
            error += dx;
            y += step_y;
        }
    }
    visit(x1, y1);
}

/// Plot a single pixel with bounds checking.
///
/// When `raw` is true (F/I mode), writes the 4 bytes directly as-is without any
/// alpha blending — the 4-byte chunk represents a raw float32 or int32 LE value.
///
/// When `raw` is false, writes the normalized color directly. Pillow's default
/// drawing context uses the native-mode `point8`/`point32` primitives; alpha is
/// a stored channel, not an implicit blend factor. RGBA-on-RGB drawing uses a
/// separate explicit blend mode at the binding/API layer.
#[inline]
pub(crate) fn plot<C: DrawCanvas>(
    canvas: &mut C,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
    w: u32,
    h: u32,
    raw: bool,
) {
    if x < 0 || y < 0 || x as u32 >= w || y as u32 >= h {
        return;
    }
    let (x, y) = (x as u32, y as u32);
    let _ = raw;
    canvas.put_rgba(x, y, [color.0, color.1, color.2, color.3]);
}

#[inline]
pub(crate) fn blend_u8(src: u8, dst: u8, alpha: u8, inv_alpha: u16) -> u8 {
    let a = alpha as u16;
    (((src as u16 * a) + (dst as u16 * inv_alpha) + 127) / 255) as u8
}

/// PIL-style single-channel blend:
///   BLEND(mask, dst, src) = DIV255(dst * (255 - mask) + src * mask)
/// where DIV255(x) = (x + 127) / 255  (round-to-nearest via +127 before /255 truncation)
///
/// This exactly matches PIL's ImagingFill2 → fill_mask_L C implementation.
/// Using the simpler unsigned formula (fg*cov + bg*(255-cov))/255 truncates,
/// which differs by 1 from PIL's rounded result for some cov values.
#[inline]
pub(crate) fn pil_blend(fg: u8, bg: u8, cov: u8) -> u8 {
    let x = (bg as u32) * (255u32 - cov as u32) + (fg as u32) * (cov as u32);
    // DIV255 with rounding: (x + 127) / 255
    // Note: `(x + 127 + (x >> 8)) >> 8` is NOT used — it is an approximation
    // that differs from the exact /255 for some inputs (e.g., x=37104 gives 145 vs 146).
    ((x + 127) / 255) as u8
}

/// Compute cubic Bezier curve subdivision points.
/// Returns a flat list of (x, y) integer pairs for the curve from t=1..steps.
/// `control_points` must have at least 8 elements: [x0, y0, x1, y1, x2, y2, x3, y3].
/// Matches Pillow's `src/libImaging/Draw.c::ImagingOutlineCurve` algorithm.
pub fn outline_curve_points(control_points: &[f64], steps: u32) -> Vec<(i32, i32)> {
    if control_points.len() < 8 || steps == 0 {
        return vec![];
    }
    // Pillow receives these values as C `float`s, not doubles. Keep the
    // subdivision arithmetic in f32 so points near a half-pixel boundary
    // follow the oracle's rounding decisions.
    let x0 = control_points[0] as f32;
    let y0 = control_points[1] as f32;
    let x1 = control_points[2] as f32;
    let y1 = control_points[3] as f32;
    let x2 = control_points[4] as f32;
    let y2 = control_points[5] as f32;
    let x3 = control_points[6] as f32;
    let y3 = control_points[7] as f32;

    let mut points = Vec::with_capacity(steps as usize);
    for i in 1..=steps {
        let t = i as f32 / steps as f32;
        let t2 = t * t;
        let t3 = t2 * t;
        let u = 1.0_f32 - t;
        let u2 = u * u;
        let u3 = u2 * u;
        // C adds 0.5 and casts to int (truncating toward zero), rather than
        // using the host language's nearest-integer rounding rule.
        let x = x0 * u3 + 3.0 * (x1 * t * u2 + x2 * t2 * u) + x3 * t3 + 0.5;
        let y = y0 * u3 + 3.0 * (y1 * t * u2 + y2 * t2 * u) + y3 * t3 + 0.5;
        points.push((x.trunc() as i32, y.trunc() as i32));
    }
    points
}

/// PIL-style ROUND_UP: away-from-zero rounding at 0.5.
pub(crate) fn round_up(f: f64) -> i32 {
    if f >= 0.0 {
        (f + 0.5).floor() as i32
    } else {
        -((-f) + 0.5).floor() as i32
    }
}

/// PIL-style ROUND_DOWN: toward-zero rounding at 0.5.
pub(crate) fn round_down(f: f64) -> i32 {
    if f >= 0.0 {
        (f - 0.5).ceil() as i32
    } else {
        -((-f) - 0.5).ceil() as i32
    }
}

/// Construct Pillow's four-edge polygon for a wide line segment.
///
/// This is scalar geometry/control-plane work shared by CPU and SIMD.  The
/// returned polygon is filled by the caller's native data-plane writer.  A
/// zero-length segment is represented by `None` because Pillow plots that
/// segment as one point instead of constructing a stroke polygon.
pub(crate) fn wide_line_polygon_points(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    width: u32,
) -> Option<[(i32, i32); 4]> {
    let delta_x = i64::from(x1) - i64::from(x0);
    let delta_y = i64::from(y1) - i64::from(y0);
    if delta_x == 0 && delta_y == 0 {
        return None;
    }

    // Pillow src/libImaging/Draw.c::ImagingDrawWideLine constructs an
    // asymmetric four-edge polygon for even widths. The two rounded ratios
    // are what keep an even requested width from becoming one pixel too wide.
    let squared_length = delta_x
        .saturating_mul(delta_x)
        .saturating_add(delta_y.saturating_mul(delta_y));
    let length = (squared_length as f64).sqrt();
    let half_width = f64::from(width.saturating_sub(1)) / 2.0;
    let ratio_max = f64::from(round_up(half_width)) / length;
    let ratio_min = f64::from(round_down(half_width)) / length;
    let offset_x_min = round_down(ratio_min * delta_y as f64);
    let offset_x_max = round_down(ratio_max * delta_y as f64);
    let offset_y_min = round_down(ratio_min * delta_x as f64);
    let offset_y_max = round_down(ratio_max * delta_x as f64);
    Some([
        (
            x0.saturating_sub(offset_x_min),
            y0.saturating_add(offset_y_max),
        ),
        (
            x1.saturating_sub(offset_x_min),
            y1.saturating_add(offset_y_max),
        ),
        (
            x1.saturating_add(offset_x_max),
            y1.saturating_sub(offset_y_min),
        ),
        (
            x0.saturating_add(offset_x_max),
            y0.saturating_sub(offset_y_min),
        ),
    ])
}

/// PIL-identical scanline polygon fill.
///
/// Uses PIL's edge-table / scanline algorithm from Draw.c:
/// 1. Build edges with inverse slope (dx = Δx/Δy)
/// 2. For each scanline, compute x-intersections from active edges
/// 3. Sort intersections and fill between pairs using ROUND_UP/ROUND_DOWN
/// 4. Horizontal edges drawn directly as filled lines
#[derive(Clone, Copy)]
struct ScanEdge {
    x0: i32,
    y0: i32,
    xmin: i32,
    xmax: i32,
    ymin: i32,
    ymax: i32,
    dx: f32,
}

impl ScanEdge {
    fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Self {
            x0,
            y0,
            xmin: x0.min(x1),
            xmax: x0.max(x1),
            ymin: y0.min(y1),
            ymax: y0.max(y1),
            dx: if y0 == y1 {
                0.0
            } else {
                (x1 - x0) as f32 / (y1 - y0) as f32
            },
        }
    }
}

fn polygon_edges(points: &[(i32, i32)]) -> Vec<ScanEdge> {
    let mut edges: Vec<ScanEdge> = Vec::with_capacity(points.len());
    for i in 0..points.len().saturating_sub(1) {
        let (x0, y0) = points[i];
        let (x1, y1) = points[i + 1];
        if y0 == y1 && i != 0 && y0 == points[i - 1].1 {
            let previous_x = points[i - 1].0;
            if let Some(last) = edges.last_mut() {
                if x1 > x0 && x0 > previous_x {
                    last.xmax = x1;
                    continue;
                }
                if x1 < x0 && x0 < previous_x {
                    last.xmin = x1;
                    continue;
                }
            }
        }
        edges.push(ScanEdge::new(x0, y0, x1, y1));
    }
    if points.last() != points.first() {
        let &(x0, y0) = points.last().expect("nonempty polygon");
        let &(x1, y1) = points.first().expect("nonempty polygon");
        edges.push(ScanEdge::new(x0, y0, x1, y1));
    }
    edges
}

fn draw_horizontal_edges<F>(edges: &[ScanEdge], width: i32, height: i32, visit: &mut F)
where
    F: FnMut(i32, i32, i32),
{
    for edge in edges {
        if edge.ymin == edge.ymax && edge.ymin >= 0 && edge.ymin < height {
            let x_start = edge.xmin.max(0);
            let x_end = edge.xmax.min(width - 1);
            if x_start <= x_end {
                visit(x_start, x_end, edge.ymin);
            }
        }
    }
}

fn vertex_intersection(edge: &ScanEdge, preceding: &[&ScanEdge], y: i32, x: f32) -> f32 {
    if (y != edge.ymin && y != edge.ymax) || edge.dx == 0.0 {
        return x;
    }
    for other in preceding {
        if (y != other.ymin && y != other.ymax) || other.dx == 0.0 {
            continue;
        }
        let yf = y as f32;
        let other_x = (yf - other.y0 as f32) * other.dx + other.x0 as f32;
        if x.round() != other_x.round() {
            continue;
        }
        let offset = if y == edge.ymax { -1 } else { 1 };
        let adjacent_x = ((y + offset - edge.y0) as f32) * edge.dx + edge.x0 as f32;
        if y + offset < other.ymin || y + offset > other.ymax {
            continue;
        }
        let adjacent_other_x = ((y + offset - other.y0) as f32) * other.dx + other.x0 as f32;
        if x > adjacent_x + 1.0 && x > adjacent_other_x + 1.0 {
            return adjacent_x.max(adjacent_other_x).round() + 1.0;
        }
        if x < adjacent_x - 1.0 && x < adjacent_other_x - 1.0 {
            return adjacent_x.min(adjacent_other_x).round() - 1.0;
        }
        break;
    }
    x
}

fn scanline_intersections(
    edges: &[&ScanEdge],
    y: i32,
    global_ymax: i32,
    intersections: &mut Vec<f32>,
) {
    let yf = y as f32;
    intersections.clear();
    for (index, edge) in edges.iter().enumerate() {
        if y < edge.ymin || y > edge.ymax {
            continue;
        }
        let x = (yf - edge.y0 as f32) * edge.dx + edge.x0 as f32;
        intersections.push(x);
        if y == edge.ymax && y < global_ymax {
            intersections.push(x);
        } else {
            let adjusted = vertex_intersection(edge, &edges[..index], y, x);
            *intersections.last_mut().expect("intersection was pushed") = adjusted;
        }
    }
}

fn fill_scanline_pairs<F>(intersections: &mut [f32], y: i32, width: i32, visit: &mut F)
where
    F: FnMut(i32, i32, i32),
{
    intersections.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mut i = 1;
    while i < intersections.len() {
        let x_start = round_up(f64::from(intersections[i - 1]));
        let x_end = round_down(f64::from(intersections[i]));
        if x_end >= 0 && x_start < width {
            let x_start = x_start.max(0);
            let x_end = x_end.min(width - 1);
            if x_start <= x_end {
                visit(x_start, x_end, y);
            }
        }
        i += 2;
    }
}

pub(crate) fn for_each_polygon_fill_span<F>(
    points: &[(i32, i32)],
    img_w: u32,
    img_h: u32,
    mut visit: F,
) where
    F: FnMut(i32, i32, i32),
{
    if points.len() < 2 {
        return;
    }
    let edges = polygon_edges(points);
    if edges.is_empty() {
        return;
    }
    let (width, height) = (img_w as i32, img_h as i32);
    draw_horizontal_edges(&edges, width, height, &mut visit);

    let global_ymin = edges
        .iter()
        .map(|edge| edge.ymin)
        .min()
        .unwrap_or(i32::MAX)
        .max(0);
    // Pillow clamps to ysize and visits that sentinel row so final-row corners
    // still take the duplicate-intersection path. The hline callback discards it.
    let global_ymax = edges
        .iter()
        .map(|edge| edge.ymax)
        .max()
        .unwrap_or(i32::MIN)
        .min(height);
    if global_ymin > global_ymax {
        return;
    }
    let edge_table: Vec<&ScanEdge> = edges.iter().filter(|edge| edge.ymin != edge.ymax).collect();
    if edge_table.is_empty() {
        return;
    }
    let mut intersections = Vec::with_capacity(edge_table.len() * 2);
    for y in global_ymin..=global_ymax {
        scanline_intersections(&edge_table, y, global_ymax, &mut intersections);
        if intersections.is_empty() || y >= height {
            continue;
        }
        fill_scanline_pairs(&mut intersections, y, width, &mut visit);
    }
}

pub(crate) fn scanline_polygon_fill<C: DrawCanvas>(
    canvas: &mut C,
    points: &[(i32, i32)],
    color: (u8, u8, u8, u8),
    img_w: u32,
    img_h: u32,
    _raw: bool,
) {
    let rgba = [color.0, color.1, color.2, color.3];
    for_each_polygon_fill_span(points, img_w, img_h, |x_start, x_end, y| {
        for x in x_start..=x_end {
            canvas.put_rgba(x as u32, y as u32, rgba);
        }
    });
}
