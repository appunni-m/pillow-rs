use crate::draw::{
    custom_font_mask, draw_color_input_from_python, draw_features_presence_from_python,
    draw_text_input_from_python, text_layout, PyDraw,
};
use crate::font::PyFont;
use crate::map_error;
use pillow_rs::{DrawTextLine, DrawTextLinesCallbacks, Mode, PilError};
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyIterator, PyString};

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_multiline_text_impl<'py>(
    draw: &mut pillow_rs::Draw,
    xy: (f64, f64),
    text: &Bound<'py, PyAny>,
    fill: Option<&Bound<'py, PyAny>>,
    font: Option<&Bound<'py, PyFont>>,
    spacing: Option<i32>,
    direction: Option<String>,
    features: Option<&Bound<'py, PyAny>>,
    language: Option<String>,
    stroke_width: f32,
    anchor: Option<String>,
    embedded_color: bool,
    font_size: Option<f32>,
    align: Option<String>,
    stroke_fill: Option<&Bound<'py, PyAny>>,
) -> PyResult<()> {
    let default_font = if font.is_none() {
        Some(
            pillow_rs::FreeTypeFont::load_default(font_size.unwrap_or(10.0))
                .map_err(map_error)?,
        )
    } else {
        None
    };
    draw.validate_text_options(&pillow_rs::ImageFontTextOptions {
        embedded_color,
        ..pillow_rs::ImageFontTextOptions::default()
    })
    .map_err(map_error)?;
    let color = draw
        .resolve_text_color_with_input(draw_color_input_from_python(fill)?)
        .map_err(map_error)?;
    let stroke_fill = if stroke_width != 0.0 {
        stroke_fill
            .map(|value| {
                draw.resolve_text_color_with_input(draw_color_input_from_python(Some(value))?)
                    .map_err(map_error)
            })
            .transpose()?
    } else {
        None
    };
    let text = draw_text_input_from_python(text)?;
    let features_invalid = draw_features_presence_from_python(features);
    let options = pillow_rs::ImageFontTextOptions {
        direction,
        embedded_color,
        features: None,
        features_invalid,
        language,
        stroke_width,
        stroke_fill,
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
            map_error(PilError::InternalError(
                "draw text font was not prepared".into(),
            ))
        })?;
    draw.multiline_text_with_optional_font_input(
        xy.0,
        xy.1,
        text,
        Some(font),
        color,
        f64::from(spacing.unwrap_or(4)),
        None,
        &options,
    )
    .map_err(map_error)
}

struct PythonDrawTextLinesCallbacks<'py> {
    py: Python<'py>,
    py_draw: Py<PyDraw>,
    mode: Mode,
    owner: Bound<'py, PyAny>,
    fill_fontmode: Option<Bound<'py, PyAny>>,
    host_helpers_fn: Bound<'py, PyAny>,
    float_fn: Bound<'py, PyAny>,
    xy: Bound<'py, PyAny>,
    image_text: Bound<'py, PyAny>,
    text: Bound<'py, PyAny>,
    font: Bound<'py, PyAny>,
    fill: Bound<'py, PyAny>,
    stroke_fill: Bound<'py, PyAny>,
    fill_ink: i64,
    stroke_ink: Option<i64>,
    stroke_width: Bound<'py, PyAny>,
    anchor: Bound<'py, PyAny>,
    align: Bound<'py, PyAny>,
    direction: Bound<'py, PyAny>,
    features: Bound<'py, PyAny>,
    language: Bound<'py, PyAny>,
    embedded_color: Bound<'py, PyAny>,
    args: Bound<'py, PyAny>,
    mask_kwargs: Bound<'py, PyAny>,
    native_text_input: bool,
}

struct PythonLineIterator<'py> {
    source: Bound<'py, PyAny>,
    iterator: Option<Bound<'py, PyIterator>>,
}

