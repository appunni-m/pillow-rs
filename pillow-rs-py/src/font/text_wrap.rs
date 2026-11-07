use super::{imagefont_text_input_from_python, imagefont_text_input_to_python};
use crate::map_error;
use pillow_rs::{
    ImageFontTextInput, ImageFontTextWrapCallbacks, ImageFontTextWrapWidthCheck, PilError,
};
use pyo3::basic::CompareOp;
use pyo3::conversion::IntoPyObjectExt;
use pyo3::exceptions::{PyTypeError, PyUnboundLocalError};
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{
    PyAnyMethods, PyBytes, PyDict, PyDictMethods, PyList, PyListMethods, PyModuleMethods,
    PyString,
};
use std::collections::HashMap;

enum WrapCallbackError {
    Core(PilError),
    Python(PyErr),
}

impl From<PilError> for WrapCallbackError {
    fn from(error: PilError) -> Self {
        Self::Core(error)
    }
}

struct PythonWrapCallbacks<'py> {
    py: Python<'py>,
    text_object: Bound<'py, PyAny>,
    input_text: Bound<'py, PyAny>,
    initial_font: Option<Bound<'py, PyAny>>,
    active_font: Option<Bound<'py, PyAny>>,
    width: Bound<'py, PyAny>,
    height: Bound<'py, PyAny>,
    scaling_name: Bound<'py, PyAny>,
    limit: Bound<'py, PyAny>,
    remaining_texts: HashMap<u64, Py<PyAny>>,
    changed_font: bool,
}

impl PythonWrapCallbacks<'_> {
    fn line_bbox(&self, text: &ImageFontTextInput) -> PyResult<Bound<'_, PyAny>> {
        let text = imagefont_text_input_to_python(self.py, text.clone())?;
        let font = self
            .active_font
            .as_ref()
            .map_or_else(|| self.py.None(), |font| font.clone().unbind());
        self.text_object
            .call_method1("_get_bbox", (text, font))
    }

    fn current_wrapper_font(&self) -> PyResult<Bound<'_, PyAny>> {
        self.text_object.getattr("font")
    }

    fn remaining_text(&self, consumed: usize) -> PyResult<Bound<'_, PyAny>> {
        let builtins = PyModule::import(self.py, "builtins")?;
        let slice = builtins
            .getattr("slice")?
            .call1((consumed, self.py.None(), self.py.None()))?;
        self.input_text.get_item(slice)
    }

    fn take_remaining_text(
        &mut self,
        wrap_id: u64,
        consumed: usize,
    ) -> PyResult<Bound<'_, PyAny>> {
        match self.remaining_texts.remove(&wrap_id) {
            Some(value) => Ok(value.bind(self.py).clone()),
            None => self.remaining_text(consumed),
        }
    }

    fn invoke_font_variant(&mut self, size: i64) -> PyResult<()> {
        let font = self.current_wrapper_font()?.unbind();
        let kwargs = PyDict::new(self.py);
        kwargs.set_item("size", size)?;
        self.active_font = Some(font.bind(self.py).call_method("font_variant", (), Some(&kwargs))?);
        self.changed_font = true;
        Ok(())
    }

    fn to_python_lines(&self, lines: &[ImageFontTextInput]) -> PyResult<Bound<'_, PyList>> {
        let result = PyList::empty(self.py);
        for line in lines {
            result.append(imagefont_text_input_to_python(self.py, line.clone())?)?;
        }
        Ok(result)
    }

    fn compare_limit(&self, size: i64, shrink: bool) -> PyResult<bool> {
        let size = size.into_py_any(self.py)?;
        if shrink {
            let builtins = PyModule::import(self.py, "builtins")?;
            let minimum = builtins.call_method1("max", (self.limit.clone(), 1))?;
            Ok(size.bind(self.py).rich_compare(&minimum, CompareOp::Eq)?.is_truthy()?)
        } else {
            Ok(size.bind(self.py).rich_compare(&self.limit, CompareOp::Eq)?.is_truthy()?)
        }
    }

    fn selected_scale_name(&self) -> PyResult<bool> {
        let shrink = PyString::new(self.py, "shrink");
        Ok(self
            .scaling_name
            .rich_compare(&shrink, CompareOp::Eq)?
            .is_truthy()?)
    }

    fn take_changed_font(&mut self) -> Option<Py<PyAny>> {
        if self.changed_font {
            self.active_font.take().map(|font| font.unbind())
        } else {
            None
        }
    }
}

