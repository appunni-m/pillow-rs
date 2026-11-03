use crate::*;
use pyo3::{PyRef, pyclass, pyfunction, wrap_pyfunction};
use pyo3::types::{PyAnyMethods, PyTupleMethods};

#[pyclass(name = "_ExpandFill", frozen)]
struct PyExpandFill {
    prepared: pillow_rs::PreparedExpandFill,
}

#[pyclass(name = "_ExpandTarget", frozen)]
struct PyExpandTarget {
    mode: pillow_rs::Mode,
}

#[pyfunction]
fn ops_validate_deform_resample(value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let value = resample_input_from_python(value)?;
    pillow_rs::imageops_validate_deform_resample(value).map_err(map_error)
}

// --- ImageOps module-level functions ---

#[pyfunction]
fn ops_autocontrast_lut(
    py: Python<'_>,
    histogram: &Bound<'_, PyAny>,
    cutoff: Option<&Bound<'_, PyAny>>,
) -> PyResult<Vec<u8>> {
    let histogram = histogram.extract::<Vec<u32>>()?;
    let cutoff_counts = autocontrast_cutoff_counts_from_python(py, &histogram, cutoff)?;
    pillow_rs::imageops_autocontrast_lut_from_histogram_and_counts(&histogram, cutoff_counts)
        .map_err(map_error)
}

