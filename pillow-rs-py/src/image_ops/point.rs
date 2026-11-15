use super::{
    imageops_global_or_builtin, imageops_mode_equals, imageops_mode_is_luma_or_rgb,
    imageops_unsupported_mode_error,
};
use crate::map_error;
use pyo3::basic::CompareOp;
use pyo3::exceptions::PyAssertionError;
use pyo3::prelude::{Bound, Py, PyAny, PyErr, PyModule, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyModuleMethods};
use pyo3::types::{PyInt, PyIterator, PyList, PyListMethods};
use pyo3::{pyfunction, wrap_pyfunction};

pub(super) struct PythonImageOpsCallbacks<'py> {
    pub(super) py: Python<'py>,
}

pub(super) enum ImageOpsCallbackError {
    Core(pillow_rs::PilError),
    Python(PyErr),
}

impl From<pillow_rs::PilError> for ImageOpsCallbackError {
    fn from(error: pillow_rs::PilError) -> Self {
        Self::Core(error)
    }
}

impl From<PyErr> for ImageOpsCallbackError {
    fn from(error: PyErr) -> Self {
        Self::Python(error)
    }
}

impl<'py> pillow_rs::ImageOpsLutCallbacks for PythonImageOpsCallbacks<'py> {
    type Image = Bound<'py, PyAny>;
    type Lut = Bound<'py, PyAny>;
    type Output = Bound<'py, PyAny>;
    type Error = ImageOpsCallbackError;

    fn image_mode_is_palette(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        let mode = image.getattr("mode")?;
        imageops_mode_equals(self.py, &mode, "P").map_err(Into::into)
    }

    fn image_mode_is_luma_or_rgb(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        let mode = image.getattr("mode")?;
        imageops_mode_is_luma_or_rgb(self.py, &mode).map_err(Into::into)
    }

    fn image_mode_is_rgb(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        let mode = image.getattr("mode")?;
        imageops_mode_equals(self.py, &mode, "RGB").map_err(Into::into)
    }

    fn unsupported_mode_error(&mut self, image: &Self::Image) -> Result<Self::Error, Self::Error> {
        let mode = image.getattr("mode")?;
        Ok(ImageOpsCallbackError::Python(
            imageops_unsupported_mode_error(self.py, &mode)?,
        ))
    }

    fn lut_length(&mut self, lut: &Self::Lut) -> Result<usize, Self::Error> {
        imageops_global_or_builtin(self.py, "len")?
            .call1((lut,))?
            .extract()
            .map_err(Into::into)
    }

    fn repeat_lut(&mut self, lut: Self::Lut) -> Result<Self::Lut, Self::Error> {
        let doubled = lut.add(&lut)?;
        doubled.add(&lut).map_err(Into::into)
    }

