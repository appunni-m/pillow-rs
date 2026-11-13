use super::{
    AspectCallbackError, PythonAspectCallbacks, aspect_callback_error, imageops_global_or_builtin,
    wrapped_image_snapshot,
};
use pyo3::prelude::{Bound, Py, PyAny, PyModule, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyInt, PyModuleMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};

impl<'py> pillow_rs::ImageOpsNativeSizeCallbacks for PythonAspectCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Error = AspectCallbackError;

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_is_instance(&mut self) -> Result<bool, Self::Error> {
        let is_instance = imageops_global_or_builtin(self.py, "isinstance")?;
        let image_type = self.py.import("pillow_rs.imageops")?.getattr("Image")?;
        is_instance
            .call1((self.image.clone(), image_type))?
            .is_truthy()
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_uses_default_getattribute(&mut self) -> Result<bool, Self::Error> {
        let type_function = imageops_global_or_builtin(self.py, "type")?;
        let image_type = type_function.call1((self.image.clone(),))?;
        let getattribute = image_type.getattr("__getattribute__")?;
        let object = imageops_global_or_builtin(self.py, "object")?;
        let default = object.getattr("__getattribute__")?;
        Ok(getattribute.is(&default))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_property_matches_base(
        &mut self,
        property: pillow_rs::ImageOpsNativeSizeProperty,
    ) -> Result<bool, Self::Error> {
        let (property_name, base_name) = match property {
            pillow_rs::ImageOpsNativeSizeProperty::Size => ("size", "_BASE_IMAGE_SIZE"),
            pillow_rs::ImageOpsNativeSizeProperty::Width => ("width", "_BASE_IMAGE_WIDTH"),
            pillow_rs::ImageOpsNativeSizeProperty::Height => ("height", "_BASE_IMAGE_HEIGHT"),
        };
        let getattr = imageops_global_or_builtin(self.py, "getattr")?;
        let type_function = imageops_global_or_builtin(self.py, "type")?;
        let image_type = type_function.call1((self.image.clone(),))?;
        let value = getattr.call1((image_type, property_name, self.py.None()))?;
        let baseline = self.py.import("pillow_rs.imageops")?.getattr(base_name)?;
        Ok(value.is(&baseline))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn size_is_exact_tuple(&mut self) -> Result<bool, Self::Error> {
        let type_function = imageops_global_or_builtin(self.py, "type")?;
        let actual = type_function.call1((self.size.clone(),))?;
        let tuple_type = imageops_global_or_builtin(self.py, "tuple")?;
        Ok(actual.is(&tuple_type))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn size_length(&mut self) -> Result<usize, Self::Error> {
        self.size.len().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        self.size.get_item(index).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn coordinate_is_exact_integer(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        let type_function = imageops_global_or_builtin(self.py, "type")?;
        let actual = type_function.call1((value.clone(),))?;
        let integer_type = imageops_global_or_builtin(self.py, "int")?;
        Ok(actual.is(&integer_type))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn coordinate_is_u32(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        if !PyInt::new(self.py, 0)
            .le(value)
            .map_err(AspectCallbackError::from)?
        {
            return Ok(false);
        }
        value.le(u32::MAX).map_err(AspectCallbackError::from)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn coordinate_as_u32(&mut self, value: &Self::Value) -> Result<u32, Self::Error> {
        value.extract().map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsAspectRouteCallbacks for PythonAspectCallbacks<'py> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T {
        self.py.detach(operation)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_image(
        &mut self,
    ) -> Result<pillow_rs::Image, <Self as pillow_rs::ImageOpsAspectCallbacks>::Error> {
        wrapped_image_snapshot(&self.image).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn host_size_pair(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<
        <Self as pillow_rs::ImageOpsAspectCallbacks>::Value,
        <Self as pillow_rs::ImageOpsAspectCallbacks>::Error,
    > {
        PyTuple::new(self.py, [width, height])
            .map(|pair| pair.into_any())
            .map_err(Into::into)
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn aspect_size_with_route_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    size: &Bound<'_, PyAny>,
    operation: pillow_rs::ImageOpsAspectOperation,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonAspectCallbacks::new(py, image, size);
    match pillow_rs::imageops_aspect_size_with_route_callbacks(&mut callbacks, operation)
        .map_err(aspect_callback_error)?
    {
        pillow_rs::ImageOpsAspectSize::Original => Ok(size.clone().unbind()),
        pillow_rs::ImageOpsAspectSize::Resized(size) => Ok(size.unbind()),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[pyfunction]
fn ops_contain_size_with_route_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    size: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    aspect_size_with_route_callbacks(py, image, size, pillow_rs::ImageOpsAspectOperation::Contain)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[pyfunction]
fn ops_cover_size_with_route_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    size: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    aspect_size_with_route_callbacks(py, image, size, pillow_rs::ImageOpsAspectOperation::Cover)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        ops_contain_size_with_route_callbacks,
        module
    )?)?;
    module.add_function(wrap_pyfunction!(
        ops_cover_size_with_route_callbacks,
        module
    )?)?;
    Ok(())
}
