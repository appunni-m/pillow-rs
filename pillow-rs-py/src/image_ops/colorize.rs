use super::*;
use pyo3::types::{PyIterator, PyList, PyListMethods, PyString, PyTuple};

struct PythonColorizeCallbacks<'py> {
    py: Python<'py>,
}

enum ColorizeCallbackError {
    Core(pillow_rs::PilError),
    Python(PyErr),
}

impl From<pillow_rs::PilError> for ColorizeCallbackError {
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

impl From<PyErr> for ColorizeCallbackError {
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

impl<'py> pillow_rs::ImageOpsColorizeCallbacks for PythonColorizeCallbacks<'py> {
    type Image = Bound<'py, PyAny>;
    type Value = Bound<'py, PyAny>;
    type Sequence = Bound<'py, PyList>;
    type Range = Bound<'py, PyAny>;
    type Iterator = Bound<'py, PyIterator>;
    type Lut = Bound<'py, PyAny>;
    type Output = Bound<'py, PyAny>;
    type Error = ColorizeCallbackError;

    fn point_is_nonnegative(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        PyInt::new(self.py, 0).le(value).map_err(Into::into)
    }

    fn point_is_at_most(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<bool, Self::Error> {
        left.le(right).map_err(Into::into)
    }

    fn point_is_at_most_255(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        value.le(255).map_err(Into::into)
    }

    fn resolve_color(&mut self, color: &Self::Value) -> Result<Self::Value, Self::Error> {
        if !color.is_instance_of::<PyString>() {
            return Ok(color.clone());
        }
        let color = color.extract::<String>()?;
        let (red, green, blue, _) = pillow_rs::parse_color_str(&color)?;
        Ok(PyTuple::new(self.py, [red, green, blue])?.into_any())
    }

    fn integer(&mut self, value: i64) -> Result<Self::Value, Self::Error> {
        Ok(PyInt::new(self.py, value).into_any())
    }

    fn new_channel(&mut self) -> Result<Self::Sequence, Self::Error> {
        Ok(PyList::empty(self.py))
    }

    fn append(
        &mut self,
        channel: &mut Self::Sequence,
        value: &Self::Value,
    ) -> Result<(), Self::Error> {
        channel.append(value)?;
        Ok(())
    }

    fn range(&mut self, stop: &Self::Value) -> Result<Self::Range, Self::Error> {
        imageops_global_or_builtin(self.py, "range")?
            .call1((stop,))
            .map_err(Into::into)
    }

    fn range_values(&mut self, range: &Self::Range) -> Result<Self::Iterator, Self::Error> {
        PyIterator::from_object(range).map_err(Into::into)
    }

    fn next_range_value(
        &mut self,
        values: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error> {
        values.next().transpose().map_err(Into::into)
    }

    fn range_length(&mut self, range: &Self::Range) -> Result<Self::Value, Self::Error> {
        imageops_global_or_builtin(self.py, "len")?
            .call1((range,))
            .map_err(Into::into)
    }

    fn component(
        &mut self,
        color: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error> {
        color.get_item(index).map_err(Into::into)
    }

    fn subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.sub(right).map_err(Into::into)
    }

    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.mul(right).map_err(Into::into)
    }

    fn floor_divide(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.floor_div(right).map_err(Into::into)
    }

    fn add(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.add(right).map_err(Into::into)
    }

    fn combine_channels(
        &mut self,
        red: Self::Sequence,
        green: Self::Sequence,
        blue: Self::Sequence,
    ) -> Result<Self::Lut, Self::Error> {
        let red_green = red.as_any().add(green.as_any())?;
        red_green.add(blue.as_any()).map_err(Into::into)
    }

    fn convert_to_rgb(&mut self, image: &Self::Image) -> Result<Self::Image, Self::Error> {
        image
            .call_method1("convert", ("RGB",))
            .map_err(Into::into)
    }

    fn point(
        &mut self,
        image: &Self::Image,
        lut: Self::Lut,
    ) -> Result<Self::Output, Self::Error> {
        image.call_method1("point", (lut,)).map_err(Into::into)
    }
}

fn map_colorize_callback_error(error: ColorizeCallbackError) -> PyErr {
    match error {
        ColorizeCallbackError::Core(error) => map_error(error),
        ColorizeCallbackError::Python(error) => error,
    }
}

#[pyfunction]
fn ops_imageops_colorize_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    black: &Bound<'_, PyAny>,
    white: &Bound<'_, PyAny>,
    mid: Option<&Bound<'_, PyAny>>,
    blackpoint: &Bound<'_, PyAny>,
    midpoint: &Bound<'_, PyAny>,
    whitepoint: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let rust_image = image.getattr("_rust_image")?;
    let rust_image = rust_image.cast::<PyImage>()?;
    let mode = rust_image
        .borrow()
        .inner
        .mode_identity()
        .map_err(map_error)?;
    let mut callbacks = PythonColorizeCallbacks { py };
    pillow_rs::imageops_colorize_with_callbacks(
        mode,
        &mut callbacks,
        image.clone(),
        black.clone(),
        white.clone(),
        mid.cloned(),
        blackpoint.clone(),
        midpoint.clone(),
        whitepoint.clone(),
    )
    .map(|result| result.unbind())
    .map_err(map_colorize_callback_error)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(ops_imageops_colorize_with_callbacks, module)?)?;
    Ok(())
}