impl<'py> PythonDrawTextLinesCallbacks<'py> {
    fn custom_mask(
        &mut self,
        line: &DrawTextLine<Bound<'py, PyAny>>,
        default_mode: &str,
        mode: &Bound<'py, PyAny>,
        fill: &Bound<'_, PyAny>,
        ink: i64,
    ) -> PyResult<()> {
        let host_helpers = self.host_helpers_fn.call0()?;
        let image_class = host_helpers.get_item(0)?;
        let getattr_fn = host_helpers.get_item(1)?;
        let isinstance_fn = host_helpers.get_item(2)?;
        let int_fn = host_helpers.get_item(3)?;
        let modf_fn = host_helpers.get_item(4)?;
        custom_font_mask::draw_custom_font_mask(
            self.py,
            self.py_draw.clone(),
            &self.owner,
            &image_class,
            &getattr_fn,
            &isinstance_fn,
            &int_fn,
            &modf_fn,
            &line.value,
            &self.image_text,
            fill,
            default_mode,
            &self.direction,
            &self.features,
            &self.language,
            mode,
            ink,
            &self.args,
            &self.mask_kwargs,
        )
    }
}

impl<'py> DrawTextLinesCallbacks for PythonDrawTextLinesCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Lines = PythonLineIterator<'py>;
    type Error = PyErr;

    fn uses_direct_route(&mut self) -> PyResult<bool> {
        text_layout::uses_direct_text_route(
            self.py,
            self.mode,
            &self.owner,
            &self.xy,
            &self.image_text,
            &self.font,
            &self.stroke_width,
            &self.anchor,
            &self.align,
            &self.direction,
            &self.features,
            &self.language,
            &self.embedded_color,
            &self.args,
            &self.mask_kwargs,
            self.native_text_input,
        )
    }

    fn draw_direct(&mut self) -> PyResult<()> {
        let rust_font = self.font.getattr("_rust_font")?;
        let spacing = self.image_text.getattr("spacing")?;
        let stroke_width = self.float_fn.call1((self.stroke_width.clone(),))?;
        let xy = self.xy.extract::<(f64, f64)>()?;
        let font = rust_font.cast::<PyFont>()?;
        let spacing = optional_i32(&spacing)?;
        let direction = optional_string(&self.direction)?;
        let features = optional_value(&self.features);
        let language = optional_string(&self.language)?;
        let stroke_width = stroke_width.extract::<f32>()?;
        let anchor = optional_string(&self.anchor)?;
        let embedded_color = self.embedded_color.extract::<bool>()?;
        let align = optional_string(&self.align)?;
        let stroke_fill = optional_value(&self.stroke_fill);
        let mut draw = self.py_draw.borrow_mut(self.py);
        draw_multiline_text_impl(
            &mut draw.draw,
            xy,
            &self.text,
            optional_value(&self.fill),
            Some(font),
            spacing,
            direction,
            features,
            language,
            stroke_width,
            anchor,
            embedded_color,
            None,
            align,
            stroke_fill,
        )
    }

    fn split_lines(&mut self) -> PyResult<Self::Lines> {
        let source = self
            .image_text
            .call_method1("_split", (&self.xy, &self.anchor, &self.align))?;
        Ok(PythonLineIterator {
            source,
            iterator: None,
        })
    }

    fn next_line(
        &mut self,
        lines: &mut Self::Lines,
    ) -> PyResult<Option<DrawTextLine<Self::Value>>> {
        if lines.iterator.is_none() {
            lines.iterator = Some(PyIterator::from_object(&lines.source)?);
        }
        let iterator = lines
            .iterator
            .as_mut()
            .ok_or_else(|| {
                pyo3::exceptions::PyRuntimeError::new_err("line iterator was not initialized")
            })?;
        let Some(line) = iterator.next() else {
            return Ok(None);
        };
        Ok(Some(DrawTextLine { value: line? }))
    }

    fn embedded_color_is_truthy(&mut self) -> PyResult<bool> {
        self.fill_fontmode = Some(self.owner.getattr("fontmode")?);
        self.embedded_color.is_truthy()
    }

    fn draw_custom_stroke(
        &mut self,
        line: &DrawTextLine<Self::Value>,
        mode: &str,
    ) -> PyResult<()> {
        let fill = if self.stroke_fill.is_none() {
            self.fill.clone()
        } else {
            self.stroke_fill.clone()
        };
        let fontmode = self.owner.getattr("fontmode")?;
        let stroke_ink = self.stroke_ink.unwrap_or(self.fill_ink);
        self.custom_mask(line, mode, &fontmode, &fill, stroke_ink)
    }

    fn draw_custom_fill(
        &mut self,
        line: &DrawTextLine<Self::Value>,
        _mode: &str,
        embedded_color: bool,
    ) -> PyResult<()> {
        let fontmode = self.fill_fontmode.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "fill font mode was not prepared before the text mask",
            )
        })?;
        let mode = if embedded_color {
            PyString::new(self.py, "RGBA").into_any()
        } else {
            fontmode
        };
        let fill = self.fill.clone();
        let fill_ink = self.fill_ink;
        self.custom_mask(line, _mode, &mode, &fill, fill_ink)
    }

}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_text_lines<'py>(
    py: Python<'py>,
    py_draw: Py<PyDraw>,
    mode: Mode,
    owner: &Bound<'py, PyAny>,
    host_helpers_fn: &Bound<'py, PyAny>,
    float_fn: &Bound<'py, PyAny>,
    xy: &Bound<'py, PyAny>,
    image_text: &Bound<'py, PyAny>,
    text: &Bound<'py, PyAny>,
    font: &Bound<'py, PyAny>,
    fill: &Bound<'py, PyAny>,
    stroke_fill: &Bound<'py, PyAny>,
    fill_ink: i64,
    stroke_ink: Option<i64>,
    stroke_width: &Bound<'py, PyAny>,
    anchor: &Bound<'py, PyAny>,
    align: &Bound<'py, PyAny>,
    direction: &Bound<'py, PyAny>,
    features: &Bound<'py, PyAny>,
    language: &Bound<'py, PyAny>,
    embedded_color: &Bound<'py, PyAny>,
    args: &Bound<'py, PyAny>,
    mask_kwargs: &Bound<'py, PyAny>,
    native_text_input: bool,
) -> PyResult<()> {
    let mut callbacks = PythonDrawTextLinesCallbacks {
        py,
        py_draw,
        mode,
        owner: owner.clone(),
        fill_fontmode: None,
        host_helpers_fn: host_helpers_fn.clone(),
        float_fn: float_fn.clone(),
        xy: xy.clone(),
        image_text: image_text.clone(),
        text: text.clone(),
        font: font.clone(),
        fill: fill.clone(),
        stroke_fill: stroke_fill.clone(),
        fill_ink,
        stroke_ink,
        stroke_width: stroke_width.clone(),
        anchor: anchor.clone(),
        align: align.clone(),
        direction: direction.clone(),
        features: features.clone(),
        language: language.clone(),
        embedded_color: embedded_color.clone(),
        args: args.clone(),
        mask_kwargs: mask_kwargs.clone(),
        native_text_input,
    };
    pillow_rs::draw_text_lines_with_callbacks(mode, stroke_ink, fill_ink, &mut callbacks)
}

fn optional_value<'a, 'py>(
    value: &'a Bound<'py, PyAny>,
) -> Option<&'a Bound<'py, PyAny>> {
    (!value.is_none()).then_some(value)
}

/// TODO(pillow-rs#3ptb): verify direct-route PyFont, Option<String>, and
/// out-of-range coordinate errors and conversion order against Pillow.
fn optional_string(value: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if value.is_none() {
        Ok(None)
    } else {
        value.extract().map(Some)
    }
}

fn optional_i32(value: &Bound<'_, PyAny>) -> PyResult<Option<i32>> {
    if value.is_none() {
        Ok(None)
    } else {
        value.extract().map(Some)
    }
}

