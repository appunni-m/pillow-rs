// AS PER DESIGN — DO NOT REMOVE: Deferred lint cleanup. See CODEBASE_AUDIT.md Fix 2.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::redundant_clone)]

use pillow_rs::PilError;
use pillow_rs::{Image as RsImage, PutDataValue};
use pyo3::ToPyObject;
use pyo3::exceptions::{
    PyAttributeError, PyOverflowError, PySystemError, PyTypeError, PyValueError,
};
use pyo3::prelude::Bound;
use pyo3::prelude::Py;
use pyo3::prelude::PyAny;
use pyo3::prelude::PyErr;
use pyo3::prelude::PyModule;
use pyo3::prelude::PyObject;
use pyo3::prelude::PyRefMut;
use pyo3::prelude::PyResult;
use pyo3::prelude::Python;
use pyo3::pyclass;
use pyo3::pyfunction;
use pyo3::pymethods;
use pyo3::pymodule;
use pyo3::types::PyAnyMethods;
use pyo3::types::PyBytes;
use pyo3::types::PyBytesMethods;
use pyo3::types::PyDict;
use pyo3::types::PyDictMethods;
use pyo3::types::PyInt;
use pyo3::types::PyList;
use pyo3::types::PyListMethods;
use pyo3::types::PyModuleMethods;
use pyo3::types::PyTuple;
use pyo3::types::PyTupleMethods;
use pyo3::types::PyType;
use pyo3::types::PyTypeMethods;
use pyo3::wrap_pyfunction;
use std::path::PathBuf;

#[pyclass(name = "Image")]
pub struct PyImage {
    inner: RsImage,
}

