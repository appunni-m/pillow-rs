use crate::PyImage;
use crate::draw::PyDraw;
use crate::map_error;
use super::text_color_mask_impl;
use pillow_rs::{DrawTextMaskCallbacks, DrawTextMaskPosition, PilError};
use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyAttributeError, PyTypeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyResult, Python};
use pyo3::types::{
    PyAnyMethods, PyDict, PyDictMethods, PyInt, PyModule, PyString, PyTuple,
};

enum TextMaskError {
    Core(PilError),
    Python(PyErr),
}

impl From<PilError> for TextMaskError {
    fn from(error: PilError) -> Self {
        Self::Core(error)
    }
}

fn map_text_mask_error(error: TextMaskError) -> PyErr {
    match error {
        TextMaskError::Core(error) => map_error(error),
        TextMaskError::Python(error) => error,
    }
}

struct PythonTextMaskCallbacks<'py> {
    py: Python<'py>,
    py_draw: Py<PyDraw>,
    owner: Bound<'py, PyAny>,
    image_class: Bound<'py, PyAny>,
    getattr_fn: Bound<'py, PyAny>,
    isinstance_fn: Bound<'py, PyAny>,
    int_fn: Bound<'py, PyAny>,
    modf_fn: Bound<'py, PyAny>,
    image_text: Bound<'py, PyAny>,
    direction: Bound<'py, PyAny>,
    features: Bound<'py, PyAny>,
    language: Bound<'py, PyAny>,
    requested_mode: Bound<'py, PyAny>,
    ink: i64,
    args: Bound<'py, PyAny>,
    mask_kwargs: Bound<'py, PyAny>,
}

impl<'py> PythonTextMaskCallbacks<'py> {
    fn extra_positionals(
        &self,
        mut positionals: Vec<Bound<'py, PyAny>>,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let mut args = self.args.try_iter()?;
        while let Some(value) = args.next() {
            positionals.push(value?);
        }
        PyTuple::new(self.py, positionals)
    }

    fn keyword_arguments(
        &self,
        line: &Bound<'py, PyAny>,
        start: &Bound<'py, PyAny>,
        include_options: bool,
    ) -> PyResult<(Bound<'py, PyDict>, Bound<'py, PyDict>)> {
        let keywords = PyDict::new(self.py);
        if include_options {
            keywords.set_item("direction", self.direction.clone())?;
            keywords.set_item("features", self.features.clone())?;
            keywords.set_item("language", self.language.clone())?;
            keywords.set_item("stroke_width", self.image_text.getattr("stroke_width")?)?;
            keywords.set_item("stroke_filled", true)?;
            keywords.set_item("anchor", line.getattr("anchor")?)?;
            keywords.set_item("ink", self.ink)?;
        }
        keywords.set_item("start", start.clone())?;
        let extra = self.mask_kwargs.cast::<PyDict>()?.copy()?;
        Ok((keywords, extra))
    }

    fn invoke_keyword_callback(
        &self,
        method: &Bound<'py, PyAny>,
        positionals: &Bound<'py, PyTuple>,
        keywords: &Bound<'py, PyDict>,
        extra_keywords: &Bound<'py, PyDict>,
    ) -> PyResult<Bound<'py, PyAny>> {
        PyModule::import(self.py, "pillow_rs.imagedraw")?
            .getattr("_invoke_text_mask_callback")?
            .call1((method, positionals, keywords, extra_keywords))
    }

    fn unpack_mask_and_offset(
        &self,
        result: Bound<'py, PyAny>,
    ) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>)> {
        let mut values = result.try_iter()?;
        let Some(mask) = values.next() else {
            return Err(PyValueError::new_err(
                "not enough values to unpack (expected 2, got 0)",
            ));
        };
        let mask = mask?;
        let Some(offset) = values.next() else {
            return Err(PyValueError::new_err(
                "not enough values to unpack (expected 2, got 1)",
            ));
        };
        let offset = offset?;
        if values.next().transpose()?.is_some() {
            return Err(PyValueError::new_err(
                "too many values to unpack (expected 2)",
            ));
        }
        Ok((mask, offset))
    }
}

