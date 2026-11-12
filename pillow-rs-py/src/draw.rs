use crate::font::PyFont;
use crate::*;
use pyo3::types::PyInt;
use pyo3::{Py, Python, pyclass, pymethods};

pub(super) mod custom_font_mask;
mod floodfill;
mod input;
mod regular_polygon;
mod rounded_rectangle;
mod text_lines;

use input::{
    draw_bitmap_position_from_python, draw_box_input_from_python,
    draw_circle_center_input_from_python, draw_points_input_from_python,
};

fn draw_line_with_input(
    draw: &mut pillow_rs::Draw,
    xy: &Bound<'_, PyAny>,
    color: (u8, u8, u8, u8),
    width: &Bound<'_, PyAny>,
    joint: Option<&str>,
) -> PyResult<i32> {
    let width = c_int_from_python(width)?;
    let points = draw_points_input_from_python(xy)?;
    draw.polyline_with_input_joint(points, color, width.max(0) as u32, joint)
        .map_err(map_error)?;
    Ok(width)
}
pub(crate) use input::{
    draw_color_input_from_python, draw_features_presence_from_python, draw_text_input_from_python,
};

pub(super) mod text_layout;

// --- ImageDraw ---

#[pyclass(name = "Outline")]
pub struct PyOutline {
    points: Vec<(i32, i32)>,
}

#[pymethods]
impl PyOutline {
    #[new]
    fn new() -> Self {
        Self { points: Vec::new() }
    }

    #[pyo3(name = "move")]
    fn move_to(&mut self, x: i32, y: i32) {
        self.points.clear();
        self.points.push((x, y));
    }

    fn line(&mut self, x: i32, y: i32) {
        self.points.push((x, y));
    }

    fn curve(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) -> PyResult<()> {
        let (x0, y0) = self.points.last().copied().ok_or_else(|| {
            pyo3::exceptions::PyValueError::new_err("outline has no current point")
        })?;
        self.points.extend(pillow_rs::outline_curve_points(
            &[x0 as f64, y0 as f64, x1, y1, x2, y2, x3, y3],
            32,
        ));
        Ok(())
    }

    fn close(&mut self) {
        if self.points.len() > 2 && self.points.first() != self.points.last() {
            self.points.push(self.points[0]);
        }
    }
}

#[pyclass(name = "ImageDraw")]
pub struct PyDraw {
    draw: pillow_rs::Draw,
}

#[pyclass(name = "_LineCurveJointPlan")]
struct PyLineCurveJointPlan {
    plan: pillow_rs::DrawCurveJointPlan,
}

#[pymethods]
impl PyLineCurveJointPlan {
    fn _angles(&self) -> (f64, f64) {
        self.plan.angles()
    }

    fn _gap_angle(&self, first: bool) -> f64 {
        self.plan.gap_angle(first)
    }
}

#[pymethods]
impl PyDraw {
    #[staticmethod]
    #[pyo3(signature = (pixel, x, y, value, border=None, thresh=0, native_image=None))]
    fn _floodfill(
        py: Python<'_>,
        pixel: &Bound<'_, PyAny>,
        x: &Bound<'_, PyAny>,
        y: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
        border: Option<&Bound<'_, PyAny>>,
        thresh: i32,
        native_image: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let threshold = PyInt::new(py, thresh);
        floodfill::execute(
            py,
            pixel,
            x,
            y,
            value,
            border,
            threshold.as_any(),
            native_image,
        )
    }

