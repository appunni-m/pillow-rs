use crate::map_error;
use pillow_rs::{
    ImageFontTextBoundsCallbacks, ImageFontTextPositionArithmetic, ImageFontTextPositionCallbacks,
    ImageFontTextPositionComparison,
};
use pyo3::IntoPyObject;
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyAssertionError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{
    PyAnyMethods, PyBytes, PyDict, PyDictMethods, PyIterator, PyList, PyListMethods,
    PyModuleMethods, PyString,
};

enum BoundsCallbackError {
    Core(pillow_rs::PilError),
    Python(PyErr),
}

impl From<pillow_rs::PilError> for BoundsCallbackError {
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

struct PythonBoundsCallbacks<'py> {
    py: Python<'py>,
    text_object: Bound<'py, PyAny>,
    lines: Bound<'py, PyIterator>,
    pending_bbox: Option<Bound<'py, PyAny>>,
}

impl ImageFontTextBoundsCallbacks for PythonBoundsCallbacks<'_> {
    type Value = Py<PyAny>;
    type Error = BoundsCallbackError;

    fn next_line(&mut self) -> Result<Option<Self::Value>, Self::Error> {
        match self.lines.next() {
            Some(Ok(line)) => Ok(Some(line.unbind())),
            Some(Err(error)) => Err(BoundsCallbackError::Python(error)),
            None => Ok(None),
        }
    }

    fn begin_line_bounds(&mut self, line: &Self::Value) -> Result<(), Self::Error> {
        let line = line.bind(self.py);
        let text = line.getattr("text").map_err(BoundsCallbackError::Python)?;
        let anchor = line
            .getattr("anchor")
            .map_err(BoundsCallbackError::Python)?;
        let kwargs = PyDict::new(self.py);
        kwargs
            .set_item("anchor", anchor)
            .map_err(BoundsCallbackError::Python)?;
        let bbox = self
            .text_object
            .call_method("_get_bbox", (text,), Some(&kwargs))
            .map_err(BoundsCallbackError::Python)?;
        self.pending_bbox = Some(bbox);
        Ok(())
    }

    fn bbox_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        let Some(bbox) = &self.pending_bbox else {
            return Err(
                pillow_rs::PilError::InternalError("text bounds were not prepared".into()).into(),
            );
        };
        bbox.get_item(index)
            .map(|value| value.unbind())
            .map_err(BoundsCallbackError::Python)
    }

    fn line_offset(
        &mut self,
        line: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error> {
        line.bind(self.py)
            .getattr(if index % 2 == 0 { "x" } else { "y" })
            .map(|value| value.unbind())
            .map_err(BoundsCallbackError::Python)
    }

    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error> {
        left.bind(self.py)
            .add(&right.bind(self.py))
            .map(|value| value.unbind())
            .map_err(BoundsCallbackError::Python)
    }

    fn minimum(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let select_right = right
            .bind(self.py)
            .rich_compare(left.bind(self.py), CompareOp::Lt)
            .and_then(|value| value.is_truthy())
            .map_err(BoundsCallbackError::Python)?;
        Ok(if select_right {
            right.clone_ref(self.py)
        } else {
            left.clone_ref(self.py)
        })
    }

    fn maximum(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let select_right = left
            .bind(self.py)
            .rich_compare(right.bind(self.py), CompareOp::Lt)
            .and_then(|value| value.is_truthy())
            .map_err(BoundsCallbackError::Python)?;
        Ok(if select_right {
            right.clone_ref(self.py)
        } else {
            left.clone_ref(self.py)
        })
    }

    fn empty_bounds(&mut self) -> Result<[Self::Value; 4], Self::Error> {
        Err(BoundsCallbackError::Python(PyAssertionError::new_err("")))
    }
}

struct PythonPositionCallbacks<'py> {
    py: Python<'py>,
    text_object: Bound<'py, PyAny>,
    xy: Bound<'py, PyAny>,
    requested_anchor: Option<Bound<'py, PyAny>>,
    align: Bound<'py, PyAny>,
    line_source: Option<Bound<'py, PyAny>>,
    line_iterator: Option<Bound<'py, PyIterator>>,
    operator: Bound<'py, PyModule>,
}

impl<'py> PythonPositionCallbacks<'py> {
    fn new(
        py: Python<'py>,
        text_object: &Bound<'py, PyAny>,
        xy: &Bound<'py, PyAny>,
        anchor: Option<&Bound<'py, PyAny>>,
        align: &Bound<'py, PyAny>,
        lines: Option<&Bound<'py, PyAny>>,
    ) -> PyResult<Self> {
        Ok(PythonPositionCallbacks {
            py,
            text_object: text_object.clone(),
            xy: xy.clone(),
            requested_anchor: anchor.cloned(),
            align: align.clone(),
            line_source: lines.cloned(),
            line_iterator: None,
            operator: PyModule::import(py, "operator")?,
        })
    }