fn host_path_from_python(value: &Bound<'_, PyAny>) -> PyResult<Option<PathBuf>> {
    if let Ok(path) = value.extract::<PathBuf>() {
        return Ok(Some(path));
    }
    let Ok(bytes) = value.downcast::<PyBytes>() else {
        return Ok(None);
    };
    if bytes.as_bytes().contains(&0) {
        return Err(PyValueError::new_err("embedded null byte"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(Some(std::ffi::OsStr::from_bytes(bytes.as_bytes()).into()))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(bytes.as_bytes().to_vec())
            .map(PathBuf::from)
            .map(Some)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }
}

#[allow(unsafe_code)]
fn python_is_sequence(value: &Bound<'_, PyAny>) -> bool {
    // SAFETY: `Bound` guarantees a non-null, GIL-bound borrowed pointer for
    // this call. `PySequence_Check` only inspects the object's type slots and
    // neither steals a reference nor stores the pointer.
    unsafe { pyo3::ffi::PySequence_Check(value.as_ptr()) != 0 }
}

fn putdata_value_from_python(value: &Bound<'_, PyAny>, mode: &str) -> PyResult<PutDataValue> {
    if matches!(mode, "1" | "L" | "P" | "I" | "F") {
        if python_is_sequence(value) {
            return Err(PyTypeError::new_err("sequence must be flattened"));
        }
        // Pillow's numeric `_putdata` path deliberately clears conversion
        // errors after writing the sentinel returned by PyFloat_AsDouble.
        return Ok(PutDataValue::Number(value.extract::<f64>().unwrap_or(-1.0)));
    }

    if value.is_instance_of::<PyInt>() {
        return value.extract::<i64>().map(PutDataValue::Packed);
    }

    let Ok(tuple) = value.downcast::<PyTuple>() else {
        return Err(PyTypeError::new_err("color must be int or tuple"));
    };
    let tuple_len = tuple.len();
    if tuple_len == 1 {
        let packed = tuple.get_item(0)?;
        if packed.is_instance_of::<PyInt>() {
            return packed.extract::<i64>().map(PutDataValue::Packed);
        }
        if matches!(mode, "LA" | "PA") {
            return Err(PySystemError::new_err(
                "new style getargs format but argument is not a tuple",
            ));
        }
        return Err(PyTypeError::new_err(
            "color must be int, or tuple of one, three or four elements",
        ));
    }

    let valid_arity = match mode {
        "LA" | "PA" => tuple_len == 2,
        "RGB" | "RGBA" | "CMYK" | "YCbCr" | "HSV" => {
            matches!(tuple_len, 3 | 4)
        }
        _ => false,
    };
    if !valid_arity {
        let message = if matches!(mode, "LA" | "PA") {
            "color must be int, or tuple of one or two elements"
        } else {
            "color must be int, or tuple of one, three or four elements"
        };
        return Err(PyTypeError::new_err(message));
    }

    let mut components = Vec::with_capacity(tuple_len);
    components.push(tuple.get_item(0)?.extract::<i64>()? as i128);
    for index in 1..tuple_len {
        components.push(tuple.get_item(index)?.extract::<i32>()? as i128);
    }
    Ok(PutDataValue::Components(components))
}

#[pymethods]
impl PyImage {
    #[new]
    fn py_new() -> PyResult<Self> {
        // Default 1x1 RGB image for compatibility
        let img = RsImage::new(1, 1, "RGB", (0, 0, 0, 0)).map_err(map_error)?;
        Ok(PyImage { inner: img })
    }

    #[classmethod]
    #[pyo3(signature = (mode, size, color=None))]
    fn new(
        _cls: &Bound<'_, PyType>,
        mode: &str,
        size: (u32, u32),
        color: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        // Thin binding: extract Python types, delegate logic to core
        let (hex, single, rgb, rgba, la, int32_val, float_val) = if let Some(val) = color {
            (
                val.extract::<String>().ok(),
                val.extract::<u8>().ok(),
                val.extract::<(u8, u8, u8)>().ok(),
                val.extract::<(u8, u8, u8, u8)>().ok(),
                val.extract::<(u8, u8)>().ok(),
                val.extract::<i32>().ok(),
                val.extract::<f64>().ok(),
            )
        } else {
            (None, None, None, None, None, None, None)
        };
        let c = pillow_rs::resolve_new_color(
            mode,
            hex.as_deref(),
            single,
            rgb,
            rgba,
            la,
            int32_val,
            float_val,
        )
        .map_err(map_error)?;
        let img = if mode == "P" {
            if let Some(index) = single {
                RsImage::new_palette_index(size.0, size.1, index)
            } else if color.is_none() {
                RsImage::new_palette_index(size.0, size.1, 0)
            } else {
                // Preserve tuple provenance: Image::new owns tuple-color
                // palette allocation, while the resolver normalizes modes
                // that do not distinguish tuples from scalar samples.
                let tuple_color = if let Some((r, g, b, a)) = rgba {
                    if a != 255 {
                        return Err(PyValueError::new_err(
                            "cannot add non-opaque RGBA color to RGB palette",
                        ));
                    }
                    (r, g, b, a)
                } else {
                    rgb.map(|(r, g, b)| (r, g, b, 255)).unwrap_or(c)
                };
                RsImage::new(size.0, size.1, mode, tuple_color).map_err(map_error)?
            }
        } else {
            RsImage::new(size.0, size.1, mode, c).map_err(map_error)?
        };
        Ok(PyImage { inner: img })
    }

    #[classmethod]
    fn open(_cls: &Bound<'_, PyType>, fp: &Bound<'_, PyAny>) -> PyResult<Self> {
        if let Some(path) = host_path_from_python(fp)? {
            let bytes = std::fs::read(path).map_err(|error| map_error(error.into()))?;
            let img = RsImage::open_bytes(bytes).map_err(map_error)?;
            Ok(PyImage { inner: img })
        } else {
            let bytes = fp.call_method0("read")?.extract::<Vec<u8>>()?;
            let img = RsImage::open_bytes(bytes).map_err(map_error)?;
            Ok(PyImage { inner: img })
        }
    }

    fn save(&mut self, fp: &Bound<'_, PyAny>, format: Option<String>) -> PyResult<()> {
        let inferred;
        let format = if let Some(format) = format.as_deref() {
            format
        } else if let Some(path) = host_path_from_python(fp)? {
            inferred = path
                .extension()
                .and_then(|extension| extension.to_str())
                .ok_or_else(|| {
                    map_error(PilError::UnknownFormat(
                        "Cannot determine format from path".into(),
                    ))
                })?
                .to_owned();
            &inferred
        } else {
            return Err(map_error(PilError::UnknownFormat(
                "Cannot determine format from file object".into(),
            )));
        };
        let encoded = self.inner.encode(format).map_err(map_error)?;
        if let Some(path) = host_path_from_python(fp)? {
            std::fs::write(path, encoded).map_err(|error| map_error(error.into()))
        } else {
            fp.call_method1("write", (PyBytes::new(fp.py(), &encoded),))?;
            Ok(())
        }
    }

    fn resize(&self, size: (u32, u32), resample: Option<String>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .resize(size, resample.as_deref())
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn crop(&self, box_coords: (u32, u32, u32, u32)) -> PyResult<PyImage> {
        let rs = self.inner.crop(box_coords).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }
    /// Crop using PIL box format (left, top, right, bottom).
    /// Computes width and height internally.
    fn crop_box(&self, left: u32, top: u32, right: u32, bottom: u32) -> PyResult<PyImage> {
        let rs = self
            .inner
            .crop_box(left, top, right, bottom)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (angle, expand=false, fillcolor=None))]
    fn rotate(
        &self,
        angle: f64,
        expand: Option<bool>,
        fillcolor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyImage> {
        let _ = fillcolor;
        let rs = self
            .inner
            .rotate(angle, expand.unwrap_or(false), None)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn transpose(&self, method: &str) -> PyResult<PyImage> {
        let rs = self.inner.transpose(method).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (mode, matrix=None, dither=None, palette=None, colors=None))]
    fn convert(
        &self,
        mode: &str,
        matrix: Option<Vec<f64>>,
        dither: Option<String>,
        palette: Option<String>,
        colors: Option<u32>,
    ) -> PyResult<PyImage> {
        let rs = self
            .inner
            .convert(mode, matrix, dither.as_deref(), palette.as_deref(), colors)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, box_coords=None, mask=None))]
    fn paste(
        &mut self,
        im: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
        mask: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        use pillow_rs::PasteSource;
        // Thin binding: extract Python types, core handles all logic
        let is_abbreviated = box_coords.is_some_and(|b| b.downcast::<PyImage>().is_ok());
        if is_abbreviated && mask.is_some() {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "If using second argument as mask, third argument must be None",
            ));
        }
        let effective_mask = if is_abbreviated { box_coords } else { mask };

        let src_image = im
            .downcast::<PyImage>()
            .ok()
            .map(|p| p.borrow().inner.clone());
        let src_single = im
            .extract::<(u8,)>()
            .ok()
            .map(|value| value.0)
            .or_else(|| im.extract::<u8>().ok());
        let src_la = im.extract::<(u8, u8)>().ok();
        let src_rgb = im.extract::<(u8, u8, u8)>().ok();
        let src_rgba = im.extract::<(u8, u8, u8, u8)>().ok();
        let source = if let Some(img) = src_image {
            PasteSource::Image(img)
        } else if let Some((r, g, b, a)) = src_rgba {
            PasteSource::Rgba(r, g, b, a)
        } else if let Some((r, g, b)) = src_rgb {
            PasteSource::Rgb(r, g, b)
        } else if let Some((l, a)) = src_la {
            PasteSource::LumaAlpha(l, a)
        } else if let Some(value) = src_single {
            PasteSource::Scalar(value)
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "im must be Image or color",
            ));
        };

        let (parsed_region, parsed_position) = if is_abbreviated {
            (None, None)
        } else {
            (
                box_coords.and_then(|b| b.extract::<(i32, i32, i32, i32)>().ok()),
                box_coords.and_then(|b| b.extract::<(i32, i32)>().ok()),
            )
        };
        if !is_abbreviated
            && box_coords.is_some()
            && parsed_region.is_none()
            && parsed_position.is_none()
        {
            let length = if let Some(box_coords) = box_coords {
                box_coords.len()?
            } else {
                0
            };
            return Err(PyTypeError::new_err(format!(
                "argument 2 must be sequence of length 4, not {length}"
            )));
        }

        let parsed_mask = match effective_mask {
            Some(value) => match value.downcast::<PyImage>() {
                Ok(image) => Some(image.borrow().inner.clone()),
                Err(_) => {
                    let type_name = value.get_type().name()?;
                    return Err(PyAttributeError::new_err(format!(
                        "'{type_name}' object has no attribute 'load'"
                    )));
                }
            },
            None => None,
        };

        if let Some(region) = parsed_region {
            self.inner
                .paste(source, Some(region), parsed_mask.as_ref())
                .map_err(map_error)
        } else {
            self.inner
                .paste_at(source, parsed_position, parsed_mask.as_ref())
                .map_err(map_error)
        }
    }

    fn split(&self) -> PyResult<Vec<PyImage>> {
        let bands = self.inner.split().map_err(map_error)?;
        Ok(bands
            .into_iter()
            .map(|img| PyImage { inner: img })
            .collect())
    }

    fn filter(&self, filter_type: &str) -> PyResult<PyImage> {
        let rs = self.inner.filter(filter_type).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn kernel_filter(
        &self,
        kernel: Vec<f32>,
        scale: f32,
        offset: i32,
        size: u32,
    ) -> PyResult<PyImage> {
        let rs = self
            .inner
            .kernel_filter(&kernel, scale, offset, size)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn copy(&self) -> PyImage {
        PyImage {
            inner: self.inner.copy(),
        }
    }

    fn tobytes(&self) -> PyResult<Vec<u8>> {
        self.inner.tobytes().map_err(map_error)
    }

    fn tobytes_unpacked(&self) -> PyResult<Vec<u8>> {
        self.inner.tobytes_unpacked().map_err(map_error)
    }

    fn tobytes_formatted(&self, mode: &str) -> PyResult<Vec<u8>> {
        self.inner.tobytes_formatted(mode).map_err(map_error)
    }
    fn tobytes_encoded(
        &self,
        mode: &str,
        encoder_name: &str,
        args: Vec<String>,
    ) -> PyResult<Vec<u8>> {
        self.inner
            .tobytes_encoded(mode, encoder_name, &args)
            .map_err(map_error)
    }

    /// Return palette data (RGB triples) for P-mode quantized images.
    fn palette(&self) -> Option<Vec<u8>> {
        self.inner.palette()
    }
    /// Return palette trimmed of trailing zero triples, matching PIL's getpalette().
    fn getpalette_trimmed(&self) -> Option<Vec<u8>> {
        self.inner.getpalette_trimmed()
    }

    fn getpalette_rgba(&self) -> Option<Vec<u8>> {
        self.inner.getpalette_rgba()
    }

    fn palette_mode(&self) -> Option<String> {
        self.inner.palette_mode().map(str::to_owned)
    }

    fn pending_transparency_index(&self) -> Option<u8> {
        match self.inner.pending_palette_transparency() {
            Some(pillow_rs::PaletteTransparency::Index(index)) => Some(index),
            _ => None,
        }
    }

    fn pending_transparency_table(&self) -> Option<Vec<u8>> {
        match self.inner.pending_palette_transparency() {
            Some(pillow_rs::PaletteTransparency::Table(alpha)) => Some(alpha),
            _ => None,
        }
    }

    fn has_transparency_data(&self) -> bool {
        self.inner.has_transparency_data()
    }

    fn apply_transparency(&mut self) -> PyResult<()> {
        self.inner.apply_transparency().map_err(map_error)
    }

    #[pyo3(signature = (data, rawmode="RGB"))]
    fn putpalette(&mut self, data: Vec<u8>, rawmode: &str) -> PyResult<()> {
        self.inner.putpalette(&data, rawmode).map_err(map_error)
    }

    fn get_child_images(&self) -> Vec<PyImage> {
        self.inner
            .get_child_images()
            .into_iter()
            .map(|img| PyImage { inner: img })
            .collect()
    }

    fn getexif(&self) -> Vec<u8> {
        self.inner.getexif()
    }

    fn getxmp(&self) -> std::collections::HashMap<String, String> {
        self.inner.getxmp()
    }

    fn getim(&self) -> PyResult<String> {
        // Pillow exposes an `Imaging` pointer in a named PyCapsule. A pointer to
        // this Rust wrapper is not ABI-compatible with `Imaging` and could make
        // capsule consumers dereference an invalid layout, so keep this endpoint
        // visibly unsupported until a genuine compatibility layer exists.
        Err(pyo3::exceptions::PyNotImplementedError::new_err(
            "getim requires a Pillow Imaging-compatible capsule",
        ))
    }

    fn thumbnail(&mut self, size: (u32, u32), resample: Option<String>) -> PyResult<()> {
        let filter = resample
            .as_deref()
            .and_then(|s| pillow_rs::parse_resample(Some(s)).ok());
        self.inner.thumbnail(size, filter).map_err(map_error)
    }

    fn quantize(&self, colors: Option<i32>, dither: Option<bool>) -> PyResult<PyImage> {
        let colors = colors.unwrap_or(256);
        if !(1..=256).contains(&colors) {
            return Err(PyValueError::new_err("bad number of colors"));
        }
        let rs = self
            .inner
            .quantize(colors as u32, 0, None, dither.unwrap_or(true))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getbbox(&self, alpha_only: Option<bool>) -> PyResult<Option<(u32, u32, u32, u32)>> {
        self.inner
            .getbbox(alpha_only.unwrap_or(true))
            .map_err(map_error)
    }

    fn getextrema(&self) -> PyResult<Vec<(u8, u8)>> {
        self.inner.getextrema().map_err(map_error)
    }
    /// Return extrema formatted as PIL expects.
    fn getextrema_formatted(&self) -> PyResult<PyObject> {
        let raw = self.inner.getextrema().map_err(map_error)?;
        Python::with_gil(|py| {
            if raw.len() == 1 {
                Ok((raw[0].0, raw[0].1).to_object(py))
            } else {
                let tuples: Vec<PyObject> =
                    raw.iter().map(|&(a, b)| (a, b).to_object(py)).collect();
                Ok(PyTuple::new(py, tuples)?.to_object(py))
            }
        })
    }

    fn stat(&self) -> PyResult<Vec<Vec<f64>>> {
        self.inner.stat().map_err(map_error)
    }

    fn stat_formatted(&self) -> PyResult<PyObject> {
        use pillow_rs::StatValue;
        let result = self.inner.stat_formatted().map_err(map_error)?;
        Python::with_gil(|py| {
            let dict = pyo3::types::PyDict::new(py);
            macro_rules! set {
                ($key:expr, $field:ident) => {
                    let v = match &result.$field {
                        StatValue::Int(v) => v.to_object(py),
                        StatValue::Float(v) => v.to_object(py),
                        StatValue::IntList(v) => v.to_object(py),
                        StatValue::FloatList(v) => v.to_object(py),
                        StatValue::ExtremaSingle(v) => v.to_object(py),
                        StatValue::ExtremaList(v) => v.to_object(py),
                    };
                    dict.set_item($key, v)?;
                };
            }
            set!("count", count);
            set!("sum", sum);
            set!("sum2", sum2);
            set!("mean", mean);
            set!("median", median);
            set!("rms", rms);
            set!("var", var);
            set!("stddev", stddev);
            set!("extrema", extrema);
            Ok(dict.to_object(py))
        })
    }

    fn histogram(&self) -> PyResult<Vec<u32>> {
        self.inner.histogram().map_err(map_error)
    }

    fn gaussian_blur(&self, radius: Option<f64>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .gaussian_blur(radius.unwrap_or(2.0) as f32)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn unsharp_mask(
        &self,
        radius: Option<f64>,
        percent: Option<i32>,
        threshold: Option<u8>,
    ) -> PyResult<PyImage> {
        let rs = self
            .inner
            .unsharp_mask(
                radius.unwrap_or(2.0) as f32,
                percent.unwrap_or(150),
                threshold.unwrap_or(3),
            )
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn max_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .max_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn min_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .min_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn median_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .median_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn box_blur(&self, radius: Option<f64>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .box_blur(radius.unwrap_or(2.0) as f32)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn mode_filter(&self, size: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .mode_filter(size.unwrap_or(3))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn rank_filter(&self, size: Option<u32>, rank: Option<u32>) -> PyResult<PyImage> {
        let rs = self
            .inner
            .rank_filter(size.unwrap_or(3), rank.unwrap_or(0))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn color3dlut(
        &self,
        size: (u32, u32, u32),
        table: Vec<f64>,
        channels: Option<u32>,
        target_mode: Option<&str>,
    ) -> PyResult<PyImage> {
        let rs = self
            .inner
            .color3dlut(size, table, channels.unwrap_or(3), target_mode)
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getchannel(&mut self, channel: i32) -> PyResult<PyImage> {
        let rs = self.inner.getchannel(channel).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn load(&mut self) -> PyResult<()> {
        self.inner.load().map_err(map_error)
    }

    fn putalpha(&mut self, alpha: u8) -> PyResult<()> {
        self.inner.putalpha(alpha).map_err(map_error)
    }

    fn reduce(&self, factor: u32) -> PyResult<PyImage> {
        let rs = self.inner.reduce(factor).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn alpha_composite(&mut self, im: &Bound<'_, PyImage>) -> PyResult<()> {
        let source = im.borrow().inner.clone();
        self.inner
            .alpha_composite(&source, (0, 0), (0, 0))
            .map_err(map_error)
    }

    fn getcolors(&mut self, maxcolors: Option<u32>) -> PyResult<Option<Vec<(u32, Vec<u8>)>>> {
        self.inner
            .getcolors(maxcolors.unwrap_or(256))
            .map_err(map_error)
    }
    /// Return getcolors formatted as PIL expects.
    fn getcolors_formatted(&mut self, maxcolors: Option<u32>) -> PyResult<Option<PyObject>> {
        let raw = self
            .inner
            .getcolors(maxcolors.unwrap_or(256))
            .map_err(map_error)?;
        let mode = self.inner.mode().map_err(map_error)?;
        Python::with_gil(|py| match raw {
            None => Ok(None),
            Some(results) => {
                let n_bands = match mode.as_str() {
                    "L" | "1" | "P" | "I" | "F" => 1,
                    "LA" | "PA" => 2,
                    "RGB" | "YCbCr" | "HSV" => 3,
                    _ => 4,
                };
                let out = pyo3::types::PyList::empty(py);
                for (count, color) in results {
                    let color_value = if n_bands == 1 {
                        color[0].to_object(py)
                    } else {
                        PyTuple::new(py, color.iter().take(n_bands).copied())?.to_object(py)
                    };
                    let entry = PyTuple::new(py, [count.to_object(py), color_value])?;
                    out.append(entry)?;
                }
                Ok(Some(out.to_object(py)))
            }
        })
    }

    fn getdata(&mut self, band: Option<i32>) -> PyResult<Vec<u8>> {
        self.inner.getdata(band).map_err(map_error)
    }
    /// Return getdata formatted as PIL expects.
    fn getdata_formatted(&mut self, band: Option<i32>) -> PyResult<PyObject> {
        let raw = self.inner.getdata(band).map_err(map_error)?;
        let mode = self.inner.mode().map_err(map_error)?;
        Python::with_gil(|py| {
            let n_bands = match mode.as_str() {
                "L" | "1" | "P" | "I" | "F" => 1,
                "LA" | "PA" => 2,
                "RGB" | "YCbCr" | "HSV" => 3,
                _ => 4,
            };
            if n_bands == 1 {
                Ok(raw.to_object(py))
            } else {
                let out = pyo3::types::PyList::empty(py);
                for chunk in raw.chunks_exact(n_bands) {
                    out.append(PyTuple::new(py, chunk.iter().copied())?)?;
                }
                Ok(out.to_object(py))
            }
        })
    }

    fn getprojection(&mut self) -> PyResult<(Vec<u32>, Vec<u32>)> {
        self.inner.getprojection().map_err(map_error)
    }

    fn entropy(&mut self) -> PyResult<f64> {
        self.inner.entropy().map_err(map_error)
    }

    fn seek(&mut self, frame: u32) -> PyResult<()> {
        self.inner.seek(frame).map_err(map_error)
    }

    fn tell(&self) -> u32 {
        self.inner.tell()
    }

    /// Pre-built LUT: must have exactly 256 * n_bands entries (PIL requirement).
    fn point(&self, lut: Vec<u8>) -> PyResult<PyImage> {
        pillow_rs::image_eval(&self.inner, &lut)
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    /// point() with band replication derived from the Rust image mode.
    fn point_replicated(&self, lut: Vec<u8>) -> PyResult<PyImage> {
        pillow_rs::image_eval_replicated_for_image(&self.inner, &lut)
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    /// Validate pre-built LUT length before calling point.
    /// PIL: LUT must have exactly 256 * n_bands entries.
    fn point_validated(&self, lut: Vec<u8>) -> PyResult<PyImage> {
        pillow_rs::image_eval_validated(&self.inner, &lut)
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    fn effect_spread(&self, distance: u32) -> PyResult<PyImage> {
        pillow_rs::image_effect_spread(&self.inner, distance)
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    #[pyo3(signature = (size, method, data=None, resample=0, fill=1, fillcolor=None))]
    fn transform(
        &self,
        size: (u32, u32),
        method: &str,
        data: Option<Vec<f64>>,
        resample: Option<i32>,
        fill: Option<i32>,
        fillcolor: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyImage> {
        let mode = self.inner.mode().unwrap_or_else(|_| "RGB".to_string());
        let _ = (resample, fill);
        // PIL's default fill for CMYK is (0,0,0,0) — white/transparent (no ink).
        // For other modes, default fill is black (0,0,0,255).
        let default_fill = if mode == "CMYK" {
            (0, 0, 0, 0)
        } else {
            (0, 0, 0, 255)
        };
        let fill = if let Some(fc) = fillcolor {
            if let Ok((r, g, b)) = fc.extract::<(u8, u8, u8)>() {
                (r, g, b, 255)
            } else if let Ok((r, g, b, a)) = fc.extract::<(u8, u8, u8, u8)>() {
                (r, g, b, a)
            } else if let Ok(i) = fc.extract::<u8>() {
                (i, i, i, 255)
            } else {
                default_fill
            }
        } else {
            default_fill
        };

        match method {
            "AFFINE" => {
                let matrix = data.ok_or_else(|| {
                    pyo3::exceptions::PyValueError::new_err("AFFINE requires data")
                })?;
                let transformed = if mode == "P" {
                    if let Some(index) = parse_palette_transform_fill(fillcolor)? {
                        self.inner
                            .transform_affine_palette_index(size, &matrix, index)
                    } else {
                        self.inner.transform_affine(size, &matrix, fill)
                    }
                } else {
                    self.inner.transform_affine(size, &matrix, fill)
                };
                transformed.map(|i| PyImage { inner: i }).map_err(map_error)
            }
            "MESH" => {
                let mesh_data = data
                    .ok_or_else(|| pyo3::exceptions::PyValueError::new_err("MESH requires data"))?;
                self.inner
                    .transform_mesh(size, mesh_data, fill)
                    .map(|i| PyImage { inner: i })
                    .map_err(map_error)
            }
            _ => Err(pyo3::exceptions::PyNotImplementedError::new_err(format!(
                "Transform method '{}' not yet implemented",
                method
            ))),
        }
    }

    #[staticmethod]
    fn frombytes(mode: &str, size: (u32, u32), data: Vec<u8>) -> PyResult<PyImage> {
        pillow_rs::Image::frombytes(mode, size, &data)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[pyo3(signature = (dest_map, source_palette=None))]
    fn remap_palette(
        &mut self,
        dest_map: Vec<u8>,
        source_palette: Option<Vec<u8>>,
    ) -> PyResult<PyImage> {
        self.inner
            .remap_palette_with_source(&dest_map, source_palette.as_deref())
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    fn tobitmap(&mut self) -> PyResult<Vec<u8>> {
        self.inner.tobitmap().map_err(map_error)
    }

    fn effect_noise(&self, sigma: Option<f64>) -> PyResult<PyImage> {
        pillow_rs::image_effect_noise(&self.inner, sigma.unwrap_or(10.0))
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn blend(
        _cls: &Bound<'_, PyType>,
        image1: &Bound<'_, PyImage>,
        image2: &Bound<'_, PyImage>,
        alpha: f64,
    ) -> PyResult<PyImage> {
        let im1 = image1.borrow();
        let im2 = image2.borrow();
        pillow_rs::image_blend(&im1.inner, &im2.inner, alpha)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn composite(
        _cls: &Bound<'_, PyType>,
        image1: &Bound<'_, PyImage>,
        image2: &Bound<'_, PyImage>,
        mask: &Bound<'_, PyImage>,
    ) -> PyResult<PyImage> {
        let im1 = image1.borrow();
        let im2 = image2.borrow();
        let m = mask.borrow();
        pillow_rs::image_composite(&im1.inner, &im2.inner, &m.inner)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    #[classmethod]
    fn merge(_cls: &Bound<'_, PyType>, mode: &str, bands: &Bound<'_, PyAny>) -> PyResult<PyImage> {
        let mut images = Vec::new();
        for item in bands.iter()? {
            let obj = item?;
            let py_img = obj.downcast::<PyImage>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err("bands must be a sequence of Image objects")
            })?;
            images.push(py_img.borrow().inner.clone());
        }
        pillow_rs::image_merge(mode, &images)
            .map(|img| PyImage { inner: img })
            .map_err(map_error)
    }

    fn close(&self) -> PyResult<()> {
        // No-op: Rust's Drop handles cleanup
        Ok(())
    }

    fn verify(&self) -> PyResult<()> {
        self.inner.verify().map_err(map_error)
    }

    fn enhance_brightness(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| py.allow_threads(|| inner.enhance_brightness(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_contrast(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| py.allow_threads(|| inner.enhance_contrast(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_color(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| py.allow_threads(|| inner.enhance_color(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_sharpness(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::with_gil(|py| py.allow_threads(|| inner.enhance_sharpness(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getpixel(&mut self, xy: (u32, u32)) -> PyResult<(u8, u8, u8, u8)> {
        self.inner.getpixel(xy.0, xy.1).map_err(map_error)
    }

    fn getpixel_formatted(&mut self, xy: (u32, u32), mode: &str) -> PyResult<PyObject> {
        let (r, g, b, a) = self.inner.getpixel(xy.0, xy.1).map_err(map_error)?;
        Python::with_gil(|py| {
            Ok(match mode {
                "L" | "1" => r.to_object(py),
                "LA" | "PA" => (r, a).to_object(py),
                "RGB" => (r, g, b).to_object(py),
                "RGBA" => (r, g, b, a).to_object(py),
                "P" => r.to_object(py), // P mode stored as RGB; r is the palette index proxy
                _ => (r, g, b).to_object(py),
            })
        })
    }

    fn putdata(slf: &Bound<'_, Self>, data: &Bound<'_, PyAny>) -> PyResult<()> {
        Self::putdata_formatted(slf, data, 1.0, 0.0)
    }

    #[pyo3(signature = (data, scale=1.0, offset=0.0))]
    fn putdata_formatted(
        slf: &Bound<'_, Self>,
        data: &Bound<'_, PyAny>,
        scale: f64,
        offset: f64,
    ) -> PyResult<()> {
        if !python_is_sequence(data) {
            return Err(PyTypeError::new_err("argument must be a sequence"));
        }
        let entry_count = data
            .len()
            .map_err(|_| PyTypeError::new_err("argument must be a sequence"))?;

        let (width, height, mode) = {
            let image = slf.try_borrow()?;
            let (width, height) = image.inner.size().map_err(map_error)?;
            let mode = image.inner.mode().map_err(map_error)?;
            (width, height, mode)
        };
        let pixel_count = u64::from(width) * u64::from(height);
        if entry_count as u128 > u128::from(pixel_count) {
            return Err(PyTypeError::new_err("too many data entries"));
        }

        // Pillow's image8 fast path reads the underlying bytes directly, even
        // for a bytes subclass that overrides Python iteration.
        if matches!(mode.as_str(), "1" | "L" | "P") {
            if let Ok(bytes) = data.downcast::<PyBytes>() {
                let values = bytes
                    .as_bytes()
                    .iter()
                    .copied()
                    // Pillow reads the terminating NUL when a bytes subtype
                    // reports one more item than its physical payload. Extend
                    // that behavior safely for every missing item instead of
                    // following its unchecked C pointer read out of bounds.
                    .chain(std::iter::repeat(0))
                    .take(entry_count)
                    .map(|value| PutDataValue::Number(f64::from(value)))
                    .collect::<Vec<_>>();
                return slf
                    .try_borrow_mut()?
                    .inner
                    .putdata_values(&values, scale, offset)
                    .map_err(map_error);
            }
        }

        let write_item = |pixel_index, item: Bound<'_, PyAny>| -> PyResult<()> {
            // No PyImage borrow may span coercion: __index__ and __float__ can
            // re-enter this same public image and must observe earlier writes.
            let value = putdata_value_from_python(&item, &mode)?;
            slf.try_borrow_mut()?
                .inner
                .putdata_value_at(pixel_index, &value, scale, offset)
                .map_err(map_error)
        };

        // CPython's PySequence_Fast retains exact lists and tuples instead of
        // copying them. Read each exact-list item only when its pixel is due so
        // coercing an earlier item can replace a later one, as Pillow exposes.
        if let Ok(list) = data.downcast_exact::<PyList>() {
            for pixel_index in 0..entry_count {
                write_item(pixel_index, list.get_item(pixel_index)?)?;
            }
            return Ok(());
        }
        if let Ok(tuple) = data.downcast_exact::<PyTuple>() {
            for pixel_index in 0..entry_count {
                write_item(pixel_index, tuple.get_item(pixel_index)?)?;
            }
            return Ok(());
        }

        // Pillow calls PySequence_Fast before coercing any pixel. Materialize
        // generic sequence items first, map iteration failures to its fixed
        // error, then process only the count reported by the original sequence.
        let iterator = data
            .iter()
            .map_err(|_| PyTypeError::new_err("argument must be a sequence"))?;
        let mut items = Vec::new();
        for item in iterator {
            items.push(item.map_err(|_| PyTypeError::new_err("argument must be a sequence"))?);
        }

        for (pixel_index, item) in items.into_iter().take(entry_count).enumerate() {
            write_item(pixel_index, item)?;
        }
        Ok(())
    }

    fn putpixel(&mut self, xy: (u32, u32), value: &Bound<'_, PyAny>) -> PyResult<()> {
        let (r, g, b, a) = if let Ok((r, g, b)) = value.extract::<(u8, u8, u8)>() {
            (r, g, b, 255)
        } else if let Ok((r, g, b, a)) = value.extract::<(u8, u8, u8, u8)>() {
            (r, g, b, a)
        } else if let Ok(val) = value.extract::<u8>() {
            (val, val, val, 255)
        } else {
            return Err(pyo3::exceptions::PyTypeError::new_err(
                "value must be int, RGB tuple, or RGBA tuple",
            ));
        };
        self.inner
            .putpixel(xy.0, xy.1, r, g, b, a)
            .map_err(map_error)
    }
    /// Mode-aware putpixel: expands values according to PIL's per-mode semantics.
    fn putpixel_mode(&mut self, xy: (u32, u32), value: &Bound<'_, PyAny>) -> PyResult<()> {
        let mode = self.inner.mode().map_err(map_error)?;
        if let Ok(v) = value.extract::<u8>() {
            return self
                .inner
                .putpixel_mode(xy.0, xy.1, v, &mode)
                .map_err(map_error);
        }
        if let Ok((r, g, b)) = value.extract::<(u8, u8, u8)>() {
            return self
                .inner
                .putpixel(xy.0, xy.1, r, g, b, 255)
                .map_err(map_error);
        }
        if let Ok((r, g, b, a)) = value.extract::<(u8, u8, u8, u8)>() {
            return self
                .inner
                .putpixel(xy.0, xy.1, r, g, b, a)
                .map_err(map_error);
        }
        if let Ok(list) = value.extract::<Vec<u8>>() {
            let (r, g, b, a) = match list.len() {
                2 => (list[0], 0, 0, list[1]),
                3 => (list[0], list[1], list[2], 255),
                4 => (list[0], list[1], list[2], list[3]),
                _ => {
                    return Err(pyo3::exceptions::PyValueError::new_err(
                        "invalid color length",
                    ));
                }
            };
            return self
                .inner
                .putpixel(xy.0, xy.1, r, g, b, a)
                .map_err(map_error);
        }
        Err(pyo3::exceptions::PyTypeError::new_err(
            "value must be int, tuple, or list",
        ))
    }

    #[getter]
    fn size(&mut self) -> PyResult<(u32, u32)> {
        self.inner.size().map_err(map_error)
    }

    #[getter]
    fn width(&mut self) -> PyResult<u32> {
        let (w, _) = self.inner.size().map_err(map_error)?;
        Ok(w)
    }

    #[getter]
    fn height(&mut self) -> PyResult<u32> {
        let (_, h) = self.inner.size().map_err(map_error)?;
        Ok(h)
    }

    #[getter]
    fn mode(&mut self) -> PyResult<String> {
        self.inner.mode().map_err(map_error)
    }

    fn explicit_mode(&self) -> PyResult<Option<String>> {
        Ok(self.inner.explicit_mode().map(|s| s.to_string()))
    }

    #[getter]
    fn format(&self) -> Option<String> {
        self.inner.format_name()
    }

    fn __repr__(&mut self) -> String {
        match self.inner.size() {
            Ok((w, h)) => {
                let mode = self.inner.mode().unwrap_or_else(|_| "?".into());
                let fmt = self.inner.format_name().unwrap_or_else(|| "Unknown".into());
                format!("<Image size={}x{} mode={} format={}>", w, h, mode, fmt)
            }
            Err(_) => "<Image [error loading]>".into(),
        }
    }
}

fn palette_transform_fill_type_error() -> PyErr {
    PyTypeError::new_err("color must be int or single-element tuple")
}

fn clamp_palette_transform_index(value: &Bound<'_, PyAny>) -> PyResult<u8> {
    let value = value.extract::<i64>().map_err(|error| {
        if error.is_instance_of::<PyOverflowError>(value.py()) {
            PyOverflowError::new_err("int too big to convert")
        } else {
            error
        }
    })?;
    Ok(value.clamp(0, i64::from(u8::MAX)) as u8)
}

fn validate_palette_transform_color<'py>(
    values: impl IntoIterator<Item = Bound<'py, PyAny>>,
    len: usize,
    allow_single: bool,
) -> PyResult<Option<u8>> {
    let values: Vec<_> = values.into_iter().collect();
    if allow_single && len == 1 {
        let value = &values[0];
        if value.is_instance_of::<PyInt>() {
            return clamp_palette_transform_index(value).map(Some);
        }
        return Err(palette_transform_fill_type_error());
    }
    if !matches!(len, 3 | 4) || values.iter().any(|value| !value.is_instance_of::<PyInt>()) {
        return Err(palette_transform_fill_type_error());
    }

    // Pillow 12.2 ImagePalette.getcolor rejects non-opaque RGBA before
    // converting RGB components to bytes.
    if len == 4 {
        let alpha = values[3].extract::<i64>().ok();
        if alpha != Some(i64::from(u8::MAX)) {
            return Err(PyValueError::new_err(
                "cannot add non-opaque RGBA color to RGB palette",
            ));
        }
    }
    for value in &values[..3] {
        let component = value
            .extract::<i64>()
            .map_err(|_| PyValueError::new_err("bytes must be in range(0, 256)"))?;
        if !(0..=i64::from(u8::MAX)).contains(&component) {
            return Err(PyValueError::new_err("bytes must be in range(0, 256)"));
        }
    }
    // Image.transform replaces the temporary color palette with the source
    // palette, so a valid RGB/RGBA fill remains raw palette index zero.
    Ok(None)
}

fn parse_palette_transform_fill(fillcolor: Option<&Bound<'_, PyAny>>) -> PyResult<Option<u8>> {
    let Some(fillcolor) = fillcolor else {
        return Ok(None);
    };
    if fillcolor.is_instance_of::<PyInt>() {
        return clamp_palette_transform_index(fillcolor).map(Some);
    }
    if let Ok(color) = fillcolor.extract::<String>() {
        pillow_rs::parse_color_str(&color).map_err(|_| {
            let repr = fillcolor
                .repr()
                .map_or_else(|_| format!("{color:?}"), |repr| repr.to_string());
            PyValueError::new_err(format!("unknown color specifier: {repr}"))
        })?;
        return Ok(None);
    }
    if let Ok(values) = fillcolor.downcast::<PyTuple>() {
        return validate_palette_transform_color(values.iter(), values.len(), true);
    }
    if let Ok(values) = fillcolor.downcast::<PyList>() {
        return validate_palette_transform_color(values.iter(), values.len(), false);
    }
    Err(palette_transform_fill_type_error())
}

fn map_error(e: PilError) -> PyErr {
    match e {
        PilError::IOError(msg) => pyo3::exceptions::PyOSError::new_err(msg),
        PilError::OsError(msg) => pyo3::exceptions::PyOSError::new_err(msg),
        PilError::AssertionError(msg) => pyo3::exceptions::PyAssertionError::new_err(msg),
        PilError::IndexError(msg) => pyo3::exceptions::PyIndexError::new_err(msg),
        PilError::KeyError(msg) => pyo3::exceptions::PyKeyError::new_err(msg),
        PilError::UnsupportedLibraqm => {
            let message = PilError::UnsupportedLibraqm.to_string();
            pyo3::exceptions::PyKeyError::new_err(
                message
                    .strip_prefix('\'')
                    .and_then(|inner| inner.strip_suffix('\''))
                    .unwrap_or(&message)
                    .to_owned(),
            )
        }
        PilError::UnidentifiedImageError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::ValueError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::SyntaxError(msg) => pyo3::exceptions::PySyntaxError::new_err(msg),
        PilError::SystemError(msg) => pyo3::exceptions::PySystemError::new_err(msg),
        PilError::TypeError(msg) => pyo3::exceptions::PyTypeError::new_err(msg),
        PilError::ImageError(err) => pyo3::exceptions::PyException::new_err(err.to_string()),
        PilError::NotImplementedError(msg) => pyo3::exceptions::PyNotImplementedError::new_err(msg),
        PilError::UnknownFormat(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::Io(err) => pyo3::exceptions::PyOSError::new_err(err.to_string()),
        // AS PER DESIGN — DO NOT REMOVE: New error variants from Fix 9
        PilError::PaletteError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
        PilError::InternalError(msg) => pyo3::exceptions::PyException::new_err(msg),
        PilError::DimensionError(msg) => pyo3::exceptions::PyValueError::new_err(msg),
    }
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
fn resolve_array_layout(
    shape: Vec<usize>,
    typestr: &str,
    mode: Option<&str>,
) -> PyResult<(String, String, usize, usize, usize, bool)> {
    let layout = pillow_rs::resolve_array_layout(&shape, typestr, mode).map_err(map_error)?;
    Ok((
        layout.mode,
        layout.raw_mode,
        layout.width,
        layout.height,
        layout.dimensions,
        layout.mode_reinterprets_dtype,
    ))
}

/// Activate a compute backend. Returns true if the backend exists on this machine.
#[pyfunction]
fn enable_backend(name: &str) -> PyResult<bool> {
    match pillow_rs::Backend::parse(name) {
        Some(b) => pillow_rs::enable_backend(b).map_err(map_error),
        None => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown backend: {}",
            name
        ))),
    }
}

/// Deactivate a compute backend. Returns true if it was active.
#[pyfunction]
fn disable_backend(name: &str) -> PyResult<bool> {
    match pillow_rs::Backend::parse(name) {
        Some(b) => pillow_rs::disable_backend(b).map_err(map_error),
        None => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown backend: {}",
            name
        ))),
    }
}

/// List backends that exist on this machine.
#[pyfunction]
fn available_backends() -> Vec<String> {
    pillow_rs::available_backends()
        .iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect()
}

/// List currently active backends (priority order).
#[pyfunction]
fn active_backends() -> PyResult<Vec<String>> {
    Ok(pillow_rs::active_backends()
        .map_err(map_error)?
        .into_iter()
        .map(|b| format!("{:?}", b).to_lowercase())
        .collect())
}

/// Check if a specific backend is active.
#[pyfunction]
fn backend_enabled(name: &str) -> PyResult<bool> {
    match pillow_rs::Backend::parse(name) {
        Some(b) => pillow_rs::backend_enabled(b).map_err(map_error),
        None => Err(pyo3::exceptions::PyValueError::new_err(format!(
            "unknown backend: {}",
            name
        ))),
    }
}

// --- Utility functions (moved from Python to satisfy "thin wrapper" rule) ---

/// Align each scanline to a 4-byte boundary (Qt/BMP compatibility).
/// Matches PIL's `ImageQt._toqclass_helper` align8to32 padding logic.
#[pyfunction]
#[pyo3(signature = (data, width, bits_per_pixel=8))]
fn align_row_to_32(data: Vec<u8>, width: u32, bits_per_pixel: u8) -> PyResult<Vec<u8>> {
    pillow_rs::align_row_to_32(&data, width, bits_per_pixel).map_err(map_error)
}

/// Create an Image from a flat or nested list of integer pixel values.
///
/// Accepts `[0, 128, 255, …]` (flat) or `[[0, 1], [2, 3], …]` (nested rows)
/// and returns a single-row Image with the appropriate width and mode.
#[pyfunction]
fn fromarray_pixel_list(data: &Bound<'_, PyAny>, mode: Option<&str>) -> PyResult<PyImage> {
    // Try extracting as flat Vec<i32> first
    let flat: Vec<i32> = if let Ok(v) = data.extract::<Vec<i32>>() {
        v
    } else if let Ok(nested) = data.extract::<Vec<Vec<i32>>>() {
        nested.into_iter().flatten().collect()
    } else {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "fromarray_pixel_list: expected list of ints or nested list of ints",
        ));
    };

    let bytes = pillow_rs::flatten_pixel_list(&flat).map_err(map_error)?;
    let n_bands = mode.map_or(1, |m| m.len() as u32);
    let w = flat.len() as u32 / n_bands;
    if w == 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "fromarray_pixel_list: not enough pixel values for the given mode",
        ));
    }
    let effective_mode = mode.unwrap_or("L");
    pillow_rs::Image::frombytes(effective_mode, (w, 1), &bytes)
        .map(|img| PyImage { inner: img })
        .map_err(map_error)
}

/// Flatten mesh transform data (list of (bbox, quad) tuples) into a flat f64 Vec.
///
/// Accepts `[(bbox, quad), …]` where each bbox is `[x0, y0, x1, y1]` and each
/// quad is `[x0, y0, …, x3, y3]` (8 coords).  Returns `[b0,b1,b2,b3, q0,…,q7, …]`.
#[pyfunction]
fn mesh_flatten(items: Vec<(Vec<f64>, Vec<f64>)>) -> PyResult<Vec<f64>> {
    let mut flat = Vec::with_capacity(items.len() * 12);
    for (bbox, quad) in &items {
        if bbox.len() != 4 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "mesh_flatten: each bbox must have exactly 4 values [x0, y0, x1, y1]",
            ));
        }
        if quad.len() != 8 {
            return Err(pyo3::exceptions::PyValueError::new_err(
                "mesh_flatten: each quad must have exactly 8 values [x0, y0, …, x3, y3]",
            ));
        }
        flat.extend_from_slice(bbox);
        flat.extend_from_slice(quad);
    }
    Ok(flat)
}

/// Apply a Python callable to the range 0..255 to produce a LUT, then
/// replicate for `n_bands` channels.  Used by `Image.eval` and `Image.point`.
#[pyfunction]
fn make_lut(func: &Bound<'_, PyAny>, n_bands: u32) -> PyResult<Vec<u8>> {
    let mut table = Vec::with_capacity(256);
    for i in 0..256u32 {
        let result = func.call1((i,)).map_err(|e| {
            pyo3::exceptions::PyValueError::new_err(format!("LUT function failed: {}", e))
        })?;
        let v = result.extract::<i32>().map_err(|_| {
            pyo3::exceptions::PyValueError::new_err("LUT function must return an integer")
        })?;
        table.push((v & 0xFF) as u8);
    }
    if n_bands > 1 {
        table = table.repeat(n_bands as usize);
    }
    Ok(table)
}

#[pymodule]
fn _core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    pyo3_log::init();

    m.add_class::<PyImage>()?;
    m.add_class::<PyDraw>()?;
    m.add_class::<PyOutline>()?;
    m.add_class::<PyFont>()?;
    m.add_class::<PyPilFont>()?;
    m.add_function(wrap_pyfunction!(transposed_font_bbox, m)?)?;
    m.add_function(wrap_pyfunction!(validate_transposed_font_length, m)?)?;
    m.add_function(wrap_pyfunction!(resolve_array_layout, m)?)?;

    // ImageOps functions
    m.add_function(wrap_pyfunction!(ops_autocontrast, m)?)?;
    m.add_function(wrap_pyfunction!(ops_equalize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_invert, m)?)?;
    m.add_function(wrap_pyfunction!(ops_flip, m)?)?;
    m.add_function(wrap_pyfunction!(ops_mirror, m)?)?;
    m.add_function(wrap_pyfunction!(ops_posterize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_solarize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_grayscale, m)?)?;
    m.add_function(wrap_pyfunction!(ops_colorize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_contain, m)?)?;
    m.add_function(wrap_pyfunction!(ops_cover, m)?)?;
    m.add_function(wrap_pyfunction!(ops_fit, m)?)?;
    m.add_function(wrap_pyfunction!(ops_pad, m)?)?;
    m.add_function(wrap_pyfunction!(ops_scale, m)?)?;
    m.add_function(wrap_pyfunction!(ops_expand, m)?)?;
    m.add_function(wrap_pyfunction!(ops_crop_border, m)?)?;
    m.add_function(wrap_pyfunction!(exif_get_orientation, m)?)?;
    m.add_function(wrap_pyfunction!(exif_remove_orientation, m)?)?;

    // ImageChops functions
    m.add_function(wrap_pyfunction!(chops_add, m)?)?;
    m.add_function(wrap_pyfunction!(chops_subtract, m)?)?;
    m.add_function(wrap_pyfunction!(chops_multiply, m)?)?;
    m.add_function(wrap_pyfunction!(chops_screen, m)?)?;
    m.add_function(wrap_pyfunction!(chops_darker, m)?)?;
    m.add_function(wrap_pyfunction!(chops_lighter, m)?)?;
    m.add_function(wrap_pyfunction!(chops_difference, m)?)?;
    m.add_function(wrap_pyfunction!(chops_invert, m)?)?;

    // More ImageChops
    m.add_function(wrap_pyfunction!(chops_add_modulo, m)?)?;
    m.add_function(wrap_pyfunction!(chops_subtract_modulo, m)?)?;
    m.add_function(wrap_pyfunction!(chops_constant, m)?)?;
    m.add_function(wrap_pyfunction!(chops_hard_light, m)?)?;
    m.add_function(wrap_pyfunction!(chops_soft_light, m)?)?;
    m.add_function(wrap_pyfunction!(chops_overlay, m)?)?;
    m.add_function(wrap_pyfunction!(chops_logical_and, m)?)?;
    m.add_function(wrap_pyfunction!(chops_logical_or, m)?)?;
    m.add_function(wrap_pyfunction!(chops_logical_xor, m)?)?;
    m.add_function(wrap_pyfunction!(chops_offset, m)?)?;

    // ImageColor
    m.add_function(wrap_pyfunction!(getrgb, m)?)?;
    m.add_function(wrap_pyfunction!(getcolor, m)?)?;
    m.add_function(wrap_pyfunction!(palette_search, m)?)?;
    m.add_function(wrap_pyfunction!(palette_getcolor_append, m)?)?;
    m.add_function(wrap_pyfunction!(palette_getcolor_validate, m)?)?;
    m.add_function(wrap_pyfunction!(palette_save_to_file, m)?)?;
    m.add_function(wrap_pyfunction!(palette_to_text, m)?)?;

    // ImageDraw helpers
    m.add_function(wrap_pyfunction!(outline_curve, m)?)?;

    // ImageStat module helpers
    m.add_function(wrap_pyfunction!(stat_from_list, m)?)?;

    // Image module functions
    m.add_function(wrap_pyfunction!(image_merge, m)?)?;
    m.add_function(wrap_pyfunction!(image_blend, m)?)?;
    m.add_function(wrap_pyfunction!(image_composite, m)?)?;
    m.add_function(wrap_pyfunction!(image_linear_gradient, m)?)?;
    m.add_function(wrap_pyfunction!(image_radial_gradient, m)?)?;
    m.add_function(wrap_pyfunction!(image_effect_mandelbrot, m)?)?;

    // GPU functions
    m.add_function(wrap_pyfunction!(enable_backend, m)?)?;
    m.add_function(wrap_pyfunction!(disable_backend, m)?)?;
    m.add_function(wrap_pyfunction!(available_backends, m)?)?;
    m.add_function(wrap_pyfunction!(active_backends, m)?)?;
    m.add_function(wrap_pyfunction!(backend_enabled, m)?)?;

    // ImageFilter helper functions
    m.add_function(wrap_pyfunction!(color3dlut_check_size, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_new, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_generate, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_transform, m)?)?;
    m.add_function(wrap_pyfunction!(color3dlut_repr, m)?)?;
    m.add_function(wrap_pyfunction!(stat_from_list, m)?)?;
    m.add_function(wrap_pyfunction!(kernel_prepare, m)?)?;

    // Utility functions (moved from Python)
    m.add_function(wrap_pyfunction!(align_row_to_32, m)?)?;
    m.add_function(wrap_pyfunction!(fromarray_pixel_list, m)?)?;
    m.add_function(wrap_pyfunction!(mesh_flatten, m)?)?;
    m.add_function(wrap_pyfunction!(make_lut, m)?)?;

    Ok(())
}

// --- ImageFont ---

#[pyclass(name = "ImageFont", unsendable)]
pub struct PyFont {
    inner: pillow_rs::FreeTypeFont,
}

#[pymethods]
impl PyFont {
    #[staticmethod]
    fn truetype(fp: &str, size: f64) -> PyResult<Self> {
        let data = std::fs::read(fp).map_err(|e| {
            pyo3::exceptions::PyOSError::new_err(format!("Cannot read font file: {}", e))
        })?;
        let font = pillow_rs::imagefont_from_bytes(data, size as f32).map_err(map_error)?;
        Ok(PyFont { inner: font })
    }

    #[staticmethod]
    fn truetype_from_bytes(data: Vec<u8>, size: f64) -> PyResult<Self> {
        let font = pillow_rs::imagefont_from_bytes(data, size as f32).map_err(map_error)?;
        Ok(PyFont { inner: font })
    }

    #[staticmethod]
    #[pyo3(signature = (size=None))]
    fn load_default(size: Option<f32>) -> PyResult<Self> {
        let sz = size.unwrap_or(10.0);
        let font = pillow_rs::imagefont_load_default(sz).map_err(map_error)?;
        Ok(PyFont { inner: font })
    }

    fn getbbox(&self, text: &str) -> PyResult<(i32, i32, i32, i32)> {
        pillow_rs::imagefont_getbbox(&self.inner, text).map_err(map_error)
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn getbbox_with_options(
        &self,
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<(f32, f32, f32, f32)> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getbbox_with_options(&self.inner, text, &options).map_err(map_error)
    }

    fn getmask_alpha(&self, text: &str) -> PyResult<(u32, u32, Vec<u8>)> {
        pillow_rs::imagefont_getmask(&self.inner, text).map_err(map_error)
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, ink=None, start=None))]
    fn getmask_alpha_with_options(
        &self,
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<(f64, f64)>,
    ) -> PyResult<(u32, u32, Vec<u8>)> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ink,
            start,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getmask_with_options(&self.inner, text, &options).map_err(map_error)
    }

    fn get_transposed_mask_image(
        &self,
        text: &str,
        orientation: Option<&str>,
    ) -> PyResult<PyImage> {
        let (width, height, pixels) =
            pillow_rs::imagefont_get_transposed_mask(&self.inner, text, orientation)
                .map_err(map_error)?;
        let inner = RsImage::from_luma_mask(width, height, pixels).map_err(map_error)?;
        Ok(PyImage { inner })
    }

    #[pyo3(signature = (text, start=None))]
    fn getmask2_image(
        &self,
        text: &str,
        start: Option<(f64, f64)>,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let (width, height, pixels, offset) = pillow_rs::imagefont_getmask2_with_start(
            &self.inner,
            text,
            start.unwrap_or((0.0, 0.0)),
        )
        .map_err(map_error)?;
        let inner = RsImage::from_luma_mask(width, height, pixels).map_err(map_error)?;
        Ok((PyImage { inner }, offset))
    }

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None, ink=None, start=None, stroke_filled=false, has_args=false, has_kwargs=false))]
    fn getmask2_image_with_options(
        &self,
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<(f64, f64)>,
        stroke_filled: bool,
        has_args: bool,
        has_kwargs: bool,
    ) -> PyResult<(PyImage, (i32, i32))> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            stroke_filled,
            anchor,
            ink,
            start,
            has_args,
            has_kwargs,
        };
        let (width, height, pixels, offset) =
            pillow_rs::imagefont_getmask2_with_options(&self.inner, text, &options)
                .map_err(map_error)?;
        let inner = RsImage::from_luma_mask(width, height, pixels).map_err(map_error)?;
        Ok((PyImage { inner }, offset))
    }

    fn getlength(&self, text: &str) -> PyResult<i32> {
        pillow_rs::imagefont_native_getlength_26dot6(&self.inner, text).map_err(map_error)
    }

    fn getsize(&self, text: &str) -> PyResult<((i32, i32), (i32, i32))> {
        pillow_rs::imagefont_native_getsize(&self.inner, text).map_err(map_error)
    }

    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None, stroke_width=0.0, stroke_filled=false, anchor=None, ink=None, start=None))]
    fn render_with_options(
        &self,
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        stroke_filled: bool,
        anchor: Option<String>,
        ink: Option<i64>,
        start: Option<(f64, f64)>,
    ) -> PyResult<(u32, u32, Vec<u8>, (i32, i32))> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            stroke_width,
            stroke_filled,
            anchor,
            ink,
            start,
            has_args: false,
            has_kwargs: false,
        };
        pillow_rs::imagefont_native_render(&self.inner, text, &options).map_err(map_error)
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

    #[pyo3(signature = (text, mode=None, direction=None, features=None, language=None))]
    fn getlength_with_options(
        &self,
        text: &str,
        mode: Option<String>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
    ) -> PyResult<f32> {
        let options = pillow_rs::ImageFontTextOptions {
            mode,
            direction,
            features,
            language,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        pillow_rs::imagefont_getlength_with_options(&self.inner, text, &options).map_err(map_error)
    }

    fn getmetrics(&self) -> (u32, u32) {
        pillow_rs::imagefont_getmetrics(&self.inner)
    }

    fn has_variations(&self) -> bool {
        pillow_rs::imagefont_has_variations(&self.inner)
    }

    fn get_variation_axes(&self) -> PyResult<Vec<(i32, i32, i32, Vec<u8>)>> {
        pillow_rs::imagefont_get_variation_axes(&self.inner)
            .map(|axes| {
                axes.into_iter()
                    .map(|axis| (axis.minimum, axis.default, axis.maximum, axis.name))
                    .collect()
            })
            .map_err(map_error)
    }

    fn get_variation_names(&self) -> PyResult<Vec<Vec<u8>>> {
        pillow_rs::imagefont_get_variation_names(&self.inner).map_err(map_error)
    }

    fn getvarnames(&self) -> PyResult<Vec<Vec<u8>>> {
        pillow_rs::imagefont_native_getvarnames(&self.inner).map_err(map_error)
    }

    fn getvaraxes(&self) -> PyResult<Vec<PyObject>> {
        Python::with_gil(|py| {
            pillow_rs::imagefont_native_getvaraxes(&self.inner)
                .map(|axes| {
                    axes.into_iter()
                        .map(|axis| {
                            let dict = PyDict::new(py);
                            dict.set_item("minimum", axis.minimum).expect("dict set");
                            dict.set_item("default", axis.default).expect("dict set");
                            dict.set_item("maximum", axis.maximum).expect("dict set");
                            dict.set_item("name", PyBytes::new(py, &axis.name))
                                .expect("dict set");
                            dict.into()
                        })
                        .collect()
                })
                .map_err(map_error)
        })
    }

    fn set_variation_by_name(&mut self, name: Vec<u8>) -> PyResult<()> {
        pillow_rs::imagefont_set_variation_by_name(&mut self.inner, &name).map_err(map_error)
    }

    fn setvarname(&mut self, instance_index: i64) -> PyResult<()> {
        pillow_rs::imagefont_native_setvarname(&mut self.inner, instance_index).map_err(map_error)
    }

    fn set_variation_by_axes(&mut self, axes: Vec<f32>) -> PyResult<()> {
        pillow_rs::imagefont_set_variation_by_axes(&mut self.inner, &axes).map_err(map_error)
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

    fn get_name(&self) -> (Option<String>, Option<String>) {
        let (family, style) = pillow_rs::imagefont_getname(&self.inner);
        (family.map(ToOwned::to_owned), style.map(ToOwned::to_owned))
    }

    fn get_size(&self) -> f32 {
        pillow_rs::imagefont_size(&self.inner)
    }
}

#[pyclass(name = "PilFont", unsendable)]
pub struct PyPilFont {
    inner: pillow_rs::PilFont,
    file: Option<String>,
}

#[pymethods]
impl PyPilFont {
    #[staticmethod]
    fn load(filename: &str) -> PyResult<Self> {
        let (inner, file) =
            load_pilfont_from_path(std::path::Path::new(filename)).map_err(map_error)?;
        Ok(Self {
            inner,
            file: Some(file),
        })
    }

    #[staticmethod]
    fn load_path(py: Python<'_>, filename: &str) -> PyResult<Self> {
        let directories = py
            .import("sys")?
            .getattr("path")?
            .extract::<Vec<String>>()?;
        for directory in directories {
            let path = std::path::Path::new(&directory).join(filename);
            match load_pilfont_from_path(&path) {
                Ok((inner, file)) => {
                    return Ok(Self {
                        inner,
                        file: Some(file),
                    });
                }
                Err(error) if pilfont_load_path_catches(&error) => {}
                Err(error) => return Err(map_error(error)),
            }
        }

        let mut message = format!("cannot find font file \"{filename}\" in sys.path");
        if std::path::Path::new(filename).exists() {
            message.push_str(&format!(
                ", did you mean ImageFont.load(\"{filename}\") instead?"
            ));
        }
        Err(pyo3::exceptions::PyOSError::new_err(message))
    }

    #[staticmethod]
    fn load_default() -> PyResult<Self> {
        let inner = pillow_rs::PilFont::load_default().map_err(map_error)?;
        Ok(Self { inner, file: None })
    }

    #[getter]
    fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    #[getter]
    fn info(&self, py: Python<'_>) -> Vec<Py<PyBytes>> {
        self.inner
            .info()
            .iter()
            .map(|line| PyBytes::new(py, line).unbind())
            .collect()
    }

    fn getsize(&self, text: Vec<u8>) -> PyResult<(i32, i32)> {
        self.inner.getsize(&text).map_err(map_error)
    }

    #[pyo3(signature = (text, mode=""))]
    fn getmask(&self, text: Vec<u8>, mode: &str) -> PyResult<PyImage> {
        let _ = mode;
        let image = self
            .inner
            .getmask(&text)
            .and_then(|mask| mask.to_image())
            .map_err(map_error)?;
        Ok(PyImage { inner: image })
    }
}

fn load_pilfont_from_path(
    path: &std::path::Path,
) -> Result<(pillow_rs::PilFont, String), PilError> {
    let metrics = std::fs::read(path).map_err(PilError::from)?;
    let root = path.with_extension("");

    for extension in [".png", ".gif", ".pbm"] {
        let mut candidate_name = root.as_os_str().to_os_string();
        candidate_name.push(extension);
        let candidate = std::path::PathBuf::from(candidate_name);
        let Ok(bitmap) = std::fs::read(&candidate) else {
            continue;
        };
        let Ok(image) = pillow_rs::PilFont::open_pilfont_glyph_image(bitmap) else {
            continue;
        };
        match pilfont_from_glyph_image(&metrics, image) {
            Ok(font) => return Ok((font, candidate.to_string_lossy().into_owned())),
            Err(PilError::TypeError(message)) if message == "invalid font image mode" => continue,
            Err(error) => return Err(error),
        }
    }

    Err(PilError::OsError(format!(
        "cannot find glyph data file {}.{{gif|pbm|png}}",
        root.display()
    )))
}

fn pilfont_from_glyph_image(
    metrics: &[u8],
    image: pillow_rs::PilFontGlyphImage,
) -> Result<pillow_rs::PilFont, PilError> {
    match image {
        pillow_rs::PilFontGlyphImage::Image(image) => {
            pillow_rs::PilFont::from_pilfont_data(metrics, image)
        }
        deferred @ pillow_rs::PilFontGlyphImage::DeferredRenderError { .. } => {
            pillow_rs::PilFont::from_pilfont_glyph_data(metrics, deferred)
        }
    }
}

fn pilfont_load_path_catches(error: &PilError) -> bool {
    matches!(
        error,
        PilError::IOError(_)
            | PilError::OsError(_)
            | PilError::UnidentifiedImageError(_)
            | PilError::ImageError(_)
            | PilError::Io(_)
    )
}

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
            20,
        ));
        Ok(())
    }

    fn close(&mut self) {
        if self.points.len() > 2 && self.points.first() != self.points.last() {
            self.points.push(self.points[0]);
        }
    }

    #[getter]
    fn _points(&self) -> Vec<(i32, i32)> {
        self.points.clone()
    }
}

#[pyclass(name = "ImageDraw")]
pub struct PyDraw {
    draw: pillow_rs::Draw,
}

fn extract_draw_points(xy: &Bound<'_, PyAny>) -> PyResult<Vec<(i32, i32)>> {
    if let Ok(points) = xy.extract::<Vec<(i32, i32)>>() {
        return Ok(points);
    }
    let flat = xy.extract::<Vec<i32>>()?;
    if flat.len() < 2 || flat.len() % 2 != 0 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "wrong number of coordinates",
        ));
    }
    Ok(flat
        .chunks_exact(2)
        .map(|point| (point[0], point[1]))
        .collect())
}

