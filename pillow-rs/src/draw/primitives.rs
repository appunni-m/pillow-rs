use super::{
    Draw, DrawBoxInput, DrawPointsInput, RegularPolygonCircle, RegularPolygonSides,
    normalize_draw_point_input, normalize_draw_points, normalize_rounded_rectangle_box,
};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Draw {
    /// Draws a line from `(x0, y0)` to `(x1, y1)`.
    ///
    /// `fill` is an RGBA byte tuple and `width` is the stroke width in pixels.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn line(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: (u8, u8, u8, u8),
        width: u32,
    ) -> Result<(), PilError> {
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawLine {
                x0,
                y0,
                x1,
                y1,
                fill,
                width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws consecutive line segments through `points`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when fewer than two points are given.
    /// Deferred pipeline execution reports materialization failures later.
    pub fn polyline(
        &mut self,
        points: &[(i32, i32)],
        fill: (u8, u8, u8, u8),
        width: u32,
    ) -> Result<(), PilError> {
        if points.len() < 2 {
            return Err(PilError::ValueError(
                "wrong number of coordinates".to_owned(),
            ));
        }
        for segment in points.windows(2) {
            self.line(
                segment[0].0,
                segment[0].1,
                segment[1].0,
                segment[1].1,
                fill,
                width,
            )?;
        }
        Ok(())
    }

    /// Normalizes and draws a Python-facing line coordinate sequence.
    pub fn polyline_with_input(
        &mut self,
        input: DrawPointsInput,
        fill: (u8, u8, u8, u8),
        width: u32,
    ) -> Result<(), PilError> {
        self.polyline_with_input_joint(input, fill, width, None)
    }

    /// Normalizes and draws a Python-facing line, including Pillow's optional
    /// rounded-joint treatment for thick polylines.
    pub fn polyline_with_input_joint(
        &mut self,
        input: DrawPointsInput,
        fill: (u8, u8, u8, u8),
        width: u32,
        joint: Option<&str>,
    ) -> Result<(), PilError> {
        let points = normalize_draw_points(input, true)?;
        if points.len() < 2 {
            return Ok(());
        }
        self.polyline(&points, fill, width)?;

        // This mirrors ImageDraw.ImageDraw.line's Python-level implementation:
        // the native draw_lines call paints the segments first, then curve
        // joints are filled with pieslices. Widths up to four intentionally
        // retain the native line-only behavior.
        if joint != Some("curve") || width <= 4 {
            return Ok(());
        }

        let width_f = f64::from(width);
        for window in points.windows(3) {
            let [start, point, end] = [window[0], window[1], window[2]];
            let angles = [
                f64::from(point.0 - start.0)
                    .atan2(f64::from(start.1 - point.1))
                    .to_degrees()
                    .rem_euclid(360.0),
                f64::from(end.0 - point.0)
                    .atan2(f64::from(point.1 - end.1))
                    .to_degrees()
                    .rem_euclid(360.0),
            ];
            if angles[0] == angles[1] {
                continue;
            }

            let flipped = (angles[1] > angles[0] && angles[1] - 180.0 > angles[0])
                || (angles[1] < angles[0] && angles[1] + 180.0 > angles[0]);
            let coords = (
                (f64::from(point.0) - width_f / 2.0 + 1.0) as i32,
                (f64::from(point.1) - width_f / 2.0 + 1.0) as i32,
                (f64::from(point.0) + width_f / 2.0 - 1.0) as i32,
                (f64::from(point.1) + width_f / 2.0 - 1.0) as i32,
            );
            let (start_angle, end_angle) = if flipped {
                (angles[1] + 90.0, angles[0] + 90.0)
            } else {
                (angles[0] - 90.0, angles[1] - 90.0)
            };
            self.pieslice(
                coords.0,
                coords.1,
                coords.2,
                coords.3,
                start_angle - 90.0,
                end_angle - 90.0,
                Some(fill),
                None,
                1,
            )?;

            if width > 8 {
                let distance = width_f / 2.0 - 1.0;
                let coord_at_angle = |angle: f64| {
                    let angle = (angle - 90.0).to_radians();
                    let dx = distance * angle.cos();
                    let dy = distance * angle.sin();
                    let adjust = |coordinate: f64, delta: f64| {
                        coordinate
                            + if delta > 0.0 {
                                delta.floor()
                            } else {
                                delta.ceil()
                            }
                    };
                    (
                        adjust(f64::from(point.0), dx) as i32,
                        adjust(f64::from(point.1), dy) as i32,
                    )
                };
                let gap_points = if flipped {
                    vec![
                        coord_at_angle(angles[0] + 90.0),
                        point,
                        coord_at_angle(angles[1] + 90.0),
                    ]
                } else {
                    vec![
                        coord_at_angle(angles[0] - 90.0),
                        point,
                        coord_at_angle(angles[1] - 90.0),
                    ]
                };
                self.polyline(&gap_points, fill, 3)?;
            }
        }
        Ok(())
    }

    /// Draws a rectangle bounded by `(x0, y0, x1, y1)`.
    ///
    /// `fill` paints the interior when present. `outline` paints the border
    /// when present. `width` controls outline thickness in pixels.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the right or bottom edge precedes
    /// the corresponding left or top edge, matching Pillow's public draw
    /// validation before the operation is queued.
    pub fn rectangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        if x1 < x0 {
            return Err(PilError::ValueError(
                "x1 must be greater than or equal to x0".into(),
            ));
        }
        if y1 < y0 {
            return Err(PilError::ValueError(
                "y1 must be greater than or equal to y0".into(),
            ));
        }
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawRectangle {
                x0,
                y0,
                x1,
                y1,
                fill,
                outline,
                width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws an ellipse inside `(x0, y0, x1, y1)`.
    ///
    /// Fill, outline, and width follow Pillow `ImageDraw.ellipse` semantics.
    /// The backend uses Pillow's Bresenham-style quarter-ellipse algorithm.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn ellipse(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        // Pillow's _imaging ellipse dispatch rejects a reversed bounding box
        // before reaching ImagingDrawEllipse; keep that public validation in
        // core so every binding reports the same error.
        if x1 < x0 {
            return Err(PilError::ValueError(
                "x1 must be greater than or equal to x0".into(),
            ));
        }
        if y1 < y0 {
            return Err(PilError::ValueError(
                "y1 must be greater than or equal to y0".into(),
            ));
        }
        let (fill, outline) = self.shape_inks(fill, outline);
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawEllipse {
                x0,
                y0,
                x1,
                y1,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws a polygon from integer vertices.
    ///
    /// Fewer than three points is a no-op. `fill` paints the interior and
    /// `outline` paints the boundary when present.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn polygon(
        &mut self,
        points: &[(i32, i32)],
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline);
        if points.len() < 2 {
            return Ok(());
        }
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawPolygon {
                points: points.to_vec().into(),
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Normalizes and draws a Python-facing polygon coordinate sequence.
    pub fn polygon_with_input(
        &mut self,
        input: DrawPointsInput,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let points = normalize_draw_points(input, false)?;
        self.polygon(&points, fill, outline, width)
    }

    /// Normalize and draw a public `ImageDraw.shape` point sequence.
    pub fn shape_with_input(
        &mut self,
        input: DrawPointsInput,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
    ) -> Result<(), PilError> {
        let points = normalize_draw_point_input(input)?;
        self.shape(&points, fill, outline)
    }

    /// Draws a regular polygon from Pillow's bounding-circle representation.
    ///
    /// Vertex generation, Pillow's two-decimal rounding, and side-count
    /// validation live in core so every binding uses the same geometry.
    pub fn regular_polygon(
        &mut self,
        bounding_circle: RegularPolygonCircle,
        n_sides: RegularPolygonSides,
        rotation: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let n_sides = match n_sides {
            RegularPolygonSides::Value(value) if value > 2 => usize::try_from(value)
                .map_err(|_| PilError::ValueError("n_sides should be an int > 2".into()))?,
            RegularPolygonSides::Value(_) | RegularPolygonSides::Invalid => {
                return Err(PilError::ValueError("n_sides should be an int > 2".into()));
            }
        };
        let (cx, cy, radius) = match bounding_circle {
            RegularPolygonCircle::Flat(x, y, radius)
            | RegularPolygonCircle::Nested(x, y, radius) => (x, y, radius),
            RegularPolygonCircle::Invalid => {
                return Err(PilError::ValueError(
                    "bounding_circle should contain 2D coordinates and a radius (e.g. (x, y, r) or ((x, y), r) )".into(),
                ));
            }
        };

        // Match PIL's _compute_regular_polygon_vertices exactly: start from
        // (radius, 0), rotate by (270 - 0.5*degrees-per-side + rotation),
        // round each coordinate to two decimals, then truncate to integers.
        let n = n_sides as f64;
        let degrees_per_side = 360.0 / n;
        let start_angle = 270.0 - 0.5 * degrees_per_side + rotation;
        let mut points = Vec::with_capacity(n_sides);
        for index in 0..n_sides {
            let angle = start_angle + degrees_per_side * index as f64;
            let angle = if angle > 360.0 { angle - 360.0 } else { angle };
            let theta = (360.0 - angle).to_radians();
            let x = ((radius * theta.cos() + cx) * 100.0).round() / 100.0;
            let y = ((radius * theta.sin() + cy) * 100.0).round() / 100.0;
            points.push((x as i32, y as i32));
        }
        self.polygon(&points, fill, outline, width)
    }

    /// Fills a closed outline using Pillow's `ImageDraw.shape` ink order.
    ///
    /// Pillow draws `fill` first and `outline` last, but its outline primitive
    /// fills the complete path. Therefore `outline`, when present, is the
    /// effective color for the whole shape.
    pub fn shape(
        &mut self,
        points: &[(i32, i32)],
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
    ) -> Result<(), PilError> {
        // Pillow's ImageDraw._getink() supplies the context's default ink
        // when both arguments are omitted. This is deliberately shape-only:
        // the experimental API calls draw_outline directly, and its default
        // differs from the wrapper's ordinary color parser.
        let Some(ink) = outline.or(fill).or_else(|| self.default_shape_ink()) else {
            return Ok(());
        };
        self.polygon(points, Some(ink), None, 1)
    }

    /// Draws one or more individual points.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn point(&mut self, points: &[(i32, i32)], fill: (u8, u8, u8, u8)) -> Result<(), PilError> {
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawPoint {
                points: points.to_vec().into(),
                fill,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Normalizes and draws a Python-facing point coordinate sequence.
    pub fn point_with_input(
        &mut self,
        input: DrawPointsInput,
        fill: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let points = normalize_draw_point_input(input)?;
        self.point(&points, fill)
    }

    /// Normalizes and draws a Python-facing rounded-rectangle box.
    pub fn rounded_rectangle_with_input(
        &mut self,
        input: DrawBoxInput,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let (x0, y0, x1, y1) = normalize_rounded_rectangle_box(input)?;
        self.rounded_rectangle(x0, y0, x1, y1, radius, fill, outline, width)
    }
}
