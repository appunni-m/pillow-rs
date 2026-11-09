use super::PyDraw;
use crate::map_error;
use pyo3::basic::CompareOp;
use pyo3::conversion::IntoPyObjectExt;
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyString, PyTuple, PyTupleMethods};
use pyo3::{pyclass, pymethods};

struct PythonRoundedRectanglePreparation<'py> {
    py: Python<'py>,
    corners: Bound<'py, PyAny>,
}

impl<'py> pillow_rs::RoundedRectanglePreparationCallbacks
    for PythonRoundedRectanglePreparation<'py>
{
    type Value = Py<PyAny>;
    type Error = PyErr;

    fn integer(&mut self, value: i32) -> PyResult<Self::Value> {
        value.into_py_any(self.py)
    }

    fn multiply(&mut self, left: &Self::Value, right: &Self::Value) -> PyResult<Self::Value> {
        Ok(left.bind(self.py).mul(right.bind(self.py))?.unbind())
    }

    fn round(&mut self, value: &Self::Value) -> PyResult<Self::Value> {
        let builtins = PyModule::import(self.py, "builtins")?;
        Ok(builtins
            .getattr("round")?
            .call1((value.bind(self.py),))?
            .unbind())
    }

    fn less_than(&mut self, left: &Self::Value, right: &Self::Value) -> PyResult<bool> {
        self.compare(left, right, CompareOp::Lt)
    }

    fn subtract(&mut self, left: &Self::Value, right: &Self::Value) -> PyResult<Self::Value> {
        Ok(left.bind(self.py).sub(right.bind(self.py))?.unbind())
    }

    fn greater_equal(&mut self, left: &Self::Value, right: &Self::Value) -> PyResult<bool> {
        self.compare(left, right, CompareOp::Ge)
    }

    fn equals_zero(&mut self, value: &Self::Value) -> PyResult<bool> {
        let zero = self.integer(0)?;
        self.compare(value, &zero, CompareOp::Eq)
    }

    fn all_corners(&mut self) -> PyResult<bool> {
        self.corner_aggregate("all")
    }

    fn any_corners(&mut self) -> PyResult<bool> {
        self.corner_aggregate("any")
    }

    fn floor_half(&mut self, value: &Self::Value) -> PyResult<Self::Value> {
        let two = self.integer(2)?;
        Ok(value.bind(self.py).floor_div(two.bind(self.py))?.unbind())
    }

    fn to_integer(&mut self, value: &Self::Value) -> PyResult<Self::Value> {
        let builtins = PyModule::import(self.py, "builtins")?;
        Ok(builtins
            .getattr("int")?
            .call1((value.bind(self.py),))?
            .unbind())
    }
}

impl pillow_rs::RoundedRectangleOutlineCallbacks for PythonRoundedRectanglePreparation<'_> {
    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> PyResult<Self::Value> {
        Ok(left.bind(self.py).add(right.bind(self.py))?.unbind())
    }

    fn in_place_add(&mut self, left: &Self::Value, right: &Self::Value) -> PyResult<Self::Value> {
        self.in_place_numeric(left, right, "iadd")
    }

    fn in_place_subtract(
        &mut self,
        left: &Self::Value,
        right: &Self::Value,
    ) -> PyResult<Self::Value> {
        self.in_place_numeric(left, right, "isub")
    }

    fn corner_truthy(&mut self, index: usize) -> PyResult<bool> {
        self.corners.get_item(index)?.is_truthy()
    }
}

impl<'py> PythonRoundedRectanglePreparation<'py> {
    fn in_place_numeric(
        &self,
        left: &Py<PyAny>,
        right: &Py<PyAny>,
        operation: &str,
    ) -> PyResult<Py<PyAny>> {
        let operator = PyModule::import(self.py, "operator")?;
        Ok(operator
            .call_method1(operation, (left.bind(self.py), right.bind(self.py)))?
            .unbind())
    }

    fn compare(&self, left: &Py<PyAny>, right: &Py<PyAny>, operation: CompareOp) -> PyResult<bool> {
        left.bind(self.py)
            .rich_compare(right.bind(self.py), operation)?
            .is_truthy()
    }

    fn corner_aggregate(&self, operation: &str) -> PyResult<bool> {
        let builtins = PyModule::import(self.py, "builtins")?;
        builtins
            .getattr(operation)?
            .call1((self.corners.clone(),))?
            .extract()
    }
}

