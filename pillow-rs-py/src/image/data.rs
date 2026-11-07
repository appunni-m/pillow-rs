use super::RsImage;
use crate::*;
use pyo3::exceptions::PyBufferError;
use pyo3::prelude::PyModule;
use pyo3::pymethods;

fn palette_alpha_bytes(value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let memoryview = PyModule::import(value.py(), "builtins")?
        .getattr("memoryview")?
        .call1((value,))?;
    if !memoryview.getattr("c_contiguous")?.extract::<bool>()? {
        return Err(PyBufferError::new_err(
            "underlying buffer is not C-contiguous",
        ));
    }
    Ok(memoryview
        .call_method0("tobytes")?
        .extract::<Vec<u8>>()?)
}

struct PreparedPalettePutPixel<'py> {
    value: pillow_rs::PutPixelValue,
    deferred_alpha: Option<Bound<'py, PyAny>>,
    reserved: Vec<u32>,
}

fn prepare_palette_putpixel<'py>(
    value: &Bound<'py, PyAny>,
    mode: pillow_rs::Mode,
    palette_mode: Option<&str>,
    info: Option<&Bound<'py, PyDict>>,
) -> PyResult<Option<PreparedPalettePutPixel<'py>>> {
    let is_palette = matches!(
        mode,
        pillow_rs::Mode::ImagePalette8 | pillow_rs::Mode::ImagePaletteAlpha8
    );
    if !is_palette
        || !(value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>())
        || !matches!(value.len()?, 3 | 4)
    {
        return Ok(None);
    }
    let deferred_alpha = if mode == pillow_rs::Mode::ImagePaletteAlpha8 && value.len()? == 4 {
        Some(value.get_item(3)?)
    } else {
        None
    };
    // The palette wrapper precedes coordinate parsing. PA's alpha conversion
    // happens later, after palette allocation and coordinate bounds checks.
    let rgb_only = mode == pillow_rs::Mode::ImagePaletteAlpha8 || palette_mode != Some("RGBA");
    if mode == pillow_rs::Mode::ImagePalette8
        && rgb_only
        && value.len()? == 4
        && !value.get_item(3)?.eq(255)?
    {
        return Err(PyValueError::new_err(
            "cannot add non-opaque RGBA color to RGB palette",
        ));
    }
    let components = PyTuple::new(
        value.py(),
        (0..if rgb_only { 3 } else { value.len()? })
            .map(|index| value.get_item(index))
            .collect::<PyResult<Vec<_>>>()?,
    )?;
    let value = putpixel_value_from_python(components.as_any(), mode)?;
    let mut reserved = Vec::with_capacity(2);
    if let Some(info) = info {
        for key in ["background", "transparency"] {
            if let Some(value) = info.get_item(key)?
                && let Ok(value) = value.extract::<f64>()
                && (0.0..=255.0).contains(&value)
                && value.fract() == 0.0
            {
                reserved.push(value as u32);
            }
        }
    }
    Ok(Some(PreparedPalettePutPixel {
        value,
        deferred_alpha,
        reserved,
    }))
}

// TODO(pillow-rs#3ptb): parity-check negative pixel indices, C-int boundaries,
// and invalid coordinate error order after the CPU source gate.
// Pillow ImagingCore adds each negative coordinate to its matching dimension
// before applying the common bounds check for getpixel and putpixel.
fn image_coordinates_in_bounds(
    image: &pillow_rs::Image,
    (x, y): (i32, i32),
) -> PyResult<(u32, u32)> {
    let (width, height) = image.size().map_err(map_error)?;
    let x = if x < 0 {
        i64::from(width) + i64::from(x)
    } else {
        i64::from(x)
    };
    let y = if y < 0 {
        i64::from(height) + i64::from(y)
    } else {
        i64::from(y)
    };
    if x < 0 || y < 0 || x >= i64::from(width) || y >= i64::from(height) {
        return Err(pyo3::exceptions::PyIndexError::new_err(
            "image index out of range",
        ));
    }
    Ok((x as u32, y as u32))
}

