use super::PyImage;
use crate::errors::map_error;
use crate::inputs::{
    c_int_from_python, nonnegative_output_size_items_from_python, transform_data_from_python,
    transform_fill_after_size_from_python, transform_fill_preflight_from_python,
};
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use pyo3::{pyclass, pymethods};

#[pyclass(name = "_TransformPlan")]
pub(super) struct PyTransformPlan {
    destination: Option<pillow_rs::PreparedTransformOutput>,
}

#[pyclass(name = "_TransformDataPlan", frozen)]
pub(super) struct PyTransformDataPlan {
    method: i32,
    data: Option<pillow_rs::TransformData>,
    resample: i32,
}

#[pyclass(name = "_TransformMeshState")]
pub(super) struct PyTransformMeshState {
    inner: Option<pillow_rs::TransformMeshAccumulator>,
}

fn validate_transform_resample_membership(
    py: Python<'_>,
    value: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let supported = PyTuple::new(py, [0, 2, 3])?;
    if supported.contains(value)? {
        return Ok(());
    }

    let unusable = PyTuple::new(py, [4, 5, 1])?;
    let code = if unusable.contains(value)? {
        let names = PyDict::new(py);
        names.set_item(4, "Image.Resampling.BOX")?;
        names.set_item(5, "Image.Resampling.HAMMING")?;
        names.set_item(1, "Image.Resampling.LANCZOS")?;
        let name = names
            .as_any()
            .call_method1("__getitem__", (value,))?
            .extract::<String>()?;
        Some(match name.as_str() {
            "Image.Resampling.LANCZOS" => 1,
            "Image.Resampling.BOX" => 4,
            "Image.Resampling.HAMMING" => 5,
            _ => unreachable!("resampling name came from the local mapping"),
        })
    } else {
        None
    };
    let display = value.str()?.to_string();
    Err(map_error(pillow_rs::transform_resample_public_error(
        code, &display,
    )))
}

fn transform_mesh_resample_from_python_after_load(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    let code = c_int_from_python(value)?;
    if matches!(code, 0 | 2 | 3) {
        return Ok(code);
    }
    let display = value.str()?.to_string();
    Err(map_error(pillow_rs::transform_resample_public_error(
        Some(code),
        &display,
    )))
}

fn transform_mesh_box_from_python(value: &Bound<'_, PyAny>) -> PyResult<(i64, i64, i64, i64)> {
    let mut coordinates = [0_i64; 4];
    for (index, coordinate) in coordinates.iter_mut().enumerate() {
        let item = value.get_item(index)?;
        *coordinate = i64::from(c_int_from_python(&item)?);
    }
    Ok((
        coordinates[0],
        coordinates[1],
        coordinates[2],
        coordinates[3],
    ))
}

fn transform_mesh_coefficients_from_python(value: &Bound<'_, PyAny>) -> PyResult<[f64; 8]> {
    let mut values = [0.0; 8];
    let mut items = value.try_iter()?;
    for value in &mut values {
        let item = items
            .next()
            .transpose()?
            .ok_or_else(|| pyo3::exceptions::PyIndexError::new_err("tuple index out of range"))?;
        *value = item.extract::<f64>()?;
    }
    Ok(values)
}

