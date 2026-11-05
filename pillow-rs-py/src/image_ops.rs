use crate::*;
use pyo3::basic::CompareOp;
use pyo3::prelude::{PyErr, PyModule};
use pyo3::{PyRef, pyclass, pyfunction, wrap_pyfunction};
use pyo3::types::{PyAnyMethods, PyFloat, PyModuleMethods, PyTuple, PyTupleMethods};

#[pyclass(name = "_ExpandFill", frozen)]
struct PyExpandFill {
    prepared: pillow_rs::PreparedExpandFill,
}

#[pyclass(name = "_ExpandTarget", frozen)]
struct PyExpandTarget {
    mode: pillow_rs::Mode,
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
fn ops_equalize_lut(
    image: &Bound<'_, PyImage>,
    histogram: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<Vec<u8>> {
    // TODO(pillow-rs#3ptb): Preserve arbitrary subclass histogram return
    // objects through Pillow's slicing and iteration instead of narrowing to
    // the native Vec<u32> result here.
    let histogram = histogram.extract::<Vec<u32>>()?;
    let inner = image.borrow().inner.clone();
    py.detach(|| pillow_rs::imageops_equalize_lut_from_histogram(&inner, &histogram))
        .map_err(map_error)
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
fn ops_colorize_lut(
    py: Python<'_>,
    black: &Bound<'_, PyAny>,
    white: &Bound<'_, PyAny>,
    mid: Option<&Bound<'_, PyAny>>,
    blackpoint: &Bound<'_, PyAny>,
    midpoint: &Bound<'_, PyAny>,
    whitepoint: &Bound<'_, PyAny>,
) -> PyResult<Vec<u8>> {
    let black = parse_colorize_color(black)?;
    let white = parse_colorize_color(white)?;
    let mid = mid.map(parse_colorize_color).transpose()?;
    let low = colorize_range_length(py, blackpoint)?;
    let (first, second) = if mid.is_some() {
        (
            colorize_subtraction_range_length(py, midpoint, blackpoint)?,
            colorize_subtraction_range_length(py, whitepoint, midpoint)?,
        )
    } else {
        (
            colorize_subtraction_range_length(py, whitepoint, blackpoint)?,
            0,
        )
    };
    let high_difference = py
        .import("operator")?
        .getattr("sub")?
        .call1((256, whitepoint))?;
    let high = colorize_range_length(py, &high_difference)?;
    pillow_rs::imageops_colorize_lut_from_range_lengths(
        black,
        white,
        mid,
        (low, first, second, high),
    )
    .map_err(map_error)
}

#[pyfunction]
fn ops_scale_geometry_preflight(
    py: Python<'_>,
    image: &Bound<'_, PyImage>,
    factor: f64,
) -> PyResult<(Py<PyAny>, Py<PyAny>)> {
    let inner = image.borrow().inner.clone();
    let (width, height) = py
        .detach(|| pillow_rs::imageops_scale_geometry_preflight(&inner, factor))
        .map_err(map_error)?;
    let round = py.import("builtins")?.getattr("round")?;
    let width = round.call1((width,))?.unbind();
    let height = round.call1((height,))?.unbind();
    Ok((width, height))
}

#[pyfunction]
fn ops_scale_size_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    factor: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonScaleCallbacks::new(py, image);
    pillow_rs::imageops_scale_size_with_callbacks(&mut callbacks, factor)
        .map_err(aspect_callback_error)
        .map(|size| size.unbind())
}

#[pyfunction]
fn ops_contain_geometry_preflight(
    py: Python<'_>,
    image: &Bound<'_, PyImage>,
    width: u32,
    height: u32,
) -> PyResult<(u32, u32)> {
    let inner = image.borrow().inner.clone();
    py.detach(|| pillow_rs::imageops_contain_geometry_preflight(&inner, width, height))
        .map_err(map_error)
}

#[pyfunction]
fn ops_cover_geometry_preflight(
    py: Python<'_>,
    image: &Bound<'_, PyImage>,
    width: u32,
    height: u32,
) -> PyResult<(u32, u32)> {
    let inner = image.borrow().inner.clone();
    py.detach(|| pillow_rs::imageops_cover_geometry_preflight(&inner, width, height))
        .map_err(map_error)
}

enum AspectCallbackError {
    Core(pillow_rs::PilError),
    Python(PyErr),
}

impl From<pillow_rs::PilError> for AspectCallbackError {
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

struct PythonScaleCallbacks<'py> {
    py: Python<'py>,
    image: Bound<'py, PyAny>,
}

impl<'py> PythonScaleCallbacks<'py> {
    fn new(py: Python<'py>, image: &Bound<'py, PyAny>) -> Self {
        Self {
            py,
            image: image.clone(),
        }
    }
}

struct PythonAspectCallbacks<'py> {
    py: Python<'py>,
    image: Bound<'py, PyAny>,
    size: Bound<'py, PyAny>,
}

struct PythonBorderCallbacks<'py> {
    py: Python<'py>,
    image: Bound<'py, PyAny>,
}

