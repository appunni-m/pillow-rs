use super::RsImage;
use crate::*;
use pyo3::pymethods;

fn prepare_palette_putpixel<'py>(
    image: &mut pillow_rs::Image,
    value: &Bound<'py, PyAny>,
    mode: &str,
    info: Option<&Bound<'py, PyDict>>,
) -> PyResult<(Option<pillow_rs::PutPixelValue>, Option<Bound<'py, PyAny>>)> {
    let palette_color = matches!(mode, "P" | "PA")
        && (value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>())
        && matches!(value.len()?, 3 | 4);
    let deferred_alpha = if palette_color && mode == "PA" && value.len()? == 4 {
        Some(value.get_item(3)?)
    } else {
        None
    };
    let prepared = if palette_color {
        // The palette wrapper precedes coordinate parsing. PA's alpha
        // conversion happens later, after allocation and bounds checks.
        let rgb_only = mode == "PA" || image.palette_mode() != Some("RGBA");
        if mode == "P" && rgb_only && value.len()? == 4 && !value.get_item(3)?.eq(255)? {
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
        Some(
            image
                .prepare_putpixel_palette(value, &reserved)
                .map_err(map_error)?,
        )
    } else {
        None
    };
    Ok((prepared, deferred_alpha))
}

fn putpixel_coordinates_in_bounds(
    image: &pillow_rs::Image,
    xy: &Bound<'_, PyAny>,
) -> PyResult<(u32, u32)> {
    let (x, y) = putpixel_coordinates_from_python(xy)?;
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

    fn indexed_color_table(&self, mode: &str) -> PyResult<Vec<(u8, u8, u8)>> {
        self.inner.indexed_color_table(mode).map_err(map_error)
    }

    fn palette_mode(&self) -> Option<String> {
        self.inner.palette_mode().map(str::to_owned)
    }

    fn has_transparency_data(&self) -> bool {
        self.inner.has_transparency_data()
    }

    fn apply_transparency(&mut self, py: Python<'_>) -> PyResult<()> {
        py.detach(|| self.inner.apply_transparency())
            .map_err(map_error)
    }

    #[pyo3(signature = (data, rawmode="RGB"))]
    fn putpalette(&mut self, data: Vec<u8>, rawmode: &str, py: Python<'_>) -> PyResult<()> {
        let rawmode = rawmode.to_owned();
        py.detach(|| self.inner.putpalette(&data, &rawmode))
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
    fn getdata_formatted(&mut self, band: Option<i32>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(band) = band {
            if let Some(values) = self.inner.getdata_band_view(band).map_err(map_error)? {
                // Copy owned P/L storage directly into the immutable Python
                // snapshot, avoiding a Rust-side buffer and extra detach.
                return Ok(PyBytes::new(py, values).into());
            }

            if matches!(&self.inner, RsImage::Loaded(_) | RsImage::Paletted(_)) {
                let output_len = self.inner.getdata_band_len(band).map_err(map_error)?;
                let bytes = PyBytes::new_with(py, output_len, |output| {
                    py.detach(|| self.inner.getdata_band_into(band, output))
                        .map_err(map_error)
                })?;
                return Ok(bytes.into());
            }

            let output_len = py
                .detach(|| self.inner.getdata_band_len(band))
                .map_err(map_error)?;
            let bytes = PyBytes::new_with(py, output_len, |output| {
                py.detach(|| self.inner.getdata_band_into(band, output))
                    .map_err(map_error)
            })?;
            return Ok(bytes.into());
        }

        let mode = self.inner.mode().map_err(map_error)?;
        if !matches!(
            mode.as_str(),
            "I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N"
        ) {
            let values = py.detach(|| self.inner.getdata(None)).map_err(map_error)?;
            return Ok(PyBytes::new(py, &values).into());
        }

        let formatted = py
            .detach(|| self.inner.getdata_formatted(band))
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

    fn getpixel_formatted(&self, xy: (u32, u32), py: Python<'_>) -> PyResult<Py<PyAny>> {
        if let Some(rgb) = self
            .inner
            .getpixel_rgb_if_loaded(xy.0, xy.1)
            .map_err(map_error)?
        {
            return PyTuple::new(py, rgb)?.into_py_any(py);
        }
        let value = py
            .detach(|| self.inner.getpixel_formatted(xy.0, xy.1))
            .map_err(map_error)?;
        Python::attach(|py| match value {
            pillow_rs::FormattedPixelValue::Scalar(value) => Ok(value.into_py_any(py)?),
            pillow_rs::FormattedPixelValue::Integer(value) => Ok(value.into_py_any(py)?),
            pillow_rs::FormattedPixelValue::Float(value) => Ok(value.into_py_any(py)?),
            pillow_rs::FormattedPixelValue::Components(values) => {
                Ok(PyTuple::new(py, values)?.into_py_any(py)?)
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
        &mut self,
        xy: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
        info: Option<&Bound<'_, PyDict>>,
    ) -> PyResult<()> {
        let mode = self.inner.mode().map_err(map_error)?;
        let (prepared, deferred_alpha) =
            prepare_palette_putpixel(&mut self.inner, value, &mode, info)?;
        let (x, y) = putpixel_coordinates_in_bounds(&self.inner, xy)?;
        // `putpixel_value_from_python` must retain flexible coercion and error
        // ordering for arbitrary sequences. Ordinary RGB byte tuples contain
        // exact built-in ints, so write those native channels directly and
        // skip constructing a temporary component vector.
        if mode == "RGB"
            && prepared.is_none()
            && let Some([r, g, b]) = exact_rgb_tuple_from_python(value)
        {
            return self.inner.putpixel(x, y, r, g, b, 255).map_err(map_error);
        }
        let value = match prepared {
            Some(pillow_rs::PutPixelValue::Components(mut values)) if deferred_alpha.is_some() => {
                if let Some(alpha) = deferred_alpha {
                    values[1] = chops_offset_from_python(&alpha)? as i64;
                }
                pillow_rs::PutPixelValue::Components(values)
            }
            Some(value) => value,
            None => putpixel_value_from_python(value, &mode)?,
        };
        let result = if mode == "RGB" {
            match value {
                pillow_rs::PutPixelValue::Integer(value) => {
                    // The mode was already resolved above for Python value
                    // parsing. RGB integer inks are stored as the low three
                    // bytes, so send that normalized pixel directly instead
                    // of resolving the same mode again in putpixel_value.
                    let bytes = value.to_le_bytes();
                    self.inner
                        .putpixel(x, y, bytes[0], bytes[1], bytes[2], bytes[3])
                }
                value => self.inner.putpixel_value(x, y, value),
            }
        } else {
            self.inner.putpixel_value(x, y, value)
        };
        result.map_err(map_error)
    }
}
