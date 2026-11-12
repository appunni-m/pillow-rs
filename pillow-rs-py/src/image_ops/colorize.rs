use super::imageops_global_or_builtin;
use super::point::{PythonImageOpsCallbacks, map_imageops_callback_error};
use pyo3::basic::CompareOp;
use pyo3::prelude::{Bound, Py, PyAny, PyModule, PyResult, Python};
use pyo3::types::{
    PyAnyMethods, PyInt, PyIterator, PyList, PyListMethods, PyModuleMethods, PyString,
};
use pyo3::{pyfunction, wrap_pyfunction};

impl<'py> pillow_rs::ImageOpsColorizeCallbacks for PythonImageOpsCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Sequence = Bound<'py, PyList>;
    type Range = Bound<'py, PyAny>;
    type Iterator = Bound<'py, PyIterator>;

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn image_mode_is_luma(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        Ok(image
            .getattr("mode")?
            .rich_compare(PyString::new(self.py, "L"), CompareOp::Eq)?
            .is_truthy()?)
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn image_mode_name(&mut self, image: &Self::Image) -> Result<String, Self::Error> {
        image.getattr("mode")?.str()?.extract().map_err(Into::into)
    }

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
        let is_instance = imageops_global_or_builtin(self.py, "isinstance")?;
        let string_type = imageops_global_or_builtin(self.py, "str")?;
        let is_string = is_instance.call1((color, string_type))?.is_truthy()?;
        if !is_string {
            return Ok(color.clone());
        }
        let pil = self.py.import("PIL")?;
        let image_color = pil.getattr("ImageColor")?;
        image_color
            .getattr("getcolor")?
            .call1((color, "RGB"))
            .map_err(Into::into)
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

    fn component(&mut self, color: &Self::Value, index: usize) -> Result<Self::Value, Self::Error> {
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

    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error> {
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
        image.call_method1("convert", ("RGB",)).map_err(Into::into)
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
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
    let mut callbacks = PythonImageOpsCallbacks { py };
    pillow_rs::imageops_colorize_with_callbacks(
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
    .map_err(map_imageops_callback_error)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        ops_imageops_colorize_with_callbacks,
        module
    )?)?;
    Ok(())
}
