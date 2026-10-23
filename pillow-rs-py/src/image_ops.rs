use crate::*;
use pyo3::{pyfunction, wrap_pyfunction};

#[pyfunction]
fn ops_validate_deform_resample(value: Option<&Bound<'_, PyAny>>) -> PyResult<()> {
    let value = resample_input_from_python(value)?;
    pillow_rs::imageops_validate_deform_resample(value).map_err(map_error)
}

// --- ImageOps module-level functions ---

#[pyfunction]
fn ops_autocontrast(
    image: &Bound<'_, PyImage>,
    cutoff: Option<f64>,
    mask: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let c = cutoff.unwrap_or(0.0);
    let mask = imageops_mask_from_python(mask)?;
    let rs = Python::attach(|py| {
        py.detach(|| pillow_rs::imageops_autocontrast_with_mask(&inner, c, mask))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
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
        let mut below = Box::new([false; 256]);
        for (sample, result) in below.iter_mut().enumerate() {
            *result = sample.into_pyobject_or_pyerr(py)?.lt(threshold)?;
        }
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
fn ops_contain(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyImage> {
    let filter = resample_input_from_python(filter)?;
    let inner = image.borrow().inner.clone();
    let rs = Python::attach(|py| {
        py.detach(|| pillow_rs::imageops_contain_with_input(&inner, size.0, size.1, filter))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_cover(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyImage> {
    let filter = resample_input_from_python(filter)?;
    let inner = image.borrow().inner.clone();
    let rs = Python::attach(|py| {
        py.detach(|| pillow_rs::imageops_cover_with_input(&inner, size.0, size.1, filter))
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

#[pyfunction]
fn ops_fit(
    image: &Bound<'_, PyImage>,
    size: (u32, u32),
    filter: Option<&Bound<'_, PyAny>>,
    bleed: Option<f64>,
    centering: &Bound<'_, PyAny>,
) -> PyResult<PyImage> {
    let filter = resample_input_from_python(filter)?;
    let centering = centering_from_python(centering);
    let inner = image.borrow().inner.clone();
    let rs = Python::attach(|py| {
        py.detach(|| {
            pillow_rs::imageops_fit_with_input(
                &inner,
                size.0,
                size.1,
                filter,
                bleed.unwrap_or(0.0),
                centering,
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
    filter: Option<&Bound<'_, PyAny>>,
    color: Option<&Bound<'_, PyAny>>,
    centering: &Bound<'_, PyAny>,
) -> PyResult<PyImage> {
    let filter = resample_input_from_python(filter)?;
    let centering = centering_from_python(centering);
    let color = imageops_color_from_python(color);

    let inner = image.borrow().inner.clone();
    let rs = Python::attach(|py| {
        py.detach(|| {
            pillow_rs::imageops_pad_with_input(&inner, size.0, size.1, filter, color, centering)
        })
    })
    .map_err(map_error)?;
    Ok(PyImage { inner: rs })
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

    // Preserve the public scalar/string/component input until core knows the
    // image mode. CMYK and RGBX use four-byte storage too, but their fourth
    // samples are K and X rather than RGBA alpha.
    let fill = imageops_color_from_python(Some(fill));

    let inner = image.borrow().inner.clone();
    let rs = Python::attach(|py| {
        py.detach(|| pillow_rs::imageops_expand_with_input(&inner, border_val, fill))
    })
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
    m.add_function(wrap_pyfunction!(ops_autocontrast, m)?)?;
    m.add_function(wrap_pyfunction!(ops_equalize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_validate_deform_resample, m)?)?;
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
    m.add_function(wrap_pyfunction!(ops_exif_transpose, m)?)?;
    Ok(())
}
