use super::arrays::image_info_to_python;
use crate::*;
use pyo3::basic::CompareOp;
use pyo3::pymethods;
use pyo3::{pyfunction, wrap_pyfunction};

fn imaging_core_mask(mask: Option<&Bound<'_, PyAny>>) -> PyResult<Option<pillow_rs::Image>> {
    let Some(mask) = mask else {
        return Ok(None);
    };
    let core = mask.cast::<super::PyImage>().map_err(|_| {
        let type_name = mask
            .get_type()
            .name()
            .map_or_else(|_| "object".into(), |name| name.to_string());
        pyo3::exceptions::PyTypeError::new_err(format!(
            "argument 2 must be ImagingCore, not {type_name}"
        ))
    })?;
    Ok(Some(core.borrow().inner.clone()))
}

#[pymethods]
impl PyImage {
    fn getextrema(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        self.getextrema_formatted(py)
    }

    /// Expose ImagingCore's low-level color count to Filter callbacks.
    fn getcolors(
        slf: Py<Self>,
        maxcolors: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<Option<Py<PyAny>>> {
        Self::getcolors_formatted(slf, Some(maxcolors), Some(false), py)
    }

    /// ImagingCore orders optional arguments as extrema, then mask.
    #[pyo3(signature = (extrema=None, mask=None))]
    fn histogram(
        slf: Py<Self>,
        extrema: Option<&Bound<'_, PyAny>>,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<Vec<u32>> {
        let mask = imaging_core_mask(mask)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let mode = inner.mode_identity().map_err(map_error)?;
        // ImagingCore parses an I/F range before the native histogram sees
        // the mask; the native masked histogram then ignores that range.
        let extrema = histogram_extrema_from_python(extrema, mode, false)?;
        py.detach(move || inner.histogram_with_mask_and_extrema(mask.as_ref(), extrema))
            .map_err(map_error)
    }

    /// ImagingCore orders optional arguments as extrema, then mask.
    #[pyo3(signature = (extrema=None, mask=None))]
    fn entropy(
        slf: Py<Self>,
        extrema: Option<&Bound<'_, PyAny>>,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<f64> {
        let mask = imaging_core_mask(mask)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let mode = inner.mode_identity().map_err(map_error)?;
        let extrema = histogram_extrema_from_python(extrema, mode, false)?;
        py.detach(move || inner.entropy_with_mask_and_extrema(mask.as_ref(), extrema))
            .map_err(map_error)
    }

    fn getbbox(
        &self,
        alpha_only: Option<bool>,
        py: Python<'_>,
    ) -> PyResult<Option<(u32, u32, u32, u32)>> {
        let alpha_only = alpha_only.unwrap_or(true);
        py.detach(|| self.inner.getbbox(alpha_only))
            .map_err(map_error)
    }

    /// Return extrema formatted as PIL expects.
    fn getextrema_formatted(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let formatted = py
            .detach(|| self.inner.getextrema_formatted())
            .map_err(map_error)?;
        Python::attach(|py| match formatted {
            pillow_rs::FormattedExtrema::Empty => Ok(py.None()),
            pillow_rs::FormattedExtrema::EmptyMultiple(bands) => {
                let values: Vec<Py<PyAny>> = (0..bands).map(|_| py.None()).collect();
                Ok(PyTuple::new(py, values)?.into_py_any(py)?)
            }
            pillow_rs::FormattedExtrema::Single((minimum, maximum)) => {
                Ok((minimum, maximum).into_py_any(py)?)
            }
            pillow_rs::FormattedExtrema::Multiple(values) => {
                let tuples: Vec<Py<PyAny>> = values
                    .into_iter()
                    .map(|(minimum, maximum)| (minimum, maximum).into_py_any(py))
                    .collect::<PyResult<_>>()?;
                Ok(PyTuple::new(py, tuples)?.into_py_any(py)?)
            }
            pillow_rs::FormattedExtrema::MultipleInteger(values) => {
                let tuples: Vec<Py<PyAny>> = values
                    .into_iter()
                    .map(|(minimum, maximum)| (minimum, maximum).into_py_any(py))
                    .collect::<PyResult<_>>()?;
                Ok(PyTuple::new(py, tuples)?.into_py_any(py)?)
            }
            pillow_rs::FormattedExtrema::MultipleFloat(values) => {
                let tuples: Vec<Py<PyAny>> = values
                    .into_iter()
                    .map(|(minimum, maximum)| (minimum, maximum).into_py_any(py))
                    .collect::<PyResult<_>>()?;
                Ok(PyTuple::new(py, tuples)?.into_py_any(py)?)
            }
            pillow_rs::FormattedExtrema::Integer((minimum, maximum)) => {
                Ok((minimum, maximum).into_py_any(py)?)
            }
            pillow_rs::FormattedExtrema::Float((minimum, maximum)) => {
                Ok((minimum, maximum).into_py_any(py)?)
            }
        })
    }
    /// Band names for the active image, delegated to the Rust core.
    fn getbands(&self) -> PyResult<Py<PyAny>> {
        let bands = self.inner.getbands().map_err(map_error)?;
        Python::attach(|py| {
            let objs: Vec<Py<PyAny>> = bands
                .iter()
                .map(|band| band.into_py_any(py))
                .collect::<PyResult<_>>()?;
            PyTuple::new(py, objs)?.into_py_any(py)
        })
    }

    #[pyo3(signature = (mask=None))]
    fn stat_formatted(
        &self,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let mask = imageops_mask_from_python(mask)?;
        let result = py
            .detach(|| self.inner.stat_formatted_with_mask(mask))
            .map_err(map_error)?;
        stat_result_to_python(&result)
    }

    #[pyo3(signature = (mask=None, extrema=None))]
    fn histogram_with_input(
        slf: Py<Self>,
        mask: Option<&Bound<'_, PyAny>>,
        extrema: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<Vec<u32>> {
        let mask = image_analysis_mask_from_python(mask)?;
        if matches!(&mask, pillow_rs::ImageAnalysisMask::Invalid(_)) {
            let inner = slf.bind(py).borrow().inner.clone();
            return py
                .detach(move || inner.histogram_with_input(mask))
                .map_err(map_error);
        }
        let masked = matches!(&mask, pillow_rs::ImageAnalysisMask::Image(_));
        let mode = {
            slf.bind(py)
                .borrow()
                .inner
                .mode_identity()
                .map_err(map_error)?
        };
        let extrema = histogram_extrema_from_python(extrema, mode, masked)?;
        let inner = slf.bind(py).borrow().inner.clone();
        py.detach(move || inner.histogram_with_input_and_extrema(mask, extrema))
            .map_err(map_error)
    }

    /// Return getcolors formatted as PIL expects.
    #[pyo3(signature = (maxcolors=None, histogram_mode=None))]
    fn getcolors_formatted(
        slf: Py<Self>,
        maxcolors: Option<&Bound<'_, PyAny>>,
        histogram_mode: Option<bool>,
        py: Python<'_>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let inner = slf.bind(py).borrow().inner.clone();
        let mode = inner.mode_identity().map_err(map_error)?;
        let rust_histogram_mode = matches!(
            mode,
            pillow_rs::Mode::One | pillow_rs::Mode::ImageLuma8 | pillow_rs::Mode::ImagePalette8
        );
        let histogram_mode = histogram_mode.unwrap_or(rust_histogram_mode);
        let formatted = if histogram_mode {
            py.detach(move || inner.getcolors_via_histogram_mode())
                .map_err(map_error)?
        } else {
            let maxcolors = maxcolors
                .map(|value| value.extract::<i32>())
                .transpose()?
                .unwrap_or(256);
            py.detach({
                let inner = inner.clone();
                move || inner.getcolors_via_low_level_mode_with_signed_limit(maxcolors)
            })
            .map_err(map_error)?
        };
        let Some(results) = formatted else {
            return Ok(None);
        };
        let values = PyList::new(
            py,
            results
                .into_iter()
                .map(|(count, color)| getcolors::PyGetColorsEntry { count, color }),
        )?;
        if histogram_mode {
            if let Some(maxcolors) = maxcolors {
                let too_many = PyInt::new(py, values.len())
                    .rich_compare(maxcolors, CompareOp::Gt)?
                    .is_truthy()?;
                if too_many {
                    return Ok(None);
                }
            }
        }
        Ok(Some(values.into_py_any(py)?))
    }

    fn getprojection(&mut self, py: Python<'_>) -> PyResult<(Vec<u32>, Vec<u32>)> {
        py.detach(|| self.inner.getprojection()).map_err(map_error)
    }

    #[pyo3(signature = (mask=None, extrema=None))]
    fn entropy_with_input(
        slf: Py<Self>,
        mask: Option<&Bound<'_, PyAny>>,
        extrema: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<f64> {
        let mask = image_analysis_mask_from_python(mask)?;
        if matches!(&mask, pillow_rs::ImageAnalysisMask::Invalid(_)) {
            let inner = slf.bind(py).borrow().inner.clone();
            return py
                .detach(move || inner.entropy_with_input(mask))
                .map_err(map_error);
        }
        let masked = matches!(&mask, pillow_rs::ImageAnalysisMask::Image(_));
        let mode = {
            slf.bind(py)
                .borrow()
                .inner
                .mode_identity()
                .map_err(map_error)?
        };
        let extrema = histogram_extrema_from_python(extrema, mode, masked)?;
        let inner = slf.bind(py).borrow().inner.clone();
        py.detach(move || inner.entropy_with_input_and_extrema(mask, extrema))
            .map_err(map_error)
    }

    fn compatibility_info(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        image_info_to_python(py, self.inner.compatibility_info())
    }

    fn converted_compatibility_info(
        &self,
        py: Python<'_>,
        target_mode: &str,
    ) -> PyResult<Py<PyAny>> {
        let info = pillow_rs::Mode::from_pillow_name(target_mode)
            .map(|mode| self.inner.converted_compatibility_info(mode))
            .unwrap_or_default();
        image_info_to_python(py, info)
    }
}

#[pyfunction]
fn stat_from_histogram(data: Vec<f64>) -> PyResult<Py<PyAny>> {
    let result = pillow_rs::stat_from_histogram(&data);
    stat_result_to_python(&result)
}

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(stat_from_histogram, m)?)?;
    Ok(())
}