#[pyclass(name = "_RoundedRectangleProgram")]
pub struct PyRoundedRectangleProgram {
    inner: pillow_rs::RoundedRectangleProgram<Py<PyAny>>,
}

#[pymethods]
impl PyRoundedRectangleProgram {
    #[pyo3(signature = (corner=None, width=None, outline_width=None))]
    fn next(
        &mut self,
        py: Python<'_>,
        corner: Option<bool>,
        width: Option<&Bound<'_, PyAny>>,
        outline_width: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Py<PyAny>> {
        let response = match (corner, width, outline_width) {
            (Some(value), None, None) => {
                Some(pillow_rs::RoundedRectangleProgramResponse::Corner(value))
            }
            (None, Some(_), None) => Some(pillow_rs::RoundedRectangleProgramResponse::Width),
            (None, None, Some(value)) => {
                Some(pillow_rs::RoundedRectangleProgramResponse::OutlineEnabled(
                    value.rich_compare(0, CompareOp::Ne)?.is_truthy()?,
                ))
            }
            (None, None, None) => None,
            _ => {
                return Err(PyTypeError::new_err(
                    "only one rounded-rectangle response may be supplied",
                ));
            }
        };
        let mut callbacks = PythonRoundedRectanglePreparation {
            py,
            corners: py.None().into_bound(py),
        };
        let step = self
            .inner
            .next_step(response, &mut callbacks)
            .map_err(|error| match error {
                pillow_rs::RoundedRectangleProgramError::Core(error) => map_error(error),
                pillow_rs::RoundedRectangleProgramError::Callback(error) => error,
            })?;
        rounded_rectangle_step_to_python(py, step, width)
    }
}

#[pymethods]
impl PyDraw {
    #[pyo3(name = "_prepare_rounded_rectangle")]
    fn prepare_rounded_rectangle<'py>(
        &mut self,
        py: Python<'py>,
        coordinates: &Bound<'py, PyTuple>,
        radius: &Bound<'py, PyAny>,
        corners: &Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let coordinates = [
            coordinates.get_item(0)?.unbind(),
            coordinates.get_item(1)?.unbind(),
            coordinates.get_item(2)?.unbind(),
            coordinates.get_item(3)?.unbind(),
        ];
        let radius = radius.clone().unbind();
        let mut callbacks = PythonRoundedRectanglePreparation {
            py,
            corners: corners.clone(),
        };
        let prepared = pillow_rs::prepare_rounded_rectangle_with_callbacks(
            coordinates,
            radius,
            &mut callbacks,
        )
        .map_err(|error| match error {
            pillow_rs::RoundedRectanglePreparationError::Core(error) => map_error(error),
            pillow_rs::RoundedRectanglePreparationError::Callback(error) => error,
        })?;
        rounded_rectangle_prepared_to_python(py, prepared)
    }

    #[pyo3(name = "_rounded_rectangle_program")]
    fn rounded_rectangle_program(
        &mut self,
        xy: &Bound<'_, PyAny>,
        diameter: &Bound<'_, PyAny>,
        radius: &Bound<'_, PyAny>,
        fill: Option<&Bound<'_, PyAny>>,
        outline: Option<&Bound<'_, PyAny>>,
        full_x: bool,
        full_y: bool,
    ) -> PyResult<PyRoundedRectangleProgram> {
        let outline_color = outline.map(|_| self.color(outline)).transpose()?;
        let outline_ink = outline.map(|value| value.extract::<i32>()).transpose()?;
        let fill_color = fill.map(|_| self.color(fill)).transpose()?;
        let fill_ink = fill.map(|value| value.extract::<i32>()).transpose()?;
        let coordinates = xy.cast::<PyTuple>()?;
        let coordinates = [
            coordinates.get_item(0)?.unbind(),
            coordinates.get_item(1)?.unbind(),
            coordinates.get_item(2)?.unbind(),
            coordinates.get_item(3)?.unbind(),
        ];
        Ok(PyRoundedRectangleProgram {
            inner: pillow_rs::RoundedRectangleProgram::new(
                coordinates,
                diameter.clone().unbind(),
                radius.clone().unbind(),
                full_x,
                full_y,
                fill_color,
                outline_color,
                fill_ink,
                outline_ink,
            ),
        })
    }