#[pymethods]
impl PyDraw {
    #[new]
    #[pyo3(signature = (image, mode=None))]
    fn new(image: &Bound<'_, PyImage>, mode: Option<String>) -> PyResult<Self> {
        let borrowed = image.borrow();
        let draw = pillow_rs::Draw::new(borrowed.inner.clone(), mode);
        Ok(PyDraw { draw })
    }

    fn line(
        &mut self,
        xy: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        let pts = extract_draw_points(xy)?;
        let w = width.map_or(1, |w| if w > 0 { w } else { 1 });
        self.draw.polyline(&pts, color, w).map_err(map_error)
    }

    fn rectangle(
        &mut self,
        xy: (i32, i32, i32, i32),
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
            .rectangle(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                fill_color,
                out_color,
                width.unwrap_or(1),
            )
            .map_err(map_error)
    }

    fn ellipse(
        &mut self,
        xy: (i32, i32, i32, i32),
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
            .ellipse(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                fill_color,
                out_color,
                width.unwrap_or(1),
            )
            .map_err(map_error)
    }

    fn bitmap(
        &mut self,
        xy: (f64, f64),
        bitmap: &Bound<'_, PyImage>,
        fill: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<()> {
        let fill_color = self.color(fill)?;
        let bmp = bitmap.borrow();
        self.draw
            .bitmap(xy.0 as i32, xy.1 as i32, &bmp.inner, Some(fill_color))
            .map_err(map_error)?;
        Ok(())
    }

    fn regular_polygon(
        &mut self,
        bounding_circle: &Bound<'_, PyAny>,
        n_sides: u32,
        rotation: Option<f64>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let (cx, cy, r): (f64, f64, f64) =
            if let Ok((x, y, r)) = bounding_circle.extract::<(f64, f64, f64)>() {
                (x, y, r)
            } else if let Ok(((x, y), r)) = bounding_circle.extract::<((f64, f64), f64)>() {
                (x, y, r)
            } else if let Ok((x, y, r)) = bounding_circle.extract::<(i32, i32, i32)>() {
                (x as f64, y as f64, r as f64)
            } else {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(
                    "bounding_circle must be (x,y,r) or ((x,y),r)",
                ));
            };
        // Match PIL's _compute_regular_polygon_vertices exactly
        // PIL: start from (radius, 0), rotate by (270 - 0.5*deg_per_side + rotation)
        // PIL uses round(x, 2) for 2-decimal float precision, then C truncates to int
        let rot = rotation.unwrap_or(0.0);
        let n = n_sides as f64;
        let deg_per_side = 360.0 / n;
        let start_angle = 270.0 - 0.5 * deg_per_side + rot;
        let mut pts = Vec::with_capacity(n_sides as usize);
        for i in 0..n_sides {
            let angle_deg = start_angle + deg_per_side * i as f64;
            let angle_deg = if angle_deg > 360.0 {
                angle_deg - 360.0
            } else {
                angle_deg
            };
            // PIL: point[0]*cos(360-deg) - point[1]*sin(360-deg) + centroid
            // with start_point = (r, 0), so simplifies to r*cos(360-angle) + cx
            let theta = (360.0 - angle_deg).to_radians();
            // CRITICAL: match PIL's round(x,2) then truncate to int.
            // Without round-to-2dp, fp imprecision (e.g. cos(270°)=~-6e-17)
            // causes truncation to 24 instead of 25 for vertex (25,10).
            let x_raw = r * theta.cos() + cx;
            let y_raw = r * theta.sin() + cy;
            let x = ((x_raw * 100.0).round() / 100.0) as i32;
            let y = ((y_raw * 100.0).round() / 100.0) as i32;
            pts.push((x, y));
        }
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
            .polygon(&pts, fill_color, out_color, width.unwrap_or(1))
            .map_err(map_error)
    }

