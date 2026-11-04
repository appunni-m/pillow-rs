use super::RsImage;
use super::arrays::image_info_value_to_python;
use super::point::point_transform_from_python;
use crate::*;
use pyo3::types::PyModuleMethods;
use pyo3::{pyclass, pyfunction, pymethods};

#[pyclass(name = "_RotatePlan", frozen)]
pub(super) struct PyRotatePlan {
    prepared: pillow_rs::PreparedRotate,
}

#[pyclass(name = "_PasteBoxPlan", frozen)]
pub(super) struct PyPasteBoxPlan {
    prepared: pillow_rs::PasteBoxPlan,
    pending_coordinates: Option<DeferredPasteBoxCoordinates>,
    has_mask: bool,
    position_size_available: bool,
}

#[pyclass(name = "_PasteSourcePlan", frozen)]
pub(super) struct PyPasteSourcePlan {
    prepared: Option<pillow_rs::PasteSourcePlan>,
    deferred_source: Option<pyo3::Py<PyAny>>,
}

#[pyclass(name = "_ThumbnailPlan", frozen)]
pub(super) struct PyThumbnailPlan {
    prepared: pillow_rs::ThumbnailPlan,
    #[pyo3(get)]
    output_size: (u32, u32),
}

#[pyfunction(name = "_thumbnail_filter_code")]
pub(super) fn thumbnail_filter_code(
    resample: Option<&Bound<'_, PyAny>>,
) -> PyResult<i32> {
    let input = resample_input_from_python(resample)?;
    let filter = pillow_rs::parse_resample_input(input).map_err(map_error)?;
    Ok(resample_filter_code(filter))
}

