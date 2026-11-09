use std::cell::{Cell, RefCell};

use pyo3::exceptions::{
    PyIndexError, PyMemoryError, PyOverflowError, PySystemError, PyTypeError, PyValueError,
};
use pyo3::prelude::{Bound, Py, PyAny, PyModule, PyRef, PyResult, Python};
use pyo3::types::{
    PyAnyMethods, PyFloat, PyInt, PyList, PyListMethods, PyModuleMethods, PySequence,
    PySequenceMethods, PySlice, PyTuple, PyTupleMethods, PyTypeMethods,
};
use pyo3::{pyclass, pymethods};

use crate::{PillowFloatPointBuffer, inputs::c_int_from_python};
use pillow_rs::ImagePath;

#[pyclass(name = "Path", unsendable)]
pub(crate) struct PyImagePath {
    inner: RefCell<ImagePath>,
    mapping: Cell<bool>,
}

struct PathPointBuilder {
    points: Vec<[f64; 2]>,
    pending_x: Option<f64>,
}

impl PathPointBuilder {
    fn with_capacity(capacity: usize) -> PyResult<Self> {
        let mut points = Vec::new();
        points
            .try_reserve_exact(capacity.max(1))
            .map_err(|_| PyMemoryError::new_err(()))?;
        Ok(Self {
            points,
            pending_x: None,
        })
    }

    fn push_coordinate(&mut self, coordinate: f64) {
        if let Some(x) = self.pending_x.take() {
            self.points.push([x, coordinate]);
        } else {
            self.pending_x = Some(coordinate);
        }
    }

    fn push_pair(&mut self, [x, y]: [f64; 2]) {
        self.push_coordinate(x);
        self.push_coordinate(y);
    }

    fn finish(self) -> PyResult<ImagePath> {
        if self.pending_x.is_some() {
            return Err(PyValueError::new_err("wrong number of coordinates"));
        }
        Ok(ImagePath::from_points(self.points))
    }

    fn copy_points(points: &[[f64; 2]]) -> PyResult<ImagePath> {
        let mut copy = Self::with_capacity(points.len())?;
        for point in points.iter().copied() {
            copy.push_pair(point);
        }
        copy.finish()
    }
}

impl PyImagePath {
    fn from_core(inner: ImagePath) -> Self {
        Self {
            inner: RefCell::new(inner),
            mapping: Cell::new(false),
        }
    }

    pub(crate) fn points_for_draw(&self) -> Vec<[f64; 2]> {
        self.inner.borrow().as_points().to_vec()
    }

    fn id_pointer(&self) -> isize {
        self.inner.borrow().as_points().as_ptr() as isize
    }

    fn index(value: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<isize> {
        let has_index = value.get_type().hasattr("__index__")?;
        if !has_index {
            return Err(PyTypeError::new_err(format!(
                "Path indices must be integers, not {}",
                value.get_type().name()?
            )));
        }
        Self::sequence_index(value, py)
    }

    fn sequence_index(value: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<isize> {
        value.extract::<isize>().map_err(|error| {
            if error.is_instance_of::<PyOverflowError>(py) {
                PyIndexError::new_err("cannot fit 'int' into an index-sized integer")
            } else {
                error
            }
        })
    }

    fn pair_from_tuple(value: &Bound<'_, PyAny>) -> PyResult<[f64; 2]> {
        if value.cast::<PyTuple>().is_err() {
            return Err(PySystemError::new_err(
                "new style getargs format but argument is not a tuple",
            ));
        }
        value.extract::<(f64, f64)>().map(|(x, y)| [x, y])
    }

    fn assign_point_from_tuple(&self, index: usize, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let tuple = value.cast::<PyTuple>().map_err(|_| {
            PySystemError::new_err("new style getargs format but argument is not a tuple")
        })?;
        if tuple.len() != 2 {
            return Self::pair_from_tuple(value).map(|_| ());
        }

        let x = tuple.get_item(0)?.extract::<f64>()?;
        let current = self.inner.borrow().point(index);
        if let Some(mut point) = current {
            point[0] = x;
            self.inner.borrow_mut().set_point(index, point);
        }

        let y = tuple.get_item(1)?.extract::<f64>()?;
        let current = self.inner.borrow().point(index);
        if let Some(mut point) = current {
            point[1] = y;
            self.inner.borrow_mut().set_point(index, point);
        }
        Ok(())
    }

    fn exact_arg_count(args: &Bound<'_, PyTuple>, count: usize, name: &str) -> PyResult<()> {
        if args.len() != count {
            return Err(PyTypeError::new_err(format!(
                "{name}() takes exactly {count} argument{} ({} given)",
                if count == 1 { "" } else { "s" },
                args.len()
            )));
        }
        Ok(())
    }

    fn sequence_item<'py>(
        value: &Bound<'py, PySequence>,
        index: usize,
    ) -> PyResult<Bound<'py, PyAny>> {
        value.get_item(index)
    }

