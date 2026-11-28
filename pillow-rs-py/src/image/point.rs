use super::execution::{PyExecutionContext, route_from_context, route_pending_image};
use crate::*;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::types::{PyAnyMethods, PyInt, PyList, PyListMethods, PyString, PyTuple, PyTupleMethods};
use pyo3::{pyclass, pymethods};

/// Affine transform object passed to scalar Image.point callables.
///
/// Pillow uses this object to recognize expressions such as ``x + 1`` and
/// ``x * 0.5`` without enumerating a byte lookup table. Its arithmetic follows
/// Python's numeric protocols, and the resulting scale and offset are applied
/// by the Rust core.
// TODO(pillow-rs#3ptb): add public-access, custom-numeric, and fault parity cases.
#[pyclass(name = "ImagePointTransform", module = "PIL.Image", subclass)]
pub struct PyPointTransform {
    scale: Py<PyAny>,
    offset: Py<PyAny>,
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn point_transform_neg(value: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
    let scale = value.getattr("scale")?.neg()?.unbind();
    let offset = value.getattr("offset")?.neg()?.unbind();
    Ok(Py::new(py, PyPointTransform { scale, offset })?.into_any())
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn point_transform_add(
    value: &Bound<'_, PyAny>,
    other: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
    let value_scale = value.getattr("scale")?;
    let value_offset = value.getattr("offset")?;
    let (scale, offset) = if other.extract::<PyRef<'_, PyPointTransform>>().is_ok() {
        let other_scale = other.getattr("scale")?;
        let other_offset = other.getattr("offset")?;
        (
            value_scale.add(&other_scale)?.unbind(),
            value_offset.add(&other_offset)?.unbind(),
        )
    } else {
        (
            value_scale.clone().unbind(),
            value_offset.add(other)?.unbind(),
        )
    };
    Ok(Py::new(py, PyPointTransform { scale, offset })?.into_any())
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn point_transform_multiply(
    value: &Bound<'_, PyAny>,
    other: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
    if other.extract::<PyRef<'_, PyPointTransform>>().is_ok() {
        return Ok(py.NotImplemented());
    }
    let scale = value.getattr("scale")?.mul(other)?.unbind();
    let offset = value.getattr("offset")?.mul(other)?.unbind();
    Ok(Py::new(py, PyPointTransform { scale, offset })?.into_any())
}

/// Probes a callable with Pillow's affine point-transform sentinel.
///
/// The callback is intentionally invoked while the GIL is held.  Only the
/// resulting scalar parameters cross into the Rust core, so the core never
/// needs to retain or call a Python object.
// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub(super) fn point_transform_from_python(
    input: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<(f64, f64)> {
    let seed = Py::new(
        py,
        PyPointTransform {
            scale: PyInt::new(py, 1).unbind().into_any(),
            offset: PyInt::new(py, 0).unbind().into_any(),
        },
    )?;
    let result = input.call1((seed,))?;
    if result.extract::<PyRef<'_, PyPointTransform>>().is_ok() {
        let scale = result.getattr("scale")?.extract::<f64>()?;
        let offset = result.getattr("offset")?.extract::<f64>()?;
        return Ok((scale, offset));
    }
    Ok((0.0, result.extract::<f64>()?))
}