    fn direction_comparison(&self, operation: CompareOp) -> PyResult<bool> {
        let direction = self.text_object.getattr("direction")?;
        direction
            .rich_compare(PyString::new(self.py, "ttb"), operation)?
            .is_truthy()
    }

    fn line_font(&self) -> PyResult<Bound<'py, PyAny>> {
        self.text_object.getattr("font")
    }

    fn call_arithmetic(
        &self,
        method: &str,
        left: &Bound<'py, PyAny>,
        right: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyAny>> {
        self.operator.call_method1(method, (left, right))
    }

    fn ensure_line_source(&mut self) -> PyResult<Bound<'py, PyAny>> {
        if let Some(source) = &self.line_source {
            return Ok(source.clone());
        }
        let text = self.text_object.getattr("text")?;
        let split = text.getattr("split")?;
        let separator = if text.is_instance_of::<PyString>() {
            PyString::new(self.py, "\n").into_any()
        } else {
            PyBytes::new(self.py, b"\n").into_any()
        };
        let source = split.call1((separator,))?;
        self.line_source = Some(source.clone());
        Ok(source)
    }
}

impl<'py> ImageFontTextPositionCallbacks for PythonPositionCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Line = Bound<'py, PyAny>;
    type Error = BoundsCallbackError;

    fn anchor(&mut self) -> Result<Self::Value, Self::Error> {
        let anchor = match &self.requested_anchor {
            Some(anchor) => anchor.clone(),
            None => {
                let value = if self
                    .direction_comparison(CompareOp::Eq)
                    .map_err(BoundsCallbackError::Python)?
                {
                    "lt"
                } else {
                    "la"
                };
                PyString::new(self.py, value).into_any()
            }
        };
        if self.requested_anchor.is_some()
            && anchor.len().map_err(BoundsCallbackError::Python)? != 2
        {
            return Err(BoundsCallbackError::Python(PyValueError::new_err(
                "anchor must be a 2 character string",
            )));
        }
        Ok(anchor)
    }

    fn line_count(&mut self) -> Result<usize, Self::Error> {
        self.ensure_line_source()
            .and_then(|source| source.len())
            .map_err(BoundsCallbackError::Python)
    }

    fn single_line(&mut self) -> Result<Self::Value, Self::Error> {
        self.ensure_line_source()
            .and_then(|source| source.get_item(0))
            .map_err(BoundsCallbackError::Python)
    }

    fn begin_lines(&mut self) -> Result<(), Self::Error> {
        let source = self
            .ensure_line_source()
            .map_err(BoundsCallbackError::Python)?;
        self.line_iterator =
            Some(PyIterator::from_object(&source).map_err(BoundsCallbackError::Python)?);
        Ok(())
    }

    fn next_line(&mut self) -> Result<Option<Self::Value>, Self::Error> {
        let Some(iterator) = &mut self.line_iterator else {
            return Err(pillow_rs::PilError::InternalError(
                "text layout iterator was not initialized".into(),
            )
            .into());
        };
        match iterator.next() {
            Some(Ok(line)) => Ok(Some(line)),
            Some(Err(error)) => Err(BoundsCallbackError::Python(error)),
            None => Ok(None),
        }
    }

    fn anchor_component_contains(
        &mut self,
        anchor: &Self::Value,
        index: usize,
        values: &str,
    ) -> Result<bool, Self::Error> {
        let component = anchor
            .get_item(index)
            .map_err(BoundsCallbackError::Python)?;
        PyString::new(self.py, values)
            .call_method1("__contains__", (component,))
            .and_then(|result| result.is_truthy())
            .map_err(BoundsCallbackError::Python)
    }

    fn anchor_component_is(
        &mut self,
        anchor: &Self::Value,
        index: usize,
        value: &str,
    ) -> Result<bool, Self::Error> {
        anchor
            .get_item(index)
            .and_then(|component| {
                component.rich_compare(PyString::new(self.py, value), CompareOp::Eq)
            })
            .and_then(|result| result.is_truthy())
            .map_err(BoundsCallbackError::Python)
    }

    fn justified_anchor(&mut self, anchor: &Self::Value) -> Result<Self::Value, Self::Error> {
        let component = anchor.get_item(1).map_err(BoundsCallbackError::Python)?;
        self.operator
            .call_method1("add", (PyString::new(self.py, "l"), component))
            .map_err(BoundsCallbackError::Python)
    }

    fn direction_is_top_to_bottom(&mut self) -> Result<bool, Self::Error> {
        self.direction_comparison(CompareOp::Eq)
            .map_err(BoundsCallbackError::Python)
    }

    fn direction_is_not_top_to_bottom(&mut self) -> Result<bool, Self::Error> {
        self.direction_comparison(CompareOp::Ne)
            .map_err(BoundsCallbackError::Python)
    }

    fn font_mode(&mut self) -> Result<Self::Value, Self::Error> {
        self.text_object
            .call_method0("_get_fontmode")
            .map_err(BoundsCallbackError::Python)
    }

    fn line_spacing(&mut self, font_mode: &Self::Value) -> Result<Self::Value, Self::Error> {
        let font = self.line_font().map_err(BoundsCallbackError::Python)?;
        let getbbox = font
            .getattr("getbbox")
            .map_err(BoundsCallbackError::Python)?;
        let direction = self.py.None();
        let features = self
            .text_object
            .getattr("features")
            .map_err(BoundsCallbackError::Python)?;
        let language = self
            .text_object
            .getattr("language")
            .map_err(BoundsCallbackError::Python)?;
        let stroke_width = self
            .text_object
            .getattr("stroke_width")
            .map_err(BoundsCallbackError::Python)?;
        let bbox = getbbox
            .call1(("A", font_mode, direction, features, language, &stroke_width))
            .map_err(BoundsCallbackError::Python)?;
        let bottom = bbox.get_item(3).map_err(BoundsCallbackError::Python)?;
        let stroke_width = self
            .text_object
            .getattr("stroke_width")
            .map_err(BoundsCallbackError::Python)?;
        let with_stroke = self
            .call_arithmetic("add", &bottom, &stroke_width)
            .map_err(BoundsCallbackError::Python)?;
        let spacing = self
            .text_object
            .getattr("spacing")
            .map_err(BoundsCallbackError::Python)?;
        self.call_arithmetic("add", &with_stroke, &spacing)
            .map_err(BoundsCallbackError::Python)
    }

    fn line_width(
        &mut self,
        line: &Self::Value,
        font_mode: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let font = self.line_font().map_err(BoundsCallbackError::Python)?;
        let getlength = font
            .getattr("getlength")
            .map_err(BoundsCallbackError::Python)?;
        let direction = self
            .text_object
            .getattr("direction")
            .map_err(BoundsCallbackError::Python)?;
        let features = self
            .text_object
            .getattr("features")
            .map_err(BoundsCallbackError::Python)?;
        let language = self
            .text_object
            .getattr("language")
            .map_err(BoundsCallbackError::Python)?;
        getlength
            .call1((line, font_mode, direction, features, language))
            .map_err(BoundsCallbackError::Python)
    }

    fn split_words(&mut self, line: &Self::Value) -> Result<Vec<Self::Value>, Self::Error> {
        let split = line.getattr("split").map_err(BoundsCallbackError::Python)?;
        let separator = if line.is_instance_of::<PyString>() {
            PyString::new(self.py, " ").into_any()
        } else {
            PyBytes::new(self.py, b" ").into_any()
        };
        let words = split
            .call1((separator,))
            .map_err(BoundsCallbackError::Python)?;
        let iterator = PyIterator::from_object(&words).map_err(BoundsCallbackError::Python)?;
        iterator
            .map(|word| word.map_err(BoundsCallbackError::Python))
            .collect()
    }

    fn origin_x(&mut self) -> Result<Self::Value, Self::Error> {
        self.xy.get_item(0).map_err(BoundsCallbackError::Python)
    }

    fn origin_y(&mut self) -> Result<Self::Value, Self::Error> {
        self.xy.get_item(1).map_err(BoundsCallbackError::Python)
    }

    fn alignment_is(&mut self, value: &str) -> Result<bool, Self::Error> {
        self.align
            .rich_compare(PyString::new(self.py, value), CompareOp::Eq)
            .and_then(|result| result.is_truthy())
            .map_err(BoundsCallbackError::Python)
    }

    fn alignment_is_left_or_justify(&mut self) -> Result<bool, Self::Error> {
        let left = PyString::new(self.py, "left")
            .rich_compare(&self.align, CompareOp::Eq)
            .and_then(|result| result.is_truthy())
            .map_err(BoundsCallbackError::Python)?;
        if left {
            return Ok(true);
        }
        PyString::new(self.py, "justify")
            .rich_compare(&self.align, CompareOp::Eq)
            .and_then(|result| result.is_truthy())
            .map_err(BoundsCallbackError::Python)
    }

    fn arithmetic(
        &mut self,
        operation: ImageFontTextPositionArithmetic,
        left: &Self::Value,
        right: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let method = match operation {
            ImageFontTextPositionArithmetic::Add => "add",
            ImageFontTextPositionArithmetic::Subtract => "sub",
            ImageFontTextPositionArithmetic::Multiply => "mul",
            ImageFontTextPositionArithmetic::Divide => "truediv",
            ImageFontTextPositionArithmetic::AddAssign => "iadd",
            ImageFontTextPositionArithmetic::SubtractAssign => "isub",
        };
        self.call_arithmetic(method, left, right)
            .map_err(BoundsCallbackError::Python)
    }

    fn compare_numbers(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
        operation: ImageFontTextPositionComparison,
    ) -> Result<bool, Self::Error> {
        let comparison = match operation {
            ImageFontTextPositionComparison::GreaterThan => CompareOp::Gt,
            ImageFontTextPositionComparison::NotEqual => CompareOp::Ne,
        };
        left.rich_compare(right, comparison)
            .and_then(|result| result.is_truthy())
            .map_err(BoundsCallbackError::Python)
    }

    fn positioned_line(
        &mut self,
        x: &Self::Value,
        y: &Self::Value,
        anchor: &Self::Value,
        text: &Self::Value,
    ) -> Result<Self::Line, Self::Error> {
        PyModule::import(self.py, "pillow_rs.imagetext")
            .and_then(|module| module.getattr("_Line"))
            .and_then(|line_type| line_type.call1((x, y, anchor, text)))
            .map_err(BoundsCallbackError::Python)
    }

    fn integer(&mut self, value: i64) -> Result<Self::Value, Self::Error> {
        match value.into_pyobject(self.py) {
            Ok(value) => Ok(value.into_any()),
            Err(never) => match never {},
        }
    }

    fn float(&mut self, value: f64) -> Result<Self::Value, Self::Error> {
        match value.into_pyobject(self.py) {
            Ok(value) => Ok(value.into_any()),
            Err(never) => match never {},
        }
    }
}