impl ImageFontTextWrapCallbacks for PythonWrapCallbacks<'_> {
    type Error = WrapCallbackError;

    fn width_check(
        &mut self,
        text: &ImageFontTextInput,
        check: ImageFontTextWrapWidthCheck,
    ) -> Result<bool, Self::Error> {
        let bbox = self.line_bbox(text).map_err(WrapCallbackError::Python)?;
        let right = bbox.get_item(2).map_err(WrapCallbackError::Python)?;
        let comparison = match check {
            ImageFontTextWrapWidthCheck::Fits => CompareOp::Le,
            ImageFontTextWrapWidthCheck::Exceeds => CompareOp::Gt,
        };
        right
            .rich_compare(&self.width, comparison)
            .and_then(|value| value.is_truthy())
            .map_err(WrapCallbackError::Python)
    }

    fn height_fits(
        &mut self,
        lines: &[ImageFontTextInput],
        last_line: &ImageFontTextInput,
    ) -> Result<bool, Self::Error> {
        let lines = self.to_python_lines(lines).map_err(WrapCallbackError::Python)?;
        let kwargs = PyDict::new(self.py);
        kwargs
            .set_item("lines", lines)
            .map_err(WrapCallbackError::Python)?;
        let positioned = self
            .text_object
            .call_method("_split", (), Some(&kwargs))
            .map_err(WrapCallbackError::Python)?;
        let last = positioned
            .get_item(-1)
            .and_then(|line| line.get_item(1))
            .map_err(WrapCallbackError::Python)?;
        let bbox = self
            .line_bbox(last_line)
            .map_err(WrapCallbackError::Python)?;
        let bottom = bbox.get_item(3).map_err(WrapCallbackError::Python)?;
        let operator = PyModule::import(self.py, "operator")
            .map_err(WrapCallbackError::Python)?;
        let total = operator
            .call_method1("add", (last, bottom))
            .map_err(WrapCallbackError::Python)?;
        let exceeds = total
            .rich_compare(&self.height, CompareOp::Gt)
            .and_then(|value| value.is_truthy())
            .map_err(WrapCallbackError::Python)?;
        Ok(!exceeds)
    }

    fn capture_remaining_text(
        &mut self,
        _remaining: &ImageFontTextInput,
        consumed: usize,
        wrap_id: u64,
    ) -> Result<(), Self::Error> {
        let remaining = self
            .remaining_text(consumed)
            .map_err(WrapCallbackError::Python)?;
        self.remaining_texts.insert(wrap_id, remaining.unbind());
        Ok(())
    }

    fn has_remaining_text(
        &mut self,
        _remaining: &ImageFontTextInput,
        consumed: usize,
        wrap_id: u64,
    ) -> Result<bool, Self::Error> {
        let remaining = match self.remaining_texts.get(&wrap_id) {
            Some(remaining) => remaining.bind(self.py).clone(),
            None => self
                .remaining_text(consumed)
                .map_err(WrapCallbackError::Python)?,
        };
        let has_remaining = remaining
            .is_truthy()
            .map_err(WrapCallbackError::Python)?;
        self.remaining_texts.insert(wrap_id, remaining.unbind());
        Ok(has_remaining)
    }

    fn discard_remaining_text(&mut self, wrap_id: u64) {
        self.remaining_texts.remove(&wrap_id);
    }

    fn scaling_is_shrink(&mut self) -> Result<bool, Self::Error> {
        self.selected_scale_name().map_err(WrapCallbackError::Python)
    }

    fn initial_size(&mut self, round_up: bool) -> Result<i64, Self::Error> {
        let math = PyModule::import(self.py, "math").map_err(WrapCallbackError::Python)?;
        let function = if round_up { "ceil" } else { "floor" };
        let font = self.initial_font.as_ref().ok_or_else(|| {
            WrapCallbackError::Python(PyTypeError::new_err("missing text scaling font"))
        })?;
        math.call_method1(function, (font.getattr("size").map_err(WrapCallbackError::Python)?,))
            .and_then(|size| size.extract::<i64>())
            .map_err(WrapCallbackError::Python)
    }

    fn shrink_limit_reached(&mut self, size: i64) -> Result<bool, Self::Error> {
        self.compare_limit(size, true).map_err(WrapCallbackError::Python)
    }

    fn grow_limit_reached(&mut self, size: i64) -> Result<bool, Self::Error> {
        self.compare_limit(size, false).map_err(WrapCallbackError::Python)
    }

    fn font_variant(&mut self, size: i64) -> Result<(), Self::Error> {
        self.invoke_font_variant(size)
            .map_err(WrapCallbackError::Python)
    }

    fn restore_font(&mut self, size: i64) -> Result<(), Self::Error> {
        let selected_size = size;
        let font = self
            .current_wrapper_font()
            .map_err(WrapCallbackError::Python)?
            .unbind();
        let font_size = font
            .bind(self.py)
            .getattr("size")
            .map_err(WrapCallbackError::Python)?
            .unbind();
        let size = size.into_py_any(self.py).map_err(WrapCallbackError::Python)?;
        let changed = size
            .bind(self.py)
            .rich_compare(font_size.bind(self.py), CompareOp::Ne)
            .and_then(|value| value.is_truthy())
            .map_err(WrapCallbackError::Python)?;
        if changed {
            self.invoke_font_variant(selected_size)
                .map_err(WrapCallbackError::Python)?;
        } else {
            self.active_font = None;
            self.changed_font = false;
        }
        Ok(())
    }

    fn missing_grow_wrap(&mut self) -> Result<(), Self::Error> {
        if self.changed_font {
            if let Some(font) = &self.active_font {
                self.text_object
                    .setattr("font", font)
                    .map_err(WrapCallbackError::Python)?;
            }
        }
        Err(WrapCallbackError::Python(PyUnboundLocalError::new_err(
            "cannot access local variable 'last_wrap' where it is not associated with a value",
        )))
    }
}

