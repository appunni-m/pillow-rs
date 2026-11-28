use super::PyImage;
use super::execution::route_from_context;
use crate::inputs::{
    c_int_from_python, imaging_core_transparency_key_from_python, paste_box_sequence_coordinates,
    paste_source_from_imaging_core, transform_box_sequence_coordinates,
};
use crate::map_error;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::{PyAnyMethods, PyTypeMethods};

// VERIFY: source=reviewed@2026-11-24; parity=n/a; benchmark=n/a
fn imaging_core_transform_method(method: i32) -> PyResult<pillow_rs::TransformMethod> {
    match method {
        0 => Ok(pillow_rs::TransformMethod::Affine),
        2 => Ok(pillow_rs::TransformMethod::Perspective),
        3 => Ok(pillow_rs::TransformMethod::Quad),
        _ => Err(PyValueError::new_err("unknown transformation method")),
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=n/a; benchmark=n/a
fn imaging_core_transform_data_from_python(
    value: &Bound<'_, PyAny>,
    method: i32,
) -> PyResult<Vec<f64>> {
    let expected = match method {
        0 => Some(6),
        2 | 3 => Some(8),
        _ => None,
    };
    let items = value.try_iter()?.collect::<PyResult<Vec<_>>>()?;
    if expected != Some(items.len()) {
        return Err(PyValueError::new_err("wrong number of matrix entries"));
    }
    items
        .into_iter()
        .map(|item| item.extract::<f64>())
        .collect()
}

#[pymethods]
impl PyImage {
    // Internal low-level ImagingCore mutation. Public Image.transform uses
    // _transform_apply/_transform_mesh_record; its wrapper call is the parity
    // target, so this method has no separate public parity or benchmark lane.
    // VERIFY: source=reviewed@2026-11-24; parity=n/a; benchmark=n/a
    #[pyo3(signature = (box_coords, source, method, data, resample=None, fill=None))]
    fn transform(
        slf: Py<Self>,
        box_coords: &Bound<'_, PyAny>,
        source: &Bound<'_, PyAny>,
        method: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
        resample: Option<&Bound<'_, PyAny>>,
        fill: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let coordinates = transform_box_sequence_coordinates(box_coords)?;
        let source = match source.cast::<PyImage>() {
            Ok(source) => source,
            Err(_) => {
                let type_name = source.get_type().name()?.to_string();
                return Err(PyTypeError::new_err(format!(
                    "argument 2 must be ImagingCore, not {type_name}"
                )));
            }
        };
        let method_code = c_int_from_python(method)?;
        let resample = resample.map(c_int_from_python).transpose()?.unwrap_or(0);
        let fill = fill.map(c_int_from_python).transpose()?.unwrap_or(1) != 0;
        let data = imaging_core_transform_data_from_python(data, method_code)?;
        let method = imaging_core_transform_method(method_code)?;
        let route = route_from_context(None, py);
        let source = source.borrow().inner.clone();
        let destination = slf.bind(py).borrow().inner.clone();
        let (result, destination) = py.detach(move || {
            let mut destination = destination;
            let result = match route {
                Some(route) => destination.imaging_core_transform_with_execution_context(
                    &source,
                    coordinates,
                    method,
                    data,
                    resample,
                    fill,
                    route.requested_backend,
                    route.pinned_backend,
                    route.context,
                ),
                None => destination.imaging_core_transform(
                    &source,
                    coordinates,
                    method,
                    data,
                    resample,
                    fill,
                ),
            };
            (result, destination)
        });
        slf.bind(py).borrow_mut().inner = destination;
        result.map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    fn isblock(&self) -> bool {
        self.inner.imaging_core_isblock()
    }

    // TODO(pillow-rs#3ptb): freeze direct ImagingCore convert_into mode-pair
    // outputs, dimension and unsupported-mode faults, destination metadata,
    // aliasing, and selected/fallback receipts in the CPU parity contract.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    fn convert_into(slf: Py<Self>, destination: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let route = route_from_context(None, py);
        let source = slf.bind(py).borrow().inner.clone();
        let mut destination_image = destination.bind(py).borrow().inner.clone();
        let (result, destination_image) = py.detach(move || {
            let result = match route {
                Some(route) => source.imaging_core_convert_into_with_execution_context(
                    &mut destination_image,
                    route.requested_backend,
                    route.pinned_backend,
                    route.context,
                ),
                None => source.imaging_core_convert_into(&mut destination_image),
            };
            (result, destination_image)
        });
        result.map_err(map_error)?;
        destination.bind(py).borrow_mut().inner = destination_image;
        Ok(())
    }

    // TODO(pillow-rs#3ptb): freeze convert_transparent output, argument faults,
    // and selected/fallback receipts in the CPU parity contract.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    fn convert_transparent(
        &self,
        mode: &str,
        transparency: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<PyImage> {
        let transparency = imaging_core_transparency_key_from_python(transparency)?;
        let source_mode = self.inner.mode_identity().map_err(map_error)?;
        let target = pillow_rs::Mode::from_pillow_name(mode).ok_or_else(|| {
            let source = source_mode.pillow_name().unwrap_or("unknown");
            PyValueError::new_err(format!(
                "conversion from {source} to {} not supported in convert_transparent",
                mode.chars().take(10).collect::<String>()
            ))
        })?;
        let route = route_from_context(None, py);
        let source = self.inner.clone();
        let inner = py.detach(move || match route {
            Some(route) => source.imaging_core_convert_transparent_with_execution_context(
                target,
                transparency,
                route.requested_backend,
                route.pinned_backend,
                route.context,
            ),
            None => source.imaging_core_convert_transparent(target, transparency),
        });
        inner.map(|inner| PyImage { inner }).map_err(map_error)
    }

    // TODO(pillow-rs#3ptb): verify direct ImagingCore Paste mode-pair outputs,
    // scalar color coercion, mutation timing, argument faults, and route receipts.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    #[pyo3(signature = (source, box_coords, mask=None))]
    fn paste(
        slf: Py<Self>,
        source: &Bound<'_, PyAny>,
        box_coords: &Bound<'_, PyAny>,
        mask: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let coordinates = paste_box_sequence_coordinates(box_coords, py)?;
        let mask = match mask {
            Some(mask) => match mask.cast::<PyImage>() {
                Ok(mask) => pillow_rs::PasteMaskInput::Image(Box::new(mask.borrow().inner.clone())),
                Err(_) => {
                    let type_name = mask.get_type().name()?.to_string();
                    return Err(PyTypeError::new_err(format!(
                        "argument 3 must be ImagingCore, not {type_name}"
                    )));
                }
            },
            None => pillow_rs::PasteMaskInput::None,
        };
        let inner = slf.bind(py).borrow().inner.clone();
        let mode = inner.mode_identity().map_err(map_error)?;
        let source = paste_source_from_imaging_core(source, mode)?;
        let source = match source {
            pillow_rs::PasteSourceInput::Image(image) => inner
                .prepare_resolved_paste_image_source(*image)
                .map_err(map_error)?,
            source => {
                pillow_rs::Image::prepare_paste_source_for_mode(mode, source).map_err(map_error)?
            }
        };
        let has_mask = !matches!(&mask, pillow_rs::PasteMaskInput::None);
        let box_plan = pillow_rs::Image::prepare_paste_box(
            pillow_rs::PasteBoxInput::Values(coordinates),
            has_mask,
            true,
        )
        .map_err(map_error)?;
        let route = route_from_context(None, py);
        let (result, inner) = py.detach(move || {
            let mut inner = inner;
            let result = match route {
                Some(route) => inner.paste_prepared_with_execution_route(
                    box_plan,
                    source,
                    mask,
                    pillow_rs::CpuExecutionRoute::new(route.requested_backend, route.context)
                        .with_pinned_backend(route.pinned_backend),
                ),
                None => inner.paste_prepared(box_plan, source, mask),
            };
            (result, inner)
        });
        slf.bind(py).borrow_mut().inner = inner;
        result.map_err(map_error)
    }

    // TODO(pillow-rs#3ptb): verify direct SetMode output, mode-fault order,
    // and selected/fallback receipts for declared modes and architectures.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    fn setmode(&mut self, mode: &str, py: Python<'_>) -> PyResult<()> {
        let target = pillow_rs::Mode::from_pillow_name(mode).ok_or_else(|| {
            map_error(pillow_rs::PilError::ValueError(
                "image has wrong mode".into(),
            ))
        })?;
        let route = route_from_context(None, py);
        let inner = self.inner.clone();
        let (result, inner) = py.detach(move || {
            let mut inner = inner;
            let result = match route {
                Some(route) => inner.imaging_core_setmode_with_execution_context(
                    target,
                    route.requested_backend,
                    route.pinned_backend,
                    route.context,
                ),
                None => inner.imaging_core_setmode(target),
            };
            (result, inner)
        });
        self.inner = inner;
        result.map_err(map_error)
    }

    // TODO(pillow-rs#3ptb): verify PutBand cross-mode same-type rejection,
    // exact-mode P palette transfer, output, validation order, retained lanes,
    // single-band versus multiband overlong buffers, and route receipts.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    fn putband(slf: Py<Self>, source: Py<Self>, band: i32, py: Python<'_>) -> PyResult<()> {
        let route = route_from_context(None, py);
        let source = source.bind(py).borrow().inner.clone();
        let inner = slf.bind(py).borrow().inner.clone();
        let (result, inner) = py.detach(move || {
            let mut inner = inner;
            let result = match route {
                Some(route) => inner.imaging_core_putband_with_execution_context(
                    &source,
                    band,
                    route.requested_backend,
                    route.pinned_backend,
                    route.context,
                ),
                None => inner.imaging_core_putband(&source, band),
            };
            (result, inner)
        });
        slf.bind(py).borrow_mut().inner = inner;
        result.map_err(map_error)
    }

    // TODO(pillow-rs#3ptb): verify FillBand outputs, validation order, retained
    // lanes, overlong buffers, and route receipts across declared modes.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    fn fillband(&mut self, band: i32, color: i32, py: Python<'_>) -> PyResult<()> {
        let route = route_from_context(None, py);
        let inner = self.inner.clone();
        let (result, inner) = py.detach(move || {
            let mut inner = inner;
            let result = match route {
                Some(route) => inner.imaging_core_fillband_with_execution_context(
                    band,
                    color,
                    route.requested_backend,
                    route.pinned_backend,
                    route.context,
                ),
                None => inner.imaging_core_fillband(band, color),
            };
            (result, inner)
        });
        self.inner = inner;
        result.map_err(map_error)
    }
}