#[pyfunction(name = "_thumbnail_reducing_gap")]
pub(super) fn thumbnail_reducing_gap(
    value: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<f64>> {
    value.map(|value| value.extract::<f64>()).transpose()
}

pub(super) fn register_adapter_functions(m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(thumbnail_filter_code, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(thumbnail_reducing_gap, m)?)?;
    Ok(())
}

fn rotate_input_from_python(
    resample: Option<&Bound<'_, PyAny>>,
    expand: Option<&Bound<'_, PyAny>>,
    center: Option<&Bound<'_, PyAny>>,
    translate: Option<&Bound<'_, PyAny>>,
    fillcolor: Option<&Bound<'_, PyAny>>,
) -> PyResult<(
    pillow_rs::RotatePointInput,
    pillow_rs::RotatePointInput,
    pillow_rs::RotateExpandInput,
    pillow_rs::RotateResampleInput,
    pillow_rs::ImageOpsColor,
)> {
    let center = rotate_point_input_from_python(center)?;
    let translate = rotate_point_input_from_python(translate)?;
    let expand = rotate_expand_input_from_python(expand)?;
    let resample = rotate_resample_input_from_python(resample)?;
    let fillcolor = imageops_color_from_python(fillcolor);
    Ok((center, translate, expand, resample, fillcolor))
}

#[pymethods]
impl PyImage {
    fn _resize_filter_code(
        slf: Py<Self>,
        resample: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<i32> {
        let _keep_self_alive = slf;
        let input = resample_input_from_python(resample)?;
        let filter = pillow_rs::parse_resample_input(input).map_err(map_error)?;
        Ok(resample_filter_code(filter))
    }

    #[pyo3(signature = (size, resample=None, box_coords=None, reducing_gap=None))]
    fn resize(
        slf: Py<Self>,
        size: &Bound<'_, PyAny>,
        resample: Option<&Bound<'_, PyAny>>,
        box_coords: Option<&Bound<'_, PyAny>>,
        reducing_gap: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let size = size.extract::<(i64, i64)>()?;
        let box_coords = box_coords
            .map(|value| value.extract::<(f64, f64, f64, f64)>())
            .transpose()?;
        let reducing_gap = reducing_gap.map(|value| value.extract::<f64>()).transpose()?;
        let resample = resample_input_from_python(resample)?;
        let image = slf.bind(py).borrow();
        let rs = py
            .detach(|| {
                image
                    .inner
                    .resize_with_options(size, resample, box_coords, reducing_gap)
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn crop(
        slf: Py<Self>,
        box_coords: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let box_coords = box_coords
            .map(|value| value.extract::<(f64, f64, f64, f64)>())
            .transpose()?;
        let image = slf.bind(py).borrow();
        let rs = py
            .detach(|| image.inner.crop_float(box_coords))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (angle, resample=None, expand=None, center=None, translate=None, fillcolor=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow Image.rotate API.
    fn rotate(
        slf: Py<Self>,
        angle: &Bound<'_, PyAny>,
        resample: Option<&Bound<'_, PyAny>>,
        expand: Option<&Bound<'_, PyAny>>,
        center: Option<&Bound<'_, PyAny>>,
        translate: Option<&Bound<'_, PyAny>>,
        fillcolor: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let angle = angle.extract::<f64>()?;
        let (center, translate, expand, resample, fillcolor) =
            rotate_input_from_python(resample, expand, center, translate, fillcolor)?;
        let image = slf.bind(py).borrow();
        let rs = py
            .detach(|| {
                image
                    .inner
                    .rotate_with_input(angle, resample, expand, center, translate, fillcolor)
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (angle, resample=None, expand=None, center=None, translate=None, fillcolor=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the public Pillow Image.rotate API.
    fn _prepare_rotate(
        slf: Py<Self>,
        angle: &Bound<'_, PyAny>,
        resample: Option<&Bound<'_, PyAny>>,
        expand: Option<&Bound<'_, PyAny>>,
        center: Option<&Bound<'_, PyAny>>,
        translate: Option<&Bound<'_, PyAny>>,
        fillcolor: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyRotatePlan> {
        let angle = angle.extract::<f64>()?;
        let (center, translate, expand, resample, fillcolor) =
            rotate_input_from_python(resample, expand, center, translate, fillcolor)?;
        let image = slf.bind(py).borrow();
        let prepared = image
            .inner
            .prepare_rotate_with_input(angle, resample, expand, center, translate, fillcolor)
            .map_err(map_error)?;
        Ok(PyRotatePlan { prepared })
    }

    fn _rotate_prepared(
        &self,
        plan: pyo3::PyRef<'_, PyRotatePlan>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let prepared = plan.prepared;
        let inner = py
            .detach(|| self.inner.rotate_prepared(prepared))
            .map_err(map_error)?;
        Ok(PyImage { inner })
    }

    fn transpose(
        slf: Py<Self>,
        method: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let input = transpose_input_from_python(method)?;
        let image = slf.bind(py).borrow();
        // Ordinary transpose only queues metadata. Paletted source cloning
        // copies its index buffer, so keep that work outside the GIL.
        let rs = if matches!(&image.inner, RsImage::Paletted(_)) {
            py.detach(|| image.inner.transpose_with_input(input))
        } else {
            image.inner.transpose_with_input(input)
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
            None => pillow_rs::DitherInput::None,
            Some(value) => {
                if let Ok(int_value) = value.extract::<u32>() {
                    pillow_rs::DitherInput::Integer(int_value)
                } else if let Ok(string_value) = value.extract::<String>() {
                    pillow_rs::DitherInput::Name(string_value)
                } else {
                    pillow_rs::DitherInput::Invalid(value.get_type().name()?.to_string())
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

    #[pyo3(signature = (box_coords, has_mask, position_size_available, known_length=None))]
    fn _paste_prepare_box(
        slf: Py<Self>,
        box_coords: Option<&Bound<'_, PyAny>>,
        has_mask: bool,
        position_size_available: bool,
        known_length: Option<usize>,
    ) -> PyResult<PyPasteBoxPlan> {
        let _keep_self_alive = slf;
        let (box_input, pending_coordinates) =
            paste_box_for_staged_plan(box_coords, known_length)?;
        let prepared = pillow_rs::Image::prepare_paste_box(
            box_input,
            has_mask,
            position_size_available,
        )
        .map_err(map_error)?;
        Ok(PyPasteBoxPlan {
            prepared,
            pending_coordinates,
            has_mask,
            position_size_available,
        })
    }

    fn _paste_prepare_source(
        slf: Py<Self>,
        source: &Bound<'_, PyAny>,
        is_image_source: bool,
    ) -> PyResult<PyPasteSourcePlan> {
        let py = source.py();
        let mode = {
            let image = slf.bind(py).borrow();
            image.inner.mode_identity().map_err(map_error)?
        };
        let source_input = if is_image_source {
            paste_source_from_python(source, mode)?
        } else if source.cast::<PyString>().is_ok() {
            pillow_rs::PasteSourceInput::String(source.extract::<String>()?)
        } else {
            return Ok(PyPasteSourcePlan {
                prepared: None,
                deferred_source: Some(source.clone().unbind()),
            });
        };
        let image = slf.bind(py).borrow();
        let prepared = image.inner.prepare_paste_source(source_input).map_err(map_error)?;
        Ok(PyPasteSourcePlan {
            prepared: Some(prepared),
            deferred_source: None,
        })
    }

    fn _paste_prepare_resolved_image_source(
        &self,
        source: &Bound<'_, PyImage>,
    ) -> PyResult<PyPasteSourcePlan> {
        let source = source.borrow();
        let prepared = self
            .inner
            .prepare_resolved_paste_image_source(source.inner.clone())
            .map_err(map_error)?;
        Ok(PyPasteSourcePlan {
            prepared: Some(prepared),
            deferred_source: None,
        })
    }

    #[pyo3(signature = (box_plan, source_plan, mask=None))]
    fn _paste_apply_prepared(
        slf: pyo3::Py<Self>,
        box_plan: pyo3::PyRef<'_, PyPasteBoxPlan>,
        source_plan: pyo3::PyRef<'_, PyPasteSourcePlan>,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let box_plan = match &box_plan.pending_coordinates {
            Some(coordinates) => {
                let coordinates = resolve_deferred_paste_box_coordinates(coordinates, py)?;
                pillow_rs::Image::prepare_paste_box(
                    pillow_rs::PasteBoxInput::Values(coordinates),
                    box_plan.has_mask,
                    box_plan.position_size_available,
                )
                .map_err(map_error)?
            }
            None => box_plan.prepared.clone(),
        };
        let source_plan = match &source_plan.prepared {
            Some(prepared) => prepared.clone(),
            None => {
                let source = source_plan.deferred_source.as_ref().ok_or_else(|| {
                    PyTypeError::new_err("prepared Paste source was missing")
                })?;
                let source = source.bind(py);
                let mode = slf
                    .bind(py)
                    .borrow()
                    .inner
                    .mode_identity()
                    .map_err(map_error)?;
                let input = paste_source_from_python(&source, mode)?;
                pillow_rs::Image::prepare_paste_source_for_mode(mode, input)
                    .map_err(map_error)?
            }
        };
        let mask = paste_mask_from_python(mask)?;
        let mut destination = slf.bind(py).borrow_mut();
        let destination = &mut destination.inner;
        py.detach(|| destination.paste_prepared(box_plan, source_plan, mask))
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
        if !filter.hasattr("_apply")? {
            return Err(PyTypeError::new_err(
                "filter argument should be ImageFilter.Filter instance or class",
            ));
        }
        let type_name = filter.get_type().name()?.to_string();
        let validation_name = filter
            .getattr("name")
            .ok()
            .and_then(|value| value.extract::<String>().ok())
            .unwrap_or_else(|| type_name.clone());
        self.inner
            .validate_filter(&validation_name)
            .map_err(map_error)?;
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

    fn _thumbnail_preflight(
        &self,
        size: (i64, i64),
    ) -> PyResult<Option<PyThumbnailPlan>> {
        let prepared = self.inner.prepare_thumbnail(size).map_err(map_error)?;
        Ok(prepared.map(|prepared| PyThumbnailPlan {
            output_size: prepared.output_size(),
            prepared,
        }))
    }

    fn _thumbnail_apply_prepared(
        &mut self,
        plan: pyo3::PyRef<'_, PyThumbnailPlan>,
        filter_code: i32,
        reducing_gap: Option<f64>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let prepared = plan.prepared;
        py.detach(|| {
            self.inner.thumbnail_with_plan(
                prepared,
                Some(pillow_rs::ResampleInput::Code(i64::from(filter_code))),
                reducing_gap,
            )
        })
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

    #[pyo3(signature = (size=3))]
    fn max_filter(&self, size: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let size = filter_size_from_python(size, false)?;
        let rs = py
            .detach(|| self.inner.max_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (size=3))]
    fn min_filter(&self, size: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let size = filter_size_from_python(size, false)?;
        let rs = py
            .detach(|| self.inner.min_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (size=3))]
    fn median_filter(&self, size: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let size = filter_size_from_python(size, false)?;
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

    #[pyo3(signature = (size=3))]
    fn mode_filter(&self, size: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let size = filter_size_from_python(size, true)?;
        let rs = py
            .detach(|| self.inner.mode_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (size=3, rank=0))]
    fn rank_filter(
        &self,
        size: &Bound<'_, PyAny>,
        rank: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let size = filter_size_from_python(size, false)?;
        let rank = filter_rank_from_python(rank)?;
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
        let target_mode = target_mode
            .map(|name| {
                pillow_rs::Mode::from_pillow_name(name).ok_or_else(|| {
                    map_error(pillow_rs::PilError::ValueError(
                        "unrecognized image mode".into(),
                    ))
                })
            })
            .transpose()?;
        let rs = py
            .detach(|| self.inner.color3dlut(input, target_mode))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getchannel(
        slf: Py<Self>,
        channel: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let selector = if let Ok(channel) = channel.extract::<i32>() {
            pillow_rs::ChannelSelector::Index(channel)
        } else if let Ok(channel) = channel.extract::<String>() {
            pillow_rs::ChannelSelector::Name(channel)
        } else {
            pillow_rs::ChannelSelector::Invalid(channel.get_type().name()?.to_string())
        };
        let image = slf.bind(py).borrow();
        let image = &image.inner;
        let rs = py
            .detach(|| image.getchannel_selector(selector))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn putalpha_input(
        slf: Py<Self>,
        alpha: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let input = if let Some(mask) = image_from_python(alpha) {
            pillow_rs::PutAlphaInput::Image(Box::new(mask))
        } else if let Ok(value) = alpha.extract::<i64>() {
            pillow_rs::PutAlphaInput::Integer(value)
        } else if alpha.is_instance_of::<PyInt>() {
            // Clamp arbitrary-size Python integers in the core's byte domain
            // without narrowing or converting through floating point.
            let value = if alpha.lt(0)? { i64::MIN } else { i64::MAX };
            pillow_rs::PutAlphaInput::Integer(value)
        } else {
            pillow_rs::PutAlphaInput::Invalid(alpha.get_type().name()?.to_string())
        };
        // `__index__` conversion above may re-enter this image; borrow it only
        // once host-language coercion has finished.
        let mut image = slf.bind(py).borrow_mut();
        let image = &mut image.inner;
        py.detach(|| image.putalpha_with_input(input))
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
        slf: Py<Self>,
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
        // Sequence conversion above may execute Python code that re-enters
        // this image. Acquire its mutable borrow only for the core write.
        let mut image = slf.bind(py).borrow_mut();
        let image = &mut image.inner;
        py.detach(|| {
            image.alpha_composite_public(&source_image, dest, source_box)
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
        let target_mode = mode.and_then(pillow_rs::Mode::from_pillow_name);
        if target_mode == Some(pillow_rs::Mode::ImageF32) {
            let source_mode = self.inner.mode_identity().map_err(map_error)?;
            if matches!(
                source_mode,
                pillow_rs::Mode::One
                    | pillow_rs::Mode::ImageLuma8
                    | pillow_rs::Mode::ImagePalette8
            ) {
                let lut = if input.is_callable() {
                    pillow_rs::image_eval_float_lut(|sample| {
                        input.call1((sample,))?.extract::<f64>()
                    })?
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
            let source_mode = self.inner.mode_identity().map_err(map_error)?;
            if matches!(
                source_mode,
                pillow_rs::Mode::ImageI32
                    | pillow_rs::Mode::ImageI16
                    | pillow_rs::Mode::ImageI16L
                    | pillow_rs::Mode::ImageI16B
                    | pillow_rs::Mode::ImageI16N
                    | pillow_rs::Mode::ImageF32
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
        let mode = self.inner.mode_identity().map_err(map_error)?;
        let fillcolor = transform_fill_from_python(fillcolor, mode)?;
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
        slf: Py<Self>,
        dest_map: &Bound<'_, PyAny>,
        source_palette: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        // These sequence extractions can call user-defined iteration methods.
        let dest_map = dest_map.extract::<Vec<u8>>()?;
        let source_palette = source_palette
            .map(|palette| palette.extract::<Vec<u8>>())
            .transpose()?;
        let image = slf.bind(py).borrow();
        let image = &image.inner;
        let remapped = py
            .detach(|| match source_palette.as_deref() {
                None => image.remap_palette(&dest_map),
                Some(source_palette) => {
                    image.remap_palette_with_source(&dest_map, Some(source_palette))
                }
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
