use crate::draw::draw_features_presence_from_python;
use crate::*;
use pillow_rs::PilError;
use pyo3::{pyclass, pyfunction, pymethods, wrap_pyfunction};

mod pilfont;
mod text_layout;
mod text_wrap;

use pilfont::PyPilFont;

type PythonFontBBox = (Py<PyAny>, Py<PyAny>, Py<PyAny>, Py<PyAny>);

#[pyfunction]
fn transposed_font_orientation(orientation: &Bound<'_, PyAny>) -> PyResult<Option<String>> {
    if orientation.is_none() {
        return Ok(None);
    }
    Ok(pillow_rs::normalize_transposed_font_input(
        transpose_input_from_python(orientation)?,
    ))
}

#[pyfunction]
fn transposed_font_bbox(
    bbox: (i32, i32, i32, i32),
    orientation: Option<&str>,
) -> (i32, i32, i32, i32) {
    pillow_rs::transposed_bbox(bbox, orientation)
}

#[pyfunction]
fn validate_transposed_font_length(orientation: Option<&str>) -> PyResult<()> {
    pillow_rs::validate_transposed_length(orientation).map_err(map_error)
}

#[pyfunction]
fn imagefont_text_font_mode(mode: &str, embedded_color: bool) -> &'static str {
    let mode = pillow_rs::Mode::from_pillow_name(mode)
        .unwrap_or(pillow_rs::Mode::ImageLuma8);
    pillow_rs::imagefont_text_font_mode(mode, embedded_color)
}

/// Enable a backend route for future automatic image routing.
///
/// Returns true if this call changed the backend from inactive to active.
// --- ImageFont ---

#[pyclass(name = "ImageFont", unsendable)]
pub(super) struct PyFont {
    pub(super) inner: pillow_rs::FreeTypeFont,
}

fn font_bbox_value_to_python(
    py: Python<'_>,
    value: pillow_rs::ImageFontBBoxValue,
) -> PyResult<Py<PyAny>> {
    match value {
        pillow_rs::ImageFontBBoxValue::Integer(value) => value.into_py_any(py),
        pillow_rs::ImageFontBBoxValue::Float(value) => value.into_py_any(py),
    }
}

#[pyfunction]
fn imagefont_normalize_bbox(
    py: Python<'_>,
    bbox: (f64, f64, f64, f64),
) -> PyResult<PythonFontBBox> {
    let values = pillow_rs::normalize_font_bbox(bbox);
    Ok((
        font_bbox_value_to_python(py, values[0])?,
        font_bbox_value_to_python(py, values[1])?,
        font_bbox_value_to_python(py, values[2])?,
        font_bbox_value_to_python(py, values[3])?,
    ))
}

#[pyfunction]
fn imagefont_normalize_layout_engine(
    py: Python<'_>,
    layout_engine: &Bound<'_, PyAny>,
) -> PyResult<String> {
    let value = if layout_engine.is_none() {
        None
    } else {
        layout_engine.extract::<i64>().ok()
    };
    let (name, requested_raqm) = pillow_rs::normalize_layout_engine(value);
    if requested_raqm {
        PyErr::warn(
            py,
            &py.get_type::<PyUserWarning>(),
            pyo3::ffi::c_str!(
                "Raqm layout was requested, but Raqm is not available. Falling back to basic layout."
            ),
            3,
        )?;
    }
    Ok(name.to_owned())
}

#[pyfunction]
fn transpose_from_int(value: i64) -> &'static str {
    pillow_rs::transpose_name_from_int(value)
}

/// Marshal Python's path/file-like font source into core-owned input.
/// Filesystem access and Python protocol calls stay at this host boundary;
/// source validation and font loading remain in `pillow-rs`.
fn imagefont_source_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageFontSourceInput> {
    if let Some(path) = host_path_from_python(value)? {
        let data = std::fs::read(&path)
            .map_err(|_| pyo3::exceptions::PyOSError::new_err("cannot open resource"))?;
        return Ok(pillow_rs::ImageFontSourceInput::Bytes(data));
    }
    if value.hasattr("read")? {
        let data = value.call_method0("read")?.extract::<Vec<u8>>()?;
        return Ok(pillow_rs::ImageFontSourceInput::Bytes(data));
    }
    Ok(pillow_rs::ImageFontSourceInput::Invalid)
}