    fn number(value: &Bound<'_, PyAny>) -> PyResult<f64> {
        if value.is_instance_of::<PyFloat>() {
            return value.extract::<f64>();
        }
        if value.is_instance_of::<PyInt>() {
            let value = value.extract::<std::os::raw::c_long>()?;
            return Ok(f64::from(value as f32));
        }
        value.extract::<f64>()
    }

    fn is_number(value: &Bound<'_, PyAny>) -> bool {
        [
            "__add__",
            "__sub__",
            "__mul__",
            "__matmul__",
            "__truediv__",
            "__floordiv__",
            "__mod__",
            "__divmod__",
            "__pow__",
            "__lshift__",
            "__rshift__",
            "__and__",
            "__xor__",
            "__or__",
            "__neg__",
            "__pos__",
            "__abs__",
            "__invert__",
            "__int__",
            "__float__",
            "__index__",
        ]
        .iter()
        .any(|name| value.get_type().hasattr(name).unwrap_or(false))
    }

    fn list_pair(value: &Bound<'_, PyAny>) -> PyResult<[f64; 2]> {
        let list = value
            .cast::<PyList>()
            .map_err(|_| PyValueError::new_err("coordinate list must contain numbers"))?;
        if list.len() != 2 {
            return Err(PyValueError::new_err(
                "coordinate list must contain exactly 2 coordinates",
            ));
        }
        let mut pair = [0.0; 2];
        for (index, coordinate) in pair.iter_mut().enumerate() {
            let value = list.get_item(index)?;
            if !Self::is_number(&value) {
                return Err(PyValueError::new_err(
                    "coordinate list must contain numbers",
                ));
            }
            *coordinate = Self::number(&value)?;
        }
        Ok(pair)
    }

    fn append_coordinate(value: &Bound<'_, PyAny>, path: &mut PathPointBuilder) -> PyResult<()> {
        if Self::is_number(value) {
            path.push_coordinate(Self::number(value)?);
            return Ok(());
        }
        if value.cast::<PyList>().is_ok() {
            path.push_pair(Self::list_pair(value)?);
            return Ok(());
        }
        if value.cast::<PyTuple>().is_ok() {
            path.push_pair(
                Self::pair_from_tuple(value)
                    .map_err(|_| PyValueError::new_err("incorrect coordinate type"))?,
            );
            return Ok(());
        }
        Err(PyValueError::new_err("incorrect coordinate type"))
    }

    fn points_from_buffer(value: &Bound<'_, PyAny>) -> Option<PyResult<ImagePath>> {
        let buffer = PillowFloatPointBuffer::from_python(value)?;
        let mut path = match PathPointBuilder::with_capacity(buffer.coordinate_count() / 2) {
            Ok(path) => path,
            Err(error) => return Some(Err(error)),
        };
        for index in (0..buffer.coordinate_count()).step_by(2) {
            let point = buffer
                .coordinate(index)
                .and_then(|x| buffer.coordinate(index + 1).map(|y| [x, y]));
            match point {
                Ok(point) => path.push_pair(point),
                Err(error) => return Some(Err(error)),
            }
        }
        Some(path.finish())
    }

