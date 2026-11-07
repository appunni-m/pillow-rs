use super::*;
use pyo3::types::{PyIterator, PySlice, PyStringMethods};

struct PythonAutocontrastLutCallbacks<'py> {
    py: Python<'py>,
}

struct PythonAutocontrastCallbacks<'py> {
    py: Python<'py>,
    image: Bound<'py, PyAny>,
}

struct PythonAutocontrastSourceCallbacks<'py> {
    _py: Python<'py>,
}

enum AutocontrastCallbackError {
    Core(pillow_rs::PilError),
    Python(PyErr),
}

impl From<pillow_rs::PilError> for AutocontrastCallbackError {
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

impl From<PyErr> for AutocontrastCallbackError {
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

impl<'py> pillow_rs::ImageOpsAutocontrastSourceCallbacks
    for PythonAutocontrastSourceCallbacks<'py>
{
    type Image = Bound<'py, PyAny>;
    type Value = Bound<'py, PyAny>;
    type Mask = Bound<'py, PyAny>;
    type PreserveTone = Bound<'py, PyAny>;
    type Error = AutocontrastCallbackError;

    fn convert_to_luma(&mut self, image: &Self::Image) -> Result<Self::Image, Self::Error> {
        image
            .call_method1("convert", ("L",))
            .map_err(Into::into)
    }

    fn histogram(
        &mut self,
        image: &Self::Image,
        mask: &Self::Mask,
    ) -> Result<Self::Value, Self::Error> {
        image
            .call_method1("histogram", (mask,))
            .map_err(Into::into)
    }

    fn preserve_tone_is_truthy(
        &mut self,
        value: &Self::PreserveTone,
    ) -> Result<bool, Self::Error> {
        value.is_truthy().map_err(Into::into)
    }

    fn mode(&mut self, image: &Self::Image) -> Result<pillow_rs::Mode, Self::Error> {
        let rust_image = image.getattr("_rust_image")?;
        let rust_image = rust_image
            .cast::<PyImage>()
            .map_err(PyErr::from)
            .map_err(AutocontrastCallbackError::from)?;
        rust_image
            .borrow()
            .inner
            .mode_identity()
            .map_err(map_error)
            .map_err(AutocontrastCallbackError::from)
    }
}

impl<'py> pillow_rs::ImageOpsAutocontrastResultCallbacks for PythonAutocontrastCallbacks<'py> {
    type Lut = Bound<'py, PyAny>;
    type Output = Bound<'py, PyAny>;
    type Error = AutocontrastCallbackError;

    fn unsupported_mode_error(&mut self) -> Result<Self::Error, Self::Error> {
        let mode = self.image.getattr("mode")?;
        let mode = mode.str()?.to_string_lossy().into_owned();
        Ok(AutocontrastCallbackError::Python(
            pyo3::exceptions::PyOSError::new_err(format!("not supported for mode {mode}")),
        ))
    }

    fn lut_length(&mut self, lut: &Self::Lut) -> Result<usize, Self::Error> {
        imageops_global_or_builtin(self.py, "len")?
            .call1((lut,))?
            .extract()
            .map_err(Into::into)
    }

    fn repeat_lut(&mut self, lut: Self::Lut) -> Result<Self::Lut, Self::Error> {
        lut.mul(3).map_err(Into::into)
    }

    fn point(&mut self, lut: Self::Lut) -> Result<Self::Output, Self::Error> {
        self.image.call_method1("point", (lut,)).map_err(Into::into)
    }
}

fn autocontrast_callback_error(error: AutocontrastCallbackError) -> PyErr {
    match error {
        AutocontrastCallbackError::Core(error) => map_error(error),
        AutocontrastCallbackError::Python(error) => error,
    }
}

impl<'py> pillow_rs::ImageOpsAutocontrastCallbacks for PythonAutocontrastLutCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Iterator = Bound<'py, PyIterator>;
    type Error = AutocontrastCallbackError;

    fn new_lut(&mut self) -> Result<Self::Value, Self::Error> {
        Ok(PyList::empty(self.py).into_any())
    }

    fn channel_starts(
        &mut self,
        histogram: &Self::Value,
    ) -> Result<Self::Iterator, Self::Error> {
        // Python evaluates the `range` name before evaluating `len(histogram)`.
        let range = imageops_global_or_builtin(self.py, "range")?;
        let length = imageops_global_or_builtin(self.py, "len")?
            .call1((histogram,))?;
        let starts = range.call1((0, length, 256))?;
        PyIterator::from_object(&starts).map_err(Into::into)
    }