/// Classify a Python font text argument before handing it to the core.
///
/// This is deliberately limited to host-object conversion: the core owns the
/// byte-text interpretation and every font operation consumes the same input
/// enum.
pub(super) fn imagefont_text_input_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageFontTextInput> {
    crate::inputs::font_text_input_from_python(value)
}

pub(super) fn imagefont_text_input_to_python(
    py: Python<'_>,
    text: pillow_rs::ImageFontTextInput,
) -> PyResult<Py<PyAny>> {
    match text {
        pillow_rs::ImageFontTextInput::Text(text) => text.into_py_any(py),
        pillow_rs::ImageFontTextInput::TextCodepoints(codepoints) => {
            let bytes = codepoints
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>();
            Ok(PyBytes::new(py, &bytes)
                .call_method1("decode", ("utf-32-le", "surrogatepass"))?
                .unbind())
        }
        pillow_rs::ImageFontTextInput::Bytes(bytes) => {
            Ok(PyBytes::new(py, &bytes).into_any().unbind())
        }
        pillow_rs::ImageFontTextInput::InvalidType => {
            Err(PyTypeError::new_err("expected string or bytes"))
        }
    }
}

fn imagefont_variation_name_input_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::ImageFontVariationNameInput> {
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(pillow_rs::ImageFontVariationNameInput::Bytes(
            bytes.as_bytes().to_vec(),
        ));
    }
    if let Ok(name) = value.extract::<String>() {
        return Ok(pillow_rs::ImageFontVariationNameInput::Text(name));
    }
    Ok(pillow_rs::ImageFontVariationNameInput::InvalidType(
        value.get_type().name()?.to_string(),
    ))
}

fn imagefont_start_from_python(value: Option<&Bound<'_, PyAny>>) -> (Option<(f64, f64)>, bool) {
    let Some(value) = value else {
        return (None, false);
    };
    match value.extract::<(f64, f64)>() {
        Ok(start) => (Some(start), false),
        Err(_) => (None, true),
    }
}

fn pilfont_text_input_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::PilFontTextInput> {
    if let Ok(text) = value.extract::<String>() {
        return Ok(pillow_rs::PilFontTextInput::Text(text));
    }
    value
        .extract::<Vec<u8>>()
        .map(pillow_rs::PilFontTextInput::Bytes)
}

fn variation_axes_to_python(
    py: Python<'_>,
    axes: Vec<pillow_rs::ImageFontVariationAxis>,
) -> PyResult<Vec<Py<PyAny>>> {
    axes.into_iter()
        .map(|axis| {
            let dict = PyDict::new(py);
            dict.set_item("minimum", axis.minimum)?;
            dict.set_item("default", axis.default)?;
            dict.set_item("maximum", axis.maximum)?;
            dict.set_item("name", PyBytes::new(py, &axis.name))?;
            Ok(dict.into())
        })
        .collect()
}

#[pymethods]
impl PyFont {
    #[staticmethod]
    #[pyo3(signature = (fp, size, index=0, encoding="", layout_engine=None))]
    fn truetype(
        fp: &Bound<'_, PyAny>,
        size: f64,
        index: usize,
        encoding: &str,
        layout_engine: Option<String>,
    ) -> PyResult<Self> {
        let options = pillow_rs::ImageFontLoadOptions {
            index: Some(index),
            encoding: (!encoding.is_empty()).then(|| encoding.to_owned()),
            layout_engine,
        };
        let source = imagefont_source_from_python(fp)?;
        let font =
            pillow_rs::imagefont_from_source(source, size as f32, &options).map_err(map_error)?;
        Ok(PyFont { inner: font })
    }

