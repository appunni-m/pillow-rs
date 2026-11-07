use super::*;
use pyo3::prelude::Py;
use pyo3::types::{PyDict, PyDictMethods, PyIterator};

struct PythonPadCallbacks<'py> {
    aspect: super::PythonAspectCallbacks<'py>,
}

enum PadCallbackError {
    Core(pillow_rs::PilError),
    Python(PyErr),
}

impl From<pillow_rs::PilError> for PadCallbackError {
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

impl From<PyErr> for PadCallbackError {
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

impl From<super::AspectCallbackError> for PadCallbackError {
    fn from(error: super::AspectCallbackError) -> Self {
        match error {
            super::AspectCallbackError::Core(error) => Self::Core(error),
            super::AspectCallbackError::Python(error) => Self::Python(error),
        }
    }
}

impl<'py> PythonPadCallbacks<'py> {
    fn with_native_size_callbacks<T>(
        &mut self,
        operation: impl FnOnce(
            &mut super::PythonAspectCallbacks<'py>,
        ) -> Result<T, super::AspectCallbackError>,
    ) -> Result<T, PadCallbackError> {
        operation(&mut self.aspect).map_err(Into::into)
    }

    fn exact_type_is(
        &self,
        value: &Bound<'py, PyAny>,
        expected_name: &str,
    ) -> PyResult<bool> {
        let type_function = super::imageops_global_or_builtin(self.aspect.py, "type")?;
        let actual = type_function.call1((value,))?;
        let expected = super::imageops_global_or_builtin(self.aspect.py, expected_name)?;
        Ok(actual.is(&expected))
    }

    fn exact_number_type(&self, value: &Bound<'py, PyAny>) -> PyResult<bool> {
        let type_function = super::imageops_global_or_builtin(self.aspect.py, "type")?;
        let actual = type_function.call1((value,))?;
        let integer = super::imageops_global_or_builtin(self.aspect.py, "int")?;
        let floating = super::imageops_global_or_builtin(self.aspect.py, "float")?;
        Ok(Self::type_matches(&actual, &integer)?
            || Self::type_matches(&actual, &floating)?)
    }

    fn type_matches(
        actual: &Bound<'py, PyAny>,
        expected: &Bound<'py, PyAny>,
    ) -> PyResult<bool> {
        if actual.is(expected) {
            return Ok(true);
        }
        actual
            .rich_compare(expected, CompareOp::Eq)?
            .is_truthy()
    }

