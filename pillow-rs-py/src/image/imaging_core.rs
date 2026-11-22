use super::PyImage;
use super::execution::route_from_context;
use crate::inputs::{
    c_int_from_python, paste_box_sequence_coordinates, paste_source_from_imaging_core,
};
use crate::map_error;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::pymethods;
use pyo3::types::{PyAnyMethods, PySequence, PySequenceMethods, PyTypeMethods};

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
    fn isblock(&self) -> bool {
        self.inner.imaging_core_isblock()
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
        let transparency = if let Ok(sequence) = transparency.cast::<PySequence>() {
            if sequence.len()? != 3 {
                return Err(PyTypeError::new_err(
                    "transparency must contain exactly three integer components",
                ));
            }
            pillow_rs::ImagingCoreTransparencyKey::Rgb(
                c_int_from_python(&sequence.get_item(0)?)?,
                c_int_from_python(&sequence.get_item(1)?)?,
                c_int_from_python(&sequence.get_item(2)?)?,
            )
        } else {
            pillow_rs::ImagingCoreTransparencyKey::Scalar(c_int_from_python(transparency)?)
        };
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

    // TODO(pillow-rs#3ptb): verify direct ImagingCore Paste color conversion,
    // pixel mutation, argument fault order, and selected/fallback receipts.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
        let source = inner.prepare_paste_source(source).map_err(map_error)?;
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
                Some(route) => {
                    let result = inner.paste_prepared_with_execution_context(
                        box_plan,
                        source,
                        mask,
                        route.context,
                    );
                    if result.is_ok() {
                        if let Some(backend) = route.pinned_backend {
                            inner = inner.clone().use_backend(backend);
                        }
                    }
                    result
                }
                None => inner.paste_prepared(box_plan, source, mask),
            };
            (result, inner)
        });
        slf.bind(py).borrow_mut().inner = inner;
        result.map_err(map_error)
    }

    // TODO(pillow-rs#3ptb): verify direct SetMode output, mode-fault order,
    // and selected/fallback receipts for declared modes and architectures.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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

    // TODO(pillow-rs#3ptb): verify PutBand output, validation order, retained
    // lanes, and selected/fallback receipts across declared modes.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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

    // TODO(pillow-rs#3ptb): verify FillBand output, validation order, and
    // selected/fallback receipts across declared modes.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
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