    #[staticmethod]
    #[pyo3(signature = (size=None))]
    fn load_default(size: Option<f32>) -> PyResult<Self> {
        let sz = size.unwrap_or(10.0);
        let font = pillow_rs::imagefont_load_default(sz).map_err(map_error)?;
        Ok(PyFont { inner: font })
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow font API.
    fn getbbox_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<(f32, f32, f32, f32)> {
        let text = imagefont_text_input_from_python(text)?;
        let features_invalid = draw_features_presence_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            embedded_color: false,
            direction,
            features: None,
            features_invalid,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getbbox_input_with_options(
            &self.inner,
            text,
            &options,
        )
        .map_err(map_error)
    }

    #[pyo3(signature = (text, xy, mode, embedded_color, spacing, stroke_width, anchor, align))]
    fn _image_text_split_positions(
        &self,
        py: Python<'_>,
        text: &Bound<'_, PyAny>,
        xy: (f64, f64),
        mode: &str,
        embedded_color: bool,
        spacing: f64,
        stroke_width: f32,
        anchor: Option<String>,
        align: String,
    ) -> PyResult<Vec<(f64, f64, String, Py<PyAny>)>> {
        let mode = pillow_rs::Mode::from_pillow_name(mode)
            .unwrap_or(pillow_rs::Mode::ImageLuma8);
        let options = pillow_rs::ImageFontTextOptions {
            embedded_color,
            stroke_width,
            anchor,
            align: Some(align),
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let lines = pillow_rs::imagefont_text_split_positions_input(
            &self.inner,
            mode,
            xy,
            imagefont_text_input_from_python(text)?,
            spacing,
            &options,
        )
        .map_err(map_error)?;
        lines
            .into_iter()
            .map(|line| {
                Ok((
                    line.x,
                    line.y,
                    line.anchor,
                    imagefont_text_input_to_python(py, line.text)?,
                ))
            })
            .collect()
    }

    #[pyo3(signature = (text, xy, mode, embedded_color, spacing, stroke_width, anchor, align))]
    fn _image_text_layout_bbox(
        &self,
        text: &Bound<'_, PyAny>,
        xy: (f64, f64),
        mode: &str,
        embedded_color: bool,
        spacing: f64,
        stroke_width: f32,
        anchor: Option<String>,
        align: String,
    ) -> PyResult<(f64, f64, f64, f64)> {
        let mode = pillow_rs::Mode::from_pillow_name(mode)
            .unwrap_or(pillow_rs::Mode::ImageLuma8);
        let options = pillow_rs::ImageFontTextOptions {
            embedded_color,
            stroke_width,
            anchor,
            align: Some(align),
            anchor_invalid_length_error: true,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_text_layout_bbox_input(
            &self.inner,
            mode,
            xy,
            imagefont_text_input_from_python(text)?,
            spacing,
            &options,
        )
        .map_err(map_error)
    }

    fn _image_text_get_length(
        &self,
        text: &Bound<'_, PyAny>,
        mode: &str,
        embedded_color: bool,
    ) -> PyResult<f32> {
        let text = imagefont_text_input_from_python(text)?;
        let mode = pillow_rs::Mode::from_pillow_name(mode)
            .unwrap_or(pillow_rs::Mode::ImageLuma8);
        pillow_rs::imagefont_text_length(&self.inner, mode, embedded_color, text)
            .map_err(map_error)
    }

    #[pyo3(signature = (text, width, height, mode, embedded_color, spacing, stroke_width))]
    #[allow(clippy::too_many_arguments)]
    fn _image_text_wrap(
        &self,
        py: Python<'_>,
        text: &Bound<'_, PyAny>,
        width: f64,
        height: Option<f64>,
        mode: &str,
        embedded_color: bool,
        spacing: f64,
        stroke_width: f32,
    ) -> PyResult<(Vec<Py<PyAny>>, Py<PyAny>)> {
        let text = imagefont_text_input_from_python(text)?;
        let mode = pillow_rs::Mode::from_pillow_name(mode)
            .unwrap_or(pillow_rs::Mode::ImageLuma8);
        let result = pillow_rs::imagefont_text_wrap(
            &self.inner,
            mode,
            text,
            width,
            height,
            spacing,
            stroke_width,
            embedded_color,
        )
        .map_err(map_error)?;
        let lines = result
            .lines
            .into_iter()
            .map(|line| imagefont_text_input_to_python(py, line))
            .collect::<PyResult<Vec<_>>>()?;
        let remaining = imagefont_text_input_to_python(py, result.remaining)?;
        Ok((lines, remaining))
    }

    #[pyo3(signature = (text, width, height, mode, embedded_color, spacing, stroke_width, scaling, limit))]
    #[allow(clippy::too_many_arguments)]
    fn _image_text_wrap_scaled(
        &self,
        py: Python<'_>,
        text: &Bound<'_, PyAny>,
        width: f64,
        height: f64,
        mode: &str,
        embedded_color: bool,
        spacing: f64,
        stroke_width: f32,
        scaling: &str,
        limit: f64,
    ) -> PyResult<Option<(Vec<Py<PyAny>>, Py<PyAny>, Option<Py<PyFont>>)>> {
        let text = imagefont_text_input_from_python(text)?;
        let mode = pillow_rs::Mode::from_pillow_name(mode)
            .unwrap_or(pillow_rs::Mode::ImageLuma8);
        let scaling = pillow_rs::imagefont_text_scaling(scaling, limit);
        let Some((result, font)) = pillow_rs::imagefont_text_wrap_scaled(
            &self.inner,
            mode,
            text,
            width,
            height,
            spacing,
            stroke_width,
            embedded_color,
            scaling,
        )
        .map_err(map_error)?
        else {
            return Ok(None);
        };
        let lines = result
            .lines
            .into_iter()
            .map(|line| imagefont_text_input_to_python(py, line))
            .collect::<PyResult<Vec<_>>>()?;
        let remaining = imagefont_text_input_to_python(py, result.remaining)?;
        let font = font
            .map(|inner| Py::new(py, PyFont { inner }))
            .transpose()?;
        Ok(Some((lines, remaining, font)))
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, ink=None, start=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow font API.
    fn getmask_alpha_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyImage> {
        let text = imagefont_text_input_from_python(text)?;
        let features_invalid = draw_features_presence_from_python(features);
        let (start, start_invalid) = imagefont_start_from_python(start);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features: None,
            features_invalid,
            language,
            stroke_width,
            anchor,
            ink,
            start,
            start_invalid,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let (width, height, pixels) = pillow_rs::imagefont_getmask_input_with_options(
            &self.inner,
            text,
            &options,
        )
        .map_err(map_error)?;
        let inner =
            pillow_rs::imagefont_mask_image(width, height, pixels, &options).map_err(map_error)?;
        Ok(PyImage { inner })
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, ink=None, start=None, stroke_filled=false, has_args=false, has_kwargs=false))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow font API.
    fn getmask2_image_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<&Bound<'_, PyAny>>,
        stroke_filled: bool,
        has_args: bool,
        has_kwargs: bool,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let text = imagefont_text_input_from_python(text)?;
        let features_invalid = draw_features_presence_from_python(features);
        let (start, start_invalid) = imagefont_start_from_python(start);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            embedded_color: false,
            direction,
            features: None,
            features_invalid,
            language,
            stroke_width,
            stroke_filled,
            anchor,
            anchor_invalid_length_error: false,
            ink,
            start,
            start_invalid,
            has_args,
            has_kwargs,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let (width, height, pixels, offset) = pillow_rs::imagefont_getmask2_input_with_options(
            &self.inner,
            text,
            &options,
        )
        .map_err(map_error)?;
        let image =
            pillow_rs::imagefont_mask_image(width, height, pixels, &options).map_err(map_error)?;
        Ok((PyImage { inner: image }, offset))
    }

    fn getlength(&self, text: &Bound<'_, PyAny>) -> PyResult<i32> {
        pillow_rs::imagefont_native_getlength_26dot6_input(
            &self.inner,
            imagefont_text_input_from_python(text)?,
        )
        .map_err(map_error)
    }

    fn getsize(&self, text: &Bound<'_, PyAny>) -> PyResult<((i32, i32), (i32, i32))> {
        pillow_rs::imagefont_native_getsize_input(
            &self.inner,
            imagefont_text_input_from_python(text)?,
        )
        .map_err(map_error)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, stroke_filled=false, anchor=None, ink=None, start=None))]
    fn render_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
        stroke_width: f32,
        stroke_filled: bool,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let text = imagefont_text_input_from_python(text)?;
        let features_invalid = draw_features_presence_from_python(features);
        let (start, start_invalid) = imagefont_start_from_python(start);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            embedded_color: false,
            direction,
            features: None,
            features_invalid,
            language,
            stroke_width,
            stroke_filled,
            anchor,
            anchor_invalid_length_error: false,
            ink,
            start,
            start_invalid,
            has_args: false,
            has_kwargs: false,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let (width, height, pixels, offset) = pillow_rs::imagefont_native_render_input(
            &self.inner,
            text,
            &options,
        )
        .map_err(map_error)?;
        let image =
            pillow_rs::imagefont_mask_image(width, height, pixels, &options).map_err(map_error)?;
        Ok((PyImage { inner: image }, offset))
    }

    #[getter]
    fn family(&self) -> Option<String> {
        pillow_rs::imagefont_native_face_attrs(&self.inner)
            .0
            .map(str::to_owned)
    }

    #[getter]
    fn style(&self) -> Option<String> {
        pillow_rs::imagefont_native_face_attrs(&self.inner)
            .1
            .map(str::to_owned)
    }

    #[getter]
    fn ascent(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).2
    }

    #[getter]
    fn descent(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).3
    }

    #[getter]
    fn height(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).4
    }

    #[getter]
    fn x_ppem(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).5
    }

    #[getter]
    fn y_ppem(&self) -> u32 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).6
    }

    #[getter]
    fn glyphs(&self) -> i64 {
        pillow_rs::imagefont_native_face_attrs(&self.inner).7
    }

    #[getter]
    fn font_format(&self) -> &'static str {
        pillow_rs::imagefont_font_format(&self.inner)
    }

    #[getter]
    fn is_scalable(&self) -> bool {
        pillow_rs::imagefont_is_scalable(&self.inner)
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None))]
    fn getlength_with_options(
        &self,
        text: &Bound<'_, PyAny>,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<&Bound<'_, PyAny>>,
        language: Option<String>,
    ) -> PyResult<f32> {
        let text = imagefont_text_input_from_python(text)?;
        let features_invalid = draw_features_presence_from_python(features);
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features: None,
            features_invalid,
            language,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getlength_input_with_options(
            &self.inner,
            text,
            &options,
        )
        .map_err(map_error)
    }

    fn getmetrics(&self) -> (u32, u32) {
        pillow_rs::imagefont_getmetrics(&self.inner)
    }

    fn has_variations(&self) -> bool {
        pillow_rs::imagefont_has_variations(&self.inner)
    }

    fn get_variation_axes(&self) -> PyResult<Vec<Py<PyAny>>> {
        Python::attach(|py| {
            pillow_rs::imagefont_get_variation_axes(&self.inner)
                .map_err(map_error)
                .and_then(|axes| variation_axes_to_python(py, axes))
        })
    }

    fn get_variation_names(&self) -> PyResult<Vec<Vec<u8>>> {
        pillow_rs::imagefont_get_variation_names(&self.inner).map_err(map_error)
    }

    fn getvarnames(&self) -> PyResult<Vec<Vec<u8>>> {
        pillow_rs::imagefont_native_getvarnames(&self.inner).map_err(map_error)
    }

    fn getvaraxes(&self) -> PyResult<Vec<Py<PyAny>>> {
        Python::attach(|py| {
            pillow_rs::imagefont_native_getvaraxes(&self.inner)
                .map_err(map_error)
                .and_then(|axes| variation_axes_to_python(py, axes))
        })
    }

    fn set_variation_by_name(&mut self, name: &Bound<'_, PyAny>) -> PyResult<()> {
        pillow_rs::imagefont_set_variation_by_name_input(
            &mut self.inner,
            imagefont_variation_name_input_from_python(name)?,
        )
        .map_err(map_error)
    }

    fn setvarname(&mut self, instance_index: i64) -> PyResult<()> {
        pillow_rs::imagefont_native_setvarname(&mut self.inner, instance_index).map_err(map_error)
    }

    fn set_variation_by_axes(&mut self, axes: &Bound<'_, PyAny>) -> PyResult<()> {
        let input = if axes.cast::<PyList>().is_ok() {
            axes.extract::<Vec<f32>>().map_or(
                pillow_rs::ImageFontVariationAxesInput::Invalid,
                pillow_rs::ImageFontVariationAxesInput::Values,
            )
        } else {
            pillow_rs::ImageFontVariationAxesInput::Invalid
        };
        pillow_rs::imagefont_set_variation_by_axes_input(&mut self.inner, input).map_err(map_error)
    }

    fn setvaraxes(&mut self, axes: Vec<f32>) -> PyResult<()> {
        pillow_rs::imagefont_native_setvaraxes(&mut self.inner, &axes).map_err(map_error)
    }

    #[pyo3(signature = (size=None))]
    fn font_variant(&self, size: Option<f32>) -> PyResult<Self> {
        pillow_rs::imagefont_variant(&self.inner, size)
            .map(|inner| PyFont { inner })
            .map_err(map_error)
    }

    #[pyo3(signature = (font_bytes=None, size=None, index=None, encoding=None, layout_engine=None))]
    fn font_variant_with_options(
        &self,
        font_bytes: Option<Vec<u8>>,
        size: Option<f32>,
        index: Option<usize>,
        encoding: Option<String>,
        layout_engine: Option<String>,
    ) -> PyResult<Self> {
        let options = pillow_rs::ImageFontVariantOptions {
            font_bytes,
            size,
            index,
            encoding,
            layout_engine,
        };
        pillow_rs::imagefont_variant_with_options(&self.inner, &options)
            .map(|inner| PyFont { inner })
            .map_err(map_error)
    }

    fn get_name<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        let (family, style) = pillow_rs::imagefont_getname_optional(&self.inner);
        PyTuple::new(py, [family, style])
    }

    fn get_size(&self) -> f32 {
        pillow_rs::imagefont_size(&self.inner)
    }

    fn _source_bytes(&self, py: Python<'_>) -> Py<PyBytes> {
        PyBytes::new(py, self.inner.source_bytes()).unbind()
    }
}

pub(super) fn register_classes(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyFont>()?;
    m.add_class::<PyPilFont>()?;
    Ok(())
}
pub(super) fn register_functions(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(imagefont_text_font_mode, m)?)?;
    text_layout::register_functions(m)?;
    text_wrap::register_functions(m)?;
    m.add_function(wrap_pyfunction!(transposed_font_bbox, m)?)?;
    m.add_function(wrap_pyfunction!(validate_transposed_font_length, m)?)?;
    m.add_function(wrap_pyfunction!(transposed_font_orientation, m)?)?;
    m.add_function(wrap_pyfunction!(imagefont_normalize_bbox, m)?)?;
    m.add_function(wrap_pyfunction!(imagefont_normalize_layout_engine, m)?)?;
    m.add_function(wrap_pyfunction!(transpose_from_int, m)?)?;
    Ok(())
}
