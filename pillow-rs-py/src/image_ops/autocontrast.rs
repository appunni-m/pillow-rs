use super::{
    imageops_global_or_builtin, imageops_mode_equals, imageops_mode_is_luma_or_rgb,
    imageops_unsupported_mode_error,
};
use crate::map_error;
use pyo3::exceptions::{PyAttributeError, PyUnboundLocalError};
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyResult, Python};
use pyo3::types::{
    PyAnyMethods, PyBool, PyFloat, PyInt, PyIterator, PyList, PyListMethods, PyModuleMethods,
    PySlice, PyTuple, PyTupleMethods,
};
use pyo3::{pyfunction, wrap_pyfunction};

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
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

impl From<PyErr> for AutocontrastCallbackError {
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn convert_to_luma(&mut self, image: &Self::Image) -> Result<Self::Image, Self::Error> {
        image.call_method1("convert", ("L",)).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn histogram(
        &mut self,
        image: &Self::Image,
        mask: &Self::Mask,
    ) -> Result<Self::Value, Self::Error> {
        image.call_method1("histogram", (mask,)).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn preserve_tone_is_truthy(&mut self, value: &Self::PreserveTone) -> Result<bool, Self::Error> {
        value.is_truthy().map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsAutocontrastResultCallbacks for PythonAutocontrastCallbacks<'py> {
    type Lut = Bound<'py, PyAny>;
    type Output = Bound<'py, PyAny>;
    type Error = AutocontrastCallbackError;

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn image_mode_is_palette(&mut self) -> Result<bool, Self::Error> {
        let mode = self.image.getattr("mode")?;
        imageops_mode_equals(self.py, &mode, "P").map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn image_mode_is_luma_or_rgb(&mut self) -> Result<bool, Self::Error> {
        let mode = self.image.getattr("mode")?;
        imageops_mode_is_luma_or_rgb(self.py, &mode).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn image_mode_is_rgb(&mut self) -> Result<bool, Self::Error> {
        let mode = self.image.getattr("mode")?;
        imageops_mode_equals(self.py, &mode, "RGB").map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn palette_mode_error(&mut self) -> Result<Self::Error, Self::Error> {
        Ok(AutocontrastCallbackError::Python(
            pyo3::exceptions::PyNotImplementedError::new_err("mode P support coming soon"),
        ))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn unsupported_mode_error(&mut self) -> Result<Self::Error, Self::Error> {
        let mode = self.image.getattr("mode")?;
        Ok(AutocontrastCallbackError::Python(
            imageops_unsupported_mode_error(self.py, &mode)?,
        ))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn lut_length(&mut self, lut: &Self::Lut) -> Result<usize, Self::Error> {
        imageops_global_or_builtin(self.py, "len")?
            .call1((lut,))?
            .extract()
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn repeat_lut(&mut self, lut: Self::Lut) -> Result<Self::Lut, Self::Error> {
        let doubled = lut.add(&lut)?;
        doubled.add(&lut).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn point(&mut self, lut: Self::Lut) -> Result<Self::Output, Self::Error> {
        self.image.call_method1("point", (lut,)).map_err(Into::into)
    }
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn autocontrast_callback_error(error: AutocontrastCallbackError) -> PyErr {
    match error {
        AutocontrastCallbackError::Core(error) => map_error(error),
        AutocontrastCallbackError::Python(error) => error,
    }
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn autocontrast_cutoff_has_builtin_values(cutoff: Option<&Bound<'_, PyAny>>) -> bool {
    let Some(cutoff) = cutoff else {
        return true;
    };
    if cutoff.is_exact_instance_of::<PyTuple>() {
        let Ok(values) = cutoff.cast::<PyTuple>() else {
            return false;
        };
        return values.len() == 2
            && values
                .iter()
                .all(|value| is_builtin_autocontrast_number(&value));
    }
    is_builtin_autocontrast_number(cutoff)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn is_builtin_autocontrast_number(value: &Bound<'_, PyAny>) -> bool {
    value.is_exact_instance_of::<PyInt>()
        || value.is_exact_instance_of::<PyFloat>()
        || value.is_exact_instance_of::<PyBool>()
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn autocontrast_module_globals_match_fast_callables(py: Python<'_>) -> PyResult<bool> {
    let imageops = py.import("pillow_rs.imageops")?;
    let expected_value = match imageops.getattr("_AUTOCONTRAST_FAST_BUILTINS") {
        Ok(expected) => expected,
        Err(error) if error.is_instance_of::<PyAttributeError>(py) => return Ok(false),
        Err(error) => return Err(error),
    };
    let expected = match expected_value.cast::<PyTuple>() {
        Ok(expected) if expected.len() == 4 => expected,
        _ => return Ok(false),
    };
    let namespace = imageops.dict();
    for (index, name) in ["int", "list", "range", "sum"].into_iter().enumerate() {
        if !namespace.contains(name)? {
            continue;
        }
        let actual = namespace.get_item(name)?;
        if !actual.is(&expected.get_item(index)?) {
            return Ok(false);
        }
    }
    Ok(true)
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
fn autocontrast_unbound_local(py: Python<'_>, name: &str) -> PyResult<PyErr> {
    let version = py.import("sys")?.getattr("version_info")?;
    let major = version.get_item(0)?.extract::<u8>()?;
    let minor = version.get_item(1)?.extract::<u8>()?;
    let message = if major > 3 || minor >= 11 {
        format!("cannot access local variable '{name}' where it is not associated with a value")
    } else {
        format!("local variable '{name}' referenced before assignment")
    };
    Ok(PyUnboundLocalError::new_err(message))
}

impl<'py> pillow_rs::ImageOpsAutocontrastCallbacks for PythonAutocontrastLutCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Iterator = Bound<'py, PyIterator>;
    type Error = AutocontrastCallbackError;

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn native_lut_path_is_safe(
        &mut self,
        channel: &Self::Value,
        cutoff: Option<&Self::Value>,
    ) -> Result<bool, Self::Error> {
        if !channel.is_exact_instance_of::<PyList>()
            || channel
                .cast::<PyList>()
                .map_err(PyErr::from)?
                .iter()
                .any(|value| !value.is_exact_instance_of::<PyInt>())
            || !autocontrast_cutoff_has_builtin_values(cutoff)
        {
            return Ok(false);
        }
        // Explicit module-level callables keep their host callback route.
        // Standard builtins are assumed; their runtime identities are not probed.
        autocontrast_module_globals_match_fast_callables(self.py).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn new_lut(&mut self) -> Result<Self::Value, Self::Error> {
        Ok(PyList::empty(self.py).into_any())
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn channel_starts(&mut self, histogram: &Self::Value) -> Result<Self::Iterator, Self::Error> {
        // Python evaluates the `range` name before evaluating `len(histogram)`.
        let range = imageops_global_or_builtin(self.py, "range")?;
        let length = imageops_global_or_builtin(self.py, "len")?.call1((histogram,))?;
        let starts = range.call1((0, length, 256))?;
        PyIterator::from_object(&starts).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn next_channel_start(
        &mut self,
        starts: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error> {
        starts.next().transpose().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn histogram_channel(
        &mut self,
        histogram: &Self::Value,
        start: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let stop = start.add(256)?;
        let slice = self.py.get_type::<PySlice>().call1((start, stop))?;
        histogram.get_item(slice).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn ignore_is_integer(&mut self, ignore: &Self::Value) -> Result<bool, Self::Error> {
        let is_instance = imageops_global_or_builtin(self.py, "isinstance")?;
        let integer_type = imageops_global_or_builtin(self.py, "int")?;
        is_instance
            .call1((ignore, integer_type))?
            .is_truthy()
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn ignore_indices(&mut self, ignore: &Self::Value) -> Result<Self::Iterator, Self::Error> {
        PyIterator::from_object(ignore).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn next_ignore_index(
        &mut self,
        indices: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error> {
        indices.next().transpose().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn zero_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: &Self::Value,
    ) -> Result<(), Self::Error> {
        channel.set_item(index, 0).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn histogram_bin(
        &mut self,
        channel: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error> {
        channel.get_item(index).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn histogram_bin_at(
        &mut self,
        channel: &Self::Value,
        index: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        channel.get_item(index).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn sum_histogram_channel(&mut self, channel: &Self::Value) -> Result<Self::Value, Self::Error> {
        imageops_global_or_builtin(self.py, "sum")?
            .call1((channel,))
            .map(|value| value.into_any())
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn cutoff_component(
        &mut self,
        cutoff: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error> {
        cutoff.get_item(index).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn multiply(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.mul(right).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn floor_divide_by_hundred(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error> {
        value
            .floor_div(PyInt::new(self.py, 100))
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn int_function(&mut self) -> Result<Self::Value, Self::Error> {
        imageops_global_or_builtin(self.py, "int").map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn call_int_function(
        &mut self,
        function: &Self::Value,
        value: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        function
            .call1((value,))
            .map(|value| value.into_any())
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn greater_than(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<bool, Self::Error> {
        left.gt(right).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.sub(right).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn add_values(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.add(right).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn divide_float_255_by(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error> {
        PyFloat::new(self.py, 255.0)
            .as_any()
            .div(value)
            .map(|value| value.into_any())
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn negate_value(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error> {
        value.neg().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn lut_value_is_negative(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        value.lt(PyInt::new(self.py, 0)).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn lut_value_exceeds_byte(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        value.gt(PyInt::new(self.py, 255)).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn lut_scan_indices(&mut self, reverse: bool) -> Result<Self::Iterator, Self::Error> {
        let range = imageops_global_or_builtin(self.py, "range")?;
        let values = if reverse {
            range.call1((255, -1, -1))?
        } else {
            range.call1((256,))?
        };
        PyIterator::from_object(&values).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn non_identity_lut_bounds(
        &mut self,
        high: Option<&pillow_rs::ImageOpsAutocontrastLoopIndex<Self::Value>>,
        low: Option<&pillow_rs::ImageOpsAutocontrastLoopIndex<Self::Value>>,
    ) -> Result<Option<(Self::Value, Self::Value)>, Self::Error> {
        let high = match high {
            Some(pillow_rs::ImageOpsAutocontrastLoopIndex::Native(index)) => {
                PyInt::new(self.py, *index).into_any()
            }
            Some(pillow_rs::ImageOpsAutocontrastLoopIndex::Host(high)) => high.clone(),
            None => return Err(autocontrast_unbound_local(self.py, "hi")?.into()),
        };
        let low = match low {
            Some(pillow_rs::ImageOpsAutocontrastLoopIndex::Native(index)) => {
                PyInt::new(self.py, *index).into_any()
            }
            Some(pillow_rs::ImageOpsAutocontrastLoopIndex::Host(low)) => low.clone(),
            None => return Err(autocontrast_unbound_local(self.py, "lo")?.into()),
        };
        if high.le(&low)? {
            Ok(None)
        } else {
            Ok(Some((high.clone(), low.clone())))
        }
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn append_lut_value(
        &mut self,
        lut: &mut Self::Value,
        value: &Self::Value,
    ) -> Result<(), Self::Error> {
        lut.call_method1("append", (value,))?;
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn extend_identity_lut(&mut self, lut: &mut Self::Value) -> Result<(), Self::Error> {
        let extend = lut.getattr("extend")?;
        let list = imageops_global_or_builtin(self.py, "list")?;
        let range = imageops_global_or_builtin(self.py, "range")?;
        let values = range.call1((256,))?;
        let identity = list.call1((values,))?;
        extend.call1((identity,))?;
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn subtract_from_histogram_bin_at(
        &mut self,
        channel: &mut Self::Value,
        index: &Self::Value,
        value: &Self::Value,
    ) -> Result<(), Self::Error> {
        let bin = channel.get_item(index)?;
        let subtract = self.py.import("operator")?.getattr("isub")?;
        let result = subtract.call1((bin, value))?;
        channel.set_item(index, result).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn clear_histogram_bin(
        &mut self,
        channel: &mut Self::Value,
        index: usize,
    ) -> Result<(), Self::Error> {
        channel.set_item(index, 0).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn clear_histogram_bin_at(
        &mut self,
        channel: &mut Self::Value,
        index: &Self::Value,
    ) -> Result<(), Self::Error> {
        channel.set_item(index, 0).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn cutoff_is_nonpositive(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error> {
        cutoff.le(PyInt::new(self.py, 0)).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn histogram_bin_is_truthy(
        &mut self,
        channel: &Self::Value,
        index: usize,
    ) -> Result<bool, Self::Error> {
        channel.get_item(index)?.is_truthy().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn histogram_bin_at_is_truthy(
        &mut self,
        channel: &Self::Value,
        index: &Self::Value,
    ) -> Result<bool, Self::Error> {
        channel.get_item(index)?.is_truthy().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn zero_value(&mut self) -> Result<Self::Value, Self::Error> {
        Ok(PyInt::new(self.py, 0).into_any())
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn byte_maximum_value(&mut self) -> Result<Self::Value, Self::Error> {
        Ok(PyInt::new(self.py, 255).into_any())
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn cutoff_is_truthy(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error> {
        cutoff.is_truthy().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn cutoff_is_tuple(&mut self, cutoff: &Self::Value) -> Result<bool, Self::Error> {
        let is_instance = imageops_global_or_builtin(self.py, "isinstance")?;
        let tuple_type = imageops_global_or_builtin(self.py, "tuple")?;
        is_instance
            .call1((cutoff, tuple_type))?
            .is_truthy()
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn duplicate_cutoff(&mut self, cutoff: &Self::Value) -> Result<Self::Value, Self::Error> {
        PyTuple::new(self.py, [cutoff.clone(), cutoff.clone()])
            .map(|value| value.into_any())
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn extend_lut(&mut self, lut: &mut Self::Value, channel_lut: &[u8]) -> Result<(), Self::Error> {
        let channel_lut = PyList::new(self.py, channel_lut)?;
        lut.call_method1("extend", (channel_lut,))?;
        Ok(())
    }
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(ops_autocontrast_with_callbacks, module)?)?;
    Ok(())
}
