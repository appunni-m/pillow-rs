use super::{
    Draw, DrawCircleCenterInput, normalize_draw_circle_center, validate_rounded_rect,
};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Draw {
    /// Draws an arc along an ellipse boundary.
    ///
    /// Angles are in degrees, following Pillow's coordinate convention.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn arc(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fill: (u8, u8, u8, u8),
        _width: u32,
    ) -> Result<(), PilError> {
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawArc {
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill: Some(fill),
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws a chord inside an ellipse bounding box.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn chord(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline)?;
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawChord {
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws a pieslice inside an ellipse bounding box.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn pieslice(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        start: f64,
        end: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline)?;
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawPieslice {
                x0,
                y0,
                x1,
                y1,
                start,
                end,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws a circle centered at `(cx, cy)`.
    ///
    /// `radius` is rounded to an integer pixel radius for pipeline execution.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn circle(
        &mut self,
        cx: i32,
        cy: i32,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        let (fill, outline) = self.shape_inks(fill, outline)?;
        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawCircle {
                cx,
                cy,
                radius: radius as i32,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Normalizes and draws a Python-facing circle center.
    pub fn circle_with_input(
        &mut self,
        input: DrawCircleCenterInput,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
    ) -> Result<(), PilError> {
        let (cx, cy) = normalize_draw_circle_center(input)?;
        self.circle(cx as i32, cy as i32, radius, fill, outline, width)
    }

    /// Draws a rounded rectangle.
    ///
    /// Pillow uses `2 * radius` for corner geometry and floors half that
    /// diameter for the center strips. A zero radius draws a rectangle.
    ///
    /// # Errors
    ///
    /// A negative radius that reaches a painted corner reports Pillow's
    /// invalid-box error. Deferred pipeline execution reports materialization
    /// failures.
    pub fn rounded_rectangle(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        self.rounded_rectangle_with_diameter(
            x0,
            y0,
            x1,
            y1,
            radius * 2.0,
            fill,
            outline,
            _width,
        )
    }

    /// Draws a rounded rectangle from the diameter already computed by a
    /// Pillow-facing adapter.
    #[doc(hidden)]
    pub fn rounded_rectangle_with_diameter(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        input_diameter: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        _width: u32,
    ) -> Result<(), PilError> {
        // Pillow validates the two axes before dispatching the radius-zero
        // fallback.  Without this boundary check a reversed y axis reached
        // DrawRectangle, whose defensive kernel treats it as an empty span;
        // that silently accepted an invalid public box instead of reporting
        // Pillow's axis-specific ValueError.
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
        let mut diameter = input_diameter;
        let width_span = f64::from(x1) - f64::from(x0);
        let height_span = f64::from(y1) - f64::from(y0);
        let full_x = diameter >= width_span - 1.0;
        if full_x {
            diameter = width_span;
        }
        let full_y = diameter >= height_span - 1.0;
        if full_y {
            diameter = height_span;
        }
        if full_x && full_y {
            return self.ellipse(x0, y0, x1, y1, fill, outline, _width);
        }
        if diameter == 0.0 {
            // No corner curve, just draw a rectangle.
            return self.rectangle(x0, y0, x1, y1, fill, outline, _width);
        }
        let (fill, outline) = self.shape_inks(fill, outline)?;
        if diameter < 0.0 {
            return negative_radius_result(fill, outline, _width);
        }

        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawRoundedRect {
                x0,
                y0,
                x1,
                y1,
                radius: input_diameter / 2.0,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }

    /// Draws a rounded rectangle with Pillow's per-corner selection.
    #[allow(clippy::too_many_arguments)] // Mirrors the public draw arguments and adds corner selection.
    pub fn rounded_rectangle_with_corners(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        radius: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
        corners: [bool; 4],
        all_corners: bool,
    ) -> Result<(), PilError> {
        self.rounded_rectangle_with_corners_diameter(
            x0,
            y0,
            x1,
            y1,
            radius * 2.0,
            fill,
            outline,
            width,
            corners,
            all_corners,
        )
    }

    /// Draws selected rounded-rectangle corners from an adapter-computed
    /// diameter.
    #[doc(hidden)]
    #[allow(clippy::too_many_arguments)]
    pub fn rounded_rectangle_with_corners_diameter(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        diameter: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
        corners: [bool; 4],
        all_corners: bool,
    ) -> Result<(), PilError> {
        if all_corners && corners.iter().all(|corner| *corner) {
            return self.rounded_rectangle_with_diameter(
                x0, y0, x1, y1, diameter, fill, outline, width,
            );
        }
        self.rounded_rectangle_with_partial_corners(
            x0, y0, x1, y1, diameter, fill, outline, width, corners,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn rounded_rectangle_with_partial_corners(
        &mut self,
        x0: i32,
        y0: i32,
        x1: i32,
        y1: i32,
        diameter: f64,
        fill: Option<(u8, u8, u8, u8)>,
        outline: Option<(u8, u8, u8, u8)>,
        width: u32,
        corners: [bool; 4],
    ) -> Result<(), PilError> {
        validate_rounded_rect(x0, y0, x1, y1)?;
        if !corners.iter().any(|corner| *corner) || diameter == 0.0 {
            return self.rectangle(x0, y0, x1, y1, fill, outline, width);
        }

        let (fill, outline) = self.shape_inks(fill, outline)?;
        if diameter < 0.0 {
            return negative_radius_result(fill, outline, width);
        }
        let corner_radius = (diameter / 2.0).floor() as i32;
        let parts = partial_rounded_rect_parts(x0, y0, x1, y1, diameter, corners);
        if let Some(color) = fill {
            queue_partial_rounded_rect_fill(
                self,
                x0,
                y0,
                x1,
                y1,
                corner_radius,
                corners,
                &parts,
                color,
            );
        }
        if let Some(color) = outline.filter(|color| Some(*color) != fill && width != 0) {
            queue_partial_rounded_rect_outline(
                self,
                x0,
                y0,
                x1,
                y1,
                corner_radius,
                width,
                corners,
                &parts,
                color,
            );
        }
        Ok(())
    }
}

fn negative_radius_result(
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
) -> Result<(), PilError> {
    let draws_outline = outline.is_some_and(|color| Some(color) != fill && width != 0);
    if fill.is_some() || draws_outline {
        Err(PilError::ValueError(
            "x1 must be greater than or equal to x0".into(),
        ))
    } else {
        Ok(())
    }
}

type RoundedRectPart = (i32, i32, i32, i32, f64, f64);

fn partial_rounded_rect_parts(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    diameter: f64,
    corners: [bool; 4],
) -> Vec<RoundedRectPart> {
    [
        (
            corners[0],
            (
                x0,
                y0,
                (f64::from(x0) + diameter) as i32,
                (f64::from(y0) + diameter) as i32,
                180.0,
                270.0,
            ),
        ),
        (
            corners[1],
            (
                (f64::from(x1) - diameter) as i32,
                y0,
                x1,
                (f64::from(y0) + diameter) as i32,
                270.0,
                360.0,
            ),
        ),
        (
            corners[2],
            (
                (f64::from(x1) - diameter) as i32,
                (f64::from(y1) - diameter) as i32,
                x1,
                y1,
                0.0,
                90.0,
            ),
        ),
        (
            corners[3],
            (
                x0,
                (f64::from(y1) - diameter) as i32,
                (f64::from(x0) + diameter) as i32,
                y1,
                90.0,
                180.0,
            ),
        ),
    ]
    .into_iter()
    .filter_map(|(enabled, part)| enabled.then_some(part))
    .collect()
}

fn queue_partial_rectangle(
    draw: &mut Draw,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: (u8, u8, u8, u8),
) {
    draw.image = Image::push_op(
        &draw.image,
        PipelineOp::DrawRectangle {
            x0,
            y0,
            x1,
            y1,
            fill: Some(color),
            outline: None,
            width: 1,
            alpha_blend_rgb: draw.alpha_blend_rgb(),
        },
    );
}

fn queue_partial_pieslice(draw: &mut Draw, part: RoundedRectPart, color: (u8, u8, u8, u8)) {
    draw.image = Image::push_op(
        &draw.image,
        PipelineOp::DrawPieslice {
            x0: part.0,
            y0: part.1,
            x1: part.2,
            y1: part.3,
            start: part.4,
            end: part.5,
            fill: Some(color),
            outline: None,
            width: 1,
            alpha_blend_rgb: draw.alpha_blend_rgb(),
        },
    );
}

fn queue_partial_arc(
    draw: &mut Draw,
    part: RoundedRectPart,
    color: (u8, u8, u8, u8),
    width: u32,
) {
    draw.image = Image::push_op(
        &draw.image,
        PipelineOp::DrawArc {
            x0: part.0,
            y0: part.1,
            x1: part.2,
            y1: part.3,
            start: part.4,
            end: part.5,
            fill: Some(color),
            width,
            alpha_blend_rgb: draw.alpha_blend_rgb(),
        },
    );
}

#[allow(clippy::too_many_arguments)]
fn queue_partial_rounded_rect_fill(
    draw: &mut Draw,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: i32,
    corners: [bool; 4],
    parts: &[RoundedRectPart],
    color: (u8, u8, u8, u8),
) {
    for &part in parts {
        queue_partial_pieslice(draw, part, color);
    }
    if x1 - radius - 1 >= x0 + radius + 1 {
        queue_partial_rectangle(draw, x0 + radius + 1, y0, x1 - radius - 1, y1, color);
    }
    let mut left = [x0, y0, x0 + radius, y1];
    if corners[0] {
        left[1] += radius + 1;
    }
    if corners[3] {
        left[3] -= radius + 1;
    }
    queue_partial_rectangle(draw, left[0], left[1], left[2], left[3], color);
    let mut right = [x1 - radius, y0, x1, y1];
    if corners[1] {
        right[1] += radius + 1;
    }
    if corners[2] {
        right[3] -= radius + 1;
    }
    queue_partial_rectangle(draw, right[0], right[1], right[2], right[3], color);
}

#[allow(clippy::too_many_arguments)]
fn queue_partial_rounded_rect_outline(
    draw: &mut Draw,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: i32,
    width: u32,
    corners: [bool; 4],
    parts: &[RoundedRectPart],
    color: (u8, u8, u8, u8),
) {
    for &part in parts {
        queue_partial_arc(draw, part, color, width);
    }
    let width = i32::try_from(width).unwrap_or(i32::MAX);
    let mut top = [x0, y0, x1, y0 + width - 1];
    if corners[0] {
        top[0] += radius + 1;
    }
    if corners[1] {
        top[2] -= radius + 1;
    }
    queue_partial_rectangle(draw, top[0], top[1], top[2], top[3], color);
    let mut bottom = [x0, y1 - width + 1, x1, y1];
    if corners[3] {
        bottom[0] += radius + 1;
    }
    if corners[2] {
        bottom[2] -= radius + 1;
    }
    queue_partial_rectangle(draw, bottom[0], bottom[1], bottom[2], bottom[3], color);

    let mut left = [x0, y0, x0 + width - 1, y1];
    if corners[0] {
        left[1] += radius + 1;
    }
    if corners[3] {
        left[3] -= radius + 1;
    }
    queue_partial_rectangle(draw, left[0], left[1], left[2], left[3], color);
    let mut right = [x1 - width + 1, y0, x1, y1];
    if corners[1] {
        right[1] += radius + 1;
    }
    if corners[2] {
        right[3] -= radius + 1;
    }
    queue_partial_rectangle(draw, right[0], right[1], right[2], right[3], color);
}
