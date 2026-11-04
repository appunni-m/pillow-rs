use super::arrays::image_info_to_python;
use crate::*;
use pyo3::pymethods;
use pyo3::{pyfunction, wrap_pyfunction};

#[pymethods]
impl PyImage {
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
            let image = slf.bind(py).borrow();
            return py
                .detach(|| image.inner.histogram_with_input(mask))
                .map_err(map_error);
        }
        let masked = matches!(&mask, pillow_rs::ImageAnalysisMask::Image(_));
        let mode = {
            let image = slf.bind(py).borrow();
            image.inner.mode_identity().map_err(map_error)?
        };
        let extrema = histogram_extrema_from_python(extrema, mode, masked)?;
        let image = slf.bind(py).borrow();
        py.detach(|| image.inner.histogram_with_input_and_extrema(mask, extrema))
            .map_err(map_error)
    }

    /// Return getcolors formatted as PIL expects.
    fn getcolors_formatted(
        slf: Py<Self>,
        maxcolors: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let maxcolors = maxcolors
            .map(|value| value.extract::<i32>())
            .transpose()?
            .unwrap_or(256);
        let image = slf.bind(py).borrow();
        let formatted = py
            .detach(|| image.inner.getcolors_formatted_with_signed_limit(maxcolors))
            .map_err(map_error)?;
        Python::attach(|py| match formatted {
            None => Ok(None),
            Some(results) => Ok(Some(
                PyList::new(
                    py,
                    results
                        .into_iter()
                        .map(|(count, color)| getcolors::PyGetColorsEntry { count, color }),
                )?
                .into_py_any(py)?,
            )),
        })
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
            let image = slf.bind(py).borrow();
            return py
                .detach(|| image.inner.entropy_with_input(mask))
                .map_err(map_error);
        }
        let masked = matches!(&mask, pillow_rs::ImageAnalysisMask::Image(_));
        let mode = {
            let image = slf.bind(py).borrow();
            image.inner.mode_identity().map_err(map_error)?
        };
        let extrema = histogram_extrema_from_python(extrema, mode, masked)?;
        let image = slf.bind(py).borrow();
        py.detach(|| image.inner.entropy_with_input_and_extrema(mask, extrema))
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