#[pyfunction]
fn imagefont_text_layout_bbox_with_callbacks(
    py: Python<'_>,
    text_object: &Bound<'_, PyAny>,
    xy: &Bound<'_, PyAny>,
    anchor: Option<&Bound<'_, PyAny>>,
    align: &Bound<'_, PyAny>,
) -> PyResult<(Py<PyAny>, Py<PyAny>, Py<PyAny>, Py<PyAny>)> {
    let anchor = anchor
        .cloned()
        .unwrap_or_else(|| py.None().bind(py).clone());
    let lines = text_object.call_method1("_split", (xy, anchor, align))?;
    let lines = PyIterator::from_object(&lines)?;
    let mut callbacks = PythonBoundsCallbacks {
        py,
        text_object: text_object.clone(),
        lines,
        pending_bbox: None,
    };
    let bounds =
        pillow_rs::imagefont_text_layout_bbox_with_callbacks(&mut callbacks).map_err(|error| {
            match error {
                BoundsCallbackError::Core(error) => map_error(error),
                BoundsCallbackError::Python(error) => error,
            }
        })?;
    let [left, top, right, bottom] = bounds;
    Ok((left, top, right, bottom))
}

#[pyfunction]
#[pyo3(signature = (text_object, xy, anchor=None, align=None, lines=None))]
fn imagefont_text_split_with_callbacks<'py>(
    py: Python<'py>,
    text_object: &Bound<'py, PyAny>,
    xy: &Bound<'py, PyAny>,
    anchor: Option<&Bound<'py, PyAny>>,
    align: Option<&Bound<'py, PyAny>>,
    lines: Option<&Bound<'py, PyAny>>,
) -> PyResult<Py<PyList>> {
    let default_align = PyString::new(py, "left");
    let align = align.unwrap_or(default_align.as_any());
    let mut callbacks = PythonPositionCallbacks::new(py, text_object, xy, anchor, align, lines)?;
    let lines = pillow_rs::imagefont_text_split_with_callbacks(&mut callbacks).map_err(
        |error| match error {
            BoundsCallbackError::Core(error) => map_error(error),
            BoundsCallbackError::Python(error) => error,
        },
    )?;
    let result = PyList::empty(py);
    for line in lines {
        result.append(line)?;
    }
    Ok(result.unbind())
}

pub(super) fn register_functions(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(pyo3::wrap_pyfunction!(
        imagefont_text_layout_bbox_with_callbacks,
        module
    )?)?;
    module.add_function(pyo3::wrap_pyfunction!(
        imagefont_text_split_with_callbacks,
        module
    )?)?;
    Ok(())
}