    fn point(&mut self, image: &Self::Image, lut: Self::Lut) -> Result<Self::Output, Self::Error> {
        image.call_method1("point", (lut,)).map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsPaletteLutCallbacks for PythonImageOpsCallbacks<'py> {
    type PaletteMode = Bound<'py, PyAny>;
    type PaletteValues = Bound<'py, PyAny>;
    type PaletteIterator = Bound<'py, PyIterator>;
    type PaletteEntry = Bound<'py, PyAny>;
    type PaletteBuilder = Bound<'py, PyList>;

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn copy_palette_image(&mut self, image: &Self::Image) -> Result<Self::Image, Self::Error> {
        image.call_method0("copy").map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn palette_is_none(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        Ok(image.getattr("palette")?.is_none())
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pending; benchmark=pending
    fn palette_assertion_error(&mut self) -> Self::Error {
        ImageOpsCallbackError::Python(PyAssertionError::new_err(""))
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn palette_mode(&mut self, image: &Self::Image) -> Result<Self::PaletteMode, Self::Error> {
        image
            .getattr("palette")?
            .getattr("mode")
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn palette_values(&mut self, image: &Self::Image) -> Result<Self::PaletteValues, Self::Error> {
        image
            .call_method1("getpalette", (self.py.None(),))
            .map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn palette_values_are_none(
        &mut self,
        values: &Self::PaletteValues,
    ) -> Result<bool, Self::Error> {
        Ok(values.is_none())
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn palette_values_iter(
        &mut self,
        values: Self::PaletteValues,
    ) -> Result<Self::PaletteIterator, Self::Error> {
        values.try_iter().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn next_palette_entry(
        &mut self,
        values: &mut Self::PaletteIterator,
    ) -> Result<Option<Self::PaletteEntry>, Self::Error> {
        values.next().transpose().map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn palette_entry_is_alpha(
        &mut self,
        mode: &Self::PaletteMode,
        index: usize,
    ) -> Result<bool, Self::Error> {
        let length = imageops_global_or_builtin(self.py, "len")?.call1((mode,))?;
        let palette_index = PyInt::new(self.py, index);
        let channel_index = palette_index.as_any().rem(length)?;
        let channel = mode.get_item(channel_index)?;
        let is_not_alpha = channel.rich_compare("A", CompareOp::Ne)?.is_truthy()?;
        Ok(!is_not_alpha)
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn palette_lut_entry(
        &mut self,
        lut: &Self::Lut,
        entry: &Self::PaletteEntry,
    ) -> Result<Self::PaletteEntry, Self::Error> {
        lut.get_item(entry).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn new_palette_builder(&mut self) -> Result<Self::PaletteBuilder, Self::Error> {
        Ok(PyList::empty(self.py))
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn append_palette_entry(
        &mut self,
        builder: &mut Self::PaletteBuilder,
        entry: Self::PaletteEntry,
    ) -> Result<(), Self::Error> {
        builder.append(entry).map_err(Into::into)
    }

    // VERIFY: source=reviewed@2026-11-15; parity=pass@migration-parity-9f188c313fb74b1e8652a1b1fbd627c7; benchmark=pending
    fn put_palette(
        &mut self,
        image: Self::Image,
        values: Self::PaletteBuilder,
        mode: Self::PaletteMode,
    ) -> Result<Self::Output, Self::Error> {
        image.call_method1("putpalette", (values, mode))?;
        Ok(image)
    }
}

impl<'py> pillow_rs::ImageOpsInvertCallbacks for PythonImageOpsCallbacks<'py> {
    fn invert_lut(&mut self) -> Result<Self::Lut, Self::Error> {
        let list = imageops_global_or_builtin(self.py, "list")?;
        let range = imageops_global_or_builtin(self.py, "range")?;
        let values = range.call1((255, -1, -1))?;
        list.call1((values,)).map_err(Into::into)
    }

    fn image_mode_is_one(&mut self, image: &Self::Image) -> Result<bool, Self::Error> {
        let mode = image.getattr("mode")?;
        imageops_mode_equals(self.py, &mode, "1").map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsLutBuildCallbacks for PythonImageOpsCallbacks<'py> {
    type Value = Bound<'py, PyAny>;
    type Iterator = Bound<'py, PyIterator>;
    type Builder = Bound<'py, PyList>;

    fn new_lut_builder(&mut self) -> Result<Self::Builder, Self::Error> {
        Ok(PyList::empty(self.py))
    }

    fn lut_samples(&mut self) -> Result<Self::Iterator, Self::Error> {
        let range = imageops_global_or_builtin(self.py, "range")?;
        let samples = range.call1((256,))?;
        samples.try_iter().map_err(Into::into)
    }

    fn next_lut_sample(
        &mut self,
        samples: &mut Self::Iterator,
    ) -> Result<Option<Self::Value>, Self::Error> {
        samples.next().transpose().map_err(Into::into)
    }

    fn append_lut_value(
        &mut self,
        builder: &mut Self::Builder,
        value: Self::Value,
    ) -> Result<(), Self::Error> {
        builder.append(value).map_err(Into::into)
    }

    fn finish_lut(&mut self, builder: Self::Builder) -> Result<Self::Lut, Self::Error> {
        Ok(builder.into_any())
    }
}

impl<'py> pillow_rs::ImageOpsPosterizeCallbacks for PythonImageOpsCallbacks<'py> {
    type Bits = Bound<'py, PyAny>;

    fn eight_minus_bits(&mut self, bits: &Self::Bits) -> Result<Self::Value, Self::Error> {
        PyInt::new(self.py, 8)
            .as_any()
            .sub(bits)
            .map_err(Into::into)
    }

    fn two_to_power(&mut self, exponent: &Self::Value) -> Result<Self::Value, Self::Error> {
        PyInt::new(self.py, 2)
            .as_any()
            .pow(exponent, self.py.None())
            .map_err(Into::into)
    }

    fn subtract_one(&mut self, power: &Self::Value) -> Result<Self::Value, Self::Error> {
        power.sub(1).map_err(Into::into)
    }

    fn invert_mask(&mut self, value: &Self::Value) -> Result<Self::Value, Self::Error> {
        value.bitnot().map_err(Into::into)
    }

    fn and_sample_with_mask(
        &mut self,
        sample: &Self::Value,
        mask: &Self::Value,
    ) -> Result<Self::Value, Self::Error> {
        sample.bitand(mask).map_err(Into::into)
    }
}

impl<'py> pillow_rs::ImageOpsSolarizeCallbacks for PythonImageOpsCallbacks<'py> {
    type Threshold = Bound<'py, PyAny>;

    fn sample_is_below_threshold(
        &mut self,
        sample: &Self::Value,
        threshold: &Self::Threshold,
    ) -> Result<bool, Self::Error> {
        sample
            .rich_compare(threshold, CompareOp::Lt)?
            .is_truthy()
            .map_err(Into::into)
    }

    fn invert_sample(&mut self, sample: &Self::Value) -> Result<Self::Value, Self::Error> {
        PyInt::new(self.py, 255)
            .as_any()
            .sub(sample)
            .map_err(Into::into)
    }
}

pub(super) fn map_imageops_callback_error(error: ImageOpsCallbackError) -> PyErr {
    match error {
        ImageOpsCallbackError::Core(error) => map_error(error),
        ImageOpsCallbackError::Python(error) => error,
    }
}

#[pyfunction]
fn ops_invert(py: Python<'_>, image: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonImageOpsCallbacks { py };
    pillow_rs::imageops_invert_with_callbacks(&mut callbacks, image.clone())
        .map(|result| result.unbind())
        .map_err(map_imageops_callback_error)
}

#[pyfunction]
fn ops_posterize(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    bits: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonImageOpsCallbacks { py };
    pillow_rs::imageops_posterize_with_callbacks(&mut callbacks, image.clone(), bits.clone())
        .map(|result| result.unbind())
        .map_err(map_imageops_callback_error)
}

#[pyfunction]
fn ops_solarize(
    py: Python<'_>,
    image: &Bound<'_, PyAny>,
    threshold: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let mut callbacks = PythonImageOpsCallbacks { py };
    pillow_rs::imageops_solarize_with_callbacks(&mut callbacks, image.clone(), threshold.clone())
        .map(|result| result.unbind())
        .map_err(map_imageops_callback_error)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(ops_invert, module)?)?;
    module.add_function(wrap_pyfunction!(ops_posterize, module)?)?;
    module.add_function(wrap_pyfunction!(ops_solarize, module)?)?;
    Ok(())
}