    fn points_from_sequence(value: &Bound<'_, PyAny>) -> PyResult<ImagePath> {
        let sequence = value
            .cast::<PySequence>()
            .map_err(|_| PyTypeError::new_err("argument must be sequence"))?;
        let length = sequence.len()?;
        let mut path = PathPointBuilder::with_capacity(length)?;
        let is_list = value.cast::<PyList>().is_ok();
        let is_tuple = value.cast::<PyTuple>().is_ok();
        for index in 0..length {
            let item = if is_list {
                value.cast::<PyList>()?.get_item(index)
            } else if is_tuple {
                value.cast::<PyTuple>()?.get_item(index)
            } else {
                Self::sequence_item(sequence, index)
            };
            let item = match item {
                Ok(item) => item,
                Err(error)
                    if !is_list
                        && !is_tuple
                        && error.is_instance_of::<PyIndexError>(value.py()) =>
                {
                    break;
                }
                Err(error) => return Err(error),
            };
            Self::append_coordinate(&item, &mut path)?;
        }
        path.finish()
    }

    fn from_python(value: &Bound<'_, PyAny>) -> PyResult<ImagePath> {
        if let Ok(path) = value.extract::<PyRef<'_, PyImagePath>>() {
            return PathPointBuilder::copy_points(path.inner.borrow().as_points());
        }
        if let Ok(count) = value.extract::<isize>() {
            if count < 0 {
                return Err(PyMemoryError::new_err(()));
            }
            let mut points = Vec::new();
            points
                .try_reserve_exact((count as usize).max(1))
                .map_err(|_| PyMemoryError::new_err(()))?;
            points.resize(count as usize, [0.0, 0.0]);
            return Ok(ImagePath::from_points(points));
        }
        if let Some(points) = Self::points_from_buffer(value) {
            return points;
        }
        Self::points_from_sequence(value)
    }
}

#[pymethods]
impl PyImagePath {
    #[new]
    #[pyo3(signature = (*args))]
    fn new(args: &Bound<'_, PyTuple>) -> PyResult<Self> {
        Self::exact_arg_count(args, 1, "Path")?;
        let value = args.get_item(0)?;
        let path = Self::from_python(&value)?;
        // `PyPath_Flatten` can leave a scalar conversion error pending while
        // producing a path. Keeping the successful result here lets CPython's
        // type-call boundary report the same public SystemError as Pillow.
        Ok(Self::from_core(path))
    }

    fn __len__(&self) -> usize {
        self.inner.borrow().len()
    }

    fn __getitem__(&self, py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        if index.is_instance_of::<PySlice>() {
            // Keep Pillow 12.2's observable slice-index behavior. Its mapping
            // slot currently normalizes slices against a fixed length of four.
            let (start, stop, step): (isize, isize, isize) =
                index.call_method1("indices", (4,))?.extract()?;
            let slice_length = if step > 0 && start < stop {
                ((stop - start - 1) as usize) / (step as usize) + 1
            } else if step < 0 && start > stop {
                ((start - stop - 1) as usize) / step.unsigned_abs() + 1
            } else {
                0
            };
            if slice_length > 0 && step != 1 {
                return Err(PyTypeError::new_err("slice steps not supported"));
            }
            let sliced = if slice_length == 0 {
                PathPointBuilder::copy_points(&[])?
            } else {
                let length = self.inner.borrow().len();
                let start = start.clamp(0, length as isize) as usize;
                let stop = stop.clamp(start as isize, length as isize) as usize;
                PathPointBuilder::copy_points(&self.inner.borrow().as_points()[start..stop])?
            };
            return Py::new(py, Self::from_core(sliced)).map(|path| path.into_any());
        }
        let index = Self::index(index, py)?;
        let length = self.inner.borrow().len();
        let normalized = if index < 0 {
            length as isize + index
        } else {
            index
        };
        let Some(point) = usize::try_from(normalized)
            .ok()
            .and_then(|index| self.inner.borrow().point(index))
        else {
            return Err(PyIndexError::new_err("path index out of range"));
        };
        Ok(PyTuple::new(py, [point[0], point[1]])?.into_any().unbind())
    }

    fn __setitem__(
        &self,
        py: Python<'_>,
        index: &Bound<'_, PyAny>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let index = Self::sequence_index(index, py)?;
        let length = self.inner.borrow().len();
        if index < 0 || usize::try_from(index).map_or(true, |index| index >= length) {
            return Err(PyIndexError::new_err("path assignment index out of range"));
        }
        self.assign_point_from_tuple(index as usize, value)
    }

