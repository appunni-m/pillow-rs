use super::RsImage;
use super::execution::{
    PyExecutionContext, pyimage_from_active_route, route_from_context, route_pending_image,
};
use super::quantize::quantize_palette_from_python;
use crate::*;
use pyo3::types::PyModuleMethods;
use pyo3::{Py, pyclass, pyfunction, pymethods};

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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[pyfunction(name = "_resize_reducing_gap_bounds")]
pub(super) fn resize_reducing_gap_bounds(
    bounds: &Bound<'_, PyAny>,
    factor: &Bound<'_, PyAny>,
    safe_box: &Bound<'_, PyAny>,
) -> PyResult<(f64, f64, f64, f64)> {
    let bounds = bounds.extract::<(f64, f64, f64, f64)>()?;
    let factor = factor.extract::<(f64, f64)>()?;
    let safe_box = safe_box.extract::<(f64, f64, f64, f64)>()?;
    pillow_rs::resize_reducing_gap_bounds(bounds, factor, safe_box).map_err(map_error)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=n/a
#[pyfunction(name = "_resize_reducing_gap_filter_support")]
pub(super) fn resize_reducing_gap_filter_support(
    resample: Option<&Bound<'_, PyAny>>,
) -> PyResult<f64> {
    let input = resample_input_from_python(resample)?;
    let filter = pillow_rs::parse_resample_input(input).map_err(map_error)?;
    Ok(pillow_rs::resize_reducing_gap_filter_support(filter))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn register_adapter_functions(m: &Bound<'_, pyo3::types::PyModule>) -> PyResult<()> {
    m.add_function(pyo3::wrap_pyfunction!(thumbnail_filter_code, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(imageops_filter_code, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(thumbnail_reducing_gap, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(resize_reducing_gap_bounds, m)?)?;
    m.add_function(pyo3::wrap_pyfunction!(
        resize_reducing_gap_filter_support,
        m
    )?)?;
    Ok(())
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn convert_input_plan_from_python(
    image: &pillow_rs::Image,
    mode: pillow_rs::ConvertModeInput,
    matrix: Option<&Bound<'_, PyAny>>,
    palette: Option<&Bound<'_, PyAny>>,
    py: Python<'_>,
) -> PyResult<pillow_rs::ConvertInputPlan> {
    let mut matrix_truthiness = if matrix.is_some() {
        pillow_rs::ConvertMatrixTruthiness::NotEvaluated
    } else {
        pillow_rs::ConvertMatrixTruthiness::First(false)
    };
    let mut palette_adaptive = None;
    loop {
        let plan = image
            .prepare_convert_input_plan(mode.clone(), matrix_truthiness, palette_adaptive)
            .map_err(map_error)?;
        match plan {
            pillow_rs::ConvertInputPlan::NeedsMatrixTruthiness => {
                let truthy = matrix
                    .map(|value| value.is_truthy())
                    .transpose()?
                    .unwrap_or(false);
                matrix_truthiness = pillow_rs::ConvertMatrixTruthiness::First(truthy);
            }
            pillow_rs::ConvertInputPlan::NeedsMatrixTruthinessAgain { first } => {
                let second = matrix
                    .map(|value| value.is_truthy())
                    .transpose()?
                    .unwrap_or(false);
                matrix_truthiness = pillow_rs::ConvertMatrixTruthiness::Repeated { first, second };
            }
            pillow_rs::ConvertInputPlan::NeedsPaletteComparison => {
                palette_adaptive = Some(convert_palette_is_adaptive_from_python(py, palette)?);
            }
            ready => return Ok(ready),
        }
    }
}

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
    fn _resize_filter_code(slf: Py<Self>, resample: Option<&Bound<'_, PyAny>>) -> PyResult<i32> {
        let _keep_self_alive = slf;
        let input = resample_input_from_python(resample)?;
        let filter = pillow_rs::parse_resample_input(input).map_err(map_error)?;
        Ok(resample_filter_code(filter))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (size, resample=None, box_coords=None, reducing_gap=None))]
    fn resize(
        slf: Py<Self>,
        size: &Bound<'_, PyAny>,
        resample: Option<&Bound<'_, PyAny>>,
        box_coords: Option<&Bound<'_, PyAny>>,
        reducing_gap: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let resample = resample_input_from_python(resample)?;
        let size = size.extract::<(i64, i64)>()?;
        let box_coords = box_coords
            .map(|value| value.extract::<(f64, f64, f64, f64)>())
            .transpose()?;
        let reducing_gap = reducing_gap
            .map(|value| value.extract::<f64>())
            .transpose()?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.resize_with_options(size, resample, box_coords, reducing_gap))
            .map_err(map_error)?;
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (size, resample=None, box_coords=None, reducing_gap=None))]
    fn _resize_dispatch(
        slf: Py<Self>,
        size: &Bound<'_, PyAny>,
        resample: Option<&Bound<'_, PyAny>>,
        box_coords: Option<&Bound<'_, PyAny>>,
        reducing_gap: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let resample = resample_input_from_python(resample)?;
        let size = size.extract::<(i64, i64)>()?;
        let box_coords = box_coords
            .map(|value| value.extract::<(f64, f64, f64, f64)>())
            .transpose()?;
        let reducing_gap = reducing_gap
            .map(|value| value.extract::<f64>())
            .transpose()?;
        let inner = slf.bind(py).borrow().inner.clone();
        if let Some(plan) =
            inner.resize_reducing_gap_plan(size, resample.clone(), box_coords, reducing_gap)
        {
            let result = PyDict::new(py);
            result.set_item("kind", "reducing-gap")?;
            result.set_item("factor", (plan.x_factor, plan.y_factor))?;
            result.set_item("reduce_box", plan.safe_box)?;
            return Ok(result.into_any().unbind());
        }
        let resized = py
            .detach(move || inner.resize_with_options(size, resample, box_coords, reducing_gap))
            .map_err(map_error)?;
        Py::new(py, pyimage_from_active_route(resized, py)).map(|image| image.into_any())
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

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
        Ok(pyimage_from_active_route(inner, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
    #[cfg(feature = "test-api")]
    #[pyo3(signature = (angle, expand=false))]
    fn _migration_direct_rotate(
        slf: Py<Self>,
        angle: f64,
        expand: bool,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let inner = slf.bind(py).borrow().inner.clone();
        let inner = py
            .detach(move || {
                inner.rotate_with_input(
                    angle,
                    pillow_rs::RotateResampleInput::Code(0),
                    pillow_rs::RotateExpandInput::Boolean(expand),
                    pillow_rs::RotatePointInput::Default,
                    pillow_rs::RotatePointInput::Default,
                    pillow_rs::ImageOpsColor::None,
                )
            })
            .map_err(map_error)?;
        Ok(PyImage { inner })
    }

    // TODO(pillow-rs#3ptb): parity-check arbitrary mode truth/equality overrides,
    // custom matrix sequence callbacks, palette comparison through transparent
    // routes, and custom dither __index__ failures. Built-in inputs now follow
    // the staged route before only consumed arguments are converted.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (mode=None, matrix=None, dither=None, palette=None, colors=None, execution_context=None))]
    fn convert(
        slf: Py<Self>,
        mode: Option<&Bound<'_, PyAny>>,
        matrix: Option<&Bound<'_, PyAny>>,
        dither: Option<&Bound<'_, PyAny>>,
        palette: Option<&Bound<'_, PyAny>>,
        colors: Option<&Bound<'_, PyAny>>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let mode = convert_mode_input_from_python(mode)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let plan = convert_input_plan_from_python(&inner, mode, matrix, palette, py)?;
        let matrix = if matches!(plan, pillow_rs::ConvertInputPlan::Matrix { .. }) {
            Some(convert_matrix_from_python(matrix.ok_or_else(|| {
                PyValueError::new_err("Convert matrix plan has no matrix input")
            })?)?)
        } else {
            None
        };
        let dither = if matches!(plan, pillow_rs::ConvertInputPlan::Convert { .. }) {
            convert_dither_input_from_python(dither)?
        } else {
            pillow_rs::DitherInput::None
        };
        let colors = if matches!(plan, pillow_rs::ConvertInputPlan::Quantize { .. }) {
            colors
                .map(|value| value.extract::<i32>())
                .transpose()?
                .map(i64::from)
        } else {
            None
        };
        let route = route_from_context(execution_context, py);
        let execution_context = route.map(|route| route.context);
        let rs = py
            .detach(move || {
                inner.execute_convert_input_plan(plan, matrix, dither, colors, execution_context)
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    /// Applies Pillow's low-level matrix conversion through the typed core plan.
    // TODO(pillow-rs#3ptb): verify target-mode arity faults, matrix pixel output,
    // and active architecture/fallback receipts in the CPU parity matrix.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (mode, matrix))]
    fn convert_matrix(
        slf: Py<Self>,
        mode: &Bound<'_, PyAny>,
        matrix: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let mode_name = mode.extract::<String>()?;
        let target_mode = pillow_rs::Mode::from_pillow_name(&mode_name).ok_or_else(|| {
            map_error(pillow_rs::PilError::ValueError(
                "unrecognized image mode".into(),
            ))
        })?;
        // NO_PILLOW_SUPPORT: the Rust Convert matrix contract also admits LAB.
        let matrix = convert_matrix_from_python(matrix)?;
        let route = route_from_context(None, py);
        let context = route.map(|route| route.context);
        let image = slf.bind(py).borrow().inner.clone();
        let result = py
            .detach(move || {
                image.execute_convert_input_plan(
                    pillow_rs::ConvertInputPlan::Matrix { target_mode },
                    Some(matrix),
                    pillow_rs::DitherInput::None,
                    None,
                    context,
                )
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(result, route),
        })
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
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (box_plan, source_plan, mask=None, execution_context=None))]
    fn _paste_apply_prepared(
        slf: pyo3::Py<Self>,
        box_plan: pyo3::PyRef<'_, PyPasteBoxPlan>,
        source_plan: pyo3::PyRef<'_, PyPasteSourcePlan>,
        mask: Option<&Bound<'_, PyAny>>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
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
        let route = route_from_context(execution_context, py);
        let mut destination = slf.bind(py).borrow_mut();
        let destination = &mut destination.inner;
        py.detach(|| match route {
            Some(route) => {
                let result = destination.paste_prepared_with_execution_context(
                    box_plan,
                    source_plan,
                    mask,
                    route.context,
                );
                if result.is_ok() {
                    if let Some(backend) = route.pinned_backend {
                        *destination = (*destination).clone().use_backend(backend);
                    }
                }
                result
            }
            None => destination.paste_prepared(box_plan, source_plan, mask),
        })
        .map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn split(&self, py: Python<'_>) -> PyResult<Vec<PyImage>> {
        let route = route_from_context(None, py);
        let inner = route_pending_image(self.inner.clone(), route);
        let bands = py.detach(move || inner.split()).map_err(map_error)?;
        Ok(bands
            .into_iter()
            .map(|img| PyImage {
                inner: route_pending_image(img, route),
            })
            .collect())
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (band, execution_context=None))]
    fn getband(
        &self,
        band: &Bound<'_, PyAny>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let band = c_int_from_python(band)?;
        let route = route_from_context(execution_context, py);
        let inner = match route {
            Some(route) => self.inner.getchannel_selector_with_context(
                pillow_rs::ChannelSelector::Index(band),
                route.context,
            ),
            None => self.inner.getchannel(band),
        }
        .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(inner, route),
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
        Ok(pyimage_from_active_route(result, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn filter_name(&self, filter_type: &str, py: Python<'_>) -> PyResult<PyImage> {
        let filter_type = filter_type.to_owned();
        py.detach(|| self.inner.filter(&filter_type))
            .map(|inner| pyimage_from_active_route(inner, py))
            .map_err(map_error)
    }

    fn validate_filter(&self, filter_name: &str) -> PyResult<()> {
        self.inner.validate_filter(filter_name).map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn copy(&self, py: Python<'_>) -> PyImage {
        pyimage_from_active_route(self.inner.copy(), py)
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

    // TODO(pillow-rs#3ptb): verify in-place pixels, validation faults, and
    // selected/fallback receipts across the frozen CPU mode matrix.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn _thumbnail_apply_prepared(
        slf: Py<Self>,
        plan: pyo3::PyRef<'_, PyThumbnailPlan>,
        filter_code: i32,
        reducing_gap: Option<f64>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let prepared = plan.prepared;
        let route = route_from_context(None, py);
        let inner = slf.bind(py).borrow().inner.clone();
        let (result, inner) = py.detach(move || {
            let mut inner = inner;
            let filter = Some(pillow_rs::ResampleInput::Code(i64::from(filter_code)));
            let result = match route {
                Some(route) => inner.thumbnail_with_plan_and_execution_route(
                    prepared,
                    filter,
                    reducing_gap,
                    route.pinned_backend,
                    route.context,
                ),
                None => inner.thumbnail_with_plan(prepared, filter, reducing_gap),
            };
            (result, inner)
        });
        slf.bind(py).borrow_mut().inner = inner;
        result.map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (colors=None, method=None, kmeans=None, dither=None, palette=None, visible_mode=None, execution_context=None))]
    fn quantize(
        slf: Py<Self>,
        colors: Option<i32>,
        method: Option<i32>,
        kmeans: Option<i32>,
        dither: Option<i32>,
        palette: Option<&Bound<'_, PyAny>>,
        visible_mode: Option<&str>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let palette = palette
            .map(quantize_palette_from_python)
            .transpose()?
            .unwrap_or(pillow_rs::QuantizePalette::None);
        let visible_mode = visible_mode.and_then(pillow_rs::Mode::from_pillow_name);
        let route = route_from_context(execution_context, py);
        let execution_context = route.map(|route| route.context);
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || {
                inner.quantize_with_visible_mode_and_context(
                    colors,
                    method,
                    kmeans,
                    palette,
                    dither.map(|value| value != 0),
                    visible_mode,
                    execution_context,
                )
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn expand(slf: Py<Self>, margin: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<PyImage> {
        let margin = c_int_from_python(margin)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.imaging_core_expand(margin, margin))
            .map_err(map_error)?;
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn rankfilter(
        slf: Py<Self>,
        size: &Bound<'_, PyAny>,
        rank: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let size = c_int_from_python(size)?;
        let rank = c_int_from_python(rank)?;
        let inner = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || inner.imaging_core_rankfilter(size, rank))
            .map_err(map_error)?;
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (channel, execution_context=None))]
    fn getchannel(
        slf: Py<Self>,
        channel: &Bound<'_, PyAny>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let selector = if channel.cast::<PyString>().is_ok() {
            pillow_rs::ChannelSelector::Name(channel.extract::<String>()?)
        } else {
            pillow_rs::ChannelSelector::Index(crate::inputs::c_int_from_python(channel)?)
        };
        let route = route_from_context(execution_context, py);
        let execution_context = route.map(|route| route.context);
        let image = slf.bind(py).borrow().inner.clone();
        let rs = py
            .detach(move || match execution_context {
                Some(context) => image.getchannel_selector_with_context(selector, context),
                None => image.getchannel_selector(selector),
            })
            .map_err(map_error)?;
        Ok(PyImage {
            inner: route_pending_image(rs, route),
        })
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
        Ok(pyimage_from_active_route(rs, py))
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn effect_spread(slf: Py<Self>, distance: u32, py: Python<'_>) -> PyResult<PyImage> {
        let inner = slf.bind(py).borrow().inner.clone();
        py.detach(move || pillow_rs::image_effect_spread(&inner, distance))
            .map(|i| pyimage_from_active_route(i, py))
            .map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
            .map(|i| pyimage_from_active_route(i, py))
            .map_err(map_error)?;
        Ok(remapped)
    }

    // TODO(pillow-rs#3ptb): verify XBM mode faults, route receipts, and output
    // bytes in the frozen CPU matrix.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[pyo3(signature = (execution_context=None))]
    fn tobitmap(
        &mut self,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        py: Python<'_>,
    ) -> PyResult<Vec<u8>> {
        let execution_route = route_from_context(execution_context, py);
        let inner = route_pending_image(self.inner.clone(), execution_route);
        py.detach(|| inner.tobitmap()).map_err(map_error)
    }
}
