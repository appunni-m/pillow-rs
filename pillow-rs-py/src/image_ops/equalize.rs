use super::{
    imageops_global_or_builtin, imageops_mode_equals, imageops_mode_is_luma_or_rgb,
    imageops_unsupported_mode_error,
};
use crate::map_error;
use pillow_rs::PilError;
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyResult, Python};
use pyo3::types::{
    PyAnyMethods, PyDictMethods, PyInt, PyIterator, PyList, PyListMethods, PyModuleMethods,
    PySlice, PyTuple, PyTupleMethods, PyTypeMethods,
};
use pyo3::{pyfunction, wrap_pyfunction};

enum EqualizeLut<'py> {
    Native(Vec<u8>),
    Python(Bound<'py, PyAny>),
}

struct PythonEqualizeCallbacks<'py> {
    py: Python<'py>,
}

enum EqualizeCallbackError {
    Core(pillow_rs::PilError),
    Python(PyErr),
}

const PY_TPFLAGS_HEAPTYPE: u64 = 1 << 9;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn is_standard_equalize_builtin(value: &Bound<'_, PyAny>, name: &str) -> PyResult<bool> {
    // The module snapshot can itself contain replacements installed pre-import.
    let expected_type = match name {
        "len" | "sum" => "builtin_function_or_method",
        "range" | "list" => "type",
        _ => return Ok(false),
    };
    if value.get_type().name()?.to_string() != expected_type {
        return Ok(false);
    }
    if expected_type == "type"
        && value.getattr("__flags__")?.extract::<u64>()? & PY_TPFLAGS_HEAPTYPE != 0
    {
        // Python classes can spoof __module__ and __name__. Reject heap types
        // so a pre-import replacement for range/list cannot enter the native path.
        return Ok(false);
    }
    Ok(
        value.getattr("__module__")?.extract::<String>()? == "builtins"
            && value.getattr("__name__")?.extract::<String>()? == name,
    )
}