#[allow(clippy::too_many_arguments)]
#[pyfunction]
#[pyo3(signature = (text_object, text, width, height, scaling_name=None, limit=None, has_scaling=false, initial_font=None))]
fn imagefont_text_wrap_with_callbacks(
    py: Python<'_>,
    text_object: &Bound<'_, PyAny>,
    text: &Bound<'_, PyAny>,
    width: &Bound<'_, PyAny>,
    height: &Bound<'_, PyAny>,
    scaling_name: Option<&Bound<'_, PyAny>>,
    limit: Option<&Bound<'_, PyAny>>,
    has_scaling: bool,
    initial_font: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<(Vec<Py<PyAny>>, Py<PyAny>, Option<Py<PyAny>>, usize)>> {
    let initial_font = if has_scaling {
        Some(initial_font.cloned().ok_or_else(|| {
            PyTypeError::new_err("missing text scaling font")
        })?)
    } else {
        None
    };
    let input = wrap_text_input_from_python(py, text)?;
    let scaling_name = scaling_name
        .cloned()
        .unwrap_or_else(|| py.None().bind(py).clone());
    let limit = limit.cloned().unwrap_or_else(|| py.None().bind(py).clone());
    let mut callbacks = PythonWrapCallbacks {
        py,
        text_object: text_object.clone(),
        input_text: text.clone(),
        initial_font: initial_font.clone(),
        active_font: initial_font.clone(),
        width: width.clone(),
        height: height.clone(),
        scaling_name,
        limit,
        remaining_texts: HashMap::new(),
        changed_font: false,
    };
    let result = pillow_rs::imagefont_text_wrap_with_callbacks(
        &mut callbacks,
        input,
        !height.is_none(),
        has_scaling,
    )
    .map_err(|error| match error {
        WrapCallbackError::Core(error) => map_error(error),
        WrapCallbackError::Python(error) => error,
    })?;
    let Some(result) = result else {
        return Ok(None);
    };
    let consumed = result.consumed;
    let lines = result
        .lines
        .into_iter()
        .map(|line| imagefont_text_input_to_python(py, line))
        .collect::<PyResult<Vec<_>>>()?;
    let remaining = callbacks
        .take_remaining_text(result.wrap_id, consumed)?
        .unbind();
    Ok(Some((
        lines,
        remaining,
        callbacks.take_changed_font(),
        consumed,
    )))
}

fn wrap_text_input_from_python(
    py: Python<'_>,
    text: &Bound<'_, PyAny>,
) -> PyResult<ImageFontTextInput> {
    let input = imagefont_text_input_from_python(text)?;
    if !matches!(&input, ImageFontTextInput::InvalidType) {
        return Ok(input);
    }
    if let Ok(builtins) = PyModule::import(py, "builtins") {
        if let Ok(view) = builtins.call_method1("memoryview", (text,)) {
            if !view
                .getattr("c_contiguous")
                .and_then(|value| value.extract::<bool>())
                .unwrap_or(false)
            {
                // PyBUF_SIMPLE rejects non-contiguous exporters; let the
                // established regex path produce the same public failure.
            } else {
            if let Ok(value) = view.call_method0("tobytes") {
                if let Ok(bytes) = value.extract::<Vec<u8>>() {
                    return Ok(ImageFontTextInput::Bytes(bytes));
                }
            }
            }
        }
    }

    let regex = PyModule::import(py, "re")?;
    let pattern = PyBytes::new(py, br"\s*\S+");
    regex.call_method1("findall", (pattern, text))?;
    Err(PyTypeError::new_err("expected string or bytes"))
}

pub(super) fn register_functions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(pyo3::wrap_pyfunction!(imagefont_text_wrap_with_callbacks, module)?)?;
    Ok(())
}
