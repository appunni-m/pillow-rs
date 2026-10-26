use super::RsImage;
use super::arrays::image_info_value_to_python;
use super::point::point_transform_from_python;
use crate::*;
use pyo3::pymethods;

#[pymethods]
impl PyImage {
    #[pyo3(signature = (size, resample=None, box_coords=None, reducing_gap=None))]
    fn resize(
        &self,
        size: (i64, i64),
        resample: Option<&Bound<'_, PyAny>>,
        box_coords: Option<(f64, f64, f64, f64)>,
        reducing_gap: Option<f64>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let resample = resample_input_from_python(resample)?;
        let rs = py
            .detach(|| {
                self.inner
                    .resize_with_options(size, resample, box_coords, reducing_gap)
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn crop(&self, box_coords: Option<(f64, f64, f64, f64)>, py: Python<'_>) -> PyResult<PyImage> {
        let rs = py
            .detach(|| self.inner.crop_float(box_coords))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (angle, resample=None, expand=None, center=None, translate=None, fillcolor=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow Image.rotate API.
    fn rotate(
        &self,
        angle: f64,
        resample: Option<&Bound<'_, PyAny>>,
        expand: Option<&Bound<'_, PyAny>>,
        center: Option<&Bound<'_, PyAny>>,
        translate: Option<&Bound<'_, PyAny>>,
        fillcolor: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let center = rotate_point_input_from_python(center)?;
        let translate = rotate_point_input_from_python(translate)?;
        let expand = rotate_expand_input_from_python(expand)?;
        let resample = rotate_resample_input_from_python(resample)?;
        let fillcolor = imageops_color_from_python(fillcolor);
        let rs = py
            .detach(|| {
                self.inner
                    .rotate_with_input(angle, resample, expand, center, translate, fillcolor)
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn transpose(&self, method: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let input = transpose_input_from_python(method)?;
        // Ordinary transpose only queues metadata. Paletted source cloning
        // copies its index buffer, so keep that work outside the GIL.
        let rs = if matches!(&self.inner, RsImage::Paletted(_)) {
            py.detach(|| self.inner.transpose_with_input(input))
        } else {
            self.inner.transpose_with_input(input)
        }
        .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (mode=None, matrix=None, dither=None, palette=None, colors=None))]
    fn convert(
        &self,
        mode: Option<&Bound<'_, PyAny>>,
        matrix: Option<Vec<f64>>,
        dither: Option<&Bound<'_, PyAny>>,
        palette: Option<&Bound<'_, PyAny>>,
        colors: Option<u32>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let dither = match dither {
            None => pillow_rs::PythonDitherInput::None,
            Some(value) => {
                if let Ok(int_value) = value.extract::<u32>() {
                    pillow_rs::PythonDitherInput::Integer(int_value)
                } else if let Ok(string_value) = value.extract::<String>() {
                    pillow_rs::PythonDitherInput::Name(string_value)
                } else {
                    pillow_rs::PythonDitherInput::Invalid(value.get_type().name()?.to_string())
                }
            }
        };
        let mode = convert_mode_input_from_python(mode)?;
        let palette = convert_palette_input_from_python(palette)?;
        let rs = py
            .detach(|| {
                self.inner
                    .convert_with_input(mode, matrix, dither, palette, colors)
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, box_coords=None, mask=None))]
    fn paste(
        &mut self,
        im: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let source = paste_source_from_python(im);
        let box_coords = paste_box_from_python(box_coords)?;
        let mask = paste_mask_from_python(mask)?;
        py.detach(|| self.inner.paste_with_input(source, box_coords, mask))
            .map_err(map_error)
    }

    fn split(&self, py: Python<'_>) -> PyResult<Vec<PyImage>> {
        let bands = py.detach(|| self.inner.split()).map_err(map_error)?;
        Ok(bands
            .into_iter()
            .map(|img| PyImage { inner: img })
            .collect())
    }

    fn filter(&self, filter_type: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let filter = if filter_type.is_callable() {
            filter_type.call0()?
        } else {
            filter_type.clone()
        };
        let type_name = filter.get_type().name()?.to_string();
        let validation_name = filter
            .getattr("name")
            .ok()
            .and_then(|value| value.extract::<String>().ok())
            .unwrap_or_else(|| type_name.clone());
        self.inner
            .validate_filter(&validation_name)
            .map_err(map_error)?;
        if !filter.hasattr("_apply")? {
            return Err(PyTypeError::new_err(
                "filter argument should be ImageFilter.Filter instance or class",
            ));
        }
        let image = Py::new(
            py,
            PyImage {
                inner: self.inner.clone(),
            },
        )?;
        let result = filter.call_method1("_apply", (image,))?;
        let result = result.extract::<PyRef<'_, PyImage>>()?;
        Ok(PyImage {
            inner: result.inner.clone(),
        })
    }

    fn filter_name(&self, filter_type: &str, py: Python<'_>) -> PyResult<PyImage> {
        let filter_type = filter_type.to_owned();
        py.detach(|| self.inner.filter(&filter_type))
            .map(|inner| PyImage { inner })
            .map_err(map_error)
    }

    fn validate_filter(&self, filter_name: &str) -> PyResult<()> {
        self.inner.validate_filter(filter_name).map_err(map_error)
    }

    fn kernel_filter(
        &self,
        kernel: Option<Vec<f64>>,
        scale: Option<f64>,
        offset: f64,
        size: (u32, u32),
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let rs = py
            .detach(|| self.inner.kernel_filter(kernel, scale, offset, size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn copy(&self) -> PyImage {
        PyImage {
            inner: self.inner.copy(),
        }
    }

    fn _clone_handle(&self) -> PyImage {
        PyImage {
            inner: self.inner.clone(),
        }
    }

    #[pyo3(signature = (size, resample=None))]
    fn thumbnail(
        &mut self,
        size: (i64, i64),
        resample: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let resample = resample_input_from_python(resample)?;
        py.detach(|| self.inner.thumbnail(size, resample))
            .map_err(map_error)
    }

    #[pyo3(signature = (colors=None, method=None, kmeans=None, dither=None, palette=None))]
    fn quantize(
        &self,
        colors: Option<i32>,
        method: Option<i32>,
        kmeans: Option<i32>,
        dither: Option<bool>,
        palette: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let palette = match palette {
            None => pillow_rs::QuantizePalette::None,
            Some(value) => image_from_python(value)
                .map_or(pillow_rs::QuantizePalette::Other, |image| {
                    pillow_rs::QuantizePalette::Image(Box::new(image))
                }),
        };
        let rs = py
            .detach(|| {
                self.inner
                    .quantize_with_input(colors, method, kmeans, palette, dither)
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (radius=None))]
    fn gaussian_blur(
        &self,
        radius: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let (radius_x, radius_y) = blur_radius_pair_from_python(radius, 2.0)?;
        let rs = py
            .detach(|| {
                if radius_x == radius_y {
                    self.inner.gaussian_blur(radius_x as f32)
                } else {
                    self.inner
                        .gaussian_blur_xy(radius_x as f32, radius_y as f32)
                }
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn unsharp_mask(
        &self,
        radius: Option<f64>,
        percent: Option<i32>,
        threshold: Option<i32>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let radius = radius.unwrap_or(2.0) as f32;
        let percent = percent.unwrap_or(150);
        let threshold = threshold.unwrap_or(3);
        let rs = py
            .detach(|| self.inner.unsharp_mask(radius, percent, threshold))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn max_filter(&self, size: Option<i64>, py: Python<'_>) -> PyResult<PyImage> {
        let size = filter_size_from_python(size.unwrap_or(3), false)?;
        let rs = py
            .detach(|| self.inner.max_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn min_filter(&self, size: Option<i64>, py: Python<'_>) -> PyResult<PyImage> {
        let size = filter_size_from_python(size.unwrap_or(3), false)?;
        let rs = py
            .detach(|| self.inner.min_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn median_filter(&self, size: Option<i64>, py: Python<'_>) -> PyResult<PyImage> {
        let size = filter_size_from_python(size.unwrap_or(3), false)?;
        let rs = py
            .detach(|| self.inner.median_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (radius=None))]
    fn box_blur(&self, radius: Option<&Bound<'_, PyAny>>, py: Python<'_>) -> PyResult<PyImage> {
        let (radius_x, radius_y) = blur_radius_pair_from_python(radius, 2.0)?;
        let rs = py
            .detach(|| self.inner.box_blur_xy(radius_x as f32, radius_y as f32))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn mode_filter(&self, size: Option<i64>, py: Python<'_>) -> PyResult<PyImage> {
        let size = filter_size_from_python(size.unwrap_or(3), true)?;
        let rs = py
            .detach(|| self.inner.mode_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn rank_filter(
        &self,
        size: Option<i64>,
        rank: Option<i64>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let size = filter_size_from_python(size.unwrap_or(3), false)?;
        let rank = filter_rank_from_python(rank.unwrap_or(0))?;
        let rs = py
            .detach(|| self.inner.rank_filter(size, rank))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn color3dlut(
        &self,
        size: (u32, u32, u32),
        table: Vec<f64>,
        channels: Option<u32>,
        target_mode: Option<&str>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let input =
            pillow_rs::prepare_color3dlut(table, size, channels.unwrap_or(3)).map_err(map_error)?;
        let target_mode = target_mode.map(str::to_owned);
        let rs = py
            .detach(|| self.inner.color3dlut(input, target_mode.as_deref()))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getchannel(&mut self, channel: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let selector = if let Ok(channel) = channel.extract::<i32>() {
            pillow_rs::ChannelSelector::Index(channel)
        } else if let Ok(channel) = channel.extract::<String>() {
            pillow_rs::ChannelSelector::Name(channel)
        } else {
            pillow_rs::ChannelSelector::Invalid(channel.get_type().name()?.to_string())
        };
        let rs = py
            .detach(|| self.inner.getchannel_selector(selector))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn putalpha_input(&mut self, alpha: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<()> {
        let input = if let Some(mask) = image_from_python(alpha) {
            pillow_rs::PutAlphaInput::Image(Box::new(mask))
        } else if let Ok(value) = alpha.extract::<i64>() {
            pillow_rs::PutAlphaInput::Integer(value)
        } else {
            pillow_rs::PutAlphaInput::Invalid(alpha.get_type().name()?.to_string())
        };
        py.detach(|| self.inner.putalpha_with_input(input))
            .map_err(map_error)
    }

    fn reduce(
        &self,
        factor: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let factor = reduce_factor_from_python(factor)?;
        let box_coords = box_coords.map(reduce_box_from_python).transpose()?;
        let rs = py
            .detach(|| self.inner.reduce_public(factor, box_coords))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (im, dest=None, source=None))]
    fn alpha_composite(
        &mut self,
        im: &Bound<'_, PyImage>,
        dest: Option<&Bound<'_, PyAny>>,
        source: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let source_image = im.borrow().inner.clone();
        let dest = dest.map_or_else(
            || pillow_rs::AlphaCompositeBox::Values(vec![0, 0]),
            |value| {
                value.extract::<Vec<i64>>().map_or(
                    pillow_rs::AlphaCompositeBox::Invalid,
                    pillow_rs::AlphaCompositeBox::Values,
                )
            },
        );
        let source_box = source.map_or_else(
            || pillow_rs::AlphaCompositeBox::Values(vec![0, 0]),
            |value| {
                value.extract::<Vec<i64>>().map_or(
                    pillow_rs::AlphaCompositeBox::Invalid,
                    pillow_rs::AlphaCompositeBox::Values,
                )
            },
        );
        py.detach(|| {
            self.inner
                .alpha_composite_public(&source_image, dest, source_box)
        })
        .map_err(map_error)
    }

    /// Applies a sequence or callable LUT through the Rust-owned public path.
    #[pyo3(signature = (input, mode=None))]
    fn point(
        &self,
        input: &Bound<'_, PyAny>,
        mode: Option<&str>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let input_kind = if input.is_instance_of::<PyString>() {
            pillow_rs::EvalInputKind::String
        } else {
            pillow_rs::EvalInputKind::Other
        };
        pillow_rs::validate_eval_input(input_kind).map_err(map_error)?;
        if mode == Some("F") {
            let source_mode = self.inner.mode().map_err(map_error)?;
            if matches!(source_mode.as_str(), "1" | "L" | "P") {
                let lut = if input.is_callable() {
                    let mut values = Vec::with_capacity(256);
                    for sample in 0..256u32 {
                        values.push(input.call1((sample,))?.extract::<f64>()?);
                    }
                    values
                } else {
                    input.extract::<Vec<f64>>()?
                };
                return py
                    .detach(|| pillow_rs::image_eval_float(&self.inner, &lut))
                    .map(|i| PyImage { inner: i })
                    .map_err(map_error);
            }
        }
        if input.is_callable() {
            let mode = self.inner.mode().map_err(map_error)?;
            if matches!(
                mode.as_str(),
                "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" | "F"
            ) {
                let (scale, offset) = point_transform_from_python(input, py)?;
                return py
                    .detach(|| pillow_rs::image_eval_point_transform(&self.inner, scale, offset))
                    .map(|i| PyImage { inner: i })
                    .map_err(map_error);
            }
            return pillow_rs::image_eval_callable(&self.inner, |sample| {
                let result = input.call1((sample,)).map_err(|error| {
                    pillow_rs::PilError::ValueError(format!("LUT function failed: {error}"))
                })?;
                let rounded = result.call_method0("__round__").map_err(|error| {
                    pillow_rs::PilError::ValueError(format!("LUT function failed: {error}"))
                })?;
                rounded.extract::<i32>().map_err(|_| {
                    pillow_rs::PilError::ValueError(
                        "LUT function must return an integer".to_owned(),
                    )
                })
            })
            .map(|i| PyImage { inner: i })
            .map_err(map_error);
        }

        let lut = input.extract::<Vec<u8>>()?;
        py.detach(|| pillow_rs::image_eval_validated(&self.inner, &lut))
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    fn effect_spread(&self, distance: u32, py: Python<'_>) -> PyResult<PyImage> {
        py.detach(|| pillow_rs::image_effect_spread(&self.inner, distance))
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    #[pyo3(signature = (size, method, data=None, resample=0, fill=1, fillcolor=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow Image.transform API.
    fn transform(
        &self,
        size: (u32, u32),
        method: i32,
        data: Option<&Bound<'_, PyAny>>,
        resample: Option<i32>,
        fill: Option<i32>,
        fillcolor: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let data = transform_data_from_python(data)?;
        let mode = self.inner.mode().map_err(map_error)?;
        let fillcolor = transform_fill_from_python(fillcolor, &mode)?;
        let resample = resample.unwrap_or(0);
        let fill = fill.unwrap_or(1);
        py.detach(|| {
            self.inner
                .transform_public(size, method, data, resample, fill, fillcolor)
        })
        .map(|i| PyImage { inner: i })
        .map_err(map_error)
    }

    #[pyo3(signature = (dest_map, source_palette=None))]
    fn remap_palette(
        &mut self,
        dest_map: Vec<u8>,
        source_palette: Option<Vec<u8>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let remapped = py
            .detach(|| match source_palette.as_deref() {
                None => self.inner.remap_palette(&dest_map),
                Some(source_palette) => self
                    .inner
                    .remap_palette_with_source(&dest_map, Some(source_palette)),
            })
            .map(|i| PyImage { inner: i })
            .map_err(map_error)?;
        Ok(remapped)
    }

    fn tobitmap(&mut self, py: Python<'_>) -> PyResult<Vec<u8>> {
        py.detach(|| self.inner.tobitmap()).map_err(map_error)
    }

    fn enhance_brightness(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::attach(|py| py.detach(|| inner.enhance_brightness(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn contrast_degenerate(&self, py: Python<'_>) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = py
            .detach(|| inner.contrast_degenerate())
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_contrast(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs =
            Python::attach(|py| py.detach(|| inner.enhance_contrast(factor))).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_color(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs =
            Python::attach(|py| py.detach(|| inner.enhance_color(factor))).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn color_degenerate(&self, py: Python<'_>) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = py.detach(|| inner.color_degenerate()).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn color_transparency(&self, value: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        // Color's metadata normalizer accepts component lists and converts
        // invalid input into a warning; public putpixel has stricter errors.
        let value = if let Ok(value) = value.extract::<i64>() {
            pillow_rs::PutPixelValue::Integer(value)
        } else if let Ok(value) = value.extract::<Vec<i64>>() {
            pillow_rs::PutPixelValue::Components(value)
        } else if let Ok(value) = value.extract::<Vec<f64>>() {
            pillow_rs::PutPixelValue::FloatComponents(value)
        } else if let Ok(value) = value.extract::<f64>() {
            pillow_rs::PutPixelValue::Float(value)
        } else {
            pillow_rs::PutPixelValue::Invalid
        };
        let value = self.inner.color_transparency(value).map_err(map_error)?;
        image_info_value_to_python(py, value)
    }

    fn enhance_sharpness(&self, factor: f64) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = Python::attach(|py| py.detach(|| inner.enhance_sharpness(factor)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }
}