impl<'py> pillow_rs::ImageOpsBorderCallbacks for PythonBorderCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Error = PyErr;

    fn border_is_tuple(&mut self, border: &Self::Value) -> Result<bool, Self::Error> {
        Ok(border.is_instance_of::<PyTuple>())
    }

    fn border_length(&mut self, border: &Self::Value) -> Result<usize, Self::Error> {
        border.len()
    }

    fn border_coordinate(
        &mut self,
        border: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error> {
        border.get_item(index)
    }
}

impl<'py> pillow_rs::ImageOpsCropCallbacks for PythonBorderCallbacks<'py> {
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        let size = self.image.getattr("size")?;
        size.get_item(index)
    }

    fn subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.sub(right)
    }

    fn box_tuple(
        &mut self,
        left: &Self::Value,
        top: &Self::Value,
        right: &Self::Value,
        bottom: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        PyTuple::new(
            self.py,
            [left.clone(), top.clone(), right.clone(), bottom.clone()],
        )
        .map(|box_| box_.into_any())
    }
}

impl<'py> pillow_rs::ImageOpsExpandGeometryCallbacks for PythonBorderCallbacks<'py> {
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        let size = self.image.getattr("size")?;
        size.get_item(index)
    }

    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error> {
        left.add(right)
    }

    fn expand_geometry_tuple(
        &mut self,
        width: &Self::Value,
        height: &Self::Value,
        left: &Self::Value,
        top: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        PyTuple::new(
            self.py,
            [width.clone(), height.clone(), left.clone(), top.clone()],
        )
        .map(|geometry| geometry.into_any())
    }
}

impl<'py> PythonAspectCallbacks<'py> {
    fn new(
        py: Python<'py>,
        image: &Bound<'py, PyAny>,
        size: &Bound<'py, PyAny>,
    ) -> Self {
        Self {
            py,
            image: image.clone(),
            size: size.clone(),
        }
    }

}

impl<'py> pillow_rs::ImageOpsAspectCallbacks for PythonAspectCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Error = AspectCallbackError;

    fn image_width(&mut self) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("width")
            .map_err(AspectCallbackError::Python)
    }

    fn image_height(&mut self) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("height")
            .map_err(AspectCallbackError::Python)
    }

    fn size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        self.size
            .get_item(index)
            .map_err(AspectCallbackError::Python)
    }

    fn arithmetic(
        &mut self,
        operation: pillow_rs::ImageOpsAspectArithmetic,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        match operation {
            pillow_rs::ImageOpsAspectArithmetic::Add => left.add(right),
            pillow_rs::ImageOpsAspectArithmetic::Subtract => left.sub(right),
            pillow_rs::ImageOpsAspectArithmetic::Divide => left.div(right),
            pillow_rs::ImageOpsAspectArithmetic::Multiply => left.mul(right),
        }
        .map_err(AspectCallbackError::Python)
    }

    fn compare(
        &mut self,
        operation: pillow_rs::ImageOpsAspectComparison,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<bool, Self::Error> {
        let comparison = match operation {
            pillow_rs::ImageOpsAspectComparison::Equal => CompareOp::Eq,
            pillow_rs::ImageOpsAspectComparison::NotEqual => CompareOp::Ne,
            pillow_rs::ImageOpsAspectComparison::LessThanOrEqual => CompareOp::Le,
            pillow_rs::ImageOpsAspectComparison::GreaterThan => CompareOp::Gt,
            pillow_rs::ImageOpsAspectComparison::GreaterThanOrEqual => CompareOp::Ge,
            pillow_rs::ImageOpsAspectComparison::LessThan => CompareOp::Lt,
        };
        left.rich_compare(right, comparison)
            .and_then(|result| result.is_truthy())
            .map_err(AspectCallbackError::Python)
    }

    fn round(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error> {
        self.py
            .import("builtins")
            .and_then(|builtins| {
                builtins
                    .getattr("round")
                    .and_then(|round| round.call1((value,)))
            })
            .map_err(AspectCallbackError::Python)
    }

    fn size_pair(
        &mut self,
        width: &Self::Value,
        height: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        PyTuple::new(self.py, [width.clone(), height.clone()])
            .map(|size| size.into_any())
            .map_err(AspectCallbackError::Python)
    }
}

impl<'py> pillow_rs::ImageOpsScaleCallbacks for PythonScaleCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Error = AspectCallbackError;

    fn factor_is_non_positive(&mut self, factor: &Self::Value) -> Result<bool, Self::Error> {
        factor
            .rich_compare(pyo3::types::PyInt::new(self.py, 0), CompareOp::Le)
            .and_then(|result| result.is_truthy())
            .map_err(AspectCallbackError::Python)
    }

    fn image_width(&mut self) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("width")
            .map_err(AspectCallbackError::Python)
    }

    fn image_height(&mut self) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("height")
            .map_err(AspectCallbackError::Python)
    }

    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.mul(right).map_err(AspectCallbackError::Python)
    }

    fn round(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error> {
        self.py
            .import("builtins")
            .and_then(|builtins| {
                builtins
                    .getattr("round")
                    .and_then(|round| round.call1((value,)))
            })
            .map_err(AspectCallbackError::Python)
    }

    fn size_pair(
        &mut self,
        width: &Self::Value,
        height: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        PyTuple::new(self.py, [width.clone(), height.clone()])
            .map(|size| size.into_any())
            .map_err(AspectCallbackError::Python)
    }
}