    fn instance_has_pad_overrides(&self) -> PyResult<bool> {
        let attributes = self
            .aspect
            .image
            .getattr("__dict__")?;
        let attributes = attributes.cast::<PyDict>().map_err(PyErr::from)?;
        for name in [
            "mode",
            "resize",
            "copy",
            "_new",
            "_realize_dirty_palette",
        ] {
            if attributes.contains(name)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn values_have_non_native_numeric_types(
        &self,
        values: &Bound<'py, PyAny>,
    ) -> PyResult<bool> {
        for value in PyIterator::from_object(values)? {
            let value = value?;
            if !self.exact_number_type(&value)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn image_class(&self) -> PyResult<Bound<'py, PyAny>> {
        self.aspect
            .py
            .import("pillow_rs.imageops")?
            .getattr("Image")
    }

    fn image_class_member_matches_base(
        &self,
        member: &str,
        baseline: &str,
    ) -> PyResult<bool> {
        let current = self.image_class()?.getattr(member)?;
        let expected = self
            .aspect
            .py
            .import("pillow_rs.imageops")?
            .getattr(baseline)?;
        Ok(current.is(&expected))
    }

    fn native_pad_wrapper_is_eligible_host(&mut self) -> PyResult<bool> {
        let type_function = super::imageops_global_or_builtin(self.aspect.py, "type")?;
        let actual_type = type_function.call1((self.aspect.image.clone(),))?;
        if !actual_type.is(&self.image_class()?) {
            return Ok(false);
        }

        for (member, baseline) in [
            ("mode", "_BASE_IMAGE_MODE"),
            ("resize", "_BASE_IMAGE_RESIZE"),
            ("copy", "_BASE_IMAGE_COPY"),
            ("paste", "_BASE_IMAGE_PASTE"),
            ("__init__", "_BASE_IMAGE_INIT"),
            (
                "_realize_dirty_palette",
                "_BASE_IMAGE_REALIZE_DIRTY_PALETTE",
            ),
        ] {
            if !self.image_class_member_matches_base(member, baseline)? {
                return Ok(false);
            }
        }

        let getattr = super::imageops_global_or_builtin(self.aspect.py, "getattr")?;
        let image_type = self.image_class()?;
        let new_object = image_type.getattr("new")?;
        let new_default = image_type.getattr("new")?;
        let new_function = getattr.call1((new_object, "__func__", new_default))?;
        let expected_new = self
            .aspect
            .py
            .import("pillow_rs.imageops")?
            .getattr("_BASE_IMAGE_NEW")?;
        if !new_function.is(&expected_new)
            || !self.image_class_member_matches_base("_new", "_BASE_IMAGE_WRAPPER_NEW")?
        {
            return Ok(false);
        }

        if self.instance_has_pad_overrides()? {
            return Ok(false);
        }
        Ok(true)
    }

    fn native_pad_arguments_are_eligible_host(
        &self,
        method: &Bound<'py, PyAny>,
        color: &Bound<'py, PyAny>,
        centering: &Bound<'py, PyAny>,
    ) -> PyResult<bool> {
        let type_function = super::imageops_global_or_builtin(self.aspect.py, "type")?;
        let method_type = type_function.call1((method,))?;
        let integer_type = super::imageops_global_or_builtin(self.aspect.py, "int")?;
        let resampling_type = self.image_class()?.getattr("Resampling")?;
        if !Self::type_matches(&method_type, &integer_type)?
            && !Self::type_matches(&method_type, &resampling_type)?
        {
            return Ok(false);
        }

        if !self.exact_type_is(centering, "tuple")? {
            return Ok(false);
        }
        let length = super::imageops_global_or_builtin(self.aspect.py, "len")?
            .call1((centering,))?
            .extract::<usize>()?;
        if length != 2 {
            return Ok(false);
        }
        if self.values_have_non_native_numeric_types(centering)? {
            return Ok(false);
        }

        if color.is_none() || self.exact_number_type(color)? {
            return Ok(true);
        }
        if !self.exact_type_is(color, "tuple")? {
            return Ok(false);
        }
        Ok(!self.values_have_non_native_numeric_types(color)?)
    }
}

impl<'py> pillow_rs::ImageOpsPadCallbacks for PythonPadCallbacks<'py> {
    type Error = PadCallbackError;
    type Value = Bound<'py, PyAny>;
    type Image = Bound<'py, PyAny>;
    type Palette = Bound<'py, PyAny>;

    fn contain(
        &mut self,
        size: &Self::Value,
        method: &Self::Value,
    ) -> Result<Self::Image, Self::Error> {
        self.aspect
            .py
            .import("pillow_rs.imageops")?
            .getattr("contain")?
            .call1((self.aspect.image.clone(), size, method))
            .map_err(Into::into)
    }

    fn resized_size_matches(
        &mut self,
        resized: &Self::Image,
        size: &Self::Value,
    ) -> Result<bool, Self::Error> {
        resized
            .getattr("size")?
            .rich_compare(size, CompareOp::Eq)?
            .is_truthy()
            .map_err(Into::into)
    }

    fn new_canvas(
        &mut self,
        size: &Self::Value,
        color: &Self::Value,
    ) -> Result<Self::Image, Self::Error> {
        let constructor = self
            .aspect
            .py
            .import("pillow_rs.imageops")?
            .getattr("Image")?
            .getattr("new")?;
        let mode = self.aspect.image.getattr("mode")?;
        constructor.call1((mode, size, color)).map_err(Into::into)
    }

    fn resized_palette_is_truthy(
        &mut self,
        resized: &Self::Image,
    ) -> Result<bool, Self::Error> {
        resized.getattr("palette")?.is_truthy().map_err(Into::into)
    }

    fn resized_palette(&mut self, resized: &Self::Image) -> Result<Self::Palette, Self::Error> {
        resized.call_method0("getpalette").map_err(Into::into)
    }

    fn palette_is_none(&mut self, palette: &Self::Palette) -> Result<bool, Self::Error> {
        Ok(palette.is_none())
    }

    fn put_palette(
        &mut self,
        output: &mut Self::Image,
        palette: &Self::Palette,
    ) -> Result<(), Self::Error> {
        output
            .getattr("putpalette")?
            .call1((palette,))
            .map(|_| ())
            .map_err(Into::into)
    }

    fn width_differs(
        &mut self,
        resized: &Self::Image,
        size: &Self::Value,
    ) -> Result<bool, Self::Error> {
        let width = resized.getattr("width")?;
        let requested_width = size.get_item(0)?;
        width
            .rich_compare(requested_width, CompareOp::Ne)?
            .is_truthy()
            .map_err(Into::into)
    }

    fn centered_offset(
        &mut self,
        axis: pillow_rs::ImageOpsPadAxis,
        resized: &Self::Image,
        size: &Self::Value,
        centering: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let coordinate = match axis {
            pillow_rs::ImageOpsPadAxis::Horizontal => 0,
            pillow_rs::ImageOpsPadAxis::Vertical => 1,
        };
        let round = super::imageops_global_or_builtin(self.aspect.py, "round")?;
        let requested = size.get_item(coordinate)?;
        let dimension_name = match axis {
            pillow_rs::ImageOpsPadAxis::Horizontal => "width",
            pillow_rs::ImageOpsPadAxis::Vertical => "height",
        };
        let current = resized.getattr(dimension_name)?;
        let difference = requested.sub(&current)?;
        let maximum = super::imageops_global_or_builtin(self.aspect.py, "max")?;
        let minimum = super::imageops_global_or_builtin(self.aspect.py, "min")?;
        let center = centering.get_item(coordinate)?;
        let min_center = minimum.call1((center, 1))?;
        let clamped_center = maximum.call1((0, min_center))?;
        let centered_difference = difference.mul(&clamped_center)?;
        round.call1((centered_difference,)).map_err(Into::into)
    }

    fn paste_at(
        &mut self,
        output: &mut Self::Image,
        resized: &Self::Image,
        axis: pillow_rs::ImageOpsPadAxis,
        offset: &Self::Value,
    ) -> Result<(), Self::Error> {
        let paste = output.getattr("paste")?;
        let result = match axis {
            pillow_rs::ImageOpsPadAxis::Horizontal => {
                paste.call1((resized.clone(), (offset.clone(), 0)))
            }
            pillow_rs::ImageOpsPadAxis::Vertical => {
                paste.call1((resized.clone(), (0, offset.clone())))
            }
        };
        result.map(|_| ()).map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsNativeSizeCallbacks for PythonPadCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Error = PadCallbackError;

    fn image_is_instance(&mut self) -> Result<bool, Self::Error> {
        self.with_native_size_callbacks(|callbacks| {
            pillow_rs::ImageOpsNativeSizeCallbacks::image_is_instance(callbacks)
        })
    }

    fn image_uses_default_getattribute(&mut self) -> Result<bool, Self::Error> {
        self.with_native_size_callbacks(|callbacks| {
            pillow_rs::ImageOpsNativeSizeCallbacks::image_uses_default_getattribute(callbacks)
        })
    }

    fn image_property_matches_base(
        &mut self,
        property: pillow_rs::ImageOpsNativeSizeProperty,
    ) -> Result<bool, Self::Error> {
        self.with_native_size_callbacks(|callbacks| {
            pillow_rs::ImageOpsNativeSizeCallbacks::image_property_matches_base(
                callbacks, property,
            )
        })
    }

    fn size_is_exact_tuple(&mut self) -> Result<bool, Self::Error> {
        self.with_native_size_callbacks(|callbacks| {
            pillow_rs::ImageOpsNativeSizeCallbacks::size_is_exact_tuple(callbacks)
        })
    }

    fn size_length(&mut self) -> Result<usize, Self::Error> {
        self.with_native_size_callbacks(|callbacks| {
            pillow_rs::ImageOpsNativeSizeCallbacks::size_length(callbacks)
        })
    }

    fn size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        self.with_native_size_callbacks(|callbacks| {
            pillow_rs::ImageOpsNativeSizeCallbacks::size_coordinate(callbacks, index)
        })
    }

    fn coordinate_is_exact_integer(
        &mut self,
        value: &Self::Value,
    ) -> Result<bool, Self::Error> {
        self.with_native_size_callbacks(|callbacks| {
            pillow_rs::ImageOpsNativeSizeCallbacks::coordinate_is_exact_integer(callbacks, value)
        })
    }

    fn coordinate_is_u32(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        self.with_native_size_callbacks(|callbacks| {
            pillow_rs::ImageOpsNativeSizeCallbacks::coordinate_is_u32(callbacks, value)
        })
    }

    fn coordinate_as_u32(&mut self, value: &Self::Value) -> Result<u32, Self::Error> {
        self.with_native_size_callbacks(|callbacks| {
            pillow_rs::ImageOpsNativeSizeCallbacks::coordinate_as_u32(callbacks, value)
        })
    }
}

impl<'py> pillow_rs::ImageOpsPadRouteCallbacks for PythonPadCallbacks<'py> {
    fn native_pad_wrapper_is_eligible(&mut self) -> Result<bool, PadCallbackError> {
        self.native_pad_wrapper_is_eligible_host().map_err(Into::into)
    }

    fn native_pad_arguments_are_eligible(
        &mut self,
        method: &<Self as pillow_rs::ImageOpsPadCallbacks>::Value,
        color: &<Self as pillow_rs::ImageOpsPadCallbacks>::Value,
        centering: &<Self as pillow_rs::ImageOpsPadCallbacks>::Value,
    ) -> Result<bool, PadCallbackError> {
        self.native_pad_arguments_are_eligible_host(method, color, centering)
            .map_err(Into::into)
    }

    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T {
        self.aspect.py.detach(operation)
    }

    fn native_image_snapshot(&mut self) -> Result<pillow_rs::Image, PadCallbackError> {
        super::wrapped_image_snapshot(&self.aspect.image).map_err(Into::into)
    }

    fn resample_input(
        &mut self,
        method: &<Self as pillow_rs::ImageOpsPadCallbacks>::Value,
    ) -> Result<Option<pillow_rs::ResampleInput>, PadCallbackError> {
        crate::inputs::resample_input_from_python(Some(method)).map_err(Into::into)
    }

    fn realize_dirty_palette(&mut self) -> Result<(), PadCallbackError> {
        self.aspect
            .image
            .call_method0("_realize_dirty_palette")
            .map(|_| ())
            .map_err(Into::into)
    }

    fn fill_input(
        &mut self,
        mode: pillow_rs::Mode,
        color: &<Self as pillow_rs::ImageOpsPadCallbacks>::Value,
    ) -> Result<pillow_rs::ImageOpsColor, PadCallbackError> {
        crate::inputs::imageops_fill_from_python(self.aspect.py, Some(color), mode)
            .map_err(Into::into)
    }

    fn centering_input(
        &mut self,
        centering: &<Self as pillow_rs::ImageOpsPadCallbacks>::Value,
    ) -> Result<pillow_rs::CenteringInput, PadCallbackError> {
        Ok(crate::inputs::centering_from_python(centering))
    }

    fn wrap_native_pad_result(
        &mut self,
        image: pillow_rs::Image,
        contained: bool,
    ) -> Result<Bound<'py, PyAny>, PadCallbackError> {
        let result = Py::new(self.aspect.py, PyImage { inner: image })?;
        if contained {
            self.aspect
                .image
                .call_method1("_new", (result.bind(self.aspect.py),))
                .map_err(Into::into)
        } else {
            self.aspect
                .py
                .import("pillow_rs.imageops")?
                .getattr("Image")?
                .call1((result.bind(self.aspect.py),))
                .map_err(Into::into)
        }
    }
}

fn pad_callback_error(error: PadCallbackError) -> PyErr {
    match error {
        PadCallbackError::Core(error) => map_error(error),
        PadCallbackError::Python(error) => error,
    }
}

#[pyfunction]
fn ops_pad_with_route_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    size: &Bound<'_, PyAny>,
    method: &Bound<'_, PyAny>,
    color: &Bound<'_, PyAny>,
    centering: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonPadCallbacks {
        aspect: super::PythonAspectCallbacks::new(py, image, size),
    };
    pillow_rs::imageops_pad_with_route_callbacks(
        &mut callbacks,
        size,
        method,
        color,
        centering,
    )
    .map(|result| result.unbind())
    .map_err(pad_callback_error)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(ops_pad_with_route_callbacks, module)?)?;
    Ok(())
}