#[pymethods]
impl PyImage {
    #[pyo3(signature = (rawmode=None))]
    fn getpalette_with_input(
        &self,
        rawmode: Option<String>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let palette = self
            .inner
            .getpalette_with_input(rawmode.as_deref())
            .map_err(map_error)?;
        match palette {
            Some(values) => Ok(PyList::new(py, values)?.into_py_any(py)?),
            None => Ok(py.None()),
        }
    }

    fn indexed_color_table(&self) -> PyResult<Vec<(u8, u8, u8)>> {
        self.inner.indexed_color_table().map_err(map_error)
    }

    fn palette_mode(&self) -> Option<String> {
        self.inner.palette_mode().map(str::to_owned)
    }

    fn has_attached_palette(&self) -> bool {
        self.inner.has_attached_palette()
    }

    fn has_transparency_data(&self) -> bool {
        self.inner.has_transparency_data()
    }

    fn apply_transparency(&mut self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.apply_transparency())
            .map_err(map_error)
    }

    #[pyo3(signature = (data, rawmode="RGB"))]
    fn putpalette(
        slf: Py<Self>,
        data: &Bound<'_, PyAny>,
        rawmode: &str,
        py: Python<'_>,
    ) -> PyResult<()> {
        let data = data.extract::<Vec<u8>>()?;
        let rawmode = rawmode.to_owned();
        let mut image = slf.bind(py).borrow_mut();
        let image = &mut image.inner;
        py.detach(|| image.putpalette(&data, &rawmode))
            .map_err(map_error)
    }

    fn putpalettealpha(
        slf: Py<Self>,
        index: &Bound<'_, PyAny>,
        alpha: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let index = index.extract::<i32>()?;
        let alpha = alpha.extract::<i32>()?;
        let mut image = slf.bind(py).borrow_mut();
        let image = &mut image.inner;
        py.detach(|| image.putpalettealpha(index, alpha))
            .map_err(map_error)
    }

    fn putpalettealphas(
        slf: Py<Self>,
        values: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let values = palette_alpha_bytes(values)?;
        let mut image = slf.bind(py).borrow_mut();
        let image = &mut image.inner;
        py.detach(|| image.putpalettealphas(&values))
            .map_err(map_error)
    }

    fn commit_palette_alpha(&mut self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.commit_palette_alpha())
            .map_err(map_error)
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

    fn getim(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        // This is the live Rust-owned image object. Do not fabricate a Pillow
        // Imaging capsule: the core has no compatible C pointer to expose.
        slf
    }

    /// Return getdata formatted as PIL expects.
    fn getdata_formatted(
        slf: Py<Self>,
        band: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        // Extract a custom index object before borrowing the image: its
        // `__index__` method may re-enter and mutate this wrapper.
        let band = band.map(|value| value.extract::<i32>()).transpose()?;
        let image = slf.bind(py).borrow();
        let image = &image.inner;
        if let Some(band) = band {
            if let Some(values) = image.getdata_band_view(band).map_err(map_error)? {
                // Copy owned P/L storage directly into the immutable Python
                // snapshot, avoiding a Rust-side buffer and extra detach.
                return Ok(PyBytes::new(py, values).into());
            }

            if matches!(image, RsImage::Loaded(_) | RsImage::Paletted(_)) {
                let output_len = image.getdata_band_len(band).map_err(map_error)?;
                let bytes = PyBytes::new_with(py, output_len, |output| {
                    py.detach(|| image.getdata_band_into(band, output)).map_err(map_error)
                })?;
                return Ok(bytes.into());
            }

            let output_len = py
                .detach(|| image.getdata_band_len(band))
                .map_err(map_error)?;
            let bytes = PyBytes::new_with(py, output_len, |output| {
                py.detach(|| image.getdata_band_into(band, output)).map_err(map_error)
            })?;
            return Ok(bytes.into());
        }

        let mode = image.mode_identity().map_err(map_error)?;
        if !matches!(
            mode,
            pillow_rs::Mode::ImageI32
                | pillow_rs::Mode::ImageF32
                | pillow_rs::Mode::ImageI16
                | pillow_rs::Mode::ImageI16L
                | pillow_rs::Mode::ImageI16B
                | pillow_rs::Mode::ImageI16N
        ) {
            let values = py.detach(|| image.getdata(None)).map_err(map_error)?;
            return Ok(PyBytes::new(py, &values).into());
        }

        let formatted = py
            .detach(|| image.getdata_formatted(band))
            .map_err(map_error)?;
        Python::attach(|py| match formatted {
            // A selected band is already a one-byte-per-pixel snapshot.
            // Keep it packed for ImagingCore: its sequence interface yields
            // the same integer samples from bytes without allocating a Python
            // integer object for every pixel at getdata() call time.
            pillow_rs::FormattedImageData::Scalars(values) if band.is_some() => {
                Ok(PyBytes::new(py, &values).into())
            }
            // Keep Pillow's byte-sample result as a Python list.  PyO3 0.29
            // specializes `Vec<u8>` conversion to `bytes`, while the former
            // `ToPyObject` conversion used by this binding produced a list.
            pillow_rs::FormattedImageData::Scalars(values) => {
                let out = pyo3::types::PyList::empty(py);
                for value in values {
                    out.append(value)?;
                }
                Ok(out.into_py_any(py)?)
            }
            pillow_rs::FormattedImageData::IntegerScalars(values) => Ok(values.into_py_any(py)?),
            pillow_rs::FormattedImageData::FloatScalars(values) => Ok(values.into_py_any(py)?),
            pillow_rs::FormattedImageData::Components(values) => {
                let out = pyo3::types::PyList::empty(py);
                for value in values {
                    out.append(PyTuple::new(py, value)?)?;
                }
                Ok(out.into_py_any(py)?)
            }
        })
    }

    fn getpixel_formatted(
        slf: Py<Self>,
        xy: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        // Pillow loads before tuple(xy); conversion can execute user code and
        // therefore must happen after releasing the image borrow.
        {
            let inner = slf.bind(py).borrow().inner.clone();
            let (result, inner) = py.detach(move || {
                let mut inner = inner;
                let result = inner.load();
                (result, inner)
            });
            slf.bind(py).borrow_mut().inner = inner;
            result.map_err(map_error)?;
        }
        let coordinates = py
            .import("builtins")?
            .getattr("tuple")?
            .call1((xy,))?
            .cast_into::<PyTuple>()?;
        if coordinates.len()? != 2 {
            return Err(PyValueError::new_err(format!(
                "expected tuple of length 2, but got tuple of length {}",
                coordinates.len()?
            )));
        }
        // Match Pillow's C-int range and error while allowing __index__ hooks
        // to run in tuple order before acquiring an image borrow.
        let x = c_int_from_python(&coordinates.get_item(0)?)?;
        let y = c_int_from_python(&coordinates.get_item(1)?)?;
        let image = slf.bind(py).borrow().inner.clone();
        let xy = image_coordinates_in_bounds(&image, (x, y))?;
        if let Some(rgb) = image.getpixel_rgb_if_loaded(xy.0, xy.1).map_err(map_error)? {
            return PyTuple::new(py, rgb)?.into_py_any(py);
        }
        let value = py
            .detach(move || image.getpixel_formatted(xy.0, xy.1))
            .map_err(map_error)?;
        Python::attach(|py| match value {
            pillow_rs::FormattedPixelValue::Scalar(value) => Ok(value.into_py_any(py)?),
            pillow_rs::FormattedPixelValue::Integer(value) => Ok(value.into_py_any(py)?),
            pillow_rs::FormattedPixelValue::Float(value) => Ok(value.into_py_any(py)?),
            pillow_rs::FormattedPixelValue::Components(values) => {
                Ok(PyTuple::new(py, values)?.into_py_any(py)?)
            }
            pillow_rs::FormattedPixelValue::U16Components(values) => {
                Ok(PyTuple::new(py, values)?.into_py_any(py)?)
            }
            pillow_rs::FormattedPixelValue::FloatComponents(values) => {
                Ok(PyTuple::new(py, values.into_iter().map(f64::from))?.into_py_any(py)?)
            }
            pillow_rs::FormattedPixelValue::InlineComponents { bytes, bands } => Ok(PyTuple::new(
                py,
                bytes.into_iter().take(usize::from(bands)),
            )?
            .into_py_any(py)?),
        })
    }

    #[pyo3(signature = (data, scale=1.0, offset=0.0))]
    fn putdata_formatted(
        slf: &Bound<'_, Self>,
        data: &Bound<'_, PyAny>,
        scale: f64,
        offset: f64,
    ) -> PyResult<()> {
        putdata::putdata_formatted(slf, data, scale, offset)
    }

    /// Mode-aware putpixel: expands values according to PIL's per-mode semantics.
    #[pyo3(signature = (xy, value, info=None))]
    fn putpixel_mode(
        slf: Py<Self>,
        xy: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
        info: Option<&Bound<'_, PyDict>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let (mode, palette_mode) = {
            let image = slf.bind(py).borrow();
            (
                image.inner.mode_identity().map_err(map_error)?,
                image.inner.palette_mode().map(str::to_owned),
            )
        };
        let palette_input =
            prepare_palette_putpixel(value, mode, palette_mode.as_deref(), info)?;
        let (prepared, deferred_alpha) = match palette_input {
            Some(input) => {
                let PreparedPalettePutPixel {
                    value,
                    deferred_alpha,
                    reserved,
                } = input;
                let prepared = slf
                    .bind(py)
                    .borrow_mut()
                    .inner
                    .prepare_putpixel_palette(value, &reserved)
                    .map_err(map_error)?;
                (Some(prepared), deferred_alpha)
            }
            None => (None, None),
        };
        // Coordinate `__index__` callbacks run after palette allocation but
        // without holding a PyImage borrow.
        let coordinates = putpixel_coordinates_from_python(xy)?;
        let (x, y) = {
            let image = slf.bind(py).borrow();
            image_coordinates_in_bounds(&image.inner, coordinates)?
        };
        // `putpixel_value_from_python` must retain flexible coercion and error
        // ordering for arbitrary sequences. Ordinary RGB byte tuples contain
        // exact built-in ints, so write those native channels directly and
        // skip constructing a temporary component vector.
        let coercion_mode = slf
            .bind(py)
            .borrow()
            .inner
            .mode_identity()
            .map_err(map_error)?;
        if coercion_mode == pillow_rs::Mode::ImageRgb8
            && prepared.is_none()
            && let Some([r, g, b]) = exact_rgb_tuple_from_python(value)
        {
            return slf
                .bind(py)
                .borrow_mut()
                .inner
                .putpixel(x, y, r, g, b, 255)
                .map_err(map_error);
        }
        let value = match prepared {
            Some(pillow_rs::PutPixelValue::Components(mut values)) if deferred_alpha.is_some() => {
                if let Some(alpha) = deferred_alpha {
                    values[1] = chops_offset_from_python(&alpha)? as i64;
                }
                pillow_rs::PutPixelValue::Components(values)
            }
            Some(value) => value,
            None => putpixel_value_from_python(value, coercion_mode)?,
        };
        let write_mode = slf
            .bind(py)
            .borrow()
            .inner
            .mode_identity()
            .map_err(map_error)?;
        let mut image = slf.bind(py).borrow_mut();
        let result = if write_mode == pillow_rs::Mode::ImageRgb8 {
            match value {
                pillow_rs::PutPixelValue::Integer(value) => {
                    // RGB integer inks are stored as the low three bytes, so
                    // send that normalized pixel directly instead of
                    // resolving the same mode again in putpixel_value.
                    let bytes = value.to_le_bytes();
                    image
                        .inner
                        .putpixel(x, y, bytes[0], bytes[1], bytes[2], bytes[3])
                }
                value => image.inner.putpixel_value(x, y, value),
            }
        } else {
            image.inner.putpixel_value(x, y, value)
        };
        result.map_err(map_error)
    }
}
