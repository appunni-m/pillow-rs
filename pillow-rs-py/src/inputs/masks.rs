use super::image_from_python;
use super::{Bound, Py, PyAny, PyResult, Python};
use pyo3::conversion::IntoPyObjectExt;
use pyo3::types::{PyAnyMethods, PyDictMethods, PyTypeMethods};

pub(crate) fn imageops_mask_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::ImageOpsMask> {
    let Some(value) = value else {
        return Ok(pillow_rs::ImageOpsMask::None);
    };
    if let Some(mask) = image_from_python(value) {
        return Ok(pillow_rs::ImageOpsMask::Image(Box::new(mask)));
    }
    Ok(pillow_rs::ImageOpsMask::Invalid(
        value.get_type().name()?.to_string(),
    ))
}

pub(crate) fn image_analysis_mask_from_python(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::ImageAnalysisMask> {
    let Some(value) = value else {
        return Ok(pillow_rs::ImageAnalysisMask::None);
    };
    if !value.is_truthy()? {
        return Ok(pillow_rs::ImageAnalysisMask::None);
    };
    if let Some(mask) = image_from_python(value) {
        return Ok(pillow_rs::ImageAnalysisMask::Image(Box::new(mask)));
    }
    Ok(pillow_rs::ImageAnalysisMask::Invalid(
        value.get_type().name()?.to_string(),
    ))
}

pub(crate) fn stat_result_to_python(result: &pillow_rs::StatResult) -> PyResult<Py<PyAny>> {
    use pillow_rs::StatValue;

    Python::attach(|py| {
        let dict = pyo3::types::PyDict::new(py);
        macro_rules! set {
            ($key:expr, $field:ident) => {
                let value = match &result.$field {
                    StatValue::Int(value) => value.into_py_any(py)?,
                    StatValue::Float(value) => value.into_py_any(py)?,
                    StatValue::IntList(value) => value.into_py_any(py)?,
                    StatValue::FloatList(value) => value.into_py_any(py)?,
                    StatValue::ExtremaSingle(value) => value.into_py_any(py)?,
                    StatValue::ExtremaList(value) => value.into_py_any(py)?,
                };
                dict.set_item($key, value)?;
            };
        }
        set!("count", count);
        set!("sum", sum);
        set!("sum2", sum2);
        set!("mean", mean);
        set!("median", median);
        set!("rms", rms);
        set!("var", var);
        set!("stddev", stddev);
        set!("extrema", extrema);
        dict.into_py_any(py)
    })
}