impl From<pillow_rs::PilError> for EqualizeCallbackError {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

impl From<PyErr> for EqualizeCallbackError {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

impl<'py> pillow_rs::ImageOpsEqualizeCallbacks for PythonEqualizeCallbacks<'py> {
    type Image = Bound<'py, PyAny>;
    type Mask = Bound<'py, PyAny>;
    type Histogram = Bound<'py, PyAny>;
    type Value = Bound<'py, PyAny>;
    type Sequence = Bound<'py, PyAny>;
    type Iterator = Bound<'py, PyIterator>;
    type Lut = EqualizeLut<'py>;
    type Output = Bound<'py, PyAny>;
    type Error = EqualizeCallbackError;

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn image_mode_is_palette(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        let mode = image.getattr("mode")?;
        imageops_mode_equals(self.py, &mode, "P").map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn lut_mode_is_palette(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        let mode = image.getattr("mode")?;
        imageops_mode_equals(self.py, &mode, "P").map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn lut_mode_is_luma_or_rgb(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        let mode = image.getattr("mode")?;
        imageops_mode_is_luma_or_rgb(self.py, &mode).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn lut_mode_is_rgb(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        let mode = image.getattr("mode")?;
        imageops_mode_equals(self.py, &mode, "RGB").map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn palette_mode_error(&mut self) -> Result<Self::Error, Self::Error> {
        Ok(EqualizeCallbackError::Python(
            pyo3::exceptions::PyNotImplementedError::new_err("mode P support coming soon"),
        ))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn unsupported_mode_error(&mut self, image: &Self::Image) -> Result<Self::Error, Self::Error> {
        let mode = image.getattr("mode")?;
        Ok(EqualizeCallbackError::Python(
            imageops_unsupported_mode_error(self.py, &mode)?,
        ))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn convert_to_rgb(&mut self, image: &Self::Image) -> Result<Self::Image, Self::Error> {
        image.call_method1("convert", ("RGB",)).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn histogram(
        &mut self,
        image: &Self::Image,
        mask: &Self::Mask,
    ) -> Result<Self::Histogram, Self::Error> {
        image.call_method1("histogram", (mask,)).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_histogram(
        &mut self,
        histogram: &Self::Histogram,
    ) -> Result<Option<Vec<u32>>, Self::Error> {
        // TODO(pillow-rs#3ptb): prove replacements installed before this module
        // is imported cannot imitate a builtin C callable's type and metadata.
        // If identity is uncertain, use the host callbacks.
        let imageops = self.py.import("pillow_rs.imageops")?;
        let namespace = imageops.dict();
        let names = ["len", "range", "sum", "list"];
        for name in names {
            if namespace.get_item(name)?.is_some() {
                return Ok(None);
            }
        }
        let expected = imageops.getattr("_EQUALIZE_FAST_BUILTINS")?;
        let expected = expected.cast::<PyTuple>().map_err(PyErr::from)?;
        for (index, name) in names.into_iter().enumerate() {
            let current = imageops_global_or_builtin(self.py, name)?;
            let original = expected.get_item(index)?;
            if !current.is(&original) || !is_standard_equalize_builtin(&current, name)? {
                return Ok(None);
            }
        }
        if !histogram.is_exact_instance_of::<PyList>() {
            return Ok(None);
        }
        let histogram = histogram.cast::<PyList>().map_err(PyErr::from)?;
        if histogram.len() % 256 != 0 {
            return Ok(None);
        }

        let mut counts = Vec::with_capacity(histogram.len());
        for value in histogram.iter() {
            if !value.is_exact_instance_of::<PyInt>() {
                return Ok(None);
            }
            let Ok(value) = value.extract::<u32>() else {
                return Ok(None);
            };
            counts.push(value);
        }
        if counts.chunks_exact(256).any(|channel| {
            channel.iter().map(|&count| u64::from(count)).sum::<u64>() > u64::from(u32::MAX)
        }) {
            return Ok(None);
        }
        Ok(Some(counts))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn new_lut(&mut self) -> Result<Self::Lut, Self::Error> {
        Ok(EqualizeLut::Python(PyList::empty(self.py).into_any()))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn channel_starts(
        &mut self,
        histogram: &Self::Histogram,
    ) -> Result<Self::Iterator, Self::Error> {
        let range = imageops_global_or_builtin(self.py, "range")?;
        let length = imageops_global_or_builtin(self.py, "len")?.call1((histogram,))?;
        let starts = range.call1((0, length, 256))?;
        PyIterator::from_object(&starts).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn next_channel_start(
        &mut self,
        starts: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error> {
        starts.next().transpose().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn histogram_channel(
        &mut self,
        histogram: &Self::Histogram,
        start: &Self::Value,
    ) -> Result<Self::Sequence, Self::Error> {
        let stop = start.add(256)?;
        let slice = self.py.get_type::<PySlice>().call1((start, stop))?;
        histogram.get_item(slice).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn channel_values(&mut self, channel: &Self::Sequence) -> Result<Self::Iterator, Self::Error> {
        PyIterator::from_object(channel).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn next_channel_value(
        &mut self,
        values: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error> {
        values.next().transpose().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn value_is_truthy(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        value.is_truthy().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn new_nonzero_values(&mut self) -> Result<Self::Sequence, Self::Error> {
        Ok(PyList::empty(self.py).into_any())
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn append_nonzero_value(
        &mut self,
        values: &mut Self::Sequence,
        value: &Self::Value,
    ) -> Result<(), Self::Error> {
        values.call_method1("append", (value,))?;
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn nonzero_value_count(&mut self, values: &Self::Sequence) -> Result<Self::Value, Self::Error> {
        Ok(imageops_global_or_builtin(self.py, "len")?.call1((values,))?)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn nonzero_value_count_at_most_one(
        &mut self,
        count: &Self::Value,
    ) -> Result<bool, Self::Error> {
        Ok(count.le(PyInt::new(self.py, 1))?)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn sum_nonzero_values(&mut self, values: &Self::Sequence) -> Result<Self::Value, Self::Error> {
        imageops_global_or_builtin(self.py, "sum")?
            .call1((values,))
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn last_nonzero_value(&mut self, values: &Self::Sequence) -> Result<Self::Value, Self::Error> {
        values.get_item(-1).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn subtract_values(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.sub(right).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn floor_divide_constant(
        &mut self,
        value: &Self::Value,
        divisor: i64,
    ) -> Result<Self::Value, Self::Error> {
        value
            .floor_div(PyInt::new(self.py, divisor))
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn floor_divide_values(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.floor_div(right).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn channel_offsets(&mut self) -> Result<Self::Iterator, Self::Error> {
        let range = imageops_global_or_builtin(self.py, "range")?;
        let offsets = range.call1((256,))?;
        PyIterator::from_object(&offsets).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn next_channel_offset(
        &mut self,
        offsets: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error> {
        offsets.next().transpose().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn histogram_value(
        &mut self,
        histogram: &Self::Histogram,
        index: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        histogram.get_item(index).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn add_values(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        left.add(right).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn append_lut_value(
        &mut self,
        lut: &mut Self::Lut,
        value: &Self::Value,
    ) -> Result<(), Self::Error> {
        match lut {
            EqualizeLut::Native(_) => Err(PilError::InternalError(
                "Equalize appended a value to a native LUT".into(),
            )
            .into()),
            EqualizeLut::Python(values) => {
                values.call_method1("append", (value,))?;
                Ok(())
            }
        }
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn extend_identity_lut(&mut self, lut: &mut Self::Lut) -> Result<(), Self::Error> {
        let EqualizeLut::Python(values) = lut else {
            return Err(PilError::InternalError(
                "Equalize extended an identity LUT into a native LUT".into(),
            )
            .into());
        };
        let extend = values.getattr("extend")?;
        let list = imageops_global_or_builtin(self.py, "list")?;
        let range = imageops_global_or_builtin(self.py, "range")?;
        let range_values = range.call1((256,))?;
        let identity = list.call1((range_values,))?;
        extend.call1((identity,))?;
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn native_lut(&mut self, lut: Vec<u8>) -> Result<Self::Lut, Self::Error> {
        Ok(EqualizeLut::Native(lut))
    }

    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn point(&mut self, image: &Self::Image, lut: Self::Lut) -> Result<Self::Output, Self::Error> {
        let lut = match lut {
            EqualizeLut::Native(lut) => PyList::new(self.py, lut)?.into_any(),
            EqualizeLut::Python(lut) => lut,
        };
        image.call_method1("point", (lut,)).map_err(Into::into)
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn map_equalize_callback_error(error: EqualizeCallbackError) -> PyErr {
    match error {
        EqualizeCallbackError::Core(error) => map_error(error),
        EqualizeCallbackError::Python(error) => error,
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[pyfunction]
fn ops_imageops_equalize_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    mask: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonEqualizeCallbacks { py };
    pillow_rs::imageops_equalize_with_callbacks(&mut callbacks, image.clone(), mask.clone())
        .map(|result| result.unbind())
        .map_err(map_equalize_callback_error)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(
        ops_imageops_equalize_with_callbacks,
        module
    )?)?;
    Ok(())
}