    #[new]
    #[pyo3(signature = (image, mode=None))]
    fn new(image: &Bound<'_, PyImage>, mode: Option<&Bound<'_, PyAny>>) -> PyResult<Self> {
        // Pillow's ImageDraw constructor loads the target before validating an
        // explicit drawing mode. Keep direct calls to this binding ordered the
        // same way as the Python facade; accept the raw object so PyO3 does not
        // coerce its type before that load.
        image.call_method0("load")?;
        let mode = mode
            .map(|mode| {
                let mode = mode.extract::<String>().map_err(|_| {
                    map_error(pillow_rs::PilError::ValueError("mode mismatch".to_owned()))
                })?;
                pillow_rs::Mode::from_pillow_name(&mode).ok_or_else(|| {
                    map_error(pillow_rs::PilError::ValueError("mode mismatch".to_owned()))
                })
            })
            .transpose()?;
        let borrowed = image.borrow();
        let draw = pillow_rs::Draw::new(borrowed.inner.clone(), mode);
        draw.validate_mode().map_err(map_error)?;
        Ok(PyDraw { draw })
    }

    fn _set_palette(&mut self, data: Vec<u8>, mode: &str) -> PyResult<()> {
        self.draw.set_palette_data(&data, mode).map_err(map_error)
    }

