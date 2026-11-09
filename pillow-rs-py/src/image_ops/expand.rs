use super::PythonBorderCallbacks;
use crate::{PyImage, c_int_from_python, imageops_fill_from_python, map_error};
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyInt, PyModuleMethods, PyTuple, PyTupleMethods};
use pyo3::{pyfunction, wrap_pyfunction};

struct PythonExpandCallbacks<'py> {
    geometry: PythonBorderCallbacks<'py>,
    wrapper_type: Bound<'py, PyAny>,
}

enum ExpandCallbackError {
    Core(pillow_rs::PilError),
    Python(PyErr),
}

impl From<pillow_rs::PilError> for ExpandCallbackError {
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

impl From<PyErr> for ExpandCallbackError {
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

impl From<super::AspectCallbackError> for ExpandCallbackError {
    fn from(error: super::AspectCallbackError) -> Self {
        match error {
            super::AspectCallbackError::Core(error) => Self::Core(error),
            super::AspectCallbackError::Python(error) => Self::Python(error),
        }
    }
}

impl<'py> pillow_rs::ImageOpsBorderCallbacks for PythonExpandCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Error = ExpandCallbackError;

    fn border_is_tuple(&mut self, border: &Self::Value) -> Result<bool, Self::Error> {
        pillow_rs::ImageOpsBorderCallbacks::border_is_tuple(&mut self.geometry, border)
            .map_err(Into::into)
    }

    fn border_length(&mut self, border: &Self::Value) -> Result<usize, Self::Error> {
        pillow_rs::ImageOpsBorderCallbacks::border_length(&mut self.geometry, border)
            .map_err(Into::into)
    }

