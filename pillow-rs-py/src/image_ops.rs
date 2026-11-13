use crate::*;
use pyo3::basic::CompareOp;
use pyo3::prelude::{PyErr, PyModule};
use pyo3::types::{
    PyAnyMethods, PyDictMethods, PyFloat, PyInt, PyModuleMethods, PyString, PyTuple, PyTupleMethods,
};
use pyo3::{PyRef, pyfunction, wrap_pyfunction};

mod autocontrast;
mod colorize;
mod equalize;
mod exif;
mod expand;
mod pad;
pub(super) mod point;
mod size_route;

fn imageops_mode_equals(py: Python<'_>, mode: &Bound<'_, PyAny>, expected: &str) -> PyResult<bool> {
    mode.rich_compare(PyString::new(py, expected), CompareOp::Eq)?
        .is_truthy()
}

fn imageops_mode_is_luma_or_rgb(py: Python<'_>, mode: &Bound<'_, PyAny>) -> PyResult<bool> {
    PyTuple::new(py, ["L", "RGB"])?.contains(mode)
}

fn imageops_unsupported_mode_error(py: Python<'_>, mode: &Bound<'_, PyAny>) -> PyResult<PyErr> {
    // Pillow's f-string uses the empty format spec, which can differ from str().
    let message = PyString::new(py, "not supported for mode {}")
        .call_method1("format", (mode,))?
        .extract::<String>()?;
    Ok(pyo3::exceptions::PyOSError::new_err(message))
}

fn imageops_global_or_builtin<'py>(py: Python<'py>, name: &str) -> PyResult<Bound<'py, PyAny>> {
    let namespace = py.import("pillow_rs.imageops")?.dict();
    if let Some(value) = namespace.get_item(name)? {
        return Ok(value);
    }
    let builtins = namespace.get_item("__builtins__")?.ok_or_else(|| {
        pyo3::exceptions::PyNameError::new_err(format!("name '{name}' is not defined"))
    })?;
    if let Ok(builtins) = builtins.cast::<PyDict>() {
        return builtins.get_item(name)?.ok_or_else(|| {
            pyo3::exceptions::PyNameError::new_err(format!("name '{name}' is not defined"))
        });
    }
    builtins.getattr(name).map_err(|_| {
        pyo3::exceptions::PyNameError::new_err(format!("name '{name}' is not defined"))
    })
}

// --- ImageOps module-level functions ---

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
fn ops_grayscale(image: &Bound<'_, PyImage>, py: Python<'_>) -> PyResult<PyImage> {
    let inner = image.borrow().inner.clone();
    let rs = py
        .detach(|| pillow_rs::imageops_grayscale(&inner))
        .map_err(map_error)?;
    Ok(PyImage { inner: rs })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[pyfunction]
fn ops_scale_with_route_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    factor: &Bound<'_, PyAny>,
    resample: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonScaleCallbacks::new(py, image);
    pillow_rs::imageops_scale_with_route_callbacks(&mut callbacks, factor, resample)
        .map_err(aspect_callback_error)
        .map(|result| result.unbind())
}

enum AspectCallbackError {
    Core(pillow_rs::PilError),
    Python(PyErr),
}