    fn polygon(
        &mut self,
        xy: Vec<Vec<i32>>,
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
        let pts: Vec<(i32, i32)> = xy
            .into_iter()
            .map(|v| {
                if v.len() >= 2 {
                    (v[0], v[1])
                } else {
                    (v[0], v[0])
                }
            })
            .collect();
        self.draw
            .polygon(&pts, fill_color, out_color, width.unwrap_or(1))
            .map_err(map_error)
    }

    fn point(&mut self, xy: &Bound<'_, PyAny>, fill: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
        let color = self.color(fill)?;
        let pts = extract_draw_points(xy)?;
        self.draw.point(&pts, color).map_err(map_error)
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
            extract_draw_points(shape)
                .map_err(|_| pyo3::exceptions::PyTypeError::new_err("Unsupported shape format"))?
        };
        let fill = fill.map(|_| self.color(fill)).transpose()?;
        let outline = outline.map(|_| self.color(outline)).transpose()?;
        self.draw.shape(&points, fill, outline).map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, width=1))]
    fn arc(
        &mut self,
        xy: (i32, i32, i32, i32),
        start: f64,
        end: f64,
        fill: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        self.draw
            .arc(
                xy.0,
                xy.1,
                xy.2,
                xy.3,
                start,
                end,
                color,
                width.unwrap_or(1),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, outline=None, width=1))]
    fn chord(
        &mut self,
        xy: (i32, i32, i32, i32),
        start: f64,
        end: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill).unwrap_or((0, 0, 0, 255)));
        let oc = outline.map(|_| self.color(outline).unwrap_or((0, 0, 0, 255)));
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
                width.unwrap_or(1),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, start, end, fill=None, outline=None, width=1))]
    fn pieslice(
        &mut self,
        xy: (i32, i32, i32, i32),
        start: f64,
        end: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill).unwrap_or((0, 0, 0, 255)));
        let oc = outline.map(|_| self.color(outline).unwrap_or((0, 0, 0, 255)));
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
                width.unwrap_or(1),
            )
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, radius, fill=None, outline=None, width=1))]
    fn circle(
        &mut self,
        xy: (f64, f64),
        radius: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill).unwrap_or((0, 0, 0, 255)));
        let oc = outline.map(|_| self.color(outline).unwrap_or((0, 0, 0, 255)));
        self.draw
            .circle(xy.0 as i32, xy.1 as i32, radius, fc, oc, width.unwrap_or(1))
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, radius=0.0, fill=None, outline=None, width=1))]
    fn rounded_rectangle(
        &mut self,
        xy: (i32, i32, i32, i32),
        radius: f64,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        width: Option<u32>,
    ) -> PyResult<()> {
        let fc = fill.map(|_| self.color(fill).unwrap_or((0, 0, 0, 255)));
        let oc = outline.map(|_| self.color(outline).unwrap_or((0, 0, 0, 255)));
        self.draw
            .rounded_rectangle(xy.0, xy.1, xy.2, xy.3, radius, fc, oc, width.unwrap_or(1))
            .map_err(map_error)
    }

    #[pyo3(signature = (xy, text, fill=None, font=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn text(
        &mut self,
        xy: (f64, f64),
        text: String,
        fill: Option<&Bound<'_, PyAny>>,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        if let Some(pyfont) = font {
            let borrowed = pyfont.borrow();
            let options = pillow_rs::ImageFontTextOptions {
                direction,
                features,
                language,
                stroke_width,
                anchor,
                ..pillow_rs::ImageFontTextOptions::default()
            };
            self.draw
                .text_with_options(
                    xy.0 as i32,
                    xy.1 as i32,
                    &text,
                    &borrowed.inner,
                    color,
                    &options,
                )
                .map_err(map_error)
        } else {
            Err(pyo3::exceptions::PyNotImplementedError::new_err(
                "text() requires a font",
            ))
        }
    }

    #[pyo3(signature = (xy, text, fill=None, font=None, spacing=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn multiline_text(
        &mut self,
        xy: (f64, f64),
        text: &str,
        fill: Option<&Bound<'_, PyAny>>,
        font: Option<&Bound<'_, PyFont>>,
        spacing: Option<i32>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<()> {
        let color = self.color(fill)?;
        let sp = spacing.unwrap_or(4) as f64;
        let mut y = xy.1;
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        for line in text.split('\n') {
            if line.is_empty() {
                y += sp + 10.0;
                continue;
            }
            if let Some(pyfont) = font {
                let borrowed = pyfont.borrow();
                self.draw
                    .text_with_options(
                        xy.0 as i32,
                        y as i32,
                        line,
                        &borrowed.inner,
                        color,
                        &options,
                    )
                    .map_err(map_error)?;
                let (_, h) =
                    pillow_rs::imagefont_text_bbox(&borrowed.inner, line).map_err(map_error)?;
                y += h as f64 + sp;
            } else {
                return Err(pyo3::exceptions::PyNotImplementedError::new_err(
                    "text() requires a font",
                ));
            }
        }
        Ok(())
    }

    /// Compute text bounding box. Loads default FreeType font if font is None.
    /// Returns (left, top, right, bottom).
    #[pyo3(signature = (xy, text, font=None, direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn textbbox(
        &mut self,
        xy: (i32, i32),
        text: &str,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<(i32, i32, i32, i32)> {
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let bbox = match font {
            Some(f) => {
                let bbox =
                    pillow_rs::imagefont_getbbox_with_options(&f.borrow().inner, text, &options)
                        .map_err(map_error)?;
                (bbox.0 as i32, bbox.1 as i32, bbox.2 as i32, bbox.3 as i32)
            }
            None => {
                let font = pillow_rs::imagefont_load_default(10.0).map_err(map_error)?;
                let bbox = pillow_rs::imagefont_getbbox_with_options(&font, text, &options)
                    .map_err(map_error)?;
                (bbox.0 as i32, bbox.1 as i32, bbox.2 as i32, bbox.3 as i32)
            }
        };
        Ok((xy.0 + bbox.0, xy.1 + bbox.1, xy.0 + bbox.2, xy.1 + bbox.3))
    }

    /// Compute text length in pixels. Loads default FreeType font if font is None.
    #[pyo3(signature = (text, font=None, direction=None, features=None, language=None))]
    fn textlength(
        &mut self,
        text: &str,
        font: Option<&Bound<'_, PyFont>>,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
    ) -> PyResult<f64> {
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            features,
            language,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let w = match font {
            Some(f) => {
                pillow_rs::imagefont_getlength_with_options(&f.borrow().inner, text, &options)
                    .map_err(map_error)?
            }
            None => {
                let font = pillow_rs::imagefont_load_default(10.0).map_err(map_error)?;
                pillow_rs::imagefont_getlength_with_options(&font, text, &options)
                    .map_err(map_error)?
            }
        };
        Ok(w as f64)
    }

    /// Compute bounding box for multiline text. Matches PIL's exact algorithm.
    #[pyo3(signature = (xy, text, font=None, spacing=4, align="left", direction=None, features=None, language=None, stroke_width=0.0, anchor=None))]
    fn multiline_textbbox(
        &mut self,
        xy: (i32, i32),
        text: &str,
        font: Option<&Bound<'_, PyFont>>,
        spacing: i32,
        align: &str,
        direction: Option<String>,
        features: Option<Vec<String>>,
        language: Option<String>,
        stroke_width: f32,
        anchor: Option<String>,
    ) -> PyResult<(i32, i32, i32, i32)> {
        let default_font;
        let f: &pillow_rs::FreeTypeFont = if let Some(f) = font {
            &f.borrow().inner
        } else {
            default_font = pillow_rs::imagefont_load_default(10.0).map_err(map_error)?;
            &default_font
        };
        let options = pillow_rs::ImageFontTextOptions {
            direction,
            features,
            language,
            stroke_width,
            anchor,
            ..pillow_rs::ImageFontTextOptions::default()
        };
        let lines: Vec<&str> = text.split('\n').collect();
        if lines.is_empty() {
            return Ok((xy.0, xy.1, xy.0, xy.1));
        }
        if lines.len() == 1 {
            let bbox =
                pillow_rs::imagefont_getbbox_with_options(f, text, &options).map_err(map_error)?;
            return Ok((
                xy.0 + bbox.0 as i32,
                xy.1 + bbox.1 as i32,
                xy.0 + bbox.2 as i32,
                xy.1 + bbox.3 as i32,
            ));
        }
        // Pillow ImageText.Text::_split advances by the bottom of "A"'s
        // FreeType bbox, then unions each line's full bbox. Using only mask
        // width/height here loses the ascender bearing (and italic overhang).
        let line_height = spacing
            + pillow_rs::imagefont_getbbox_with_options(f, "A", &options)
                .map_err(map_error)?
                .3 as i32;
        let widths: Vec<f32> = lines
            .iter()
            .map(|line| pillow_rs::imagefont_getlength_with_options(f, line, &options))
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_error)?;
        let max_width = widths.iter().copied().fold(0.0_f32, f32::max);
        let x0 = xy.0 as f64;
        let y0 = xy.1 as f64;
        let mut left = f64::MAX;
        let mut top = f64::MAX;
        let mut right = f64::MIN;
        let mut bottom = f64::MIN;
        for (i, line) in lines.iter().enumerate() {
            let line_y = y0 + i as f64 * line_height as f64;
            let line_x = match align {
                "center" => x0 + (max_width as f64 - widths[i] as f64) / 2.0,
                "right" => x0 + max_width as f64 - widths[i] as f64,
                _ => x0,
            };
            let bbox =
                pillow_rs::imagefont_getbbox_with_options(f, line, &options).map_err(map_error)?;
            left = left.min(line_x + bbox.0 as f64);
            top = top.min(line_y + bbox.1 as f64);
            right = right.max(line_x + bbox.2 as f64);
            bottom = bottom.max(line_y + bbox.3 as f64);
        }
        Ok((left as i32, top as i32, right as i32, bottom as i32))
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
    fn color(&self, val: Option<&Bound<'_, PyAny>>) -> PyResult<(u8, u8, u8, u8)> {
        parse_draw_color(val, self.draw.mode())
    }
}