    fn _validate_embedded_color(&self) -> PyResult<()> {
        self.draw
            .validate_text_options(&pillow_rs::ImageFontTextOptions {
                embedded_color: true,
                ..pillow_rs::ImageFontTextOptions::default()
            })
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, fill=None, width=0, joint=None))]
    fn line(
        &mut self,
        py: Python<'_>,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        width: i32,
        joint: Option<String>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        let width = PyInt::new(py, width);
        draw_line_with_input(&mut self.draw, xy, color, width.as_any(), joint.as_deref())
            .map(|_| ())
    }

    fn _line_base(
        &mut self,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        width: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        draw_line_with_input(&mut self.draw, xy, color, width, None).map(|_| ())
    }

    fn _line_curve_joint_plan(
        &self,
        py: Python<'_>,
        angles: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyLineCurveJointPlan>>> {
        let angles = angles.extract::<Vec<f64>>()?;
        let Ok(angles) = <[f64; 2]>::try_from(angles) else {
            return Ok(None);
        };
        let Some(plan) = self.draw.curve_joint_plan_from_angles(angles) else {
            return Ok(None);
        };
        Py::new(py, PyLineCurveJointPlan { plan }).map(Some)
    }

    #[pyo3(signature = (xy, fill=None, outline=None, width=1))]
    fn rectangle(
        &mut self,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: i32,
    ) -> PyResult<()> {
        let fill_color = if let Some(_f) = fill {
            Some(self.color(fill)?)
        } else {
            None
        };
        let out_color = if let Some(_o) = outline {
            Some(self.color(outline)?)
        } else {
            None
        };
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)?).map_err(map_error)?;
        self.draw
            .rectangle(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                fill_color,
                out_color,
                width.max(0) as u32,
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, fill=None, outline=None, width=1))]
    fn ellipse(
        &mut self,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: i32,
    ) -> PyResult<()> {
        let fill_color = if let Some(_f) = fill {
            Some(self.color(fill)?)
        } else {
            None
        };
        let out_color = if let Some(_o) = outline {
            Some(self.color(outline)?)
        } else {
            None
        };
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)?).map_err(map_error)?;
        self.draw
            .ellipse(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                fill_color,
                out_color,
                width.max(0) as u32,
            )
            .map_err(map_error)
    }

    fn bitmap(
        &mut self,
        xy: &Bound<'_, PyAny>,
        bitmap: &Bound<'_, PyImage>,
        fill: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        // The Python facade loads before resolving palette ink. Keep direct
        // calls to this lower-level binding on the same error and mutation
        // order; PyImage.load releases the GIL while materializing.
        bitmap.call_method0("load")?;
        let fill = fill.map(|value| self.color(Some(value))).transpose()?;
        // Pillow flattens one point from `xy` after resolving ink.
        // Keep flat and single-nested coordinate pairs on that path.
        let (x, y) = draw_bitmap_position_from_python(xy)?;
        let bmp = bitmap.borrow();
        self.draw.bitmap(x, y, &bmp.inner, fill).map_err(map_error)
    }

    fn text_ink(&mut self, fill: Option<&Bound<'_, PyAny>>) -> PyResult<i64> {
        self.draw
            .text_ink_with_input(draw_color_input_from_python(fill)?)
            .map_err(map_error)
    }

    fn draw_ink(&mut self, fill: &Bound<'_, PyAny>) -> PyResult<i32> {
        self.draw
            .draw_ink_with_input(draw_color_input_from_python(Some(fill))?)
            .map_err(map_error)
    }

    fn text_fill_from_ink(&self, py: Python<'_>, ink: i32) -> PyResult<Py<PyAny>> {
        match self
            .draw
            .text_fill_from_ink(i64::from(ink))
            .map_err(map_error)?
        {
            pillow_rs::DrawTextFillValue::Integer(value) => value.into_py_any(py),
            pillow_rs::DrawTextFillValue::Float(value) => value.into_py_any(py),
        }
    }

    #[pyo3(name = "_draw_text_lines")]
    #[allow(clippy::too_many_arguments)]
    fn draw_text_lines(
        slf: Py<Self>,
        py: Python<'_>,
        owner: &Bound<'_, PyAny>,
        host_helpers_fn: &Bound<'_, PyAny>,
        float_fn: &Bound<'_, PyAny>,
        xy: &Bound<'_, PyAny>,
        image_text: &Bound<'_, PyAny>,
        text: &Bound<'_, PyAny>,
        font: &Bound<'_, PyAny>,
        fill: &Bound<'_, PyAny>,
        stroke_fill: &Bound<'_, PyAny>,
        fill_ink: i64,
        stroke_ink: Option<i64>,
        stroke_width: &Bound<'_, PyAny>,
        anchor: &Bound<'_, PyAny>,
        align: &Bound<'_, PyAny>,
        direction: &Bound<'_, PyAny>,
        features: &Bound<'_, PyAny>,
        language: &Bound<'_, PyAny>,
        embedded_color: &Bound<'_, PyAny>,
        args: &Bound<'_, PyAny>,
        mask_kwargs: &Bound<'_, PyAny>,
        native_text_input: bool,
    ) -> PyResult<()> {
        // Custom-font callbacks may re-enter this PyDraw through owner.bitmap.
        // The callback adapter takes short borrows only for direct Rust draws.
        let mode = {
            let draw = slf.borrow(py);
            draw.draw.draw_mode_identity().map_err(map_error)?
        };
        text_lines::draw_text_lines(
            py,
            slf,
            mode,
            owner,
            host_helpers_fn,
            float_fn,
            xy,
            image_text,
            text,
            font,
            fill,
            stroke_fill,
            fill_ink,
            stroke_ink,
            stroke_width,
            anchor,
            align,
            direction,
            features,
            language,
            embedded_color,
            args,
            mask_kwargs,
            native_text_input,
        )
    }

    fn text_color_mask(
        &mut self,
        xy: (i32, i32),
        color: &Bound<'_, PyImage>,
        coverage: &Bound<'_, PyImage>,
    ) -> PyResult<()> {
        text_color_mask_impl(&mut self.draw, xy, color, coverage)
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
    #[pyo3(signature = (bounding_circle, n_sides, rotation))]
    fn regular_polygon_vertices(
        &self,
        bounding_circle: &Bound<'_, PyAny>,
        n_sides: &Bound<'_, PyAny>,
        rotation: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<(i32, i32)>> {
        let (circle, sides, rotation) = regular_polygon::regular_polygon_inputs(
            n_sides.py(),
            bounding_circle,
            n_sides,
            rotation,
        )?;
        self.draw
            .regular_polygon_vertices(circle, sides, rotation)
            .map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
    fn polygon(
        &mut self,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fill_color = if let Some(_f) = fill {
            Some(self.color(fill)?)
        } else {
            None
        };
        let out_color = if let Some(_o) = outline {
            Some(self.color(outline)?)
        } else {
            None
        };
        self.draw
            .polygon_with_input(
                draw_points_input_from_python(xy)?,
                fill_color,
                out_color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pending
    #[pyo3(name = "_prepare_polygon_mask_input")]
    fn prepare_polygon_mask_input(&self, xy: &Bound<'_, PyAny>) -> PyResult<()> {
        self.draw
            .prepare_polygon_mask_input(draw_points_input_from_python(xy)?)
            .map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pending
    #[pyo3(name = "_polygon_outline")]
    fn polygon_outline(
        &mut self,
        xy: &Bound<'_, PyAny>,
        outline: &Bound<'_, PyAny>,
        width: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let width = c_int_from_python(width)?;
        let outline = Some(self.color(Some(outline))?);
        self.draw
            .polygon_outline_with_input(draw_points_input_from_python(xy)?, outline, width)
            .map_err(map_error)
    }

    fn point(&mut self, xy: &Bound<'_, PyAny>, fill: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let color = self.color(fill)?;
        self.draw
            .point_with_input(draw_points_input_from_python(xy)?, color)
            .map_err(map_error)
    }

    fn shape(
        &mut self,
        shape: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let points = if let Ok(mut rust_outline) = shape.extract::<PyRefMut<'_, PyOutline>>() {
            rust_outline.close();
            rust_outline.points.clone()
        } else {
            let fill = fill.map(|_| self.color(fill)).transpose()?;
            let outline = outline.map(|_| self.color(outline)).transpose()?;
            let input = draw_points_input_from_python(shape)?;
            return self
                .draw
                .shape_with_input(input, fill, outline)
                .map_err(map_error);
        };
        let fill = fill.map(|_| self.color(fill)).transpose()?;
        let outline = outline.map(|_| self.color(outline)).transpose()?;
        self.draw.shape(&points, fill, outline).map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, width=1))]
    fn arc(
        &mut self,
        xy: &Bound<'_, PyAny>,
        start: f64,
        end: f64,
        fill: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)?).map_err(map_error)?;
        self.draw
            .arc(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                start,
                end,
                color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, outline=None, width=1))]
    fn chord(
        &mut self,
        xy: &Bound<'_, PyAny>,
        start: f64,
        end: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: i32,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill)).transpose()?;
        let oc = outline.map(|_| self.color(outline)).transpose()?;
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)?).map_err(map_error)?;
        self.draw
            .chord(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                start,
                end,
                fc,
                oc,
                width.max(0) as u32,
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, outline=None, width=1))]
    fn pieslice(
        &mut self,
        xy: &Bound<'_, PyAny>,
        start: f64,
        end: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: i32,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill)).transpose()?;
        let oc = outline.map(|_| self.color(outline)).transpose()?;
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)?).map_err(map_error)?;
        self.draw
            .pieslice(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                start,
                end,
                fc,
                oc,
                width.max(0) as u32,
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, radius, fill=None, outline=None, width=1))]
    fn circle(
        &mut self,
        xy: &Bound<'_, PyAny>,
        radius: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill)).transpose()?;
        let oc = outline.map(|_| self.color(outline)).transpose()?;
        self.draw
            .circle_with_input(
                draw_circle_center_input_from_python(xy),
                radius,
                fc,
                oc,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    /// Compute text bounding box. Loads default FreeType font if font is None.
    /// Returns (left, top, right, bottom).
    #[pyo3(signature = (xy, text, font=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false, font_size=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow ImageDraw API.
    fn textbbox(
        &mut self,
        xy: &Bound<'_, PyAny>,
        text: &Bound<'_, PyAny>,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f32>,
    ) -> PyResult<(i32, i32, i32, i32)> {
        let default_font = if font.is_none() {
            Some(
                pillow_rs::FreeTypeFont::load_default(font_size.unwrap_or(10.0))
                    .map_err(map_error)?,
            )
        } else {
            None
        };
        let borrowed = font.map(|font| font.borrow());
        let font = borrowed
            .as_ref()
            .map(|font| &font.inner)
            .or(default_font.as_ref())
            .ok_or_else(|| {
                map_error(pillow_rs::PilError::InternalError(
                    "draw text font was not prepared".into(),
                ))
            })?;
        self.draw
            .validate_text_options(&pillow_rs::ImageFontTextOptions {
                embedded_color,
                ..pillow_rs::ImageFontTextOptions::default()
            })
            .map_err(map_error)?;
        let text = draw_text_input_from_python(text)?;
        let xy = xy.extract::<(i32, i32)>()?;
        let features_invalid = draw_features_presence_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features: None,
            features_invalid,
            language,
            stroke_width,
            anchor,
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_textbbox_at_with_optional_font_input(
            Some(font),
            None,
            xy,
            text,
            &options,
        )
        .map_err(map_error)
    }

    /// Compute bounding box for multiline text. Matches PIL's exact algorithm.
    #[pyo3(signature = (xy, text, font=None, spacing=4, align="left", direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false, font_size=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow ImageDraw API.
    fn multiline_textbbox(
        &mut self,
        xy: &Bound<'_, PyAny>,
        text: &Bound<'_, PyAny>,
        font: Option<&Bound<'_, PyFont>>,
        spacing: i32,
        align: &str,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f32>,
    ) -> PyResult<(i32, i32, i32, i32)> {
        let default_font = if font.is_none() {
            Some(
                pillow_rs::FreeTypeFont::load_default(font_size.unwrap_or(10.0))
                    .map_err(map_error)?,
            )
        } else {
            None
        };
        let borrowed = font.map(|font| font.borrow());
        let font = borrowed
            .as_ref()
            .map(|font| &font.inner)
            .or(default_font.as_ref())
            .ok_or_else(|| {
                map_error(pillow_rs::PilError::InternalError(
                    "draw text font was not prepared".into(),
                ))
            })?;
        self.draw
            .validate_text_options(&pillow_rs::ImageFontTextOptions {
                embedded_color,
                ..pillow_rs::ImageFontTextOptions::default()
            })
            .map_err(map_error)?;
        let text = draw_text_input_from_python(text)?;
        let xy = xy.extract::<(i32, i32)>()?;
        let features_invalid = draw_features_presence_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features: None,
            features_invalid,
            language,
            stroke_width,
            anchor,
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_multiline_textbbox_with_optional_font_input(
            Some(font),
            None,
            xy,
            text,
            spacing,
            align,
            &options,
        )
        .map_err(map_error)
    }

    #[getter]
    fn image(&self) -> PyResult<PyImage> {
        // Return a copy of the current image state
        Ok(PyImage {
            inner: self.draw_get_image()?,
        })
    }
}

