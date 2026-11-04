use crate::font::{PyFont, imagefont_text_input_from_python};
use crate::*;
use pyo3::{pyclass, pymethods};
use pyo3::types::PyTupleMethods;

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

fn draw_points_input_from_python(xy: &Bound<'_, PyAny>) -> pillow_rs::DrawPointsInput {
    if let Ok(values) = xy.extract::<Vec<i32>>() {
        return pillow_rs::DrawPointsInput::Flat(values);
    }
    if let Ok(points) = xy.extract::<Vec<Vec<i32>>>() {
        return pillow_rs::DrawPointsInput::Nested(points);
    }
    if xy.is_instance_of::<PyList>() || xy.is_instance_of::<PyTuple>() {
        return pillow_rs::DrawPointsInput::InvalidSequence;
    }
    pillow_rs::DrawPointsInput::Invalid
}

fn draw_box_input_from_python(xy: &Bound<'_, PyAny>) -> pillow_rs::DrawBoxInput {
    if let Ok(points) = xy.extract::<Vec<Vec<i32>>>() {
        return pillow_rs::DrawBoxInput::Nested(points);
    }
    if let Ok(values) = xy.extract::<Vec<i32>>() {
        return pillow_rs::DrawBoxInput::Flat(values);
    }
    pillow_rs::DrawBoxInput::Invalid
}

fn draw_circle_center_input_from_python(xy: &Bound<'_, PyAny>) -> pillow_rs::DrawCircleCenterInput {
    if xy.is_instance_of::<PyInt>() && !xy.is_instance_of::<PyBool>() {
        return pillow_rs::DrawCircleCenterInput::Integer;
    }
    if xy.is_instance_of::<PyDict>() {
        return pillow_rs::DrawCircleCenterInput::Mapping;
    }
    if xy.extract::<String>().is_ok() {
        return pillow_rs::DrawCircleCenterInput::Text;
    }
    xy.extract::<Vec<f64>>().map_or(
        pillow_rs::DrawCircleCenterInput::Invalid,
        pillow_rs::DrawCircleCenterInput::Values,
    )
}