    fn __delitem__(&self, py: Python<'_>, index: &Bound<'_, PyAny>) -> PyResult<()> {
        let index = Self::sequence_index(index, py)?;
        let length = self.inner.borrow().len();
        if index < 0 || usize::try_from(index).map_or(true, |index| index >= length) {
            return Err(PyIndexError::new_err("path assignment index out of range"));
        }
        Err(PyTypeError::new_err("cannot delete from path"))
    }

    #[pyo3(signature = (*args))]
    fn getbbox(&self, args: &Bound<'_, PyTuple>) -> PyResult<(f64, f64, f64, f64)> {
        let count = args.len();
        if count != 0 {
            return Err(PyTypeError::new_err(format!(
                "getbbox() takes no arguments ({count} given)"
            )));
        }
        Ok(self.inner.borrow().bounding_box())
    }

    #[getter]
    fn id(&self) -> isize {
        self.id_pointer()
    }

    #[pyo3(signature = (*args))]
    fn tolist<'py>(
        &self,
        py: Python<'py>,
        args: &Bound<'py, PyTuple>,
    ) -> PyResult<Bound<'py, PyList>> {
        let count = args.len();
        if count > 1 {
            return Err(PyTypeError::new_err(format!(
                "tolist() takes at most 1 argument ({count} given)"
            )));
        }
        let flat = if count == 0 {
            0
        } else {
            c_int_from_python(&args.get_item(0)?)?
        };
        let output = PyList::empty(py);
        let points = self.inner.borrow();
        for [x, y] in points.as_points().iter().copied() {
            if flat != 0 {
                output.append(x)?;
                output.append(y)?;
            } else {
                output.append((x, y))?;
            }
        }
        Ok(output)
    }

    #[pyo3(signature = (*args))]
    fn compact(&self, args: &Bound<'_, PyTuple>) -> PyResult<i32> {
        if self.mapping.get() {
            return Err(PyValueError::new_err("Path compacted during mapping"));
        }
        let count = args.len();
        if count > 1 {
            return Err(PyTypeError::new_err(format!(
                "compact() takes at most 1 argument ({count} given)"
            )));
        }
        let distance = if count == 0 {
            2.0
        } else {
            args.get_item(0)?.extract::<f64>()?
        };
        Ok(self.inner.borrow_mut().compact(distance) as i32)
    }

    #[pyo3(signature = (*args))]
    fn map(&self, args: &Bound<'_, PyTuple>) -> PyResult<()> {
        Self::exact_arg_count(args, 1, "map")?;
        let function = args.get_item(0)?;
        self.mapping.set(true);
        let result = self.map_points(&function);
        self.mapping.set(false);
        result
    }

    #[pyo3(signature = (*args))]
    fn transform(&self, args: &Bound<'_, PyTuple>) -> PyResult<()> {
        let count = args.len();
        if !(1..=2).contains(&count) {
            return Err(PyTypeError::new_err(format!(
                "transform() takes from 1 to 2 arguments ({count} given)"
            )));
        }
        let matrix_value = args.get_item(0)?;
        let matrix_sequence = matrix_value
            .cast::<PySequence>()
            .map_err(|_| PyTypeError::new_err("transform() matrix must be a six-item sequence"))?;
        let matrix_length = matrix_sequence.len()?;
        if matrix_length != 6 {
            return Err(PyTypeError::new_err(format!(
                "transform() matrix must have six values ({} given)",
                matrix_length
            )));
        }
        let mut matrix = [0.0; 6];
        for (index, value) in matrix.iter_mut().enumerate() {
            *value = matrix_sequence.get_item(index)?.extract::<f64>()?;
        }
        let wrap = if count == 2 {
            args.get_item(1)?.extract::<f64>()?
        } else {
            0.0
        };
        self.inner.borrow_mut().transform(matrix, wrap);
        Ok(())
    }
}

impl PyImagePath {
    fn map_points(&self, function: &Bound<'_, PyAny>) -> PyResult<()> {
        for index in 0..self.inner.borrow().len() {
            let Some([x, y]) = self.inner.borrow().point(index) else {
                break;
            };
            let result = function.call1((x, y))?;
            let point = Self::pair_from_tuple(&result)?;
            self.inner.borrow_mut().set_point(index, point);
        }
        Ok(())
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyImagePath>()
}