impl<'py> pillow_rs::ImageOpsFitCallbacks for PythonAspectCallbacks<'py> {
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        let size = self
            .image
            .getattr("size")
            .map_err(AspectCallbackError::Python)?;
        size.get_item(index).map_err(AspectCallbackError::Python)
    }

    fn box_tuple(
        &mut self,
        left: &Self::Value,
        top: &Self::Value,
        right: &Self::Value,
        bottom: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        PyTuple::new(
            self.py,
            [left.clone(), top.clone(), right.clone(), bottom.clone()],
        )
        .map(|box_| box_.into_any())
        .map_err(AspectCallbackError::Python)
    }

    fn fit_constant(
        &mut self,
        constant: pillow_rs::ImageOpsFitConstant,
    ) -> Result<Self::Value, Self::Error> {
        let value = match constant {
            pillow_rs::ImageOpsFitConstant::Zero => PyFloat::new(self.py, 0.0).into_any(),
            pillow_rs::ImageOpsFitConstant::Half => PyFloat::new(self.py, 0.5).into_any(),
            pillow_rs::ImageOpsFitConstant::One => PyFloat::new(self.py, 1.0).into_any(),
            pillow_rs::ImageOpsFitConstant::Two => {
                pyo3::types::PyInt::new(self.py, 2).into_any()
            }
        };
        Ok(value)
    }
}

fn aspect_size_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    size: &Bound<'_, PyAny>,
    operation: pillow_rs::ImageOpsAspectOperation,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonAspectCallbacks::new(py, image, size);
    match pillow_rs::imageops_aspect_size_with_callbacks(&mut callbacks, operation)
        .map_err(aspect_callback_error)?
    {
        pillow_rs::ImageOpsAspectSize::Original => Ok(size.clone().unbind()),
        pillow_rs::ImageOpsAspectSize::Resized(size) => Ok(size.unbind()),
    }
}

fn aspect_callback_error(error: AspectCallbackError) -> PyErr {
    match error {
        AspectCallbackError::Core(error) => map_error(error),
        AspectCallbackError::Python(error) => error,
    }
}

#[pyfunction]
fn ops_contain_size_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    size: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    aspect_size_with_callbacks(
        py,
        image,
        size,
        pillow_rs::ImageOpsAspectOperation::Contain,
    )
}

#[pyfunction]
fn ops_cover_size_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    size: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    aspect_size_with_callbacks(
        py,
        image,
        size,
        pillow_rs::ImageOpsAspectOperation::Cover,
    )
}

