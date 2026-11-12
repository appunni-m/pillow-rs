use super::{PyImage, RsImage};
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
    Ok(memoryview.call_method0("tobytes")?.extract::<Vec<u8>>()?)
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

fn image_sequence_len(width: u32, height: u32) -> PyResult<usize> {
    let length = (width as usize)
        .checked_mul(height as usize)
        .filter(|length| *length <= isize::MAX as usize)
        .ok_or_else(|| {
            pyo3::exceptions::PyOverflowError::new_err(
                "cannot fit 'int' into an index-sized integer",
            )
        })?;
    Ok(length)
}

fn load_pyimage(image: &Py<PyImage>, py: Python<'_>) -> PyResult<()> {
    let inner = image.bind(py).borrow().inner.clone();
    let (result, inner) = py.detach(move || {
        let mut inner = inner;
        let result = inner.load();
        (result, inner)
    });
    image.bind(py).borrow_mut().inner = inner;
    result.map_err(map_error)
}

fn image_sequence_index(index: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<isize> {
    if index.is_instance_of::<pyo3::types::PySlice>() {
        return Err(pyo3::exceptions::PyTypeError::new_err(
            "sequence index must be integer, not 'slice'",
        ));
    }

    let type_name = index.get_type().name()?.to_string();
    let has_index = index.get_type().hasattr("__index__")?;
    let index = match py.import("operator")?.getattr("index")?.call1((index,)) {
        Ok(index) => index,
        Err(error) if !has_index && error.is_instance_of::<pyo3::exceptions::PyTypeError>(py) => {
            return Err(pyo3::exceptions::PyTypeError::new_err(format!(
                "sequence index must be integer, not '{type_name}'"
            )));
        }
        Err(error) => return Err(error),
    };

    index.extract::<isize>().map_err(|error| {
        if error.is_instance_of::<pyo3::exceptions::PyOverflowError>(py) {
            pyo3::exceptions::PyIndexError::new_err("cannot fit 'int' into an index-sized integer")
        } else {
            error
        }
    })
}

fn formatted_pixel_to_python(
    py: Python<'_>,
    value: pillow_rs::FormattedPixelValue,
) -> PyResult<Py<PyAny>> {
    match value {
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
        pillow_rs::FormattedPixelValue::InlineComponents { bytes, bands } => {
            Ok(PyTuple::new(py, bytes.into_iter().take(usize::from(bands)))?.into_py_any(py)?)
        }
    }
}

fn validate_imaging_core_palette_ink(
    value: &Bound<'_, PyAny>,
    mode: pillow_rs::Mode,
) -> PyResult<()> {
    if !matches!(
        mode,
        pillow_rs::Mode::ImagePalette8 | pillow_rs::Mode::ImagePaletteAlpha8
    ) || value.is_instance_of::<PyInt>()
    {
        return Ok(());
    }
    let tuple = value.cast::<PyTuple>().ok();
    let accepted = match mode {
        pillow_rs::Mode::ImagePalette8 => tuple.is_some_and(|tuple| tuple.len().ok() == Some(1)),
        pillow_rs::Mode::ImagePaletteAlpha8 => {
            tuple.is_some_and(|tuple| matches!(tuple.len(), Ok(1 | 2)))
        }
        _ => true,
    };
    if accepted {
        return Ok(());
    }
    let message = if mode == pillow_rs::Mode::ImagePalette8 {
        "color must be int or single-element tuple"
    } else if tuple.is_none() {
        "color must be int or tuple"
    } else {
        "color must be int, or tuple of one or two elements"
    };
    Err(pyo3::exceptions::PyTypeError::new_err(message))
}

fn putpixel_imaging_core(
    slf: Py<PyImage>,
    xy: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<()> {
    let coordinates = putpixel_coordinates_from_python(xy)?;
    putpixel_imaging_core_at(slf, coordinates, value, py)
}

fn putpixel_imaging_core_at(
    slf: Py<PyImage>,
    coordinates: (i32, i32),
    value: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<()> {
    let image = slf.bind(py).borrow().inner.clone();
    let (x, y) = image_coordinates_in_bounds(&image, coordinates)?;
    let mode = image.mode_identity().map_err(map_error)?;
    validate_imaging_core_palette_ink(value, mode)?;
    let value = putpixel_value_from_python(value, mode)?;
    slf.bind(py)
        .borrow_mut()
        .inner
        .putpixel_value(x, y, value)
        .map_err(map_error)
}

fn getpixel_at(
    image: pillow_rs::Image,
    coordinates: (i32, i32),
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
    let xy = image_coordinates_in_bounds(&image, coordinates)?;
    if let Some(rgb) = image
        .getpixel_rgb_if_loaded(xy.0, xy.1)
        .map_err(map_error)?
    {
        return PyTuple::new(py, rgb)?.into_py_any(py);
    }
    let value = py
        .detach(move || image.getpixel_formatted(xy.0, xy.1))
        .map_err(map_error)?;
    formatted_pixel_to_python(py, value)
}

#[pymethods]
impl PyImage {
    fn setmode(&mut self, mode: &str) -> PyResult<()> {
        let target = pillow_rs::Mode::from_pillow_name(mode).ok_or_else(|| {
            map_error(pillow_rs::PilError::ValueError(
                "image has wrong mode".into(),
            ))
        })?;
        self.inner.imaging_core_setmode(target).map_err(map_error)
    }

    fn putband(slf: Py<Self>, source: Py<Self>, band: i32, py: Python<'_>) -> PyResult<()> {
        let source = source.bind(py).borrow().inner.clone();
        slf.bind(py)
            .borrow_mut()
            .inner
            .imaging_core_putband(&source, band)
            .map_err(map_error)
    }

    fn fillband(&mut self, band: i32, color: i32) -> PyResult<()> {
        self.inner
            .imaging_core_fillband(band, color)
            .map_err(map_error)
    }

    fn __len__(&self) -> PyResult<usize> {
        let (width, height) = self.inner.size().map_err(map_error)?;
        image_sequence_len(width, height)
    }

    fn __getitem__(slf: Py<Self>, index: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        load_pyimage(&slf, py)?;
        let index = image_sequence_index(index, py)?;
        let (width, height) = slf.bind(py).borrow().inner.size().map_err(map_error)?;
        let length = image_sequence_len(width, height)?;
        let length = isize::try_from(length).map_err(|_| {
            pyo3::exceptions::PyOverflowError::new_err(
                "cannot fit 'int' into an index-sized integer",
            )
        })?;
        let index = if index < 0 {
            index.saturating_add(length)
        } else {
            index
        };
        if index < 0 || index >= length {
            return Err(pyo3::exceptions::PyIndexError::new_err(
                "image index out of range",
            ));
        }
        let width = isize::try_from(width).map_err(|_| {
            pyo3::exceptions::PyOverflowError::new_err(
                "cannot fit 'int' into an index-sized integer",
            )
        })?;
        let x = (index % width) as u32;
        let y = (index / width) as u32;
        debug_assert!(y < height);

        let image = slf.bind(py).borrow().inner.clone();
        if let Some(rgb) = image.getpixel_rgb_if_loaded(x, y).map_err(map_error)? {
            return PyTuple::new(py, rgb)?.into_py_any(py);
        }
        let value = py
            .detach(move || image.getpixel_formatted(x, y))
            .map_err(map_error)?;
        formatted_pixel_to_python(py, value)
    }

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

    fn putpalettealphas(slf: Py<Self>, values: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<()> {
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

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
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
                    py.detach(|| image.getdata_band_into(band, output))
                        .map_err(map_error)
                })?;
                return Ok(bytes.into());
            }

            let output_len = py
                .detach(|| image.getdata_band_len(band))
                .map_err(map_error)?;
            let bytes = PyBytes::new_with(py, output_len, |output| {
                py.detach(|| image.getdata_band_into(band, output))
                    .map_err(map_error)
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
        load_pyimage(&slf, py)?;
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
        getpixel_at(image, (x, y), py)
    }

    fn getpixel(slf: Py<Self>, xy: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Self::getpixel_formatted(slf, xy, py)
    }

    /// Read through a stored PixelAccess view with Pillow's tuple parser.
    fn pixel_access_getpixel(&self, xy: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let coordinates = pixel_access_coordinates_from_python(xy)?;
        getpixel_at(self.inner.clone(), coordinates, py)
    }

    /// Write through a stored PixelAccess view without public palette lookup.
    fn pixel_access_putpixel(
        slf: Py<Self>,
        xy: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let coordinates = pixel_access_coordinates_from_python(xy)?;
        putpixel_imaging_core_at(slf, coordinates, value, py)
    }

    /// Validate a deletion key; Pillow's PixelAccess deletion is otherwise a no-op.
    fn pixel_access_delete(&self, xy: &Bound<'_, PyAny>) -> PyResult<()> {
        let coordinates = pixel_access_coordinates_from_python(xy)?;
        image_coordinates_in_bounds(&self.inner, coordinates)?;
        Ok(())
    }

    /// Write one pixel through the mode-native ImagingCore callback contract.
    fn putpixel(
        slf: Py<Self>,
        xy: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        putpixel_imaging_core(slf, xy, value, py)
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

    #[pyo3(signature = (data, scale=1.0, offset=0.0))]
    fn putdata(
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
        let palette_input = prepare_palette_putpixel(value, mode, palette_mode.as_deref(), info)?;
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