#[pymethods]
impl PyImage {
    fn _transform_prepare(
        slf: Py<Self>,
        size: &Bound<'_, PyAny>,
        fillcolor: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyTransformPlan> {
        let mode = {
            let image = slf.bind(py).borrow();
            image.inner.mode_identity().map_err(map_error)?
        };
        let (width, height) = nonnegative_output_size_items_from_python(size)?;
        let (pre_size_fill, deferred_fill) =
            transform_fill_preflight_from_python(py, fillcolor, mode)?;
        let deferred_fill = deferred_fill.as_ref().map(|fill| fill.bind(py));
        let pre_size_prepared = pre_size_fill
            .map(|fill| {
                let image = slf.bind(py).borrow();
                image
                    .inner
                    .prepare_transform_fill_for_mode(mode, Some(fill))
                    .map_err(map_error)
            })
            .transpose()?;
        let width = c_int_from_python(&width)?;
        let height = c_int_from_python(&height)?;
        let output_size = (width as u32, height as u32);
        let prepared = match pre_size_prepared {
            Some(prepared) => prepared,
            None => {
                let fill =
                    transform_fill_after_size_from_python(deferred_fill.or(fillcolor), mode)?;
                let image = slf.bind(py).borrow();
                image
                    .inner
                    .prepare_transform_fill_for_mode(mode, fill)
                    .map_err(map_error)?
            }
        };
        let image = slf.bind(py).borrow();
        let source = image.inner.clone();
        drop(image);
        let destination = py
            .detach(move || source.prepare_transform_output_public(prepared, output_size))
            .map_err(map_error)?;
        Ok(PyTransformPlan {
            destination: Some(destination),
        })
    }

    fn _transform_mesh_begin(
        slf: Py<Self>,
        mut plan: pyo3::PyRefMut<'_, PyTransformPlan>,
        py: Python<'_>,
    ) -> PyResult<PyTransformMeshState> {
        let destination = plan.destination.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("Transform plan has already been consumed")
        })?;
        drop(plan);
        let image = slf.bind(py).borrow();
        let source = image.inner.clone();
        drop(image);
        py.detach(move || source.prepare_transform_mesh_from_output_public(destination))
            .map(|inner| PyTransformMeshState { inner: Some(inner) })
            .map_err(map_error)
    }

    fn _transform_validate_filter(
        slf: Py<Self>,
        value: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let _keep_self_alive = slf;
        validate_transform_resample_membership(py, value)
    }

    fn _transform_mesh_record(
        slf: Py<Self>,
        mut state: pyo3::PyRefMut<'_, PyTransformMeshState>,
        box_coordinates: &Bound<'_, PyAny>,
        coefficients: &Bound<'_, PyAny>,
        resample: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let box_coordinates = transform_mesh_box_from_python(box_coordinates)?;
        let filter = match transform_mesh_resample_from_python_after_load(resample)? {
            0 => pillow_rs::ResampleFilter::Nearest,
            2 => pillow_rs::ResampleFilter::Bilinear,
            3 => pillow_rs::ResampleFilter::Bicubic,
            _ => unreachable!("validated mesh filter code"),
        };
        let coefficients = transform_mesh_coefficients_from_python(coefficients)?;
        // Python captures this PyImage before argument extraction. A
        // reentrant replacement is visible on the next record, whose mode is
        // checked against the accumulator's original output mode by the core.
        let source = slf.bind(py).borrow().inner.clone();
        let mut accumulator = state.inner.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "Transform MESH state has already been consumed",
            )
        })?;
        let (accumulator, result) = py.detach(move || {
            let result = source.apply_transform_mesh_record_public(
                &mut accumulator,
                box_coordinates,
                coefficients,
                filter,
            );
            (accumulator, result)
        });
        state.inner = Some(accumulator);
        result.map_err(map_error)
    }

    fn _transform_mesh_finish(
        slf: Py<Self>,
        mut state: pyo3::PyRefMut<'_, PyTransformMeshState>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let _keep_self_alive = slf;
        let accumulator = state.inner.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err(
                "Transform MESH state has already been consumed",
            )
        })?;
        py.detach(|| accumulator.finish())
            .map(|inner| PyImage { inner })
            .map_err(map_error)
    }

    fn _transform_data(
        slf: Py<Self>,
        method: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
        resample: &Bound<'_, PyAny>,
    ) -> PyResult<PyTransformDataPlan> {
        let _keep_self_alive = slf;
        // `_imaging._transform` parses method before resample and then
        // converts the coefficient sequence after the fixed arguments.
        let method = method.extract::<i32>()?;
        let resample = c_int_from_python(resample)?;
        let data = transform_data_from_python(Some(data))?;
        Ok(PyTransformDataPlan {
            method,
            data,
            resample,
        })
    }

    fn _transform_apply(
        slf: Py<Self>,
        mut plan: pyo3::PyRefMut<'_, PyTransformPlan>,
        data_plan: pyo3::PyRef<'_, PyTransformDataPlan>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let destination = plan.destination.take().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("Transform plan has already been consumed")
        })?;
        drop(plan);
        let method = data_plan.method;
        let data = data_plan.data.clone();
        let resample = data_plan.resample;
        drop(data_plan);
        let image = slf.bind(py).borrow();
        let source = image.inner.clone();
        drop(image);
        py.detach(move || {
            source.transform_prepared_output_public(destination, method, data, resample)
        })
        .map(|inner| PyImage { inner })
        .map_err(map_error)
    }
}