impl<'py> DrawTextMaskCallbacks for PythonTextMaskCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Error = TextMaskError;

    fn position(
        &mut self,
        line: &Self::Value,
    ) -> Result<DrawTextMaskPosition<Self::Value>, Self::Error> {
        let x_input = line.getattr("x").map_err(TextMaskError::Python)?;
        let x_int = self
            .int_fn
            .call1((x_input.clone(),))
            .map_err(TextMaskError::Python)?;
        let y_input = line.getattr("y").map_err(TextMaskError::Python)?;
        let y_int = self
            .int_fn
            .call1((y_input.clone(),))
            .map_err(TextMaskError::Python)?;
        let start_x = self
            .modf_fn
            .call1((x_input,))
            .and_then(|parts| parts.get_item(0))
            .map_err(TextMaskError::Python)?;
        let start_y = self
            .modf_fn
            .call1((y_input,))
            .and_then(|parts| parts.get_item(0))
            .map_err(TextMaskError::Python)?;
        let start = PyTuple::new(self.py, [start_x, start_y])
            .map_err(TextMaskError::Python)?
            .into_any();
        Ok(DrawTextMaskPosition {
            x: x_int,
            y: y_int,
            start,
        })
    }

    fn font_mask_mode(&mut self, _default: &str) -> Result<Self::Value, Self::Error> {
        Ok(self.requested_mode.clone())
    }

    fn mode_is_rgba(&mut self, mode: &Self::Value) -> Result<bool, Self::Error> {
        mode.rich_compare(PyString::new(self.py, "RGBA"), CompareOp::Eq)
            .and_then(|result| result.is_truthy())
            .map_err(TextMaskError::Python)
    }

    fn getmask2(
        &mut self,
        line: &Self::Value,
        mode: &Self::Value,
        start: &Self::Value,
    ) -> Result<(Self::Value, Self::Value), Self::Error> {
        let font = self
            .image_text
            .getattr("font")
            .map_err(TextMaskError::Python)?;
        let method = font.getattr("getmask2").map_err(TextMaskError::Python)?;
        let text = line.getattr("text").map_err(TextMaskError::Python)?;
        let positionals = self
            .extra_positionals(vec![text, mode.clone()])
            .map_err(TextMaskError::Python)?;
        let (keywords, extra_keywords) = self
            .keyword_arguments(line, start, true)
            .map_err(TextMaskError::Python)?;
        let result = self
            .invoke_keyword_callback(&method, &positionals, &keywords, &extra_keywords)
            .map_err(TextMaskError::Python)?;
        self.unpack_mask_and_offset(result)
            .map_err(TextMaskError::Python)
    }

    fn getmask(
        &mut self,
        line: &Self::Value,
        mode: &Self::Value,
        start: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        let font = self
            .image_text
            .getattr("font")
            .map_err(TextMaskError::Python)?;
        let method = font.getattr("getmask").map_err(TextMaskError::Python)?;
        let text = line.getattr("text").map_err(TextMaskError::Python)?;
        let direction = self.direction.clone();
        let features = self.features.clone();
        let language = self.language.clone();
        let stroke_width = self
            .image_text
            .getattr("stroke_width")
            .map_err(TextMaskError::Python)?;
        let anchor = line
            .getattr("anchor")
            .map_err(TextMaskError::Python)?;
        let mut positionals = vec![
            text,
            mode.clone(),
            direction,
            features,
            language,
            stroke_width,
            anchor,
            PyInt::new(self.py, self.ink).into_any(),
        ];
        let positionals = self
            .extra_positionals(std::mem::take(&mut positionals))
            .map_err(TextMaskError::Python)?;
        let (keywords, extra_keywords) = self
            .keyword_arguments(line, start, false)
            .map_err(TextMaskError::Python)?;
        self.invoke_keyword_callback(&method, &positionals, &keywords, &extra_keywords)
            .map_err(TextMaskError::Python)
    }

    fn getmask_text_only(&mut self, line: &Self::Value) -> Result<Self::Value, Self::Error> {
        let font = self
            .image_text
            .getattr("font")
            .map_err(TextMaskError::Python)?;
        let method = font.getattr("getmask").map_err(TextMaskError::Python)?;
        let text = line.getattr("text").map_err(TextMaskError::Python)?;
        method.call1((text,)).map_err(TextMaskError::Python)
    }

    fn is_attribute_error(&self, error: &Self::Error) -> bool {
        matches!(error, TextMaskError::Python(error) if error.is_instance_of::<PyAttributeError>(self.py))
    }

    fn is_type_error(&self, error: &Self::Error) -> bool {
        matches!(error, TextMaskError::Python(error) if error.is_instance_of::<PyTypeError>(self.py))
    }

    fn add_offset(
        &mut self,
        position: &mut DrawTextMaskPosition<Self::Value>,
        offset: &Self::Value,
        index: usize,
    ) -> Result<(), Self::Error> {
        let amount = offset.get_item(index).map_err(TextMaskError::Python)?;
        let coordinate = if index == 0 {
            &mut position.x
        } else {
            &mut position.y
        };
        *coordinate = coordinate
            .add(amount)
            .map_err(TextMaskError::Python)?;
        Ok(())
    }

    fn mask_size(&mut self, mask: &Self::Value) -> Result<Option<Self::Value>, Self::Error> {
        let size = self
            .getattr_fn
            .call1((mask, "size", self.py.None()))
            .map_err(TextMaskError::Python)?;
        Ok((!size.is_none()).then_some(size))
    }

    fn mask_dimension_is_zero(
        &mut self,
        size: &Self::Value,
        index: usize,
    ) -> Result<bool, Self::Error> {
        size.get_item(index)
            .and_then(|dimension| dimension.eq(0))
            .map_err(TextMaskError::Python)
    }

    fn embedded_image(&mut self, mask: &Self::Value) -> Result<Option<Self::Value>, Self::Error> {
        let image = self
            .getattr_fn
            .call1((mask, "_image", self.py.None()))
            .map_err(TextMaskError::Python)?;
        Ok((!image.is_none()).then_some(image))
    }

    fn is_image(&mut self, mask: &Self::Value) -> Result<bool, Self::Error> {
        self.isinstance_fn
            .call1((mask, self.image_class.clone()))
            .and_then(|result| result.is_truthy())
            .map_err(TextMaskError::Python)
    }

    fn mask_mode(
        &mut self,
        mask: &Self::Value,
        default: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        self.getattr_fn
            .call1((mask, "mode", default))
            .map_err(TextMaskError::Python)
    }

    fn mask_bytes(&mut self, mask: &Self::Value) -> Result<Self::Value, Self::Error> {
        mask.call_method0("tobytes").map_err(TextMaskError::Python)
    }

    fn image_from_bytes_method(&mut self) -> Result<Self::Value, Self::Error> {
        self.image_class
            .getattr("frombytes")
            .map_err(TextMaskError::Python)
    }

    fn image_from_bytes(
        &mut self,
        method: &Self::Value,
        mode: &Self::Value,
        size: &Self::Value,
        bytes: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        method
            .call1((mode, size, bytes))
            .map_err(TextMaskError::Python)
    }

    fn alpha_channel(&mut self, mask_image: &Self::Value) -> Result<Self::Value, Self::Error> {
        mask_image
            .call_method1("getchannel", ("A",))
            .map_err(TextMaskError::Python)
    }

    fn set_alpha(&mut self, mask_image: &Self::Value, alpha: u8) -> Result<(), Self::Error> {
        mask_image
            .call_method1("putalpha", (alpha,))
            .map(|_| ())
            .map_err(TextMaskError::Python)
    }

    fn draw_rgba_mask(
        &mut self,
        position: &DrawTextMaskPosition<Self::Value>,
        mask_image: &Self::Value,
        coverage: &Self::Value,
    ) -> Result<(), Self::Error> {
        let width = coverage
            .getattr("size")
            .and_then(|size| size.get_item(0))
            .map_err(TextMaskError::Python)?;
        let right = position
            .x
            .add(&width)
            .map_err(TextMaskError::Python)?;
        let height = coverage
            .getattr("size")
            .and_then(|size| size.get_item(1))
            .map_err(TextMaskError::Python)?;
        let bottom = position
            .y
            .add(&height)
            .map_err(TextMaskError::Python)?;
        // ImageDraw.text evaluates (x, y, x + mask.size[0], y + mask.size[1])
        // before the native paste parser converts each coordinate.
        let box_coordinates = PyTuple::new(
            self.py,
            [
                position.x.clone(),
                position.y.clone(),
                right,
                bottom,
            ],
        )
        .map_err(TextMaskError::Python)?;
        let (x, y, _right, _bottom) = box_coordinates
            .extract::<(i32, i32, i32, i32)>()
            .map_err(TextMaskError::Python)?;
        // Pillow parses the box, validates the optional typed mask, then
        // checks whether the source is an imaging object.
        let coverage = coverage
            .getattr("_rust_image")
            .map_err(TextMaskError::Python)?;
        let coverage = coverage
            .cast::<PyImage>()
            .map_err(|error| TextMaskError::Python(error.into()))?;
        let color = mask_image
            .getattr("_rust_image")
            .map_err(TextMaskError::Python)?;
        let color = color
            .cast::<PyImage>()
            .map_err(|error| TextMaskError::Python(error.into()))?;
        // This path mutates the native Draw directly and has no host callback.
        let mut draw = self.py_draw.borrow_mut(self.py);
        text_color_mask_impl(&mut draw.draw, (x, y), color, coverage)
            .map_err(TextMaskError::Python)
    }

    fn draw_bitmap(
        &mut self,
        position: &DrawTextMaskPosition<Self::Value>,
        mask_image: &Self::Value,
        fill: &Self::Value,
    ) -> Result<(), Self::Error> {
        // Do not borrow PyDraw here: the public bitmap method re-enters it.
        let method = self.owner.getattr("bitmap").map_err(TextMaskError::Python)?;
        let position = PyTuple::new(self.py, [position.x.clone(), position.y.clone()])
            .map_err(TextMaskError::Python)?;
        let positionals = PyTuple::new(self.py, [position.into_any(), mask_image.clone()])
            .map_err(TextMaskError::Python)?;
        let keywords = PyDict::new(self.py);
        keywords
            .set_item("fill", fill.clone())
            .map_err(TextMaskError::Python)?;
        method
            .call(positionals, Some(&keywords))
            .map(|_| ())
            .map_err(TextMaskError::Python)
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_custom_font_mask<'py>(
    py: Python<'py>,
    py_draw: Py<PyDraw>,
    owner: &Bound<'py, PyAny>,
    image_class: &Bound<'py, PyAny>,
    getattr_fn: &Bound<'py, PyAny>,
    isinstance_fn: &Bound<'py, PyAny>,
    int_fn: &Bound<'py, PyAny>,
    modf_fn: &Bound<'py, PyAny>,
    line: &Bound<'py, PyAny>,
    image_text: &Bound<'py, PyAny>,
    fill: &Bound<'py, PyAny>,
    default_mode: &str,
    direction: &Bound<'py, PyAny>,
    features: &Bound<'py, PyAny>,
    language: &Bound<'py, PyAny>,
    mode: &Bound<'py, PyAny>,
    ink: i64,
    args: &Bound<'py, PyAny>,
    mask_kwargs: &Bound<'py, PyAny>,
) -> PyResult<()> {
    let mut callbacks = PythonTextMaskCallbacks {
        py,
        py_draw,
        owner: owner.clone(),
        image_class: image_class.clone(),
        getattr_fn: getattr_fn.clone(),
        isinstance_fn: isinstance_fn.clone(),
        int_fn: int_fn.clone(),
        modf_fn: modf_fn.clone(),
        image_text: image_text.clone(),
        direction: direction.clone(),
        features: features.clone(),
        language: language.clone(),
        requested_mode: mode.clone(),
        ink,
        args: args.clone(),
        mask_kwargs: mask_kwargs.clone(),
    };
    pillow_rs::draw_custom_font_mask_with_callbacks(
        &mut callbacks,
        line,
        fill,
        ink,
        default_mode,
    )
    .map_err(map_text_mask_error)
}