#[pyfunction]
fn ops_equalize(image: &Bound<'_, PyImage>, mask: Option<&Bound<'_, PyAny>>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let mask = imageops_mask_from_python(mask)?;
    let rs =
        Python::attach(|py| py.detach(|| pillow_rs::imageops_equalize_with_mask(&inner, mask)))
            .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_invert(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs =
        Python::attach(|py| py.detach(|| pillow_rs::imageops_invert(&inner))).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_flip(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs =
        Python::attach(|py| py.detach(|| pillow_rs::imageops_flip(&inner))).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_mirror(image: &Bound<'_, PyImage>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs =
        Python::attach(|py| py.detach(|| pillow_rs::imageops_mirror(&inner))).map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_posterize(image: &Bound<'_, PyImage>, bits: u8) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::attach(|py| py.detach(|| pillow_rs::imageops_posterize(&inner, bits)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_solarize(image: &Bound<'_, PyImage>, threshold: &Bound<'_, PyAny>) -> PyResult<PyImage> {
    let py = image.py();
    let input = if threshold.is_exact_instance_of::<PyInt>()
        || threshold.is_exact_instance_of::<PyBool>()
    {
        // Compare before narrowing so arbitrarily large Python integers retain
        // the same byte-domain behavior without integer or float overflow.
        let value = if threshold.lt(0)? {
            0.0
        } else if threshold.gt(255)? {
            256.0
        } else {
            f64::from(threshold.extract::<u8>()?)
        };
        pillow_rs::SolarizeInput::Threshold(value)
    } else if threshold.is_exact_instance_of::<PyFloat>() {
        pillow_rs::SolarizeInput::Threshold(threshold.extract::<f64>()?)
    } else {
        let below = pillow_rs::imageops_solarize_comparison_table(|sample| {
            sample.into_pyobject_or_pyerr(py)?.lt(threshold)
        })?;
        pillow_rs::SolarizeInput::Comparisons(below)
    };
    // Custom comparisons can mutate the input image. Snapshot its current
    // Rust handle only after those calls, and propagate their first exception.
    let inner = image.borrow().inner.clone();
    let rs = py
        .detach(|| pillow_rs::imageops_solarize_with_input(&inner, input))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_grayscale(image: &Bound<'_, PyImage>, py: Python<'_>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = py
        .detach(|| pillow_rs::imageops_grayscale(&inner))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_colorize(
    image: &Bound<'_, PyImage>,
    black: &Bound<'_, PyAny>,
    white: &Bound<'_, PyAny>,
    mid: Option<&Bound<'_, PyAny>>,
    blackpoint: u8,
    midpoint: u8,
    whitepoint: u8,
) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    pillow_rs::imageops_validate_colorize_mode(&inner).map_err(map_error)?;
    let black = parse_colorize_color(black)?;
    let white = parse_colorize_color(white)?;
    let mid = mid.map(parse_colorize_color).transpose()?;
    let rs = Python::attach(|py| {
        py.detach(|| {
            pillow_rs::imageops_colorize(
                &inner, black, white, mid, blackpoint, midpoint, whitepoint,
            )
        })
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

fn parse_colorize_color(value: &Bound<'_, PyAny>) -> PyResult<(u8, u8, u8)> {
    if let Ok(color) = value.extract::<String>() {
        let (r, g, b, _) = pillow_rs::parse_color_str(&color).map_err(map_error)?;
        return Ok((r, g, b));
    }
    if let Ok((r, g, b)) = value.extract::<(u8, u8, u8)>() {
        return Ok((r, g, b));
    }
    if let Ok((r, g, b, _)) = value.extract::<(u8, u8, u8, u8)>() {
        return Ok((r, g, b));
    }
    if value.is_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err("'int' object is not subscriptable"));
    }
    Err(PyTypeError::new_err("color must be a color name or tuple"))
}

#[pyfunction]
fn ops_imageops_filter_code(filter: Option<&Bound<'_, PyAny>>) -> PyResult<i32> {
    let filter = resample_input_from_python(filter)?;
    let filter = pillow_rs::parse_imageops_filter_input(filter).map_err(map_error)?;
    Ok(resample_filter_code(filter))
}

#[pyfunction]
fn ops_contain_geometry(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
) -> PyResult<(u32, u32)> {
    let geometry_image = image.borrow().inner.clone();
    Python::attach(|py| {
        py.detach(|| {
            pillow_rs::imageops_contain_geometry_preflight(&geometry_image, size.0, size.1)
        })
    })
    .map_err(map_error)
}

#[pyfunction]
fn ops_cover_geometry(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
) -> PyResult<(u32, u32)> {
    let geometry_image = image.borrow().inner.clone();
    Python::attach(|py| {
        py.detach(|| {
            pillow_rs::imageops_cover_geometry_preflight(&geometry_image, size.0, size.1)
        })
    })
    .map_err(map_error)
}

#[pyfunction]
fn ops_fit_geometry(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    bleed: Option<f64>,
    centering: &Bound<'_, PyAny>,
) -> PyResult<((u32, u32), (f64, f64, f64, f64))> {
    let centering = centering_from_python(centering);
    let geometry_image = image.borrow().inner.clone();
    let box_coords = Python::attach(|py| {
        py.detach(|| {
            pillow_rs::imageops_fit_geometry_preflight(
                &geometry_image,
                size.0,
                size.1,
                bleed.unwrap_or(0.0),
                centering.clone(),
            )
        })
    })
    .map_err(map_error)?;
    Ok((size, box_coords))
}

#[pyfunction]
fn ops_pad(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<&Bound<'_, PyAny>>,
    color: Option<&Bound<'_, PyAny>>,
    centering: &Bound<'_, PyAny>,
) -> PyResult<(PyImage, bool)> {
    let preflight_image = image.borrow().inner.clone();
    let geometry = Python::attach(|py| {
        py.detach(|| pillow_rs::imageops_pad_geometry_preflight(&preflight_image, size.0, size.1))
    })
    .map_err(map_error)?;
    let filter = resample_input_from_python(filter)?;
    let preflight = Python::attach(|py| {
        py.detach(|| pillow_rs::imageops_pad_filter_preflight(geometry, filter))
    })
    .map_err(map_error)?;
    // Keep the contain source snapshot before color/centering protocol hooks
    // can mutate or replace the Python Image wrapper. Loaded raster writes use
    // copy-on-write and lazy pipeline nodes are persistent, so this clone keeps
    // the same logical input that Pillow's eager contain call already captured.
    let inner = image.borrow().inner.clone();
    let (fill, centering) = if preflight.needs_padding() {
        let mode = inner.mode_identity().map_err(map_error)?;
        let color = imageops_pad_color_from_python(image.py(), color, mode)?;
        let fill = pillow_rs::imageops_pad_fill_preflight(&inner, preflight, color)
            .map_err(map_error)?;
        let centering = pad_centering_from_python(
            centering,
            preflight.padded_axes(),
            preflight.padding_gaps(),
            preflight.contained_size(),
        )?;
        (fill, centering)
    } else {
        // Pillow returns `contain(...)` directly when it already fills the
        // requested size, so it never constructs Image.new or indexes
        // centering. Avoid invoking host conversion hooks on ignored values.
        (
            pillow_rs::imageops_pad_fill_preflight(
                &inner,
                preflight,
                pillow_rs::ImageOpsColor::None,
            )
            .map_err(map_error)?,
            pillow_rs::CenteringInput::Default,
        )
    };

    let (rs, uses_contain_result) = Python::attach(|py| {
        py.detach(|| {
            pillow_rs::imageops_pad_with_preflight_and_fill(
                &inner, preflight, fill, centering,
            )
        })
    })
    .map_err(map_error)?;
    Ok((PyImage { inner: rs }, uses_contain_result))
}

#[pyfunction]
fn ops_pad_preflight(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<i64>> {
    let inner = image.borrow().inner.clone();
    let geometry = Python::attach(|py| {
        py.detach(|| pillow_rs::imageops_pad_geometry_preflight(&inner, size.0, size.1))
    })
    .map_err(map_error)?;
    let filter = resample_input_from_python(filter)?;
    let preflight = Python::attach(|py| {
        py.detach(|| pillow_rs::imageops_pad_filter_preflight(geometry, filter))
    })
    .map_err(map_error)?;
    Ok(preflight.normalized_filter_code())
}

#[pyfunction]
fn ops_scale(
    image: &Bound<'_, PyImage>,
    factor: f64,
    filter: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyImage> {
    // ImageOps.scale returns image.copy() for factor == 1 before touching
    // resample. Avoid formatting an arbitrary Python filter in that branch;
    // Pillow likewise ignores even objects whose __str__ raises.
    let filter = if factor == 1.0 {
        None
    } else {
        resample_input_from_python(filter)?
    };
    let inner = image.borrow().inner.clone();
    let rs = Python::attach(|py| {
        py.detach(|| pillow_rs::imageops_scale_with_input(&inner, factor, filter))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_expand_capture_target(
    py: Python<'_>,
    size: &Bound<'_, PyAny>,
    mode_name: &Bound<'_, PyAny>,
) -> PyResult<(PyExpandTarget, (u32, u32))> {
    let size = size.cast::<PyTuple>()?;
    if size.len() != 2 {
        return Err(PyValueError::new_err(
            "Size must be a sequence of length 2",
        ));
    }
    let width = c_int_from_python(&size.get_item(0)?)?;
    let height = c_int_from_python(&size.get_item(1)?)?;
    if width < 0 || height < 0 {
        return Err(PyValueError::new_err("Width and height must be >= 0"));
    }
    // TODO(pillow-rs#3ptb): freeze valid mode overrides and invalid-mode
    // output/error ordering against the Python property value.
    let mode_name = mode_name.extract::<&str>()?;
    let mode = pillow_rs::Mode::from_pillow_name(mode_name)
        .ok_or_else(|| PyValueError::new_err("unrecognized image mode"))?;
    let size = (width as u32, height as u32);
    py.detach(move || pillow_rs::imageops_validate_expand_output_size_for_mode(mode, size))
        .map_err(map_error)?;
    Ok((PyExpandTarget { mode }, size))
}

#[pyfunction]
fn ops_expand_prepare_fill(
    py: Python<'_>,
    target: &Bound<'_, PyExpandTarget>,
    fill: &Bound<'_, PyAny>,
) -> PyResult<PyExpandFill> {
    let mode = target.borrow().mode;
    if let Ok(tuple) = fill.cast::<PyTuple>() {
        if matches!(tuple.len(), 3 | 4) {
            if mode == pillow_rs::Mode::ImagePaletteAlpha8 {
                return Err(PyTypeError::new_err(
                    "color must be int, or tuple of one or two elements",
                ));
            }
            if mode == pillow_rs::Mode::ImagePalette8
                && !tuple
                    .iter()
                    .all(|component| component.is_instance_of::<PyInt>())
            {
                return Err(PyTypeError::new_err(
                    "color must be int or single-element tuple",
                ));
            }
        }
    }
    let input = imageops_expand_color_from_python(Some(fill));
    let prepared = py
        .detach(|| pillow_rs::imageops_prepare_expand_fill_for_mode(mode, input))
        .map_err(map_error)?;
    Ok(PyExpandFill { prepared })
}

#[pyfunction]
fn ops_expand_resolve_palette_fill(
    py: Python<'_>,
    target: &Bound<'_, PyExpandTarget>,
    fill: PyRef<'_, PyExpandFill>,
) -> PyResult<PyExpandFill> {
    let mode = target.borrow().mode;
    let prepared = fill.prepared.clone();
    let prepared = py
        .detach(|| pillow_rs::imageops_finish_expand_fill_palette_from_host(mode, prepared))
        .map_err(map_error)?;
    Ok(PyExpandFill { prepared })
}

#[pyfunction]
fn ops_expand_canvas(
    py: Python<'_>,
    size: (u32, u32),
    fill: PyRef<'_, PyExpandFill>,
) -> PyResult<PyImage> {
    let prepared = fill.prepared.clone();
    let rs = py
        .detach(move || pillow_rs::imageops_expand_canvas_with_prepared_fill(size, &prepared))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_crop_border(image: &Bound<'_, PyImage>, border: u32) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = Python::attach(|py| py.detach(|| pillow_rs::imageops_crop(&inner, border)))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
#[pyo3(signature = (image, in_place=false))]
fn ops_exif_transpose(image: &Bound<'_, PyImage>, in_place: bool) -> PyResult<Option<PyImage>> {
    let inner = image.borrow().inner.clone();
    let result =
        Python::attach(|py| py.detach(|| pillow_rs::imageops_exif_transpose(&inner, in_place)))
            .map_err(map_error)?;

    if in_place {
        if let Some(transposed) = result {
            image.borrow_mut().inner = transposed;
        }
        Ok(None)
    } else {
        Ok(result.map(|inner| PyImage { inner }))
    }
}

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ops_autocontrast_lut, m)?)?;
    m.add_function(wrap_pyfunction!(ops_equalize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_validate_deform_resample, m)?)?;
    m.add_function(wrap_pyfunction!(ops_invert, m)?)?;
    m.add_function(wrap_pyfunction!(ops_flip, m)?)?;
    m.add_function(wrap_pyfunction!(ops_mirror, m)?)?;
    m.add_function(wrap_pyfunction!(ops_posterize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_solarize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_grayscale, m)?)?;
    m.add_function(wrap_pyfunction!(ops_colorize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_imageops_filter_code, m)?)?;
    m.add_function(wrap_pyfunction!(ops_contain_geometry, m)?)?;
    m.add_function(wrap_pyfunction!(ops_cover_geometry, m)?)?;
    m.add_function(wrap_pyfunction!(ops_fit_geometry, m)?)?;
    m.add_function(wrap_pyfunction!(ops_pad_preflight, m)?)?;
    m.add_function(wrap_pyfunction!(ops_pad, m)?)?;
    m.add_function(wrap_pyfunction!(ops_scale, m)?)?;
    m.add_class::<PyExpandTarget>()?;
    m.add_class::<PyExpandFill>()?;
    m.add_function(wrap_pyfunction!(ops_expand_capture_target, m)?)?;
    m.add_function(wrap_pyfunction!(ops_expand_prepare_fill, m)?)?;
    m.add_function(wrap_pyfunction!(ops_expand_resolve_palette_fill, m)?)?;
    m.add_function(wrap_pyfunction!(ops_expand_canvas, m)?)?;
    m.add_function(wrap_pyfunction!(ops_crop_border, m)?)?;
    m.add_function(wrap_pyfunction!(ops_exif_transpose, m)?)?;
    Ok(())
}