pub(super) fn text_color_mask_impl(
    draw: &mut pillow_rs::Draw,
    xy: (i32, i32),
    color: &Bound<'_, PyImage>,
    coverage: &Bound<'_, PyImage>,
) -> PyResult<()> {
    color.call_method0("load")?;
    coverage.call_method0("load")?;
    let color = color.borrow();
    let coverage = coverage.borrow();
    draw.paste_text_color_mask(xy, &color.inner, &coverage.inner)
        .map_err(map_error)
}

impl PyDraw {
    fn draw_get_image(&self) -> PyResult<pillow_rs::Image> {
        self.draw.image_clone().map_err(map_error)
    }

    /// Parse a draw color, using the image mode to determine byte representation.
    fn color(&mut self, val: Option<&Bound<'_, PyAny>>) -> PyResult<(u8, u8, u8, u8)> {
        let Some(val) = val else {
            return self
                .draw
                .resolve_draw_color_with_input(pillow_rs::DrawColorInput::None)
                .map_err(map_error);
        };
        let ink = val.extract::<i32>()?;
        self.draw.color_from_draw_ink(ink).map_err(map_error)
    }
}

pub(super) fn register_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyLineCurveJointPlan>()?;
    m.add_class::<PyDraw>()?;
    m.add_class::<PyOutline>()?;
    rounded_rectangle::register_class(m)?;
    Ok(())
}