#[pyfunction]
fn ops_fit_box_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    size: &Bound<'_, PyAny>,
    bleed: &Bound<'_, PyAny>,
    centering_x: &Bound<'_, PyAny>,
    centering_y: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonAspectCallbacks::new(py, image, size);
    pillow_rs::imageops_fit_box_with_callbacks(
        &mut callbacks,
        bleed,
        centering_x,
        centering_y,
    )
    .map_err(aspect_callback_error)
    .map(|box_| box_.unbind())
}

#[pyfunction]
fn ops_crop_box_geometry_preflight(
    py: Python<'_>,
    image: &Bound<'_, PyImage>,
    border: u32,
) -> PyResult<(u32, u32, u32, u32)> {
    let inner = image.borrow().inner.clone();
    py.detach(|| pillow_rs::imageops_crop_box_geometry_preflight(&inner, border))
        .map_err(map_error)
}

#[pyfunction]
fn ops_crop_box_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    border: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonBorderCallbacks {
        py,
        image: image.clone(),
    };
    pillow_rs::imageops_crop_box_with_callbacks(&mut callbacks, border)
        .map(|box_| box_.unbind())
}

#[pyfunction]
fn ops_expand_geometry_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    border: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonBorderCallbacks {
        py,
        image: image.clone(),
    };
    pillow_rs::imageops_expand_geometry_with_callbacks(&mut callbacks, border)
        .map(|geometry| geometry.unbind())
}

#[pyfunction]
fn ops_fit_geometry_preflight(
    py: Python<'_>,
    image: &Bound<'_, PyImage>,
    width: u32,
    height: u32,
    bleed: f64,
    centering: &Bound<'_, PyAny>,
) -> PyResult<(f64, f64, f64, f64)> {
    let centering = crate::inputs::centering_from_python(centering);
    let inner = image.borrow().inner.clone();
    py.detach(|| {
        pillow_rs::imageops_fit_geometry_preflight(&inner, width, height, bleed, centering)
    })
    .map_err(map_error)
}

#[pyfunction]
fn ops_pad_with_input_and_containment(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    width: u32,
    height: u32,
    method: &Bound<'_, PyAny>,
    color: &Bound<'_, PyAny>,
    centering: &Bound<'_, PyAny>,
) -> PyResult<(PyImage, bool)> {
    let initial = wrapped_image_snapshot(image)?;
    let geometry = py
        .detach(|| pillow_rs::imageops_pad_geometry_preflight(&initial, width, height))
        .map_err(map_error)?;
    let filter = resample_input_from_python(Some(method))?;
    let preflight = py
        .detach(|| pillow_rs::imageops_pad_filter_preflight(geometry, filter))
        .map_err(map_error)?;
    let needs_padding = preflight.needs_padding();
    let mut inner = wrapped_image_snapshot(image)?;
    if matches!(
        inner.mode_identity().map_err(map_error)?,
        pillow_rs::Mode::ImagePalette8 | pillow_rs::Mode::ImagePaletteAlpha8
    ) {
        // Image.resize realizes dirty P/PA palettes after filter validation.
        // This callback may replace the wrapped raster, so snapshot it again.
        image.call_method0("_realize_dirty_palette")?;
        inner = wrapped_image_snapshot(image)?;
    }
    let fill = if needs_padding {
        let mode = inner.mode_identity().map_err(map_error)?;
        imageops_fill_from_python(py, Some(color), mode)?
    } else {
        pillow_rs::ImageOpsColor::None
    };
    let fill = py
        .detach(|| pillow_rs::imageops_pad_fill_preflight(&inner, preflight, fill))
        .map_err(map_error)?;
    let centering = if needs_padding {
        crate::inputs::centering_from_python(centering)
    } else {
        pillow_rs::CenteringInput::Default
    };
    let (inner, is_contained) = py
        .detach(|| {
            pillow_rs::imageops_pad_with_preflight_and_fill(
                &inner,
                preflight,
                fill,
                centering,
            )
        })
        .map_err(map_error)?;
    Ok((PyImage { inner }, is_contained))
}

fn wrapped_image_snapshot(image: &Bound<'_, PyAny>) -> PyResult<pillow_rs::Image> {
    let rust_image = image.getattr("_rust_image")?;
    let borrowed = rust_image.extract::<PyRef<'_, PyImage>>()?;
    Ok(borrowed.inner.clone())
}

fn colorize_range_length(py: Python<'_>, stop: &Bound<'_, PyAny>) -> PyResult<usize> {
    let range = py.import("builtins")?.getattr("range")?.call1((stop,))?;
    // TODO(pillow-rs#3ptb): preserve arbitrary-size range stops and Pillow's
    // list-growth failure order for adversarial custom __index__/__sub__ values.
    let stop = range.getattr("stop")?.extract::<i64>()?;
    Ok(stop.max(0) as usize)
}