    #[pyo3(name = "_rounded_rectangle_step")]
    fn rounded_rectangle_step(&mut self, step: &Bound<'_, PyAny>) -> PyResult<()> {
        let step = step.cast::<PyTuple>()?;
        let kind = step.get_item(1)?.extract::<String>()?;
        // Pillow parses each primitive's scalar arguments before
        // PyPath_Flatten converts its bounds; preserve that fault order here.
        match kind.as_str() {
            "rectangle" => {
                let color = step.get_item(3)?.extract::<(u8, u8, u8, u8)>()?;
                let bounds = super::input::draw_box_input_from_python(&step.get_item(2)?)?;
                let (x0, y0, x1, y1) = pillow_rs::normalize_draw_box(bounds).map_err(map_error)?;
                self.draw
                    .rectangle(x0, y0, x1, y1, Some(color), None, 1)
                    .map_err(map_error)
            }
            "pieslice" => {
                let start = step.get_item(3)?.extract::<f64>()?;
                let end = step.get_item(4)?.extract::<f64>()?;
                let color = step.get_item(5)?.extract::<(u8, u8, u8, u8)>()?;
                let bounds = super::input::draw_box_input_from_python(&step.get_item(2)?)?;
                let (x0, y0, x1, y1) = pillow_rs::normalize_draw_box(bounds).map_err(map_error)?;
                self.draw
                    .pieslice(x0, y0, x1, y1, start, end, Some(color), None, 1)
                    .map_err(map_error)
            }
            "arc" => {
                let start = step.get_item(3)?.extract::<f64>()?;
                let end = step.get_item(4)?.extract::<f64>()?;
                let color = step.get_item(5)?.extract::<(u8, u8, u8, u8)>()?;
                let width = crate::inputs::c_int_from_python(&step.get_item(6)?)?;
                let bounds = super::input::draw_box_input_from_python(&step.get_item(2)?)?;
                let (x0, y0, x1, y1) = pillow_rs::normalize_draw_box(bounds).map_err(map_error)?;
                self.draw
                    .arc(x0, y0, x1, y1, start, end, color, width.max(0) as u32)
                    .map_err(map_error)
            }
            _ => Err(PyTypeError::new_err("invalid rounded-rectangle draw step")),
        }
    }

    #[pyo3(name = "_rounded_rectangle_border_bounds")]
    fn rounded_rectangle_border_bounds<'py>(
        &mut self,
        py: Python<'py>,
        kind: &str,
        bounds: &Bound<'py, PyTuple>,
        radius: &Bound<'py, PyAny>,
        width: &Bound<'py, PyAny>,
        corners: &Bound<'py, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let kind = match kind {
            "top" => pillow_rs::RoundedRectangleOutlineBorder::Top,
            "bottom" => pillow_rs::RoundedRectangleOutlineBorder::Bottom,
            "left" => pillow_rs::RoundedRectangleOutlineBorder::Left,
            "right" => pillow_rs::RoundedRectangleOutlineBorder::Right,
            _ => return Err(PyTypeError::new_err("invalid rounded-rectangle border")),
        };
        let bounds = [
            bounds.get_item(0)?.unbind(),
            bounds.get_item(1)?.unbind(),
            bounds.get_item(2)?.unbind(),
            bounds.get_item(3)?.unbind(),
        ];
        let radius = radius.clone().unbind();
        let width = width.clone().unbind();
        let mut callbacks = PythonRoundedRectanglePreparation {
            py,
            corners: corners.clone(),
        };
        let bounds = pillow_rs::rounded_rectangle_outline_bounds_with_callbacks(
            kind,
            bounds,
            &radius,
            &width,
            &mut callbacks,
        )?;
        Ok(
            PyTuple::new(py, bounds.into_iter().map(|value| value.into_bound(py)))?
                .into_any()
                .unbind(),
        )
    }
}

pub(super) fn register_class(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRoundedRectangleProgram>()
}

