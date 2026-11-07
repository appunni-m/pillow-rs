use crate::font::PyFont;
use pillow_rs::{DrawTextRouteCallbacks, DrawTextRouteCheck, Mode};
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyErr, PyModule, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyBool, PyBytes, PyFloat, PyInt, PyString, PyTuple};

struct PythonDrawTextRouteCallbacks<'py> {
    py: Python<'py>,
    draw_mode: Mode,
    owner: Bound<'py, PyAny>,
    xy: Bound<'py, PyAny>,
    image_text: Bound<'py, PyAny>,
    font: Bound<'py, PyAny>,
    stroke_width: Bound<'py, PyAny>,
    anchor: Bound<'py, PyAny>,
    align: Bound<'py, PyAny>,
    direction: Bound<'py, PyAny>,
    features: Bound<'py, PyAny>,
    language: Bound<'py, PyAny>,
    embedded_color: Bound<'py, PyAny>,
    args: Bound<'py, PyAny>,
    mask_kwargs: Bound<'py, PyAny>,
    native_text_input: bool,
    imagefont_module: Option<Bound<'py, PyModule>>,
    imagetext_module: Option<Bound<'py, PyModule>>,
    image_text_type: Option<Bound<'py, PyAny>>,
    spacing: Option<Bound<'py, PyAny>>,
}