    fn border_coordinate(
        &mut self,
        border: &Self::Value,
        index: usize,
    ) -> Result<Self::Value, Self::Error> {
        pillow_rs::ImageOpsBorderCallbacks::border_coordinate(&mut self.geometry, border, index)
            .map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsExpandGeometryCallbacks for PythonExpandCallbacks<'py> {
    fn image_size_coordinate(&mut self, index: usize) -> Result<Self::Value, Self::Error> {
        pillow_rs::ImageOpsExpandGeometryCallbacks::image_size_coordinate(&mut self.geometry, index)
            .map_err(Into::into)
    }

    fn add(&mut self, left: &Self::Value, right: &Self::Value) -> Result<Self::Value, Self::Error> {
        pillow_rs::ImageOpsExpandGeometryCallbacks::add(&mut self.geometry, left, right)
            .map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsExpandCallbacks for PythonExpandCallbacks<'py> {
    type Palette = Bound<'py, PyAny>;
    type Output = Bound<'py, PyAny>;
    type WrapperConstructor = Bound<'py, PyAny>;

    fn run_core<T: Send>(&mut self, operation: impl FnOnce() -> T + Send) -> T {
        self.geometry.py.detach(operation)
    }

    fn image_mode(&mut self) -> Result<Self::Value, Self::Error> {
        self.geometry.image.getattr("mode").map_err(Into::into)
    }

    fn value_is_string(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        let is_instance = super::imageops_global_or_builtin(self.geometry.py, "isinstance")?;
        let string_type = super::imageops_global_or_builtin(self.geometry.py, "str")?;
        is_instance
            .call1((value.clone(), string_type))?
            .is_truthy()
            .map_err(Into::into)
    }

    fn color_from_name(
        &mut self,
        fill: &Self::Value,
        mode: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let image_color = self.geometry.py.import("PIL")?.getattr("ImageColor")?;
        image_color
            .getattr("getcolor")?
            .call1((fill, mode))
            .map_err(Into::into)
    }

    fn image_palette(&mut self) -> Result<Option<Self::Palette>, Self::Error> {
        let palette = self.geometry.image.getattr("palette")?;
        Ok((!palette.is_none()).then_some(palette))
    }

    fn palette_is_truthy(&mut self, palette: &Self::Palette) -> Result<bool, Self::Error> {
        palette.is_truthy().map_err(Into::into)
    }

    fn palette_mode(&mut self, palette: &Self::Palette) -> Result<Self::Value, Self::Error> {
        palette.getattr("mode").map_err(Into::into)
    }

    fn copy_palette(&mut self, mode: &Self::Value) -> Result<Self::Palette, Self::Error> {
        let constructor = self
            .geometry
            .py
            .import("pillow_rs.imagepalette")?
            .getattr("ImagePalette")?;
        let data = self.geometry.image.call_method1("getpalette", (mode,))?;
        constructor.call1((mode, data)).map_err(Into::into)
    }

    fn value_is_tuple(&mut self, value: &Self::Value) -> Result<bool, Self::Error> {
        let is_instance = super::imageops_global_or_builtin(self.geometry.py, "isinstance")?;
        let tuple_type = super::imageops_global_or_builtin(self.geometry.py, "tuple")?;
        is_instance
            .call1((value.clone(), tuple_type))?
            .is_truthy()
            .map_err(Into::into)
    }

    fn value_length_equals(
        &mut self,
        value: &Self::Value,
        expected: i64,
    ) -> Result<bool, Self::Error> {
        let length = super::imageops_global_or_builtin(self.geometry.py, "len")?.call1((value,))?;
        length
            .rich_compare(PyInt::new(self.geometry.py, expected), CompareOp::Eq)?
            .is_truthy()
            .map_err(Into::into)
    }

    fn palette_get_color(
        &mut self,
        palette: &Self::Palette,
        fill: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        palette
            .call_method1("getcolor", (fill,))
            .map_err(Into::into)
    }

    fn size_pair(
        &mut self,
        width: &Self::Value,
        height: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        PyTuple::new(self.geometry.py, [width.clone(), height.clone()])
            .map(|size| size.into_any())
            .map_err(Into::into)
    }

    fn size_is_negative(&mut self, size: &Self::Value, index: usize) -> Result<bool, Self::Error> {
        size.cast::<PyTuple>()
            .map_err(PyErr::from)?
            .get_item(index)?
            .lt(0)
            .map_err(Into::into)
    }

    fn size_component_as_c_int(
        &mut self,
        size: &Self::Value,
        index: usize,
    ) -> Result<i32, Self::Error> {
        let value = size
            .cast::<PyTuple>()
            .map_err(PyErr::from)?
            .get_item(index)?;
        c_int_from_python(&value).map_err(Into::into)
    }

    fn mode_identity(&mut self, value: &Self::Value) -> Result<pillow_rs::Mode, Self::Error> {
        let name = value.extract::<String>()?;
        pillow_rs::Mode::from_pillow_name(&name)
            .ok_or_else(|| PyValueError::new_err("unrecognized image mode"))
            .map_err(Into::into)
    }

    fn fill_input(
        &mut self,
        mode: pillow_rs::Mode,
        value: &Self::Value,
    ) -> Result<pillow_rs::ImageOpsColor, Self::Error> {
        imageops_fill_from_python(self.geometry.py, Some(value), mode).map_err(Into::into)
    }

    fn capture_wrapper_constructor(&mut self) -> Result<Self::WrapperConstructor, Self::Error> {
        self.wrapper_type.call0().map_err(Into::into)
    }

    fn wrap_canvas(
        &mut self,
        constructor: &Self::WrapperConstructor,
        image: pillow_rs::Image,
    ) -> Result<Self::Output, Self::Error> {
        let native = Py::new(self.geometry.py, PyImage { inner: image })?;
        constructor.call1((native,)).map_err(Into::into)
    }

    fn put_palette(
        &mut self,
        output: &mut Self::Output,
        palette: &Self::Palette,
        mode: &Self::Value,
    ) -> Result<(), Self::Error> {
        let put_palette = output.getattr("putpalette")?;
        let palette_data = palette.getattr("palette")?;
        put_palette
            .call1((palette_data, mode))
            .map(|_| ())
            .map_err(Into::into)
    }

    fn paste(
        &mut self,
        output: &mut Self::Output,
        left: &Self::Value,
        top: &Self::Value,
    ) -> Result<(), Self::Error> {
        let paste = output.getattr("paste")?;
        let position = PyTuple::new(self.geometry.py, [left.clone(), top.clone()])?;
        paste
            .call1((self.geometry.image.clone(), position))
            .map(|_| ())
            .map_err(Into::into)
    }
}

fn expand_callback_error(error: ExpandCallbackError) -> PyErr {
    match error {
        ExpandCallbackError::Core(error) => map_error(error),
        ExpandCallbackError::Python(error) => error,
    }
}

#[pyfunction]
fn ops_expand_with_callbacks(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    border: &Bound<'_, PyAny>,
    fill: &Bound<'_, PyAny>,
    wrapper_type: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonExpandCallbacks {
        geometry: PythonBorderCallbacks {
            py,
            image: image.clone(),
        },
        wrapper_type: wrapper_type.clone(),
    };
    pillow_rs::imageops_expand_with_callbacks(&mut callbacks, border, fill)
        .map(|output| output.unbind())
        .map_err(expand_callback_error)
}

pub(super) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(ops_expand_with_callbacks, m)?)?;
    Ok(())
}