fn parse_draw_color(
    val: Option<&Bound<'_, PyAny>>,
    mode: Option<&str>,
) -> PyResult<(u8, u8, u8, u8)> {
    let v = match val {
        Some(v) => v,
        // Pillow's ImageDraw context starts with ink=-1, which is an all-255
        // `(index, alpha)` sample in PA. Other modes retain the wrapper's
        // existing black default.
        None if mode == Some("PA") => return Ok((255, 255, 255, 255)),
        None => return Ok((0, 0, 0, 255)),
    };
    // F mode (float32): convert color to f32 LE bytes
    if mode == Some("F") {
        if let Ok(f) = v.extract::<f64>() {
            let raw = f as f32;
            let bytes = raw.to_le_bytes();
            return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
        }
    }
    // I mode (int32): convert color to i32 LE bytes
    if mode == Some("I") {
        if let Ok(i) = v.extract::<i32>() {
            let bytes = i.to_le_bytes();
            return Ok((bytes[0], bytes[1], bytes[2], bytes[3]));
        }
    }
    // PA uses a palette index plus a per-pixel alpha byte. Pillow accepts
    // scalar/one-item ink as (index, 0) and a two-item tuple verbatim.
    if mode == Some("PA") {
        if let Ok((index, alpha)) = v.extract::<(u8, u8)>() {
            return Ok((index, index, index, alpha));
        }
        if let Ok((index,)) = v.extract::<(u8,)>() {
            return Ok((index, index, index, 0));
        }
        if let Ok(index) = v.extract::<u8>() {
            return Ok((index, index, index, 0));
        }
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "color must be int, or tuple of one or two elements",
        ));
    }
    if mode == Some("LA") {
        if let Ok((luma, alpha)) = v.extract::<(u8, u8)>() {
            return Ok((luma, luma, luma, alpha));
        }
        if let Ok(luma) = v.extract::<u8>() {
            return Ok((luma, luma, luma, 0));
        }
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "color must be int or tuple of one or two elements",
        ));
    }
    // Standard modes: extract as u8
    if let Ok(s) = v.extract::<String>() {
        pillow_rs::parse_color_str(&s).map_err(map_error)
    } else if let Ok((r, g, b)) = v.extract::<(u8, u8, u8)>() {
        Ok((r, g, b, 255))
    } else if let Ok((r, g, b, a)) = v.extract::<(u8, u8, u8, u8)>() {
        Ok((r, g, b, a))
    } else if let Ok(i) = v.extract::<u8>() {
        // Match PIL's _getink per-mode behavior for int fills:
        //   RGB: (R=i, G=0, B=0, A=255) — PIL puts int fill in RED only
        //   RGBA: (R=i, G=0, B=0, A=0) — PIL puts int fill in RED only, A=0
        //   LA: (L=i, A=0) — alpha=0 means "use value directly"
        //   CMYK: (C=i, M=0, Y=0, K=0)
        //   L/1/P: (i, i, i, 255) — single channel, G/B irrelevant
        //   F/I: already handled above
        match mode {
            Some("RGB") => Ok((i, 0, 0, 255)),
            Some("RGBA") => Ok((i, 0, 0, 0)),
            // PIL's _getink for LA: (L=value, A=0) where A=0 means full opacity
            Some("LA") => Ok((i, i, i, 0)),
            Some("CMYK") => Ok((i, 0, 0, 0)),
            _ => Ok((i, i, i, 255)),
        }
    } else {
        Err(pyo3::exceptions::PyTypeError::new_err(
            "Expected color tuple, int, or string",
        ))
    }
}