impl From<pillow_rs::PilError> for AspectCallbackError {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

impl From<PyErr> for AspectCallbackError {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

struct PythonScaleCallbacks<'py> {
    py: Python<'py>,
    image: Bound<'py, PyAny>,
}

impl<'py> PythonScaleCallbacks<'py> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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
    type Error = AspectCallbackError;

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn border_is_tuple(&mut self, border: &Self::Value) -> Result<bool, Self::Error> {
        let is_instance = imageops_global_or_builtin(self.py, "isinstance")?;
        let tuple_type = imageops_global_or_builtin(self.py, "tuple")?;
        is_instance
            .call1((border.clone(), tuple_type))?
            .is_truthy()
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn border_length(&mut self, border: &Self::Value) -> Result<usize, Self::Error> {
        border.len().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn border_coordinate(
        &mut self,
        border: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error> {
        border.get_item(index).map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsExpandGeometryCallbacks for PythonBorderCallbacks<'py> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("size")?
            .get_item(index)
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error> {
        left.add(right).map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsCropCallbacks for PythonBorderCallbacks<'py> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("size")?
            .get_item(index)
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.sub(right).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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
        .map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsCropRouteCallbacks for PythonBorderCallbacks<'py> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_is_exact_base_type(&mut self) -> Result<bool, Self::Error> {
        let type_function = imageops_global_or_builtin(self.py, "type")?;
        let image_type = self.py.import("pillow_rs.imageops")?.getattr("Image")?;
        Ok(type_function.call1((self.image.clone(),))?.is(&image_type))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn size_property_is_base(&mut self) -> Result<bool, Self::Error> {
        let module = self.py.import("pillow_rs.imageops")?;
        let image_type = module.getattr("Image")?;
        let current = image_type.getattr("size")?;
        let baseline = module.getattr("_BASE_IMAGE_SIZE")?;
        Ok(current.is(&baseline))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn crop_method_is_base(&mut self) -> Result<bool, Self::Error> {
        let module = self.py.import("pillow_rs.imageops")?;
        let image_type = module.getattr("Image")?;
        let current = image_type.getattr("crop")?;
        let baseline = module.getattr("_BASE_IMAGE_CROP")?;
        Ok(current.is(&baseline))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn instance_crop_is_absent(&mut self) -> Result<bool, Self::Error> {
        let attributes = self.image.getattr("__dict__")?;
        let attributes = attributes.cast::<PyDict>().map_err(PyErr::from)?;
        attributes
            .contains("crop")
            .map(|present| !present)
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn border_is_exact_integer(&mut self, border: &Self::Value) -> Result<bool, Self::Error> {
        let type_function = imageops_global_or_builtin(self.py, "type")?;
        let integer_type = imageops_global_or_builtin(self.py, "int")?;
        Ok(type_function.call1((border.clone(),))?.is(&integer_type))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn border_is_nonnegative(&mut self, border: &Self::Value) -> Result<bool, Self::Error> {
        PyInt::new(self.py, 0).le(border).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn border_fits_u32(&mut self, border: &Self::Value) -> Result<bool, Self::Error> {
        border.le(u32::MAX).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn border_as_u32(&mut self, border: &Self::Value) -> Result<u32, Self::Error> {
        border.extract().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_image(&mut self) -> Result<pillow_rs::Image, Self::Error> {
        wrapped_image_snapshot(&self.image).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T {
        self.py.detach(operation)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_box_tuple(
        &mut self,
        (left, top, right, bottom): (u32, u32, u32, u32),
    ) -> Result<Self::Value, Self::Error> {
        PyTuple::new(self.py, [left, top, right, bottom])
            .map(|box_| box_.into_any())
            .map_err(Into::into)
    }
}

impl<'py> PythonAspectCallbacks<'py> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn new(py: Python<'py>, image: &Bound<'py, PyAny>, size: &Bound<'py, PyAny>) -> Self {
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

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_width(&mut self) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("width")
            .map_err(AspectCallbackError::Python)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_height(&mut self) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("height")
            .map_err(AspectCallbackError::Python)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        self.size
            .get_item(index)
            .map_err(AspectCallbackError::Python)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn factor_is_non_positive(&mut self, factor: &Self::Value) -> Result<bool, Self::Error> {
        factor
            .rich_compare(pyo3::types::PyInt::new(self.py, 0), CompareOp::Le)
            .and_then(|result| result.is_truthy())
            .map_err(AspectCallbackError::Python)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_width(&mut self) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("width")
            .map_err(AspectCallbackError::Python)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_height(&mut self) -> Result<Self::Value, Self::Error> {
        self.image
            .getattr("height")
            .map_err(AspectCallbackError::Python)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.mul(right).map_err(AspectCallbackError::Python)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

impl<'py> pillow_rs::ImageOpsScaleRouteCallbacks for PythonScaleCallbacks<'py> {
    type Output = Bound<'py, PyAny>;

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn factor_is_one(&mut self, factor: &Self::Value) -> Result<bool, Self::Error> {
        factor
            .rich_compare(pyo3::types::PyInt::new(self.py, 1), CompareOp::Eq)
            .and_then(|result| result.is_truthy())
            .map_err(AspectCallbackError::Python)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_route_is_eligible(&mut self, factor: &Self::Value) -> Result<bool, Self::Error> {
        self.py
            .import("pillow_rs.imageops")?
            .getattr("_scale_native_types")?
            .call1((self.image.clone(), factor))?
            .is_truthy()
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_image(&mut self) -> Result<pillow_rs::Image, Self::Error> {
        wrapped_image_snapshot(&self.image).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn factor_as_f64(&mut self, factor: &Self::Value) -> Result<f64, Self::Error> {
        imageops_global_or_builtin(self.py, "float")?
            .call1((factor,))?
            .extract::<f64>()
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn number(&mut self, value: f64) -> Result<Self::Value, Self::Error> {
        Ok(PyFloat::new(self.py, value).into_any())
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T {
        self.py.detach(operation)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn copy_image(&mut self) -> Result<Self::Output, Self::Error> {
        self.image.call_method0("copy").map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn resize_image(
        &mut self,
        size: &Self::Value,
        resample: &Self::Value,
    ) -> Result<Self::Output, Self::Error> {
        self.image
            .call_method1("resize", (size, resample))
            .map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsFitCallbacks for PythonAspectCallbacks<'py> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        let size = self
            .image
            .getattr("size")
            .map_err(AspectCallbackError::Python)?;
        size.get_item(index).map_err(AspectCallbackError::Python)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn fit_constant(
        &mut self,
        constant: pillow_rs::ImageOpsFitConstant,
    ) -> Result<Self::Value, Self::Error> {
        let value = match constant {
            pillow_rs::ImageOpsFitConstant::Zero => PyFloat::new(self.py, 0.0).into_any(),
            pillow_rs::ImageOpsFitConstant::Half => PyFloat::new(self.py, 0.5).into_any(),
            pillow_rs::ImageOpsFitConstant::One => PyFloat::new(self.py, 1.0).into_any(),
            pillow_rs::ImageOpsFitConstant::Two => pyo3::types::PyInt::new(self.py, 2).into_any(),
        };
        Ok(value)
    }
}

impl<'py> pillow_rs::ImageOpsFitRouteCallbacks for PythonAspectCallbacks<'py> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_fit_inputs_are_eligible(
        &mut self,
        bleed: &Bound<'py, PyAny>,
        centering: &Bound<'py, PyAny>,
    ) -> Result<bool, AspectCallbackError> {
        self.py
            .import("pillow_rs.imageops")?
            .getattr("_fit_native_types")?
            .call1((bleed, centering))?
            .is_truthy()
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_image(&mut self) -> Result<pillow_rs::Image, AspectCallbackError> {
        wrapped_image_snapshot(&self.image).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn bleed_as_f64(&mut self, bleed: &Bound<'py, PyAny>) -> Result<f64, AspectCallbackError> {
        imageops_global_or_builtin(self.py, "float")?
            .call1((bleed,))?
            .extract::<f64>()
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_centering(
        &mut self,
        centering: &Bound<'py, PyAny>,
    ) -> Result<pillow_rs::CenteringInput, AspectCallbackError> {
        Ok(crate::inputs::centering_from_python(centering))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn unpack_centering(
        &mut self,
        centering: &Bound<'py, PyAny>,
    ) -> Result<(Bound<'py, PyAny>, Bound<'py, PyAny>), AspectCallbackError> {
        let pair = self
            .py
            .import("pillow_rs.imageops")?
            .getattr("_unpack_fit_centering")?
            .call1((centering,))?;
        Ok((pair.get_item(0)?, pair.get_item(1)?))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T {
        self.py.detach(operation)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_box_tuple(
        &mut self,
        (left, top, right, bottom): (f64, f64, f64, f64),
    ) -> Result<Bound<'py, PyAny>, AspectCallbackError> {
        PyTuple::new(
            self.py,
            [
                PyFloat::new(self.py, left),
                PyFloat::new(self.py, top),
                PyFloat::new(self.py, right),
                PyFloat::new(self.py, bottom),
            ],
        )
        .map(|box_| box_.into_any())
        .map_err(Into::into)
    }
}

fn aspect_callback_error(error: AspectCallbackError) -> PyErr {
    match error {
        AspectCallbackError::Core(error) => map_error(error),
        AspectCallbackError::Python(error) => error,
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[pyfunction]
fn ops_fit_box_with_route_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    size: &Bound<'_, PyAny>,
    bleed: &Bound<'_, PyAny>,
    centering: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonAspectCallbacks::new(py, image, size);
    pillow_rs::imageops_fit_box_with_route_callbacks(&mut callbacks, bleed, centering)
        .map_err(aspect_callback_error)
        .map(|box_| box_.unbind())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[pyfunction]
fn ops_crop_box_with_route_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    border: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonBorderCallbacks {
        py,
        image: image.clone(),
    };
    pillow_rs::imageops_crop_box_with_route_callbacks(&mut callbacks, border)
        .map_err(aspect_callback_error)
        .map(|box_| box_.unbind())
}

fn wrapped_image_snapshot(image: &Bound<'_, PyAny>) -> PyResult<pillow_rs::Image> {
    let rust_image = image.getattr("_rust_image")?;
    let borrowed = rust_image.extract::<PyRef<'_, PyImage>>()?;
    Ok(borrowed.inner.clone())
}

#[pyfunction]
#[pyo3(signature = (image, in_place=false))]
fn ops_exif_transpose(
    image: &Bound<'_, PyImage>,
    in_place: bool,
) -> PyResult<(bool, bool, Option<PyImage>)> {
    let exif_snapshot = image.borrow().inner.getexif();
    let orientation = pillow_rs::exif_get_orientation(&exif_snapshot).unwrap_or(1);
    let changed = (2..=8).contains(&orientation);
    let result = if changed {
        let inner = image.borrow().inner.clone();
        Python::attach(|py| {
            py.detach(|| {
                pillow_rs::imageops_exif_transpose_with_orientation(&inner, orientation, in_place)
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
    autocontrast::register(m)?;
    m.add_function(wrap_pyfunction!(ops_scale_with_route_callbacks, m)?)?;
    m.add_function(wrap_pyfunction!(ops_fit_box_with_route_callbacks, m)?)?;
    m.add_function(wrap_pyfunction!(ops_crop_box_with_route_callbacks, m)?)?;
    m.add_function(wrap_pyfunction!(ops_equalize, m)?)?;
    equalize::register(m)?;
    point::register(m)?;
    m.add_function(wrap_pyfunction!(ops_flip, m)?)?;
    m.add_function(wrap_pyfunction!(ops_mirror, m)?)?;
    m.add_function(wrap_pyfunction!(ops_grayscale, m)?)?;
    colorize::register(m)?;
    expand::register(m)?;
    exif::register(m)?;
    pad::register(m)?;
    size_route::register(m)?;
    m.add_function(wrap_pyfunction!(ops_exif_transpose, m)?)?;
    Ok(())
}