fn rounded_rectangle_step_to_python(
    py: Python<'_>,
    step: pillow_rs::RoundedRectangleProgramStep<Py<PyAny>>,
    width: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    use pillow_rs::{RoundedRectangleDrawStep as DrawStep, RoundedRectangleProgramStep as Step};

    match step {
        Step::ReadCorner(index) => tuple(
            py,
            [string(py, "corner")?, index.into_py_any(py)?.into_bound(py)],
        ),
        Step::ReadOutlineEnabled => tuple(py, [string(py, "outline_width")?]),
        Step::ReadWidth => tuple(py, [string(py, "width")?]),
        Step::OutlineBorder {
            kind,
            bounds,
            radius,
            color,
        } => {
            let kind = match kind {
                pillow_rs::RoundedRectangleOutlineBorder::Top => "top",
                pillow_rs::RoundedRectangleOutlineBorder::Bottom => "bottom",
                pillow_rs::RoundedRectangleOutlineBorder::Left => "left",
                pillow_rs::RoundedRectangleOutlineBorder::Right => "right",
            };
            tuple(
                py,
                [
                    string(py, "border")?,
                    string(py, kind)?,
                    PyTuple::new(py, bounds.into_iter().map(|value| value.into_bound(py)))?
                        .into_any(),
                    radius.into_bound(py),
                    PyTuple::new(py, [color.0, color.1, color.2, color.3])?.into_any(),
                ],
            )
        }
        Step::Done => tuple(py, [string(py, "done")?]),
        Step::Draw(DrawStep::Rectangle { bounds, color }) => tuple(
            py,
            [
                string(py, "draw")?,
                string(py, "rectangle")?,
                PyTuple::new(py, bounds.into_iter().map(|value| value.into_bound(py)))?.into_any(),
                PyTuple::new(py, [color.0, color.1, color.2, color.3])?.into_any(),
            ],
        ),
        Step::Draw(DrawStep::PieSlice {
            bounds,
            start,
            end,
            color,
        }) => tuple(
            py,
            [
                string(py, "draw")?,
                string(py, "pieslice")?,
                PyTuple::new(py, bounds.into_iter().map(|value| value.into_bound(py)))?.into_any(),
                start.into_py_any(py)?.into_bound(py),
                end.into_py_any(py)?.into_bound(py),
                PyTuple::new(py, [color.0, color.1, color.2, color.3])?.into_any(),
            ],
        ),
        Step::Draw(DrawStep::Arc {
            bounds,
            start,
            end,
            color,
        }) => tuple(
            py,
            [
                string(py, "draw")?,
                string(py, "arc")?,
                PyTuple::new(py, bounds.into_iter().map(|value| value.into_bound(py)))?.into_any(),
                start.into_py_any(py)?.into_bound(py),
                end.into_py_any(py)?.into_bound(py),
                PyTuple::new(py, [color.0, color.1, color.2, color.3])?.into_any(),
                // Preserve Pillow's native argument conversion order: bounds
                // and arc arguments reach the primitive before width is parsed.
                width
                    .ok_or_else(|| PyTypeError::new_err("rounded-rectangle arc requires width"))?
                    .clone(),
            ],
        ),
    }
}

fn rounded_rectangle_prepared_to_python<'py>(
    py: Python<'py>,
    prepared: pillow_rs::RoundedRectanglePreparedGeometry<Py<PyAny>>,
) -> PyResult<Py<PyAny>> {
    use pillow_rs::RoundedRectangleEarlyPath as EarlyPath;

    let route = match prepared.early_path {
        Some(EarlyPath::Ellipse) => "ellipse",
        Some(EarlyPath::Rectangle) => "rectangle",
        None => "draw",
    };
    let coordinates = PyTuple::new(
        py,
        prepared
            .coordinates
            .iter()
            .map(|value| value.bind(py).clone()),
    )?
    .into_any();
    let diameter = prepared.diameter.into_bound(py);
    let radius = prepared.radius.unwrap_or_else(|| py.None()).into_bound(py);
    tuple(
        py,
        [
            string(py, route)?,
            coordinates,
            diameter,
            radius,
            prepared.full_x.into_py_any(py)?.into_bound(py),
            prepared.full_y.into_py_any(py)?.into_bound(py),
        ],
    )
}

fn string<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyAny>> {
    Ok(PyString::new(py, value).into_any())
}

fn tuple<'py, I>(py: Python<'py>, values: I) -> PyResult<Py<PyAny>>
where
    I: IntoIterator<Item = Bound<'py, PyAny>>,
    I::IntoIter: ExactSizeIterator,
{
    Ok(PyTuple::new(py, values)?.into_any().unbind())
}