// --- ImageOps module-level functions ---

#[pyfunction]
fn ops_autocontrast(image: &Bound<'_, PyImage>, cutoff: Option<f64>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let c = cutoff.unwrap_or(0.0);
    let rs =
        Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_autocontrast(&inner, c)))
            .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_equalize(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_equalize(&inner)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_invert(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_invert(&inner)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_flip(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_flip(&inner)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_mirror(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_mirror(&inner)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_posterize(image: &Bound<'_, PyImage>, bits: u8) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs =
        Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_posterize(&inner, bits)))
            .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_solarize(image: &Bound<'_, PyImage>, threshold: Option<u8>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let t = threshold.unwrap_or(128);
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_solarize(&inner, t)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_grayscale(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_grayscale(&inner)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_colorize(
    image: &Bound<'_, PyImage>,
    black: (u8, u8, u8),
    white: (u8, u8, u8),
) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_colorize(&inner, black, white))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_contain(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<String>,
) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_contain(&inner, size.0, size.1, filter.as_deref()))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_cover(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<String>,
) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_cover(&inner, size.0, size.1, filter.as_deref()))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_fit(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<String>,
    bleed: Option<f64>,
    centering: Option<(f64, f64)>,
) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| {
            pillow_rs::imageops_fit(
                &inner,
                size.0,
                size.1,
                filter.as_deref(),
                bleed.unwrap_or(0.0),
                centering.unwrap_or((0.5, 0.5)),
            )
        })
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_pad(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<String>,
    color: Option<&Bound<'_, PyAny>>,
    centering: Option<(f64, f64)>,
) -> PyResult<PyImage> {
    // Resolve color: None -> (0,0,0,255), int -> (v,v,v,255),
    // 3-tuple -> (v0,v1,v2,255), 4-tuple as-is
    let resolved_color: Option<(u8, u8, u8, u8)> = match color {
        None => None,
        Some(c) => {
            if let Ok(i) = c.extract::<u8>() {
                Some((i, i, i, 255))
            } else if let Ok((r, g, b)) = c.extract::<(u8, u8, u8)>() {
                Some((r, g, b, 255))
            } else if let Ok((r, g, b, a)) = c.extract::<(u8, u8, u8, u8)>() {
                Some((r, g, b, a))
            } else {
                None
            }
        }
    };

    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| {
            pillow_rs::imageops_pad(
                &inner,
                size.0,
                size.1,
                filter.as_deref(),
                resolved_color,
                centering.unwrap_or((0.5, 0.5)),
            )
        })
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_scale(image: &Bound<'_, PyImage>, factor: f64, filter: Option<String>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_scale(&inner, factor, filter.as_deref()))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_expand(
    image: &Bound<'_, PyImage>,
    border: &Bound<'_, PyAny>,
    fill: &Bound<'_, PyAny>,
) -> PyResult<PyImage> {
    // Resolve border: int -> use as u32, 4-tuple -> max
    let border_val: u32 = if let Ok(i) = border.extract::<u32>() {
        i
    } else if let Ok((t, r, b, l)) = border.extract::<(u32, u32, u32, u32)>() {
        t.max(r).max(b).max(l)
    } else {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "border must be int or 4-tuple",
        ));
    };

    // Resolve fill: int -> (v, 0, 0, 0), 3-tuple -> (v0, v1, v2, 0), 4-tuple as-is
    let fill_val: (u8, u8, u8, u8) = if let Ok(i) = fill.extract::<u8>() {
        (i, 0, 0, 0)
    } else if let Ok((r, g, b)) = fill.extract::<(u8, u8, u8)>() {
        (r, g, b, 0)
    } else if let Ok((r, g, b, a)) = fill.extract::<(u8, u8, u8, u8)>() {
        (r, g, b, a)
    } else {
        (0, 0, 0, 0)
    };

    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::imageops_expand(&inner, border_val, fill_val))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_crop_border(image: &Bound<'_, PyImage>, border: u32) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::imageops_crop(&inner, border)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

/// Extract Orientation tag (0x0112) from raw EXIF bytes. Returns None if not found.
#[pyfunction]
fn exif_get_orientation(raw: Vec<u8>) -> Option<u32> {
    pillow_rs::exif_get_orientation(&raw)
}

/// Remove Orientation tag from EXIF bytes by zeroing its tag field.
#[pyfunction]
fn exif_remove_orientation(raw: Vec<u8>) -> Vec<u8> {
    pillow_rs::exif_remove_orientation(&raw)
}

// --- ImageChops module-level functions ---

#[pyfunction]
#[pyo3(signature = (image1, image2, scale=1.0, offset=0.0))]
fn chops_add(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    scale: f64,
    offset: f64,
) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs =
        Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_add(&b1, &b2, scale, offset)))
            .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