impl<'py> PythonDrawTextRouteCallbacks<'py> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        py: Python<'py>,
        draw_mode: Mode,
        owner: &Bound<'py, PyAny>,
        xy: &Bound<'py, PyAny>,
        image_text: &Bound<'py, PyAny>,
        font: &Bound<'py, PyAny>,
        stroke_width: &Bound<'py, PyAny>,
        anchor: &Bound<'py, PyAny>,
        align: &Bound<'py, PyAny>,
        direction: &Bound<'py, PyAny>,
        features: &Bound<'py, PyAny>,
        language: &Bound<'py, PyAny>,
        embedded_color: &Bound<'py, PyAny>,
        args: &Bound<'py, PyAny>,
        mask_kwargs: &Bound<'py, PyAny>,
        native_text_input: bool,
    ) -> Self {
        Self {
            py,
            draw_mode,
            owner: owner.clone(),
            xy: xy.clone(),
            image_text: image_text.clone(),
            font: font.clone(),
            stroke_width: stroke_width.clone(),
            anchor: anchor.clone(),
            align: align.clone(),
            direction: direction.clone(),
            features: features.clone(),
            language: language.clone(),
            embedded_color: embedded_color.clone(),
            args: args.clone(),
            mask_kwargs: mask_kwargs.clone(),
            native_text_input,
            imagefont_module: None,
            imagetext_module: None,
            image_text_type: None,
            spacing: None,
        }
    }

    fn module<'a>(
        module: Option<&'a Bound<'py, PyModule>>,
        name: &str,
    ) -> PyResult<&'a Bound<'py, PyModule>> {
        module.ok_or_else(|| PyRuntimeError::new_err(format!("{name} was not prepared")))
    }

    fn spacing(&self) -> PyResult<&Bound<'py, PyAny>> {
        self.spacing
            .as_ref()
            .ok_or_else(|| PyRuntimeError::new_err("draw text spacing was not prepared"))
    }

    fn exact_builtin_type<T>(&self, value: &Bound<'py, PyAny>) -> bool
    where
        T: pyo3::type_object::PyTypeInfo,
    {
        value.get_type().is(self.py.get_type::<T>())
    }

    fn native_coordinate(&self, value: &Bound<'py, PyAny>) -> PyResult<bool> {
        if self.exact_builtin_type::<PyFloat>(value) {
            let coordinate = value.extract::<f64>()?.trunc();
            return Ok(coordinate.is_finite()
                && (i32::MIN as f64..=i32::MAX as f64).contains(&coordinate));
        }
        if self.exact_builtin_type::<PyInt>(value) {
            return match value.extract::<i64>() {
                Ok(value) => Ok((-9_007_199_254_740_992..=9_007_199_254_740_992)
                    .contains(&value)
                    && i32::try_from(value).is_ok()),
                Err(_) => Ok(false),
            };
        }
        Ok(false)
    }

    fn builtin_text_and_font_methods(&self) -> PyResult<bool> {
        let module = Self::module(self.imagetext_module.as_ref(), "ImageText module")?;
        let text_is_builtin = module
            .getattr("_can_use_native_draw_text")?
            .call1((&self.image_text,))?
            .is_truthy()?;
        if !text_is_builtin {
            return Ok(false);
        }
        let font_is_builtin = module
            .getattr("_can_use_native_draw_font")?
            .call1((&self.font,))?
            .is_truthy()?;
        if !font_is_builtin {
            return Ok(false);
        }
        Ok(self
            .font
            .getattr("_rust_font")?
            .cast::<PyFont>()
            .is_ok())
    }

    fn default_font_mode(&self) -> PyResult<bool> {
        let module = PyModule::import(self.py, "pillow_rs.imagedraw")?;
        let draw_type = module.getattr("ImageDraw")?;
        if !self.owner.get_type().is(&draw_type) {
            return Ok(false);
        }
        let fontmode = self.owner.getattr("fontmode")?;
        if !self.exact_builtin_type::<PyString>(&fontmode) {
            return Ok(false);
        }
        let expected = pillow_rs::draw_text_bbox_font_mode(self.draw_mode, false);
        fontmode
            .rich_compare(PyString::new(self.py, expected), CompareOp::Eq)?
            .is_truthy()
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn uses_direct_text_route(
    py: Python<'_>,
    draw_mode: Mode,
    owner: &Bound<'_, PyAny>,
    xy: &Bound<'_, PyAny>,
    image_text: &Bound<'_, PyAny>,
    font: &Bound<'_, PyAny>,
    stroke_width: &Bound<'_, PyAny>,
    anchor: &Bound<'_, PyAny>,
    align: &Bound<'_, PyAny>,
    direction: &Bound<'_, PyAny>,
    features: &Bound<'_, PyAny>,
    language: &Bound<'_, PyAny>,
    embedded_color: &Bound<'_, PyAny>,
    args: &Bound<'_, PyAny>,
    mask_kwargs: &Bound<'_, PyAny>,
    native_text_input: bool,
) -> PyResult<bool> {
    let mut callbacks = PythonDrawTextRouteCallbacks::new(
        py,
        draw_mode,
        owner,
        xy,
        image_text,
        font,
        stroke_width,
        anchor,
        align,
        direction,
        features,
        language,
        embedded_color,
        args,
        mask_kwargs,
        native_text_input,
    );
    pillow_rs::draw_uses_direct_text_route(draw_mode, &mut callbacks)
}

impl DrawTextRouteCallbacks for PythonDrawTextRouteCallbacks<'_> {
    type Error = PyErr;

    fn begin(&mut self) -> Result<(), Self::Error> {
        let imagetext_module = PyModule::import(self.py, "pillow_rs.imagetext")?;
        self.image_text_type = Some(imagetext_module.getattr("Text")?);
        self.imagetext_module = Some(imagetext_module);
        Ok(())
    }

    fn check(&mut self, check: DrawTextRouteCheck) -> Result<bool, Self::Error> {
        match check {
            DrawTextRouteCheck::NativeTextInput => Ok(self.native_text_input),
            DrawTextRouteCheck::ExactTextWrapper => {
                let text_type = self
                    .image_text_type
                    .as_ref()
                    .ok_or_else(|| PyRuntimeError::new_err("ImageText type was not prepared"))?;
                Ok(self.image_text.get_type().is(text_type))
            }
            DrawTextRouteCheck::NativeTextValue => {
                let text = self.image_text.getattr("text")?;
                Ok(self.exact_builtin_type::<PyString>(&text)
                    || self.exact_builtin_type::<PyBytes>(&text))
            }
            DrawTextRouteCheck::ExactTextModeString => {
                let mode = self.image_text.getattr("mode")?;
                Ok(self.exact_builtin_type::<PyString>(&mode))
            }
            DrawTextRouteCheck::ExactFreeTypeFont => {
                if self.imagefont_module.is_none() {
                    self.imagefont_module =
                        Some(PyModule::import(self.py, "pillow_rs.imagefont")?);
                }
                let module = Self::module(self.imagefont_module.as_ref(), "ImageFont module")?;
                let font_type = module.getattr("FreeTypeFont")?;
                Ok(self.font.get_type().is(&font_type))
            }
            DrawTextRouteCheck::BuiltinTextAndFontMethods => {
                self.builtin_text_and_font_methods()
            }
            DrawTextRouteCheck::DefaultFontMode => self.default_font_mode(),
            DrawTextRouteCheck::ExactIntegerSpacing => {
                let spacing = self.image_text.getattr("spacing")?;
                let exact_integer = self.exact_builtin_type::<PyInt>(&spacing);
                self.spacing = Some(spacing);
                Ok(exact_integer)
            }
            DrawTextRouteCheck::SpacingFitsNativeRange => {
                Ok(matches!(self.spacing()?.extract::<i64>(), Ok(value) if (-(1_i64 << 31)..(1_i64 << 31)).contains(&value)))
            }
            DrawTextRouteCheck::ExactIntegerStrokeWidth => {
                Ok(self.exact_builtin_type::<PyInt>(&self.stroke_width))
            }
            DrawTextRouteCheck::StrokeWidthFitsNativeRange => {
                Ok(matches!(self.stroke_width.extract::<i64>(), Ok(value) if (-(1_i64 << 24)..=(1_i64 << 24)).contains(&value)))
            }
            DrawTextRouteCheck::ExactEmbeddedColorBoolean => {
                Ok(self.exact_builtin_type::<PyBool>(&self.embedded_color))
            }
            DrawTextRouteCheck::AnchorSupportedType => {
                Ok(self.anchor.is_none() || self.exact_builtin_type::<PyString>(&self.anchor))
            }
            DrawTextRouteCheck::ExactAlignmentString => {
                Ok(self.exact_builtin_type::<PyString>(&self.align))
            }
            DrawTextRouteCheck::DirectionIsNone => Ok(self.direction.is_none()),
            DrawTextRouteCheck::FeaturesAreNone => Ok(self.features.is_none()),
            DrawTextRouteCheck::LanguageIsNone => Ok(self.language.is_none()),
            DrawTextRouteCheck::ExactCoordinateTuple => {
                Ok(self.exact_builtin_type::<PyTuple>(&self.xy))
            }
            DrawTextRouteCheck::CoordinateTupleLengthTwo => Ok(self.xy.len()? == 2),
            DrawTextRouteCheck::NativeCoordinates => {
                let x = self.xy.get_item(0)?;
                let y = self.xy.get_item(1)?;
                Ok(self.native_coordinate(&x)? && self.native_coordinate(&y)?)
            }
            DrawTextRouteCheck::NoExtraArguments => Ok(!self.args.is_truthy()?),
            DrawTextRouteCheck::NoExtraKeywords => Ok(!self.mask_kwargs.is_truthy()?),
        }
    }

    fn text_mode(&mut self) -> Result<Option<Mode>, Self::Error> {
        let mode = self.image_text.getattr("mode")?;
        Ok(mode
            .extract::<String>()
            .ok()
            .and_then(|mode| Mode::from_pillow_name(&mode)))
    }
}