fn draw_color_input_from_python(
    val: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::DrawColorInput> {
    let Some(val) = val else {
        return Ok(pillow_rs::DrawColorInput::None);
    };
    if let Ok(value) = val.extract::<String>() {
        return Ok(pillow_rs::DrawColorInput::String(value));
    }
    if let Ok(value) = val.extract::<i64>() {
        return Ok(pillow_rs::DrawColorInput::Integer(value));
    }
    if let Ok(value) = val.extract::<f64>() {
        return Ok(pillow_rs::DrawColorInput::Float(value));
    }
    if val.cast::<PyTuple>().is_ok() {
        return Ok(match draw_integer_tuple_components(val) {
            Some((values, false)) => pillow_rs::DrawColorInput::TupleComponents(
                values
                    .into_iter()
                    .map(|component| match component {
                        pillow_rs::PaletteColorComponent::Integer(value) => value,
                        _ => unreachable!("integer tuple values use integer components"),
                    })
                    .collect(),
            ),
            Some((values, true)) => {
                pillow_rs::DrawColorInput::TupleComponentsWithWideInteger(values)
            }
            None => match draw_float_tuple_components(val) {
                Some(values) => pillow_rs::DrawColorInput::TupleFloatComponents(values),
                None => match draw_mixed_numeric_tuple_components(val) {
                    Some(values) => pillow_rs::DrawColorInput::TupleMixedComponents(values),
                    None => pillow_rs::DrawColorInput::InvalidTuple,
                },
            },
        });
    }
    if val.cast::<PyList>().is_ok() {
        return Ok(val.extract::<Vec<i64>>().map_or(
            pillow_rs::DrawColorInput::InvalidList,
            pillow_rs::DrawColorInput::ListComponents,
        ));
    }
    Ok(pillow_rs::DrawColorInput::Invalid)
}

/// Keeps arbitrary-size Python integers on the integer-tuple route.
///
/// Their exact magnitude preserves `ImagePalette.colors` key equality after a
/// failed byte conversion without narrowing Python's integer value.
fn draw_integer_tuple_components(
    val: &Bound<'_, PyAny>,
) -> Option<(Vec<pillow_rs::PaletteColorComponent>, bool)> {
    if let Ok(values) = val.extract::<Vec<i64>>() {
        return Some((
            values
                .into_iter()
                .map(pillow_rs::PaletteColorComponent::Integer)
                .collect(),
            false,
        ));
    }

    let tuple = val.cast::<PyTuple>().ok()?;
    let mut values = Vec::with_capacity(tuple.len());
    let mut contains_wide_integer = false;
    for component in tuple.iter() {
        if !component.is_instance_of::<PyInt>() {
            return None;
        }
        let value = crate::inputs::palette_integer_component_from_python(&component)?;
        contains_wide_integer |= matches!(
            &value,
            pillow_rs::PaletteColorComponent::WideInteger { .. }
        );
        values.push(value);
    }
    Some((values, contains_wide_integer))
}

/// Preserves Python float tuple values for palette equality and error ordering.
fn draw_float_tuple_components(val: &Bound<'_, PyAny>) -> Option<Vec<f64>> {
    let tuple = val.cast::<PyTuple>().ok()?;
    let mut values = Vec::with_capacity(tuple.len());
    for component in tuple.iter() {
        if !component.is_instance_of::<PyFloat>() {
            return None;
        }
        values.push(component.extract::<f64>().ok()?);
    }
    Some(values)
}

/// Retains integer/float kinds so P palette errors follow tuple byte order.
fn draw_mixed_numeric_tuple_components(
    val: &Bound<'_, PyAny>,
) -> Option<Vec<pillow_rs::PaletteColorComponent>> {
    let tuple = val.cast::<PyTuple>().ok()?;
    let mut values = Vec::with_capacity(tuple.len());
    let mut contains_integer = false;
    let mut contains_float = false;
    for component in tuple.iter() {
        if component.is_instance_of::<PyInt>() {
            contains_integer = true;
            values.push(crate::inputs::palette_integer_component_from_python(&component)?);
        } else if component.is_instance_of::<PyFloat>() {
            contains_float = true;
            values.push(pillow_rs::PaletteColorComponent::Float(
                component.extract::<f64>().ok()?,
            ));
        } else {
            return None;
        }
    }
    (contains_integer && contains_float).then_some(values)
}

pub(super) fn draw_features_from_python(
    features: Option<&Bound<'_, PyAny>>,
) -> (Option<Vec<String>>, bool) {
    let Some(features) = features else {
        return (None, false);
    };
    match features.extract::<Vec<String>>() {
        Ok(values) => (Some(values), false),
        Err(_) => (None, true),
    }
}

#[pymethods]
impl PyDraw {
    #[new]
    #[pyo3(signature = (image, mode=None))]
    fn new(image: &Bound<'_, PyImage>, mode: Option<String>) -> PyResult<Self> {
        // Pillow's ImageDraw constructor loads the target before validating an
        // explicit drawing mode. Keep direct calls to this binding ordered the
        // same way as the Python facade.
        image.call_method0("load")?;
        let mode = mode
            .map(|mode| {
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

    fn set_palette_reserved_indices(&mut self, indices: Vec<u32>) {
        self.draw.set_palette_reserved_indices(&indices);
    }

    fn _set_palette(&mut self, data: Vec<u8>, mode: &str) -> PyResult<()> {
        self.draw
            .set_palette_data(&data, mode)
            .map_err(map_error)
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
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
        joint: Option<String>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        let points = draw_points_input_from_python(xy);
        let w = pillow_rs::normalize_draw_width(width);
        self.draw
            .polyline_with_input_joint(points, color, w, joint.as_deref())
            .map_err(map_error)
    }

    fn rectangle(
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
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
        self.draw
            .rectangle(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                fill_color,
                out_color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    fn ellipse(
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
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
        self.draw
            .ellipse(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                fill_color,
                out_color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    fn bitmap(
        &mut self,
        xy: (f64, f64),
        bitmap: &Bound<'_, PyImage>,
        fill: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        // The Python facade loads before resolving palette ink. Keep direct
        // calls to this lower-level binding on the same error and mutation
        // order; PyImage.load releases the GIL while materializing.
        bitmap.call_method0("load")?;
        let fill_input = draw_color_input_from_python(fill)?;
        let bmp = bitmap.borrow();
        self.draw
            .bitmap_with_input(xy.0 as i32, xy.1 as i32, &bmp.inner, fill_input)
            .map_err(map_error)
    }

    fn text_ink(&mut self, fill: Option<&Bound<'_, PyAny>>) -> PyResult<i64> {
        self.draw
            .text_ink_with_input(draw_color_input_from_python(fill)?)
            .map_err(map_error)
    }

    fn text_color_mask(
        &mut self,
        xy: (i32, i32),
        color: &Bound<'_, PyImage>,
        coverage: &Bound<'_, PyImage>,
    ) -> PyResult<()> {
        color.call_method0("load")?;
        coverage.call_method0("load")?;
        let color = color.borrow();
        let coverage = coverage.borrow();
        self.draw
            .paste_text_color_mask(xy, &color.inner, &coverage.inner)
            .map_err(map_error)
    }

    fn regular_polygon(
        &mut self,
        bounding_circle: &Bound<'_, PyAny>,
        n_sides: &Bound<'_, PyAny>,
        rotation: Option<f64>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let circle = if let Ok((x, y, radius)) = bounding_circle.extract::<(f64, f64, f64)>() {
            pillow_rs::RegularPolygonCircle::Flat(x, y, radius)
        } else if let Ok(((x, y), radius)) = bounding_circle.extract::<((f64, f64), f64)>() {
            pillow_rs::RegularPolygonCircle::Nested(x, y, radius)
        } else {
            pillow_rs::RegularPolygonCircle::Invalid
        };
        let sides = n_sides.extract::<i64>().map_or(
            pillow_rs::RegularPolygonSides::Invalid,
            pillow_rs::RegularPolygonSides::Value,
        );
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
            .regular_polygon(
                circle,
                sides,
                rotation.unwrap_or(0.0),
                fill_color,
                out_color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

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
                draw_points_input_from_python(xy),
                fill_color,
                out_color,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    fn point(&mut self, xy: &Bound<'_, PyAny>, fill: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let color = self.color(fill)?;
        self.draw
            .point_with_input(draw_points_input_from_python(xy), color)
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
            let input = draw_points_input_from_python(shape);
            let fill = fill.map(|_| self.color(fill)).transpose()?;
            let outline = outline.map(|_| self.color(outline)).transpose()?;
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
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
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
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill)).transpose()?;
        let oc = outline.map(|_| self.color(outline)).transpose()?;
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
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
                pillow_rs::normalize_draw_width(width),
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
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill)).transpose()?;
        let oc = outline.map(|_| self.color(outline)).transpose()?;
        let xy =
            pillow_rs::normalize_draw_box(draw_box_input_from_python(xy)).map_err(map_error)?;
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
                pillow_rs::normalize_draw_width(width),
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

    #[pyo3(signature = (xy, radius=0.0, fill=None, outline=None, width=1))]
    fn rounded_rectangle(
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
            .rounded_rectangle_with_input(
                draw_box_input_from_python(xy),
                radius,
                fc,
                oc,
                pillow_rs::normalize_draw_width(width),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, text, fill=None, font=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false, stroke_fill=None, font_size=None, has_args=false, has_kwargs=false))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow ImageDraw API.
    fn text(
        &mut self,
        xy: (f64, f64),
        text: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        embedded_color: bool,
        stroke_fill: Option<&Bound<'_, PyAny>>,
        font_size: Option<f32>,
        has_args: bool,
        has_kwargs: bool,
    ) -> PyResult<()> {
        let default_font = if font.is_none() {
            Some(
                pillow_rs::FreeTypeFont::load_default(font_size.unwrap_or(10.0))
                    .map_err(map_error)?,
            )
        } else {
            None
        };
        self.draw
            .validate_text_options(&pillow_rs::ImageFontTextOptions {
                embedded_color,
                ..pillow_rs::ImageFontTextOptions::default()
            })
            .map_err(map_error)?;
        let (color, text_fill_is_integer) = self.text_color_and_kind(fill)?;
        let stroke_fill = if stroke_width != 0.0 {
            stroke_fill
                .map(|value| self.text_color_and_kind(Some(value)))
                .transpose()?
        } else {
            None
        };
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            stroke_width,
            stroke_fill: stroke_fill.map(|(color, _)| color),
            text_fill_is_integer,
            stroke_fill_is_integer: stroke_fill.is_some_and(|(_, is_integer)| is_integer),
            has_args,
            has_kwargs,
            anchor,
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
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
            .text_with_options_input(
                xy.0 as i32,
                xy.1 as i32,
                imagefont_text_input_from_python(text)?,
                font,
                color,
                &options,
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, text, fill=None, font=None, spacing=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false, font_size=None, align=None, stroke_fill=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow ImageDraw API.
    fn multiline_text(
        &mut self,
        xy: (f64, f64),
        text: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        font: Option<&Bound<'_, PyFont>>,
        spacing: Option<i32>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        embedded_color: bool,
        font_size: Option<f32>,
        align: Option<String>,
        stroke_fill: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let default_font = if font.is_none() {
            Some(
                pillow_rs::FreeTypeFont::load_default(font_size.unwrap_or(10.0))
                    .map_err(map_error)?,
            )
        } else {
            None
        };
        self.draw
            .validate_text_options(&pillow_rs::ImageFontTextOptions {
                embedded_color,
                ..pillow_rs::ImageFontTextOptions::default()
            })
            .map_err(map_error)?;
        let (color, text_fill_is_integer) = self.text_color_and_kind(fill)?;
        let stroke_fill = if stroke_width != 0.0 {
            stroke_fill
                .map(|value| self.text_color_and_kind(Some(value)))
                .transpose()?
        } else {
            None
        };
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            stroke_width,
            stroke_fill: stroke_fill.map(|(color, _)| color),
            text_fill_is_integer,
            stroke_fill_is_integer: stroke_fill.is_some_and(|(_, is_integer)| is_integer),
            anchor,
            align: Some(align.unwrap_or_else(|| "left".into())),
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
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
            .multiline_text_with_optional_font_input(
                xy.0,
                xy.1,
                imagefont_text_input_from_python(text)?,
                Some(font),
                color,
                f64::from(spacing.unwrap_or(4)),
                None,
                &options,
            )
            .map_err(map_error)
    }

    /// Compute text bounding box. Loads default FreeType font if font is None.
    /// Returns (left, top, right, bottom).
    #[pyo3(signature = (xy, text, font=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false, font_size=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow ImageDraw API.
    fn textbbox(
        &mut self,
        xy: (i32, i32),
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
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            stroke_width,
            anchor,
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        self.draw
            .validate_text_options(&options)
            .map_err(map_error)?;
        let borrowed = font.map(|font| font.borrow());
        pillow_rs::imagefont_textbbox_at_with_optional_font_input(
            borrowed.as_ref().map(|font| &font.inner),
            font_size,
            xy,
            imagefont_text_input_from_python(text)?,
            &options,
        )
        .map_err(map_error)
    }

    /// Compute text length in pixels. Loads default FreeType font if font is None.
    #[pyo3(signature = (text, font=None, direction=None, features=None, language=None, embedded_color=false, font_size=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow ImageDraw API.
    fn textlength(
        &mut self,
        text: &Bound<'_, PyAny>,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        embedded_color: bool,
        font_size: Option<f32>,
    ) -> PyResult<f64> {
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        self.draw
            .validate_text_options(&options)
            .map_err(map_error)?;
        let borrowed = font.map(|font| font.borrow());
        pillow_rs::imagefont_getlength_with_optional_font_input(
            borrowed.as_ref().map(|font| &font.inner),
            font_size,
            imagefont_text_input_from_python(text)?,
            &options,
        )
        .map(|width| width as f64)
        .map_err(map_error)
    }

    /// Compute bounding box for multiline text. Matches PIL's exact algorithm.
    #[pyo3(signature = (xy, text, font=None, spacing=4, align="left", direction=None, features=None, language=None, stroke_width=0.0, anchor=None, embedded_color=false, font_size=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow ImageDraw API.
    fn multiline_textbbox(
        &mut self,
        xy: (i32, i32),
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
        let (features, features_invalid) = draw_features_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            embedded_color,
            features,
            features_invalid,
            language,
            stroke_width,
            anchor,
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        self.draw
            .validate_text_options(&options)
            .map_err(map_error)?;
        let borrowed = font.map(|font| font.borrow());
        pillow_rs::imagefont_multiline_textbbox_with_optional_font_input(
            borrowed.as_ref().map(|font| &font.inner),
            font_size,
            xy,
            imagefont_text_input_from_python(text)?,
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

impl PyDraw {
    fn draw_get_image(&self) -> PyResult<pillow_rs::Image> {
        self.draw.image_clone().map_err(map_error)
    }

    /// Parse a draw color, using the image mode to determine byte representation.
    fn color(&mut self, val: Option<&Bound<'_, PyAny>>) -> PyResult<(u8, u8, u8, u8)> {
        self.draw
            .resolve_draw_color_with_input(draw_color_input_from_python(val)?)
            .map_err(map_error)
    }

    /// Parse an ImageDraw text ink while retaining integer-versus-component origin.
    fn text_color_and_kind(
        &mut self,
        val: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<((u8, u8, u8, u8), bool)> {
        let input = draw_color_input_from_python(val)?;
        let is_integer = matches!(&input, pillow_rs::DrawColorInput::Integer(_));
        self.draw
            .resolve_text_color_with_input(input)
            .map(|color| (color, is_integer))
            .map_err(map_error)
    }
}

pub(super) fn register_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDraw>()?;
    m.add_class::<PyOutline>()?;
    Ok(())
}