    fn next_channel_start(
        &mut self,
        starts: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error> {
        starts.next().transpose().map_err(Into::into)
    }

    fn histogram_channel(
        &mut self,
        histogram: &Self::Value,
        start: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let stop = start.add(256)?;
        let slice = self.py.get_type::<PySlice>().call1((start, stop))?;
        histogram.get_item(slice).map_err(Into::into)
    }

    fn ignore_is_integer(&mut self, ignore: &Self::Value) -> Result<bool, Self::Error> {
        let is_instance = imageops_global_or_builtin(self.py, "isinstance")?;
        let integer_type = imageops_global_or_builtin(self.py, "int")?;
        is_instance
            .call1((ignore, integer_type))?
            .is_truthy()
            .map_err(Into::into)
    }

    fn ignore_indices(
        &mut self,
        ignore: &Self::Value,
    ) -> Result<Self::Iterator, Self::Error> {
        PyIterator::from_object(ignore).map_err(Into::into)
    }

    fn next_ignore_index(
        &mut self,
        indices: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error> {
        indices.next().transpose().map_err(Into::into)
    }

    fn zero_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: &Self::Value,
    ) -> Result<(), Self::Error> {
        channel.set_item(index, 0).map_err(Into::into)
    }

    fn histogram_bin(
        &mut self,
        channel: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error> {
        channel.get_item(index).map_err(Into::into)
    }

    fn cutoff_component(
        &mut self,
        cutoff: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error> {
        cutoff.get_item(index).map_err(Into::into)
    }

    fn add(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.add(right).map_err(Into::into)
    }

    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.mul(right).map_err(Into::into)
    }

    fn floor_divide_by_hundred(
        &mut self,
        value: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        value
            .floor_div(PyInt::new(self.py, 100))
            .map_err(Into::into)
    }

    fn int_value(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error> {
        imageops_global_or_builtin(self.py, "int")?
            .call1((value,))
            .map(|value| value.into_any())
            .map_err(Into::into)
    }

    fn greater_than(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<bool, Self::Error> {
        left.gt(right).map_err(Into::into)
    }

    fn subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.sub(right).map_err(Into::into)
    }

    fn subtract_from_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: usize,
        value: &Self::Value,
    ) -> Result<(), Self::Error> {
        let bin = channel.get_item(index)?;
        let subtract = self.py.import("operator")?.getattr("isub")?;
        let result = subtract.call1((bin, value))?;
        channel.set_item(index, result).map_err(Into::into)
    }

    fn clear_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: usize,
    ) -> Result<(), Self::Error> {
        channel.set_item(index, 0).map_err(Into::into)
    }

    fn cutoff_is_nonpositive(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error> {
        cutoff
            .le(PyInt::new(self.py, 0))
            .map_err(Into::into)
    }

    fn histogram_bin_is_truthy(
        &mut self,
        channel: &Self::Value,
        index: usize,
    ) -> Result<bool, Self::Error> {
        channel
            .get_item(index)?
            .is_truthy()
            .map_err(Into::into)
    }

    fn zero_value(&mut self) -> Result<Self::Value, Self::Error> {
        Ok(PyInt::new(self.py, 0).into_any())
    }

    fn cutoff_is_truthy(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error> {
        cutoff.is_truthy().map_err(Into::into)
    }

    fn cutoff_is_tuple(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error> {
        let is_instance = imageops_global_or_builtin(self.py, "isinstance")?;
        let tuple_type = imageops_global_or_builtin(self.py, "tuple")?;
        is_instance
            .call1((cutoff, tuple_type))?
            .is_truthy()
            .map_err(Into::into)
    }

    fn duplicate_cutoff(&mut self, cutoff: &Self::Value) -> Result<Self::Value, Self::Error> {
        PyTuple::new(self.py, [cutoff.clone(), cutoff.clone()])
            .map(|value| value.into_any())
            .map_err(Into::into)
    }

    fn extend_lut(
        &mut self,
        lut: &mut Self::Value,
        channel_lut: &[u8],
    ) -> Result<(), Self::Error> {
        let channel_lut = PyList::new(self.py, channel_lut)?;
        lut.call_method1("extend", (channel_lut,))?;
        Ok(())
    }
}

#[pyfunction]
fn ops_autocontrast_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    cutoff: &Bound<'_, PyAny>,
    ignore: &Bound<'_, PyAny>,
    mask: &Bound<'_, PyAny>,
    preserve_tone: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut source_callbacks = PythonAutocontrastSourceCallbacks { _py: py };
    let mut lut_callbacks = PythonAutocontrastLutCallbacks { py };
    let mut result_callbacks = PythonAutocontrastCallbacks {
        py,
        image: image.clone(),
    };
    let ignore = (!ignore.is_none()).then(|| ignore.clone());
    pillow_rs::imageops_autocontrast_with_callbacks(
        &mut source_callbacks,
        &mut lut_callbacks,
        &mut result_callbacks,
        image.clone(),
        mask.clone(),
        preserve_tone.clone(),
        cutoff.clone(),
        ignore,
    )
    .map(|lut| lut.unbind())
    .map_err(autocontrast_callback_error)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(ops_autocontrast_with_callbacks, module)?)?;
    Ok(())
}
