use super::{Draw, DrawCircleCenterInput, normalize_draw_circle_center};
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
        let (fill, outline) = self.shape_inks(fill, outline);
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
        let (fill, outline) = self.shape_inks(fill, outline);
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
        let (fill, outline) = self.shape_inks(fill, outline);
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
    /// `radius` is rounded to pixels. Non-positive radii or degenerate boxes
    /// fall back to a normal rectangle.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; deferred pipeline execution reports later
    /// materialization failures.
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
        let (fill, outline) = self.shape_inks(fill, outline);
        let r = radius.round() as i32;
        let mut d = r * 2;
        // ImagingDrawRoundedRectangle joins the corners before deciding
        // whether the shape is an ellipse or a rectangle.  This matters for
        // zero-height/zero-width boxes: both joined axes dispatch to the
        // ellipse primitive, while a single joined axis keeps rectangle
        // outline width semantics.
        let full_x = d >= x1 - x0 - 1;
        if full_x {
            d = x1 - x0;
        }
        let full_y = d >= y1 - y0 - 1;
        if full_y {
            d = y1 - y0;
        }
        if full_x && full_y {
            return self.ellipse(x0, y0, x1, y1, fill, outline, _width);
        }
        if d <= 0 {
            // No corner curve, just draw a rectangle.
            self.image = Image::push_op(
                &self.image,
                PipelineOp::DrawRectangle {
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
            return Ok(());
        }

        self.image = Image::push_op(
            &self.image,
            PipelineOp::DrawRoundedRect {
                x0,
                y0,
                x1,
                y1,
                radius,
                fill,
                outline,
                width: _width,
                alpha_blend_rgb: self.alpha_blend_rgb(),
            },
        );
        Ok(())
    }
}
