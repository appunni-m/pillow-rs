use super::RsImage;
use super::arrays::image_info_value_to_python;
use super::point::point_transform_from_python;
use crate::*;
use pyo3::types::PyModuleMethods;
use pyo3::{pyclass, pyfunction, pymethods};

#[pyclass(name = "_RotatePlan", frozen)]
pub(super) struct PyRotatePlan {
    prepared: pillow_rs::PreparedRotate,
    output_size: (u32, u32),
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
pub(super) fn thumbnail_filter_code(resample: Option<&Bound<'_, PyAny>>) -> PyResult<i32> {
    let input = resample_input_from_python(resample)?;
    let filter = pillow_rs::parse_resample_input(input).map_err(map_error)?;
    Ok(resample_filter_code(filter))
}

#[pyfunction(name = "_imageops_filter_code")]
pub(super) fn imageops_filter_code(method: Option<&Bound<'_, PyAny>>) -> PyResult<i32> {
    let input = resample_input_from_python(method)?;
    let filter = pillow_rs::parse_imageops_filter_input(input).map_err(map_error)?;
    Ok(resample_filter_code(filter))
}

#[pyfunction(name = "_thumbnail_reducing_gap")]
pub(super) fn thumbnail_reducing_gap(value: Option<&Bound<'_, PyAny>>) -> PyResult<Option<f64>> {
    value.map(|value| value.extract::<f64>()).transpose()
}

pub(super) fn register_adapter_functions(m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(thumbnail_filter_code, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(imageops_filter_code, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(thumbnail_reducing_gap, m)?)?;
    Ok(())
}

fn point_lut_from_python(input: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    pillow_rs::make_lut(1, |sample| {
        let result = input.call1((sample,)).map_err(|error| {
            pillow_rs::PilError::ValueError(format!("LUT function failed: {error}"))
        })?;
        let rounded = result.call_method0("__round__").map_err(|error| {
            pillow_rs::PilError::ValueError(format!("LUT function failed: {error}"))
        })?;
        rounded.extract::<i32>().map_err(|_| {
            pillow_rs::PilError::ValueError("LUT function must return an integer".to_owned())
        })
    })
    .map_err(map_error)
}

fn point_float_lut_from_python(input: &Bound<'_, PyAny>) -> PyResult<Vec<f64>> {
    pillow_rs::image_eval_float_lut(|sample| input.call1((sample,))?.extract::<f64>())
}

#[pymethods]
impl PyImage {
    fn _resize_filter_code(slf: Py<Self>, resample: Option<&Bound<'_, PyAny>>) -> PyResult<i32> {
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
        let reducing_gap = reducing_gap
            .map(|value| value.extract::<f64>())
            .transpose()?;
        let resample = resample_input_from_python(resample)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.resize_with_options(size, resample, box_coords, reducing_gap))
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
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.crop_float(box_coords))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (angle, output_size, resample=None, expand=false, fillcolor=None))]
    #[allow(clippy::too_many_arguments)] // Mirrors the internal Pillow Rotate boundary.
    fn _prepare_rotate(
        slf: Py<Self>,
        angle: f64,
        output_size: &Bound<'_, PyAny>,
        resample: Option<&Bound<'_, PyAny>>,
        expand: bool,
        fillcolor: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyRotatePlan> {
        let mode = slf
            .bind(py)
            .borrow()
            .inner
            .mode_identity()
            .map_err(map_error)?;
        let (width, height) = nonnegative_output_size_items_from_python(output_size)?;
        let (pre_size_fill, deferred_fill) =
            rotate_fill_preflight_from_python(py, fillcolor, mode)?;
        let deferred_fill = deferred_fill.as_ref().map(|fill| fill.bind(py));
        let prepare_fill = |value| {
            let fillcolor = rotate_fill_from_python(value, mode)?;
            slf.bind(py)
                .borrow()
                .inner
                .prepare_rotate_fill_with_input(fillcolor)
                .map_err(map_error)
        };
        // Pillow classifies P sequences and resolves named/all-integer RGB(A)
        // colors before `_fill`; remaining color coercion follows C-int sizes.
        let pre_size_prepared = pre_size_fill
            .map(|fill| {
                slf.bind(py)
                    .borrow()
                    .inner
                    .prepare_rotate_fill_with_input(fill)
                    .map_err(map_error)
            })
            .transpose()?;
        let width = c_int_from_python(&width)?;
        let height = c_int_from_python(&height)?;
        let output_size = (width as u32, height as u32);
        let prepared = match pre_size_prepared {
            Some(prepared) => prepared,
            None => prepare_fill(deferred_fill.or(fillcolor))?,
        };
        let resample = rotate_resample_input_from_python(resample)?;
        let prepared = prepared
            .with_python_resample(angle, resample, expand)
            .map_err(map_error)?;
        Ok(PyRotatePlan {
            prepared,
            output_size,
        })
    }

    fn _rotate_prepared(
        slf: Py<Self>,
        plan: pyo3::PyRef<'_, PyRotatePlan>,
        affine: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        // Rotate supplies a built-in six-tuple. Parse its coefficients here,
        // after the facade has loaded the source: Pillow's `_imaging.getlist`
        // applies `PyFloat_AsDouble` to each value at this boundary, so custom
        // coefficient `__float__` callbacks keep the same place in the flow.
        let affine = affine.extract::<(f64, f64, f64, f64, f64, f64)>()?;
        let matrix = [affine.0, affine.1, affine.2, affine.3, affine.4, affine.5];
        let prepared = plan.prepared.clone().with_affine(matrix, plan.output_size);
        let inner = slf.bind(py).borrow().inner.clone();
        let inner = py
            .detach(move || inner.rotate_prepared(prepared))
            .map_err(map_error)?;
        Ok(PyImage { inner })
    }

    fn transpose(slf: Py<Self>, method: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let input = transpose_input_from_python(method)?;
        let inner = slf.bind(py).borrow().inner.clone();
        // Ordinary transpose only queues metadata. Paletted source cloning
        // copies its index buffer, so keep that work outside the GIL.
        let rs = if matches!(&inner, RsImage::Paletted(_)) {
            py.detach(move || inner.transpose_with_input(input))
        } else {
            inner.transpose_with_input(input)
        }
        .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (mode=None, matrix=None, dither=None, palette=None, colors=None))]
    fn convert(
        slf: Py<Self>,
        mode: Option<&Bound<'_, PyAny>>,
        matrix: Option<&Bound<'_, PyAny>>,
        dither: Option<&Bound<'_, PyAny>>,
        palette: Option<&Bound<'_, PyAny>>,
        colors: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let matrix = matrix
            .map(|value| value.extract::<Vec<f64>>())
            .transpose()?;
        let colors = colors.map(|value| value.extract::<u32>()).transpose()?;
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
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.convert_with_input(mode, matrix, dither, palette, colors))
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
        let (box_input, pending_coordinates) = paste_box_for_staged_plan(box_coords, known_length)?;
        let prepared =
            pillow_rs::Image::prepare_paste_box(box_input, has_mask, position_size_available)
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
        let prepared = image
            .inner
            .prepare_paste_source(source_input)
            .map_err(map_error)?;
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
                let source = source_plan
                    .deferred_source
                    .as_ref()
                    .ok_or_else(|| PyTypeError::new_err("prepared Paste source was missing"))?;
                let source = source.bind(py);
                let mode = slf
                    .bind(py)
                    .borrow()
                    .inner
                    .mode_identity()
                    .map_err(map_error)?;
                let input = paste_source_from_python(source, mode)?;
                pillow_rs::Image::prepare_paste_source_for_mode(mode, input).map_err(map_error)?
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

    fn getband(&self, band: &Bound<'_, PyAny>) -> PyResult<PyImage> {
        let band = c_int_from_python(band)?;
        let inner = self.inner.getchannel(band).map_err(map_error)?;
        Ok(PyImage { inner })
    }

    #[pyo3(signature = (size, scale, offset, kernel))]
    fn filter(
        slf: Py<Self>,
        size: (u32, u32),
        scale: Option<f64>,
        offset: f64,
        kernel: Option<Vec<f64>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let inner = slf.bind(py).borrow().inner.clone();
        let result = py
            .detach(move || inner.kernel_filter(kernel, scale, offset, size))
            .map_err(map_error)?;
        Ok(PyImage { inner: result })
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

    fn preflight_enhance_sharpness(&self) -> PyResult<()> {
        self.inner.preflight_enhance_sharpness().map_err(map_error)
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
        slf: Py<Self>,
        size: (i64, i64),
        py: Python<'_>,
    ) -> PyResult<Option<PyThumbnailPlan>> {
        let image = slf.bind(py).borrow();
        let prepared = image.inner.prepare_thumbnail(size).map_err(map_error)?;
        Ok(prepared.map(|prepared| PyThumbnailPlan {
            output_size: prepared.output_size(),
            prepared,
        }))
    }

    fn _thumbnail_apply_prepared(
        slf: Py<Self>,
        plan: pyo3::PyRef<'_, PyThumbnailPlan>,
        filter_code: i32,
        reducing_gap: Option<f64>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let prepared = plan.prepared;
        let inner = slf.bind(py).borrow().inner.clone();
        let (result, inner) = py.detach(move || {
            let mut inner = inner;
            let result = inner.thumbnail_with_plan(
                prepared,
                Some(pillow_rs::ResampleInput::Code(i64::from(filter_code))),
                reducing_gap,
            );
            (result, inner)
        });
        slf.bind(py).borrow_mut().inner = inner;
        result.map_err(map_error)
    }

    #[pyo3(signature = (colors=None, method=None, kmeans=None, dither=None, palette=None, visible_mode=None))]
    fn quantize(
        slf: Py<Self>,
        colors: Option<i32>,
        method: Option<i32>,
        kmeans: Option<i32>,
        dither: Option<i32>,
        palette: Option<&Bound<'_, PyAny>>,
        visible_mode: Option<&str>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let palette = match palette {
            None => pillow_rs::QuantizePalette::None,
            Some(value) => image_from_python(value)
                .map_or(pillow_rs::QuantizePalette::Other, |image| {
                    pillow_rs::QuantizePalette::Image(Box::new(image))
                }),
        };
        let visible_mode = visible_mode.and_then(pillow_rs::Mode::from_pillow_name);
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || {
                inner.quantize_with_visible_mode(
                    colors,
                    method,
                    kmeans,
                    palette,
                    dither.map(|value| value != 0),
                    visible_mode,
                )
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (radius=None))]
    fn gaussian_blur(
        slf: Py<Self>,
        radius: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let (radius_x, radius_y) = blur_radius_pair_from_python(radius, 2.0)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(|| {
                if radius_x == radius_y {
                    inner.gaussian_blur(radius_x as f32)
                } else {
                    inner.gaussian_blur_xy(radius_x as f32, radius_y as f32)
                }
            })
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn unsharp_mask(
        slf: Py<Self>,
        radius: Option<f64>,
        percent: Option<i32>,
        threshold: Option<i32>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let radius = radius.unwrap_or(2.0) as f32;
        let percent = percent.unwrap_or(150);
        let threshold = threshold.unwrap_or(3);
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.unsharp_mask(radius, percent, threshold))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (size=3))]
    fn max_filter(slf: Py<Self>, size: i32, py: Python<'_>) -> PyResult<PyImage> {
        let size = PyInt::new(py, size);
        let size = filter_size_from_python(size.as_any(), false)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.max_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (size=3))]
    fn min_filter(slf: Py<Self>, size: i32, py: Python<'_>) -> PyResult<PyImage> {
        let size = PyInt::new(py, size);
        let size = filter_size_from_python(size.as_any(), false)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.min_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (size=3))]
    fn median_filter(slf: Py<Self>, size: i32, py: Python<'_>) -> PyResult<PyImage> {
        let size = PyInt::new(py, size);
        let size = filter_size_from_python(size.as_any(), false)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.median_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (radius=None))]
    fn box_blur(
        slf: Py<Self>,
        radius: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let (radius_x, radius_y) = blur_radius_pair_from_python(radius, 2.0)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.box_blur_xy(radius_x as f32, radius_y as f32))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (size=3))]
    fn modefilter(slf: Py<Self>, size: i32, py: Python<'_>) -> PyResult<PyImage> {
        let size = PyInt::new(py, size);
        let size = filter_size_from_python(size.as_any(), true)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.mode_filter(size))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    #[pyo3(signature = (size=3, rank=0))]
    fn rank_filter(slf: Py<Self>, size: i32, rank: i32, py: Python<'_>) -> PyResult<PyImage> {
        let size = PyInt::new(py, size);
        let size = filter_size_from_python(size.as_any(), false)?;
        let rank = PyInt::new(py, rank);
        let rank = filter_rank_from_python(rank.as_any())?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.rank_filter(size, rank))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn expand(slf: Py<Self>, xmargin: i32, ymargin: i32, py: Python<'_>) -> PyResult<PyImage> {
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.imaging_core_expand(xmargin, ymargin))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn rankfilter(slf: Py<Self>, size: i32, rank: i32, py: Python<'_>) -> PyResult<PyImage> {
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.imaging_core_rankfilter(size, rank))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn color_lut_3d(
        slf: Py<Self>,
        mode: &str,
        filter: i32,
        channels: i32,
        size: (i32, i32, i32),
        table: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let target_mode = pillow_rs::Mode::from_pillow_name(mode).ok_or_else(|| {
            map_error(pillow_rs::PilError::ValueError(
                "unrecognized image mode".into(),
            ))
        })?;
        if filter != 2 {
            return Err(map_error(pillow_rs::PilError::ValueError(
                "Only LINEAR filter is supported.".into(),
            )));
        }
        if !(1..=4).contains(&channels) {
            return Err(map_error(pillow_rs::PilError::ValueError(
                "table_channels should be from 1 to 4".into(),
            )));
        }
        if size.0 < 2 || size.0 > 65 || size.1 < 2 || size.1 > 65 || size.2 < 2 || size.2 > 65 {
            return Err(map_error(pillow_rs::PilError::ValueError(
                "Table size in any dimension should be from 2 to 65".into(),
            )));
        }
        let table_length = channels as usize * size.0 as usize * size.1 as usize * size.2 as usize;
        let table = color_lut_table_from_python(table, table_length)?;
        let size = (size.0 as u32, size.1 as u32, size.2 as u32);
        let input = pillow_rs::prepare_color3dlut_imaging_core(table, size, channels as u32)
            .map_err(map_error)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.color3dlut(input, Some(target_mode)))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn getchannel(slf: Py<Self>, channel: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let selector = if channel.cast::<PyString>().is_ok() {
            pillow_rs::ChannelSelector::Name(channel.extract::<String>()?)
        } else {
            pillow_rs::ChannelSelector::Index(crate::inputs::c_int_from_python(channel)?)
        };
        let image = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || image.getchannel_selector(selector))
            .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn reduce(
        slf: Py<Self>,
        factor: &Bound<'_, PyAny>,
        box_coords: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let factor = reduce_factor_from_python(factor)?;
        let box_coords = box_coords.map(reduce_box_from_python).transpose()?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.reduce_public(factor, box_coords))
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
        let inner = slf.bind(py).borrow().inner.clone();
        let (result, inner) = py.detach(move || {
            let mut inner = inner;
            let result = inner.alpha_composite_public(&source_image, dest, source_box);
            (result, inner)
        });
        slf.bind(py).borrow_mut().inner = inner;
        result.map_err(map_error)
    }

    /// Applies a sequence or callable LUT through the Rust-owned public path.
    #[pyo3(signature = (input, mode=None))]
    fn point(
        slf: Py<Self>,
        input: &Bound<'_, PyAny>,
        mode: Option<&str>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        // Pillow loads before invoking a point callable. Keep that order while
        // releasing the PyImage borrow before callbacks can re-enter it.
        let inner = slf.bind(py).borrow().inner.clone();
        let (result, inner) = py.detach(move || {
            let mut inner = inner;
            let result = inner.load();
            (result, inner)
        });
        slf.bind(py).borrow_mut().inner = inner;
        result.map_err(map_error)?;
        let source_mode = slf
            .bind(py)
            .borrow()
            .inner
            .mode_identity()
            .map_err(map_error)?;
        let input_kind = if input.is_instance_of::<PyString>() {
            pillow_rs::EvalInputKind::String
        } else {
            pillow_rs::EvalInputKind::Other
        };
        pillow_rs::validate_eval_input(input_kind).map_err(map_error)?;
        let target_mode = mode.and_then(pillow_rs::Mode::from_pillow_name);
        if target_mode == Some(pillow_rs::Mode::ImageF32) {
            if matches!(
                source_mode,
                pillow_rs::Mode::One | pillow_rs::Mode::ImageLuma8 | pillow_rs::Mode::ImagePalette8
            ) {
                let lut = if input.is_callable() {
                    point_float_lut_from_python(input)?
                } else {
                    input.extract::<Vec<f64>>()?
                };
                let inner = slf.bind(py).borrow().inner.clone();
                return py
                    .detach(move || pillow_rs::image_eval_float(&inner, &lut))
                    .map(|i| PyImage { inner: i })
                    .map_err(map_error);
            }
        }
        if input.is_callable() {
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
                let inner = slf.bind(py).borrow().inner.clone();
                return py
                    .detach(move || pillow_rs::image_eval_point_transform(&inner, scale, offset))
                    .map(|i| PyImage { inner: i })
                    .map_err(map_error);
            }
            let lut = point_lut_from_python(input)?;
            let inner = slf.bind(py).borrow().inner.clone();
            return py
                .detach(move || pillow_rs::image_eval_replicated_for_image(&inner, &lut))
                .map(|i| PyImage { inner: i })
                .map_err(map_error);
        }

        let lut = input.extract::<Vec<u8>>()?;
        let inner = slf.bind(py).borrow().inner.clone();
        py.detach(move || pillow_rs::image_eval_validated(&inner, &lut))
            .map(|i| PyImage { inner: i })
            .map_err(map_error)
    }

    fn effect_spread(slf: Py<Self>, distance: u32, py: Python<'_>) -> PyResult<PyImage> {
        let inner = slf.bind(py).borrow().inner.clone();
        py.detach(move || pillow_rs::image_effect_spread(&inner, distance))
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

    fn enhance_brightness(slf: Py<Self>, factor: f64) -> PyResult<PyImage> {
        let rs = Python::attach(|py| {
            let inner = slf.bind(py).borrow().inner.clone();
            py.detach(|| inner.enhance_brightness(factor))
        })
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

    fn enhance_contrast(slf: Py<Self>, factor: f64) -> PyResult<PyImage> {
        let rs = Python::attach(|py| {
            let inner = slf.bind(py).borrow().inner.clone();
            py.detach(|| inner.enhance_contrast(factor))
        })
        .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn enhance_color(slf: Py<Self>, factor: f64) -> PyResult<PyImage> {
        let rs = Python::attach(|py| {
            let inner = slf.bind(py).borrow().inner.clone();
            py.detach(|| inner.enhance_color(factor))
        })
        .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn color_degenerate(&self, py: Python<'_>) -> PyResult<PyImage> {
        let inner = self.inner.clone();
        let rs = py.detach(|| inner.color_degenerate()).map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }

    fn color_transparency(
        slf: Py<Self>,
        value: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
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
        let image = slf.bind(py).borrow();
        let value = image.inner.color_transparency(value).map_err(map_error)?;
        image_info_value_to_python(py, value)
    }

    fn enhance_sharpness(slf: Py<Self>, factor: f64) -> PyResult<PyImage> {
        let rs = Python::attach(|py| {
            let inner = slf.bind(py).borrow().inner.clone();
            py.detach(|| inner.enhance_sharpness(factor))
        })
        .map_err(map_error)?;
        Ok(PyImage { inner: rs })
    }
}