#[pymethods]
impl PyPointTransform {
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[new]
    fn new(scale: Py<PyAny>, offset: Py<PyAny>) -> Self {
        Self { scale, offset }
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[getter]
    fn scale(&self, py: Python<'_>) -> Py<PyAny> {
        self.scale.clone_ref(py)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[setter]
    fn set_scale(&mut self, scale: Py<PyAny>) {
        self.scale = scale;
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[getter]
    fn offset(&self, py: Python<'_>) -> Py<PyAny> {
        self.offset.clone_ref(py)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[setter]
    fn set_offset(&mut self, offset: Py<PyAny>) {
        self.offset = offset;
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn __neg__(slf: Py<Self>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        point_transform_neg(slf.bind(py), py)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn __add__(slf: Py<Self>, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        point_transform_add(slf.bind(py), other, py)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn __radd__(slf: Py<Self>, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        point_transform_add(slf.bind(py), other, py)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn __sub__(slf: Py<Self>, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let negative = other.neg()?;
        Ok(slf.bind(py).add(&negative)?.unbind())
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn __rsub__(slf: Py<Self>, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let negative = slf.bind(py).neg()?;
        Ok(other.add(&negative)?.unbind())
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn __mul__(slf: Py<Self>, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        point_transform_multiply(slf.bind(py), other, py)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn __rmul__(slf: Py<Self>, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        point_transform_multiply(slf.bind(py), other, py)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn __truediv__(slf: Py<Self>, other: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if other.extract::<PyRef<'_, PyPointTransform>>().is_ok() {
            return Ok(py.NotImplemented());
        }
        let scale = slf.bind(py).getattr("scale")?.div(other)?.unbind();
        let offset = slf.bind(py).getattr("offset")?.div(other)?.unbind();
        Ok(Py::new(py, Self { scale, offset })?.into_any())
    }
}

enum PointExecution {
    ScalarTransform {
        scale: f64,
        offset: f64,
    },
    FloatLut(Vec<f64>),
    ModeLut {
        target: pillow_rs::Mode,
        lut: Vec<u8>,
    },
    ReplicatedModeLut {
        target: pillow_rs::Mode,
        lut: Vec<u8>,
    },
    ReplicatedLut(Vec<u8>),
    ValidatedLut(Vec<u8>),
}

fn execute_point(
    image: &pillow_rs::Image,
    operation: PointExecution,
    context: Option<pillow_rs::ExecutionContext>,
) -> Result<pillow_rs::Image, pillow_rs::PilError> {
    match operation {
        PointExecution::ScalarTransform { scale, offset } => match context {
            Some(context) => pillow_rs::image_eval_point_transform_with_execution_context(
                image, scale, offset, context,
            ),
            None => pillow_rs::image_eval_point_transform(image, scale, offset),
        },
        PointExecution::FloatLut(lut) => match context {
            Some(context) => {
                pillow_rs::image_eval_float_with_execution_context(image, &lut, context)
            }
            None => pillow_rs::image_eval_float(image, &lut),
        },
        PointExecution::ModeLut { target, lut } => match context {
            Some(context) => pillow_rs::image_eval_point_mode_with_execution_context(
                image, target, &lut, context,
            ),
            None => pillow_rs::image_eval_point_mode(image, target, &lut),
        },
        PointExecution::ReplicatedModeLut { target, lut } => match context {
            Some(context) => {
                pillow_rs::image_eval_point_mode_replicated_for_image_with_execution_context(
                    image, target, &lut, context,
                )
            }
            None => pillow_rs::image_eval_point_mode_replicated_for_image(image, target, &lut),
        },
        PointExecution::ReplicatedLut(lut) => match context {
            Some(context) => pillow_rs::image_eval_replicated_for_image_with_execution_context(
                image, &lut, context,
            ),
            None => pillow_rs::image_eval_replicated_for_image(image, &lut),
        },
        PointExecution::ValidatedLut(lut) => match context {
            Some(context) => {
                pillow_rs::image_eval_validated_with_execution_context(image, &lut, context)
            }
            None => pillow_rs::image_eval_validated(image, &lut),
        },
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn point_lut_items_from_python(input: &Bound<'_, PyAny>) -> PyResult<Vec<Py<PyAny>>> {
    if input.is_callable() {
        let mut values = Vec::with_capacity(256);
        for sample in 0..256u32 {
            values.push(input.call1((sample,))?.unbind());
        }
        return Ok(values);
    }

    input
        .try_iter()?
        .map(|value| value.map(|value| value.unbind()))
        .collect()
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
#[allow(unsafe_code)] // PySequence_Check preserves the ImagingCore protocol order.
fn point_float_lut_items_from_python(input: &Bound<'_, PyAny>) -> PyResult<Vec<Py<PyAny>>> {
    // Pillow's float-LUT core path checks PySequence_Check, then calls
    // PySequence_Size before snapshotting or converting any item.
    // SAFETY: `input` is a live Python object and this adapter holds the GIL.
    if unsafe { pyo3::ffi::PySequence_Check(input.as_ptr()) } == 0 {
        return Err(PyTypeError::new_err("argument must be a sequence"));
    }
    if input.len()? != 256 {
        return Err(PyValueError::new_err("wrong number of lut entries"));
    }

    let values = if let Ok(items) = input.cast::<PyList>() {
        (0..256)
            .map(|index| items.get_item(index).map(|item| item.unbind()))
            .collect::<PyResult<Vec<_>>>()?
    } else if let Ok(items) = input.cast::<PyTuple>() {
        (0..256)
            .map(|index| items.get_item(index).map(|item| item.unbind()))
            .collect::<PyResult<Vec<_>>>()?
    } else {
        point_lut_items_from_python(input)?
    };
    if values.len() != 256 {
        return Err(PyValueError::new_err("wrong number of lut entries"));
    }
    Ok(values)
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn point_round_lut_items(values: Vec<Py<PyAny>>, py: Python<'_>) -> PyResult<Vec<Py<PyAny>>> {
    let round = py.import("builtins")?.getattr("round")?;
    values
        .into_iter()
        .map(|value| round.call1((value.bind(py),)).map(|value| value.unbind()))
        .collect()
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn point_lut_to_bytes(values: &[Py<PyAny>], py: Python<'_>) -> PyResult<Vec<u8>> {
    values
        .iter()
        .map(|value| {
            value
                .bind(py)
                .extract::<std::os::raw::c_long>()
                // Pillow narrows PyLong_AsLong to C `int` before CLIP8.
                .map(|value| (value as i32).clamp(0, 255) as u8)
        })
        .collect()
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn point_lut_to_floats(values: &[Py<PyAny>], py: Python<'_>) -> PyResult<Vec<f64>> {
    values
        .iter()
        .map(|value| value.bind(py).extract::<f64>())
        .collect()
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn point_mode_rounds_lut(mode: Option<&Bound<'_, PyAny>>, py: Python<'_>) -> PyResult<bool> {
    let Some(mode) = mode else {
        return Ok(true);
    };
    mode.rich_compare(PyString::new(py, "F"), CompareOp::Ne)?
        .is_truthy()
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn point_target_mode(mode: Option<&Bound<'_, PyAny>>) -> PyResult<Option<pillow_rs::Mode>> {
    let Some(mode) = mode else {
        return Ok(None);
    };
    let mode = mode.extract::<String>()?;
    Ok(pillow_rs::Mode::from_pillow_name(&mode))
}

// Pillow's affine branch covers I, native I;16, and F. Rust also applies the
// same native-u16 operation to endian-specific I;16 identities.
// NO_PILLOW_SUPPORT: Pillow 13 rejects callable I;16L/B/N point routes.
// TODO(pillow-rs#3ptb): add target-only output and fault records for I;16L/B/N.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn point_uses_affine_transform(mode: pillow_rs::Mode) -> bool {
    matches!(
        mode,
        pillow_rs::Mode::ImageI32
            | pillow_rs::Mode::ImageI16
            | pillow_rs::Mode::ImageI16L
            | pillow_rs::Mode::ImageI16B
            | pillow_rs::Mode::ImageI16N
            | pillow_rs::Mode::ImageF32
    )
}

#[pymethods]
impl PyImage {
    /// Applies a sequence or callable LUT through the Rust-owned public path.
    // Pillow's public wrapper loads first; scalar affine transforms return
    // without consulting the supplied mode. Rust additionally supports native
    // u16 transforms for I;16L/B/N. Byte LUTs compare with "F" after callable
    // evaluation and extract the typed mode after optional round.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[pyo3(signature = (input, mode=None, execution_context=None))]
    fn point(
        slf: Py<Self>,
        input: &Bound<'_, PyAny>,
        mode: Option<&Bound<'_, PyAny>>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let source_mode = slf
            .bind(py)
            .borrow()
            .inner
            .mode_identity()
            .map_err(map_error)?;
        let callable = input.is_callable();
        let operation = if callable && point_uses_affine_transform(source_mode) {
            let (scale, offset) = point_transform_from_python(input, py)?;
            PointExecution::ScalarTransform { scale, offset }
        } else if source_mode == pillow_rs::Mode::ImageF32 {
            return Err(map_error(pillow_rs::PilError::ValueError(
                "point operation not supported for this mode".into(),
            )));
        } else {
            // Pillow constructs callable LUTs before comparing mode with "F";
            // sequence iteration and rounding happen after that comparison.
            let callback_values = callable
                .then(|| point_lut_items_from_python(input))
                .transpose()?;
            let round_lut = point_mode_rounds_lut(mode, py)?;
            // The core parser checks the mode argument before reading a
            // non-rounded F LUT; rounded LUTs are iterated before core parsing.
            let early_target_mode = if round_lut {
                None
            } else {
                Some(point_target_mode(mode)?)
            };
            let values = match callback_values {
                Some(values) => values,
                None if round_lut => point_lut_items_from_python(input)?,
                None => point_float_lut_items_from_python(input)?,
            };
            let values = if round_lut {
                point_round_lut_items(values, py)?
            } else {
                values
            };
            // TODO(pillow-rs#3ptb): verify non-string mode objects, custom __ne__,
            // and LUT exceptions against Pillow's post-round core argument parsing.
            // This typed conversion follows Pillow's LUT callback, mode comparison,
            // and optional round calls. Unknown strings retain the source mode.
            let target_mode = match early_target_mode {
                Some(target_mode) => target_mode,
                None => point_target_mode(mode)?,
            };
            if !round_lut {
                PointExecution::FloatLut(point_lut_to_floats(&values, py)?)
            } else {
                let lut = point_lut_to_bytes(&values, py)?;
                match (
                    callable,
                    target_mode.filter(|target| *target != source_mode),
                ) {
                    (true, Some(target)) => PointExecution::ReplicatedModeLut { target, lut },
                    (true, None) => PointExecution::ReplicatedLut(lut),
                    (false, Some(target)) => PointExecution::ModeLut { target, lut },
                    (false, None) => PointExecution::ValidatedLut(lut),
                }
            }
        };
        let route = route_from_context(execution_context, py);
        let inner = slf.bind(py).borrow().inner.clone();
        let inner = route_pending_image(inner, route);
        py.detach(move || match route {
            Some(route) => execute_point(&inner, operation, Some(route.context)),
            None => execute_point(&inner, operation, None),
        })
        .map(|i| PyImage {
            inner: route_pending_image(i, route),
        })
        .map_err(map_error)
    }
}