fn colorize_subtraction_range_length(
    py: Python<'_>,
    left: &Bound<'_, PyAny>,
    right: &Bound<'_, PyAny>,
) -> PyResult<usize> {
    let difference = py
        .import("operator")?
        .getattr("sub")?
        .call1((left, right))?;
    colorize_range_length(py, &difference)
}

fn parse_colorize_color(value: &Bound<'_, PyAny>) -> PyResult<(u8, u8, u8)> {
    // TODO(pillow-rs#3ptb): route arbitrary non-string color sequences through
    // ImageColor/getitem callbacks in Pillow's component-access order.
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
    let input = imageops_fill_from_python(py, Some(fill), mode)?;
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
#[pyo3(signature = (image, in_place=false))]
fn ops_exif_transpose(
    image: &Bound<'_, PyImage>,
    in_place: &Bound<'_, PyAny>,
) -> PyResult<(bool, bool, Option<PyImage>)> {
    let exif_snapshot = image.borrow().inner.getexif();
    let orientation = pillow_rs::exif_get_orientation(&exif_snapshot).unwrap_or(1);
    let changed = (2..=8).contains(&orientation);
    let in_place = in_place.is_truthy()?;
    let result = if changed {
        let inner = image.borrow().inner.clone();
        Python::attach(|py| {
            py.detach(|| {
                pillow_rs::imageops_exif_transpose_with_orientation(
                    &inner,
                    orientation,
                    in_place,
                )
            })
        })
        .map_err(map_error)?
    } else {
        None
    };

    if in_place {
        if let Some(transposed) = result {
            image.borrow_mut().inner = transposed;
        }
        Ok((changed, in_place, None))
    } else {
        Ok((changed, in_place, result.map(|inner| PyImage { inner })))
    }
}

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ops_autocontrast_lut, m)?)?;
    m.add_function(wrap_pyfunction!(ops_scale_geometry_preflight, m)?)?;
    m.add_function(wrap_pyfunction!(ops_scale_size_with_callbacks, m)?)?;
    m.add_function(wrap_pyfunction!(ops_contain_geometry_preflight, m)?)?;
    m.add_function(wrap_pyfunction!(ops_cover_geometry_preflight, m)?)?;
    m.add_function(wrap_pyfunction!(ops_contain_size_with_callbacks, m)?)?;
    m.add_function(wrap_pyfunction!(ops_cover_size_with_callbacks, m)?)?;
    m.add_function(wrap_pyfunction!(ops_fit_box_with_callbacks, m)?)?;
    m.add_function(wrap_pyfunction!(ops_crop_box_with_callbacks, m)?)?;
    m.add_function(wrap_pyfunction!(ops_expand_geometry_with_callbacks, m)?)?;
    m.add_function(wrap_pyfunction!(ops_crop_box_geometry_preflight, m)?)?;
    m.add_function(wrap_pyfunction!(ops_fit_geometry_preflight, m)?)?;
    m.add_function(wrap_pyfunction!(ops_pad_with_input_and_containment, m)?)?;
    m.add_function(wrap_pyfunction!(ops_equalize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_equalize_lut, m)?)?;
    m.add_function(wrap_pyfunction!(ops_invert, m)?)?;
    m.add_function(wrap_pyfunction!(ops_flip, m)?)?;
    m.add_function(wrap_pyfunction!(ops_mirror, m)?)?;
    m.add_function(wrap_pyfunction!(ops_posterize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_solarize, m)?)?;
    m.add_function(wrap_pyfunction!(ops_grayscale, m)?)?;
    m.add_function(wrap_pyfunction!(ops_colorize_lut, m)?)?;
    m.add_class::<PyExpandTarget>()?;
    m.add_class::<PyExpandFill>()?;
    m.add_function(wrap_pyfunction!(ops_expand_capture_target, m)?)?;
    m.add_function(wrap_pyfunction!(ops_expand_prepare_fill, m)?)?;
    m.add_function(wrap_pyfunction!(ops_expand_resolve_palette_fill, m)?)?;
    m.add_function(wrap_pyfunction!(ops_expand_canvas, m)?)?;
    m.add_function(wrap_pyfunction!(ops_exif_transpose, m)?)?;
    Ok(())
}
