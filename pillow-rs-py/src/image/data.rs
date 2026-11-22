use super::execution::{ExecutionRoute, route_from_context, route_pending_image};
use super::{PyImage, RsImage};
use crate::*;
use pyo3::exceptions::PyBufferError;
use pyo3::prelude::PyModule;
use pyo3::pymethods;
use pyo3::types::PyTupleMethods;

type ImageExecutionRoute = (Option<pillow_rs::Backend>, pillow_rs::ExecutionContext);

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn update_image_with_route<T: Send>(
    slf: Py<PyImage>,
    py: Python<'_>,
    route: ExecutionRoute,
    operation: impl FnOnce(&mut pillow_rs::Image, ExecutionRoute) -> Result<T, pillow_rs::PilError>
    + Send,
) -> PyResult<T> {
    let inner = slf.bind(py).borrow().inner.clone();
    let (result, inner) = py.detach(move || {
        let mut inner = inner;
        let result = operation(&mut inner, route);
        (result, inner)
    });
    slf.bind(py).borrow_mut().inner = inner;
    result.map_err(map_error)
}

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
// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn load_pyimage(image: &Py<PyImage>, route: ExecutionRoute, py: Python<'_>) -> PyResult<()> {
    let inner = image.bind(py).borrow().inner.clone();
    let (result, inner) = py.detach(move || {
        let mut inner = route_pending_image(inner, route);
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

// VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
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
        pillow_rs::Mode::ImagePalette8 => tuple.is_some_and(|tuple| tuple.len() == 1),
        pillow_rs::Mode::ImagePaletteAlpha8 => {
            tuple.is_some_and(|tuple| matches!(tuple.len(), 1 | 2))
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

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn putpixel_imaging_core(
    slf: Py<PyImage>,
    xy: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<()> {
    let coordinates = putpixel_coordinates_from_python(xy)?;
    let route = route_from_context(None, py);
    putpixel_imaging_core_at(slf, coordinates, value, route, py)
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn putpixel_imaging_core_at(
    slf: Py<PyImage>,
    coordinates: (i32, i32),
    value: &Bound<'_, PyAny>,
    route: ExecutionRoute,
    py: Python<'_>,
) -> PyResult<()> {
    let image = slf.bind(py).borrow().inner.clone();
    let (x, y) = image_coordinates_in_bounds(&image, coordinates)?;
    let mode = image.mode_identity().map_err(map_error)?;
    validate_imaging_core_palette_ink(value, mode)?;
    let value = putpixel_value_from_python(value, mode)?;
    let mut image = slf.bind(py).borrow_mut();
    putpixel_value_with_route(&mut image.inner, x, y, value, route).map_err(map_error)
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn getpixel_at(
    image: pillow_rs::Image,
    coordinates: (i32, i32),
    execution_route: Option<ImageExecutionRoute>,
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
    let xy = image_coordinates_in_bounds(&image, coordinates)?;
    if let Some(rgb) = image
        .getpixel_rgb_if_loaded_with_execution_route(xy.0, xy.1, execution_route)
        .map_err(map_error)?
    {
        return PyTuple::new(py, rgb)?.into_py_any(py);
    }
    let value = py
        .detach(move || image.getpixel_formatted_with_execution_route(xy.0, xy.1, execution_route))
        .map_err(map_error)?;
    formatted_pixel_to_python(py, value)
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn putpixel_value_with_route(
    image: &mut pillow_rs::Image,
    x: u32,
    y: u32,
    value: pillow_rs::PutPixelValue,
    route: ExecutionRoute,
) -> Result<(), pillow_rs::PilError> {
    let result = match route {
        Some(route) => {
            image.putpixel_value_with_context(x, y, value, route.requested_backend, route.context)
        }
        None => image.putpixel_value(x, y, value),
    };
    if result.is_ok()
        && let Some(backend) = route.and_then(|route| route.pinned_backend)
    {
        *image = image.clone().use_backend(backend);
    }
    result
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn putpixel_exact_rgb_tuple(
    image: &Bound<'_, PyImage>,
    x: u32,
    y: u32,
    [red, green, blue]: [u8; 3],
    route: ExecutionRoute,
) -> PyResult<()> {
    let mut image = image.borrow_mut();
    let result = match route {
        Some(route) => {
            image
                .inner
                .putpixel_with_execution_context(x, y, red, green, blue, 255, route.context)
        }
        None => image.inner.putpixel(x, y, red, green, blue, 255),
    };
    if result.is_ok()
        && let Some(backend) = route.and_then(|route| route.pinned_backend)
    {
        image.inner = image.inner.clone().use_backend(backend);
    }
    result.map_err(map_error)
}

#[pymethods]
impl PyImage {
    fn __len__(&self) -> PyResult<usize> {
        let (width, height) = self.inner.size().map_err(map_error)?;
        image_sequence_len(width, height)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn __getitem__(slf: Py<Self>, index: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let route = route_from_context(None, py);
        load_pyimage(&slf, route, py)?;
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

    // TODO(pillow-rs#3ptb): verify packed palette bytes, rawmode faults, the
    // missing-palette error, and direct-filter callback output in CPU parity.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    #[pyo3(signature = (*args))]
    fn getpalette(&self, args: &Bound<'_, PyTuple>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if args.len() > 1 {
            return Err(PyTypeError::new_err(format!(
                "function takes at most 1 argument ({} given)",
                args.len()
            )));
        }
        let rawmode = if args.is_empty() {
            self.inner.palette_mode().map(str::to_owned)
        } else {
            Some(args.get_item(0)?.extract::<String>()?)
        };
        let rawmode = rawmode.as_deref().unwrap_or("RGB");
        let palette = self
            .inner
            .getpalette_rawmode(rawmode)
            .map_err(map_error)?
            .ok_or_else(|| PyValueError::new_err("image has no palette"))?;
        Ok(PyBytes::new(py, &palette).into())
    }

    fn indexed_color_table(&self) -> PyResult<Vec<(u8, u8, u8)>> {
        self.inner.indexed_color_table().map_err(map_error)
    }

    fn palette_mode(&self) -> Option<String> {
        self.inner.palette_mode().map(str::to_owned)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    fn getpalettemode(&self) -> PyResult<String> {
        self.inner
            .palette_mode()
            .map(str::to_owned)
            .ok_or_else(|| PyValueError::new_err("image has no palette"))
    }

    fn has_attached_palette(&self) -> bool {
        self.inner.has_attached_palette()
    }

    fn has_transparency_data(&self) -> bool {
        self.inner.has_transparency_data()
    }

    // TODO(pillow-rs#3ptb): verify transparency commits, palette metadata,
    // errors, and route receipts at the active materialization boundary.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn apply_transparency(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, |image, route| match route {
            Some(route) => {
                image.apply_transparency_with_execution_context(route.pinned_backend, route.context)
            }
            None => image.apply_transparency(),
        })
    }

    // TODO(pillow-rs#3ptb): verify palette edits, partial mutations on errors,
    // and route receipts for public and callback-created calls.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (data, rawmode="RGB"))]
    fn putpalette(
        slf: Py<Self>,
        data: &Bound<'_, PyAny>,
        rawmode: &str,
        py: Python<'_>,
    ) -> PyResult<()> {
        let data = data.extract::<Vec<u8>>()?;
        let rawmode = rawmode.to_owned();
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, move |image, route| match route {
            Some(route) => image.putpalette_with_execution_context(
                &data,
                &rawmode,
                route.pinned_backend,
                route.context,
            ),
            None => image.putpalette(&data, &rawmode),
        })
    }

    // TODO(pillow-rs#3ptb): verify single-entry alpha writes, truncation,
    // errors, and active-route receipts after any source materialization.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn putpalettealpha(
        slf: Py<Self>,
        index: &Bound<'_, PyAny>,
        alpha: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let index = index.extract::<i32>()?;
        let alpha = alpha.extract::<i32>()?;
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, move |image, route| match route {
            Some(route) => image.putpalettealpha_with_execution_context(
                index,
                alpha,
                route.pinned_backend,
                route.context,
            ),
            None => image.putpalettealpha(index, alpha),
        })
    }

    // TODO(pillow-rs#3ptb): verify alpha-table prefix semantics, buffer faults,
    // errors, and active-route receipts after any source materialization.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn putpalettealphas(slf: Py<Self>, values: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<()> {
        let values = palette_alpha_bytes(values)?;
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, move |image, route| match route {
            Some(route) => image.putpalettealphas_with_execution_context(
                &values,
                route.pinned_backend,
                route.context,
            ),
            None => image.putpalettealphas(&values),
        })
    }

    // TODO(pillow-rs#3ptb): verify raw-palette alpha commit output and route
    // receipts for pending decoder and callback-created images.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn commit_palette_alpha(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, |image, route| match route {
            Some(route) => image
                .commit_palette_alpha_with_execution_context(route.pinned_backend, route.context),
            None => image.commit_palette_alpha(),
        })
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
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (band=None, execution_context=None))]
    fn getdata_formatted(
        slf: Py<Self>,
        band: Option<&Bound<'_, PyAny>>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        // Extract a custom index object before borrowing the image: its
        // `__index__` method may re-enter and mutate this wrapper.
        let band = band.map(|value| value.extract::<i32>()).transpose()?;
        let route = route_from_context(execution_context, py);
        let execution_route = route.map(|route| (route.requested_backend, route.context));
        let image = slf.bind(py).borrow().inner.clone();
        let image = route_pending_image(image, route);
        let image = &image;
        if let Some(band) = band {
            if let Some(values) = image
                .getdata_band_view_with_execution_route(band, execution_route)
                .map_err(map_error)?
            {
                // Copy owned P/L storage directly into the immutable Python
                // snapshot, avoiding a Rust-side buffer and extra detach.
                return Ok(PyBytes::new(py, values).into());
            }

            if matches!(image, RsImage::Loaded(_) | RsImage::Paletted(_)) {
                let output_len = image.getdata_band_len(band).map_err(map_error)?;
                let bytes = PyBytes::new_with(py, output_len, |output| {
                    py.detach(|| {
                        image.getdata_band_into_with_execution_route(band, output, execution_route)
                    })
                    .map_err(map_error)
                })?;
                return Ok(bytes.into());
            }

            let output_len = py
                .detach(|| image.getdata_band_len(band))
                .map_err(map_error)?;
            let bytes = PyBytes::new_with(py, output_len, |output| {
                py.detach(|| {
                    image.getdata_band_into_with_execution_route(band, output, execution_route)
                })
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
            let values = py
                .detach(|| image.getdata_with_execution_route(None, execution_route))
                .map_err(map_error)?;
            return Ok(PyBytes::new(py, &values).into());
        }

        let formatted = py
            .detach(|| image.getdata_formatted_with_execution_route(band, execution_route))
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

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (xy, execution_context=None))]
    fn getpixel_formatted(
        slf: Py<Self>,
        xy: &Bound<'_, PyAny>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        // Pillow loads before tuple(xy); conversion can execute user code and
        // therefore must happen after releasing the image borrow.
        let route = route_from_context(execution_context, py);
        load_pyimage(&slf, route, py)?;
        let coordinates = py
            .import("builtins")?
            .getattr("tuple")?
            .call1((xy,))?
            .cast_into::<PyTuple>()?;
        if coordinates.len() != 2 {
            return Err(PyValueError::new_err(format!(
                "expected tuple of length 2, but got tuple of length {}",
                coordinates.len()
            )));
        }
        // Match Pillow's C-int range and error while allowing __index__ hooks
        // to run in tuple order before acquiring an image borrow.
        let x = c_int_from_python(&coordinates.get_item(0)?)?;
        let y = c_int_from_python(&coordinates.get_item(1)?)?;
        let image = slf.bind(py).borrow().inner.clone();
        let execution_route = route.map(|route| (route.requested_backend, route.context));
        getpixel_at(image, (x, y), execution_route, py)
    }

    fn getpixel(slf: Py<Self>, xy: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Self::getpixel_formatted(slf, xy, None, py)
    }

    /// Read through a stored PixelAccess view with Pillow's tuple parser.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (xy, execution_context=None))]
    fn pixel_access_getpixel(
        &self,
        xy: &Bound<'_, PyAny>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let coordinates = pixel_access_coordinates_from_python(xy)?;
        let route = route_from_context(execution_context, py);
        let execution_route = route.map(|route| (route.requested_backend, route.context));
        getpixel_at(self.inner.clone(), coordinates, execution_route, py)
    }

    /// Write through a stored PixelAccess view without public palette lookup.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (xy, value, execution_context=None))]
    fn pixel_access_putpixel(
        slf: Py<Self>,
        xy: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let coordinates = pixel_access_coordinates_from_python(xy)?;
        let route = route_from_context(execution_context, py);
        putpixel_imaging_core_at(slf, coordinates, value, route, py)
    }

    /// Validate a deletion key; Pillow's PixelAccess deletion is otherwise a no-op.
    // VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
    fn pixel_access_delete(&self, xy: &Bound<'_, PyAny>) -> PyResult<()> {
        let coordinates = pixel_access_coordinates_from_python(xy)?;
        image_coordinates_in_bounds(&self.inner, coordinates)?;
        Ok(())
    }

    /// Creates a mutable PixelAccess view with this image's active route.
    // TODO(pillow-rs#3ptb): verify direct factory result, coordinate and pixel
    // faults, mutation visibility, and selected/fallback receipts.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn pixel_access(slf: Py<Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let route = super::execution::route_from_context(None, py);
        let execution_context = match route {
            Some(route) => super::execution::PyExecutionContext {
                inner: route.context,
                requested_backend: route.requested_backend,
                pinned_backend: route.pinned_backend,
            },
            None => {
                let image = slf.bind(py).borrow().inner.clone();
                let (requested_backend, inner) =
                    image.capture_cpu_execution_route().map_err(map_error)?;
                super::execution::PyExecutionContext {
                    inner,
                    requested_backend,
                    pinned_backend: image.backend(),
                }
            }
        };
        let execution_context = Py::new(py, execution_context)?;
        let pixel_access = py.import("pillow_rs.image")?.getattr("PixelAccess")?;
        Ok(pixel_access.call1((slf, execution_context))?.unbind())
    }

    /// Write one pixel through the mode-native ImagingCore callback contract.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn putpixel(
        slf: Py<Self>,
        xy: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        putpixel_imaging_core(slf, xy, value, py)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (data, scale=1.0, offset=0.0, execution_context=None))]
    fn putdata_formatted(
        slf: &Bound<'_, Self>,
        data: &Bound<'_, PyAny>,
        scale: f64,
        offset: f64,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let route = route_from_context(execution_context, py);
        let execution_route = route.map(|route| (route.requested_backend, route.context));
        let result = putdata::putdata_formatted(slf, data, scale, offset, execution_route);
        if result.is_ok()
            && let Some(backend) = route.and_then(|route| route.pinned_backend)
        {
            let mut image = slf.try_borrow_mut()?;
            image.inner = image.inner.clone().use_backend(backend);
        }
        result
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (data, scale=1.0, offset=0.0, execution_context=None))]
    fn putdata(
        slf: &Bound<'_, Self>,
        data: &Bound<'_, PyAny>,
        scale: f64,
        offset: f64,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let route = route_from_context(execution_context, py);
        let execution_route = route.map(|route| (route.requested_backend, route.context));
        let result = putdata::putdata_formatted(slf, data, scale, offset, execution_route);
        if result.is_ok()
            && let Some(backend) = route.and_then(|route| route.pinned_backend)
        {
            let mut image = slf.try_borrow_mut()?;
            image.inner = image.inner.clone().use_backend(backend);
        }
        result
    }

    /// Mode-aware putpixel: expands values according to PIL's per-mode semantics.
    #[pyo3(signature = (xy, value, info=None, execution_context=None))]
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn putpixel_mode(
        slf: Py<Self>,
        xy: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
        info: Option<&Bound<'_, PyDict>>,
        execution_context: Option<pyo3::PyRef<'_, super::PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let route = route_from_context(execution_context, py);
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
            return putpixel_exact_rgb_tuple(slf.bind(py), x, y, [r, g, b], route);
        }
        let value = match prepared {
            Some(pillow_rs::PutPixelValue::Components(mut values)) if deferred_alpha.is_some() => {
                if let Some(alpha) = deferred_alpha {
                    values[1] = i64::from(c_int_from_python(&alpha)?);
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
                    let result = match route {
                        Some(route) => image.inner.putpixel_with_execution_context(
                            x,
                            y,
                            bytes[0],
                            bytes[1],
                            bytes[2],
                            bytes[3],
                            route.context,
                        ),
                        None => image
                            .inner
                            .putpixel(x, y, bytes[0], bytes[1], bytes[2], bytes[3]),
                    };
                    if result.is_ok()
                        && let Some(backend) = route.and_then(|route| route.pinned_backend)
                    {
                        image.inner = image.inner.clone().use_backend(backend);
                    }
                    result
                }
                value => putpixel_value_with_route(&mut image.inner, x, y, value, route),
            }
        } else {
            putpixel_value_with_route(&mut image.inner, x, y, value, route)
        };
        result.map_err(map_error)
    }
}