#[pyo3(signature = (image1, image2, scale=1.0, offset=0.0))]
fn chops_subtract(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    scale: f64,
    offset: f64,
) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| {
        py.allow_threads(|| pillow_rs::chops_subtract(&b1, &b2, scale, offset))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_multiply(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_multiply(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_screen(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_screen(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_darker(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_darker(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_lighter(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_lighter(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_difference(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow().inner.clone();
    let b2 = image2.borrow().inner.clone();
    let rs = Python::with_gil(|py| py.allow_threads(|| pillow_rs::chops_difference(&b1, &b2)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_invert(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let borrowed = image.borrow();
    let rs = pillow_rs::chops_invert(&borrowed.inner).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn chops_add_modulo(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_add_modulo(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_subtract_modulo(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_subtract_modulo(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_constant(image: &Bound<'_, PyImage>, value: u8) -> PyResult<PyImage> {
    let b = image.borrow();
    pillow_rs::chops_constant(&b.inner, value)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_hard_light(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_hard_light(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_soft_light(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_soft_light(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_overlay(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_overlay(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_logical_and(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_logical_and(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_logical_or(image1: &Bound<'_, PyImage>, image2: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_logical_or(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_logical_xor(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    pillow_rs::chops_logical_xor(&b1.inner, &b2.inner)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

#[pyfunction]
fn chops_offset(image: &Bound<'_, PyImage>, xoffset: i32, yoffset: i32) -> PyResult<PyImage> {
    let b = image.borrow();
    pillow_rs::chops_offset(&b.inner, xoffset, yoffset)
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
}

// --- Image module functions ---

#[pyfunction]
fn image_merge(mode: &str, bands: &Bound<'_, PyAny>) -> PyResult<PyImage> {
    let mut band_images: Vec<pillow_rs::Image> = Vec::new();
    for item in bands.iter()? {
        let obj = item?;
        let py_img = obj.downcast::<PyImage>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err("bands must be a sequence of Image objects")
        })?;
        band_images.push(py_img.borrow().inner.clone());
    }
    let rs = pillow_rs::image_merge(mode, &band_images).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_blend(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    alpha: f64,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let rs = pillow_rs::image_blend(&b1.inner, &b2.inner, alpha).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn image_composite(
    image1: &Bound<'_, PyImage>,
    image2: &Bound<'_, PyImage>,
    mask: &Bound<'_, PyImage>,
) -> PyResult<PyImage> {
    let b1 = image1.borrow();
    let b2 = image2.borrow();
    let bm = mask.borrow();
    let rs = pillow_rs::image_composite(&b1.inner, &b2.inner, &bm.inner).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

/// Generate a 256×256 linear gradient image from black to white.
#[pyfunction]
fn image_linear_gradient(mode: &str) -> PyResult<PyImage> {
    let rs = pillow_rs::image_linear_gradient(mode).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

/// Generate a 256×256 radial gradient image from white (center) to black (edges).
#[pyfunction]
fn image_radial_gradient(mode: &str) -> PyResult<PyImage> {
    let rs = pillow_rs::image_radial_gradient(mode).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

/// Generate a Mandelbrot set image.
#[pyfunction]
fn image_effect_mandelbrot(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> PyResult<PyImage> {
    let rs = pillow_rs::image_effect_mandelbrot(size, extent, quality).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

// --- ImageColor ---

#[pyfunction]
fn getrgb(color: &str) -> PyResult<PyObject> {
    let (r, g, b, a) = pillow_rs::parse_color_str(color).map_err(map_error)?;
    Python::with_gil(|py| {
        if pillow_rs::color_has_explicit_alpha(color) {
            Ok((r, g, b, a).to_object(py))
        } else {
            Ok((r, g, b).to_object(py))
        }
    })
}

#[pyfunction]
fn palette_search(palette: Vec<u8>, r: u8, g: u8, b: u8) -> Option<usize> {
    pillow_rs::palette_getcolor(&palette, r, g, b)
}

/// PIL-compatible getcolor: search palette for (r,g,b[,a]), append if new. Returns index.
#[pyfunction]
fn palette_getcolor_append(
    palette: Vec<u8>,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    mode: &str,
) -> PyResult<usize> {
    let mut pal = palette;
    pillow_rs::palette_getcolor_append(&mut pal, r, g, b, a, mode).map_err(PyValueError::new_err)
}

/// Format palette as PIL-compatible text (header + 256-entry table).
#[pyfunction]
fn palette_to_text(palette: Vec<u8>, mode: &str) -> String {
    pillow_rs::palette_to_text(&palette, mode)
}

#[pyfunction]
fn getcolor(color: &str, mode: &str) -> PyResult<PyObject> {
    let (r, g, b, a) = pillow_rs::parse_color_str(color).map_err(map_error)?;
    let result = pillow_rs::getcolor(r, g, b, a, mode).map_err(map_error)?;
    Python::with_gil(|py| match result {
        pillow_rs::ColorValue::Gray(value) => Ok(value.to_object(py)),
        pillow_rs::ColorValue::GrayAlpha(gray, alpha) => Ok((gray, alpha).to_object(py)),
        pillow_rs::ColorValue::Rgb(r, g, b) => Ok((r, g, b).to_object(py)),
        pillow_rs::ColorValue::Rgba(r, g, b, a) => Ok((r, g, b, a).to_object(py)),
        pillow_rs::ColorValue::Hsv(h, s, v) => Ok((h, s, v).to_object(py)),
    })
}

/// Validate a color for palette mode, append it, and return the
/// updated palette and the color index.
/// Handles mode-specific logic: RGB mode rejects non-opaque RGBA,
/// RGBA mode auto-fills missing alpha to 255.
#[pyfunction]
fn palette_getcolor_validate(
    palette: Vec<u8>,
    color: Vec<u8>,
    mode: &str,
) -> PyResult<(Vec<u8>, usize)> {
    let mut pal = palette;
    let idx = pillow_rs::palette_getcolor_validate(&mut pal, &color, mode)
        .map_err(PyValueError::new_err)?;
    Ok((pal, idx))
}

/// Save palette data to a text file.
#[pyfunction]
fn palette_save_to_file(palette: Vec<u8>, mode: &str, fp: &str) -> PyResult<()> {
    let text = pillow_rs::palette_to_text(&palette, mode);
    std::fs::write(fp, text).map_err(|error| {
        pyo3::exceptions::PyOSError::new_err(format!("Cannot write palette file: {error}"))
    })
}

/// Compute cubic Bezier curve subdivision points for Outline.
/// Accepts flat list of 8 control points [x0,y0,x1,y1,x2,y2,x3,y3] and steps.
/// Returns flat list of [x,y] int pairs for the curve.
#[pyfunction]
fn outline_curve(points: Vec<f64>, steps: u32) -> Vec<Vec<i32>> {
    let result = pillow_rs::outline_curve_points(&points, steps);
    result.into_iter().map(|(x, y)| vec![x, y]).collect()
}

/// Compute basic statistics from a list of values (PIL's ImageStat fallback).
/// Returns a dict with count, sum, mean, min, max.
#[pyfunction]
fn stat_from_list(data: Vec<f64>) -> PyObject {
    let (count, sum, mean, min_val, max_val) = pillow_rs::stat_from_list(&data);
    Python::with_gil(|py| {
        let dict = pyo3::types::PyDict::new(py);
        let _ = dict.set_item("count", count as i64);
        let _ = dict.set_item("sum", sum);
        let _ = dict.set_item("mean", mean);
        let _ = dict.set_item("min", min_val);
        let _ = dict.set_item("max", max_val);
        dict.to_object(py)
    })
}

// --- ImageFilter helper functions ---

/// Validate and normalize 3D LUT size. Accepts int (converted to 3-tuple)
/// or sequence of 3 ints. Each dimension must be in [2, 65].
#[pyfunction]
fn color3dlut_check_size(size: &Bound<'_, PyAny>) -> PyResult<(u32, u32, u32)> {
    // Try as a sequence first
    if let Ok(len) = size.len() {
        if len == 3 {
            let s0: f64 = size.get_item(0)?.extract()?;
            let s1: f64 = size.get_item(1)?.extract()?;
            let s2: f64 = size.get_item(2)?.extract()?;
            let s = [s0 as i32, s1 as i32, s2 as i32];
            for &si in &s {
                if !(2..=65).contains(&si) {
                    return Err(PyValueError::new_err("Size should be in [2, 65] range."));
                }
            }
            return Ok((s[0] as u32, s[1] as u32, s[2] as u32));
        }
        return Err(PyValueError::new_err(
            "Size should be either an integer or a tuple of three integers.",
        ));
    }

    // Single value (int or float)
    let s: f64 = size.extract().map_err(|_| {
        PyValueError::new_err("Size should be either an integer or a tuple of three integers.")
    })?;
    let si = s as i32;
    if !(2..=65).contains(&si) {
        return Err(PyValueError::new_err("Size should be in [2, 65] range."));
    }
    Ok((si as u32, si as u32, si as u32))
}

/// Validate and flatten a 3D LUT table from Python object.
/// Handles flat list of floats and list of tuples.
/// Always returns a flat Vec<f64> of validated length.
#[pyfunction]
#[pyo3(signature = (table_obj, size, channels=3, copy_table=true))]
fn color3dlut_new(
    table_obj: &Bound<'_, PyAny>,
    size: (u32, u32, u32),
    channels: u32,
    copy_table: bool,
) -> PyResult<Vec<f64>> {
    let _ = copy_table;
    let items = size.0 as usize * size.1 as usize * size.2 as usize;
    let expected_len = items * channels as usize;

    // Check if table is empty
    let table_len = table_obj.len().unwrap_or(0);
    if table_len == 0 {
        return Err(PyValueError::new_err(format!(
            "The table should have either channels * size**3 float items              or size**3 items of channels-sized tuples with floats.              Table should be: {}x{}x{}x{}. Actual length: 0",
            channels, size.0, size.1, size.2
        )));
    }

    // Check if first element is a sequence (tuple/list of values) -> flatten
    let first_item = table_obj.get_item(0)?;
    let is_nested = first_item.extract::<Vec<f64>>().is_ok();

    let table: Vec<f64> = if is_nested {
        let mut flat = Vec::with_capacity(expected_len);
        for item in table_obj.iter()? {
            let item = item?;
            let pixel: Vec<f64> = item.extract().map_err(|_| {
                PyValueError::new_err(format!(
                    "The elements of the table should have a length of {}.",
                    channels
                ))
            })?;
            if pixel.len() != channels as usize {
                return Err(PyValueError::new_err(format!(
                    "The elements of the table should have a length of {}.",
                    channels
                )));
            }
            flat.extend(pixel);
        }
        flat
    } else {
        // Flat table: extract as Vec<f64>
        table_obj.extract::<Vec<f64>>().map_err(|_| {
            PyValueError::new_err(
                "Table must be a sequence of floats or a sequence of tuples of floats.",
            )
        })?
    };

    if table.len() != expected_len {
        return Err(PyValueError::new_err(format!(
            "The table should have either channels * size**3 float items              or size**3 items of channels-sized tuples with floats.              Table should be: {}x{}x{}x{}. Actual length: {}",
            channels,
            size.0,
            size.1,
            size.2,
            table.len()
        )));
    }

    Ok(table)
}

/// Generate a 3D LUT table by calling a Python callback for each position.
/// The callback receives (r, g, b) normalized to [0, 1] and returns a tuple
/// of `channels` floats.
#[pyfunction]
fn color3dlut_generate(
    size: (u32, u32, u32),
    channels: u32,
    callback: PyObject,
    py: Python,
) -> PyResult<Vec<f64>> {
    let (s1, s2, s3) = size;
    let ch = channels as usize;
    let total = (s1 as usize) * (s2 as usize) * (s3 as usize) * ch;
    let mut table = vec![0.0_f64; total];
    let mut idx = 0;
    for b in 0..s3 {
        for g in 0..s2 {
            for r in 0..s1 {
                let args = (
                    r as f64 / (s1 - 1) as f64,
                    g as f64 / (s2 - 1) as f64,
                    b as f64 / (s3 - 1) as f64,
                );
                let result = callback.call(py, args, None)?;
                let values: Vec<f64> = result.extract(py)?;
                for (i, &v) in values.iter().enumerate().take(ch) {
                    table[idx + i] = v;
                }
                idx += ch;
            }
        }
    }
    Ok(table)
}

/// Transform a 3D LUT table by calling a Python callback for each entry.
/// If `with_normals` is true, the callback receives (r_norm, g_norm, b_norm, *values).
/// Otherwise, receives (*values).
/// The callback returns a tuple of `channels_out` floats.
#[pyfunction]
fn color3dlut_transform(
    table: Vec<f64>,
    size: (u32, u32, u32),
    channels_in: u32,
    channels_out: u32,
    with_normals: bool,
    callback: PyObject,
    py: Python,
) -> PyResult<Vec<f64>> {
    let (s1, s2, s3) = size;
    let ci = channels_in as usize;
    let co = channels_out as usize;
    let total = (s1 as usize) * (s2 as usize) * (s3 as usize) * co;
    let mut out_table = vec![0.0_f64; total];
    let mut idx_in = 0;
    let mut idx_out = 0;
    for b in 0..s3 {
        for g in 0..s2 {
            for r in 0..s1 {
                let values = &table[idx_in..idx_in + ci];
                let result = if with_normals {
                    let mut normals = Vec::with_capacity(3 + ci);
                    normals.push(r as f64 / (s1 - 1) as f64);
                    normals.push(g as f64 / (s2 - 1) as f64);
                    normals.push(b as f64 / (s3 - 1) as f64);
                    normals.extend_from_slice(values);
                    let pt = PyTuple::new(py, &normals)?;
                    callback.call(py, &pt, None)?
                } else {
                    let pt = PyTuple::new(py, values)?;
                    callback.call(py, &pt, None)?
                };
                let new_values: Vec<f64> = result.extract(py)?;
                if new_values.len() != co {
                    return Err(PyValueError::new_err(format!(
                        "Callback returned {} values, expected {}",
                        new_values.len(),
                        co
                    )));
                }
                for (i, &v) in new_values.iter().enumerate() {
                    out_table[idx_out + i] = v;
                }
                idx_in += ci;
                idx_out += co;
            }
        }
    }
    Ok(out_table)
}

#[pyfunction]
fn color3dlut_repr(
    table_type: &str,
    size: (u32, u32, u32),
    channels: u32,
    target_mode: Option<&str>,
) -> String {
    pillow_rs::color3dlut_repr(table_type, size, channels, target_mode)
}

/// Prepare kernel parameters for image convolution.
/// - If `kernel` is None, creates a default kernel of all 1.0s
/// - If `scale` is None, computes it as the sum of kernel values
/// - Converts `offset` to i32
/// - Validates that size[0] == size[1] and size[0] in {3, 5}
/// Returns (kernel, scale, offset_i32, size_x).
#[pyfunction]
#[pyo3(signature = (kernel, scale=None, offset=0.0, size=(3, 3)))]
fn kernel_prepare(
    kernel: Option<Vec<f64>>,
    scale: Option<f64>,
    offset: f64,
    size: (u32, u32),
) -> PyResult<(Vec<f64>, f64, i32, u32)> {
    let (size_x, size_y) = size;
    if size_x != size_y || (size_x != 3 && size_x != 5) {
        return Err(pyo3::exceptions::PyNotImplementedError::new_err(format!(
            "Kernel size {}x{} not supported, only 3x3 and 5x5",
            size_x, size_y
        )));
    }
    let numel = (size_x * size_y) as usize;
    let k: Vec<f64> = match kernel {
        Some(k) => k,
        None => vec![1.0_f64; numel],
    };
    if k.len() != numel {
        return Err(PyValueError::new_err(format!(
            "not enough coefficients in kernel (expected {}, got {})",
            numel,
            k.len()
        )));
    }
    let computed_scale = scale.unwrap_or_else(|| k.iter().sum());
    Ok((k, computed_scale, offset as i32, size_x))
}
