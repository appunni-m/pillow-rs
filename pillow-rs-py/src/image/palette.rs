use pyo3::PyResult;
use pyo3::conversion::IntoPyObjectExt;
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, Python};

use super::PyImage;
use super::data::update_image_with_route;
use super::execution::route_from_context;
use crate::map_error;
use pyo3::exceptions::{PyBufferError, PyOverflowError};
use pyo3::prelude::PyModule;
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::{
    PyAnyMethods, PyByteArray, PyBytes, PyList, PyListMethods, PySlice, PyTuple, PyTupleMethods,
};

#[pyclass(name = "_RemapPalettePlan", frozen)]
pub(super) struct PyRemapPalettePlan {
    palette_model: pillow_rs::PaletteModel,
    inputs: RemapPaletteInputs,
}

/// Typed palette inputs prepared by replaying Pillow's per-entry protocols.
pub(crate) struct RemapPaletteInputs {
    pub(crate) mapping_indices: Vec<u8>,
    pub(crate) selected_palette: Py<PyAny>,
}

#[pymethods]
impl PyRemapPalettePlan {
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    #[getter]
    fn mapping_palette(&self, py: Python<'_>) -> Py<PyByteArray> {
        let mapping_palette = self
            .palette_model
            .remap_mapping_palette(&self.inputs.mapping_indices);
        PyByteArray::new(py, &mapping_palette).unbind()
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    #[getter]
    fn selected_palette(&self, py: Python<'_>) -> Py<PyAny> {
        self.inputs.selected_palette.clone_ref(py)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    #[getter]
    fn palette_mode(&self) -> &'static str {
        self.palette_model.pillow_name()
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    #[getter]
    fn mapping_rawmode(&self) -> &'static str {
        self.palette_model.remap_mapping_rawmode()
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(crate) fn remap_palette_inputs_from_python(
    dest_map: &Bound<'_, PyAny>,
    source_palette: &Bound<'_, PyAny>,
    palette_model: pillow_rs::PaletteModel,
) -> PyResult<RemapPaletteInputs> {
    let py = dest_map.py();
    let operators = PyModule::import(py, "operator")?;
    let multiply = operators.getattr("mul")?;
    let add = operators.getattr("add")?;
    let inplace_add = operators.getattr("iadd")?;
    let bands = palette_model.remap_palette_bands();
    let new_positions = PyList::new(py, [0; 256])?;
    let mut palette_bytes = PyBytes::new(py, &[]).into_any();

    for (new_index, old_position) in dest_map.try_iter()?.enumerate() {
        let old_position = old_position?;
        let slice_start = multiply.call1((old_position.clone(), bands))?;
        let slice_stop = add.call1((&slice_start, bands))?;
        let slice = py
            .get_type::<PySlice>()
            .call1((&slice_start, &slice_stop))?;
        let palette_entry = source_palette.get_item(slice)?;
        palette_bytes = inplace_add.call1((palette_bytes, palette_entry))?;

        // Let the real list assignment invoke __index__ once and raise its
        // native TypeError or IndexError. The temporary value is unique among
        // prior entries; finding it reveals Python's normalized list index.
        let sentinel = new_index
            .checked_add(256)
            .ok_or_else(|| PyOverflowError::new_err("enumerate index is too large"))?;
        new_positions.call_method1("__setitem__", (old_position, sentinel))?;
        let old_index = new_positions
            .call_method1("index", (sentinel,))?
            .extract::<usize>()?;
        new_positions.set_item(old_index, new_index)?;
    }

    // Pillow builds this bytearray before copying pixels. Besides preserving
    // that fault boundary, it makes maps longer than 256 fail with Python's
    // bytearray overflow instead of a Rust admission error.
    // Keep Pillow's bytearray conversion fault boundary after map processing.
    let mapping_indices = py
        .get_type::<PyByteArray>()
        .call1((new_positions,))?
        .extract::<Vec<u8>>()?;
    let selected_palette = palette_bytes.unbind();

    Ok(RemapPaletteInputs {
        mapping_indices,
        selected_palette,
    })
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
pub(crate) fn palette_model_from_python(name: &str) -> PyResult<pillow_rs::PaletteModel> {
    pillow_rs::PaletteModel::from_pillow_name(name)
        .ok_or_else(|| PyValueError::new_err("unsupported palette mode"))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
fn pyimage_putpalette(
    slf: Py<PyImage>,
    data: &Bound<'_, PyAny>,
    rawmode: &str,
    palette_mode: Option<&str>,
    attach_without_loading: bool,
    py: Python<'_>,
) -> PyResult<()> {
    let data = data.extract::<Vec<u8>>()?;
    let rawmode = rawmode.to_owned();
    let palette_model = palette_mode
        .map(super::palette_model_from_python)
        .transpose()?;
    let route = route_from_context(None, py);
    update_image_with_route(slf, py, route, move |image, route| {
        match (route, palette_model) {
            (Some(route), Some(model)) if attach_without_loading => image
                .attach_palette_with_model_and_execution_context(
                    &data,
                    model,
                    &rawmode,
                    route.pinned_backend,
                    route.context,
                ),
            (Some(route), None) if attach_without_loading => image
                .attach_palette_with_execution_context(
                    &data,
                    &rawmode,
                    route.pinned_backend,
                    route.context,
                ),
            (Some(route), Some(model)) => image.putpalette_with_model_and_execution_context(
                &data,
                model,
                &rawmode,
                route.pinned_backend,
                route.context,
            ),
            (Some(route), None) => image.putpalette_with_execution_context(
                &data,
                &rawmode,
                route.pinned_backend,
                route.context,
            ),
            (None, Some(model)) if attach_without_loading => {
                image.attach_palette_with_model(&data, model, &rawmode)
            }
            (None, None) if attach_without_loading => image.attach_palette(&data, &rawmode),
            (None, Some(model)) => image.putpalette_with_model(&data, model, &rawmode),
            (None, None) => image.putpalette(&data, &rawmode),
        }
    })
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
fn palette_alpha_bytes(value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    let memoryview = PyModule::import(value.py(), "builtins")?
        .getattr("memoryview")?
        .call1((value,))?;
    if !memoryview.getattr("c_contiguous")?.extract::<bool>()? {
        return Err(PyBufferError::new_err(
            "underlying buffer is not C-contiguous",
        ));
    }
    Ok(memoryview.call_method0("tobytes")?.extract::<Vec<u8>>()?)
}

#[pymethods]
impl PyImage {
    // TODO(pillow-rs#3ptb): verify inverse-map bytearray faults, typed palette
    // model selection, and callback mutation/fault order in the frozen matrix.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[pyo3(signature = (dest_map, source_palette=None))]
    fn remap_palette_prepare(
        &self,
        dest_map: &Bound<'_, PyAny>,
        source_palette: Option<&Bound<'_, PyAny>>,
        py: Python<'_>,
    ) -> PyResult<PyRemapPalettePlan> {
        let source_image = self.inner.clone();
        source_image
            .validate_remap_palette_mode()
            .map_err(map_error)?;
        let source_mode = source_image.mode_identity().map_err(map_error)?;
        let palette_snapshot = if source_palette.is_none() {
            Some(
                source_image
                    .remap_palette_source_snapshot()
                    .map_err(map_error)?,
            )
        } else {
            None
        };
        let (palette_model, inputs) = match source_palette {
            Some(source_palette) => {
                // Pillow checks this before it starts enumerating dest_map.
                let model = if source_palette.len()? > 768 {
                    pillow_rs::PaletteModel::Rgba
                } else {
                    pillow_rs::PaletteModel::Rgb
                };
                (
                    model,
                    remap_palette_inputs_from_python(dest_map, source_palette, model)?,
                )
            }
            None => {
                let (model, bytes) = palette_snapshot
                    .ok_or_else(|| PyValueError::new_err("image has no palette"))?;
                let source_palette = if source_mode == pillow_rs::Mode::ImageLuma8 {
                    PyByteArray::new(py, &bytes).into_any()
                } else {
                    PyBytes::new(py, &bytes).into_any()
                };
                (
                    model,
                    remap_palette_inputs_from_python(dest_map, &source_palette, model)?,
                )
            }
        };
        Ok(PyRemapPalettePlan {
            palette_model,
            inputs,
        })
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    #[pyo3(signature = (rawmode=None))]
    fn getpalette_with_input(
        &self,
        rawmode: Option<String>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        let palette = self
            .inner
            .getpalette_with_input(rawmode.as_deref())
            .map_err(map_error)?;
        match palette {
            Some(values) => Ok(PyList::new(py, values)?.into_py_any(py)?),
            None => Ok(py.None()),
        }
    }

    // TODO(pillow-rs#3ptb): verify packed palette bytes, rawmode faults, the
    // missing-palette error, and direct-filter callback output in CPU parity.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    #[pyo3(signature = (*args))]
    fn getpalette(&self, args: &Bound<'_, PyTuple>, py: Python<'_>) -> PyResult<Py<PyAny>> {
        if args.len() > 1 {
            return Err(PyTypeError::new_err(format!(
                "function takes at most 1 argument ({} given)",
                args.len()
            )));
        }
        let rawmode = if args.is_empty() {
            self.inner.palette_mode().map(str::to_owned)
        } else {
            Some(args.get_item(0)?.extract::<String>()?)
        };
        let rawmode = rawmode.as_deref().unwrap_or("RGB");
        let palette = self
            .inner
            .getpalette_rawmode(rawmode)
            .map_err(map_error)?
            .ok_or_else(|| PyValueError::new_err("image has no palette"))?;
        Ok(PyBytes::new(py, &palette).into())
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    fn indexed_color_table(&self) -> PyResult<Vec<(u8, u8, u8)>> {
        self.inner.indexed_color_table().map_err(map_error)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    fn palette_mode(&self) -> Option<String> {
        self.inner.palette_mode().map(str::to_owned)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    fn getpalettemode(&self) -> PyResult<String> {
        self.inner
            .palette_mode()
            .map(str::to_owned)
            .ok_or_else(|| PyValueError::new_err("image has no palette"))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    fn has_attached_palette(&self) -> bool {
        self.inner.has_attached_palette()
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    fn has_transparency_data(&self) -> bool {
        self.inner.has_transparency_data()
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=n/a
    fn stage_palette_attachment(slf: Py<Self>, py: Python<'_>) -> PyResult<bool> {
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, |image, route| match route {
            Some(route) => image
                .stage_palette_attachment_with_load_requirement_and_execution_context(
                    route.pinned_backend,
                    route.context,
                ),
            None => image.stage_palette_attachment_with_load_requirement(),
        })
    }

    // TODO(pillow-rs#3ptb): verify transparency commits, palette metadata,
    // errors, and route receipts at the active materialization boundary.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn apply_transparency(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, |image, route| match route {
            Some(route) => {
                image.apply_transparency_with_execution_context(route.pinned_backend, route.context)
            }
            None => image.apply_transparency(),
        })
    }

    // TODO(pillow-rs#3ptb): verify palette edits, partial mutations on errors,
    // and route receipts for public and callback-created calls.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[pyo3(signature = (data, rawmode="RGB", palette_mode=None))]
    fn putpalette(
        slf: Py<Self>,
        data: &Bound<'_, PyAny>,
        rawmode: &str,
        palette_mode: Option<&str>,
        py: Python<'_>,
    ) -> PyResult<()> {
        pyimage_putpalette(slf, data, rawmode, palette_mode, false, py)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[pyo3(signature = (data, rawmode="RGB", palette_mode=None))]
    fn attach_palette(
        slf: Py<Self>,
        data: &Bound<'_, PyAny>,
        rawmode: &str,
        palette_mode: Option<&str>,
        py: Python<'_>,
    ) -> PyResult<()> {
        pyimage_putpalette(slf, data, rawmode, palette_mode, true, py)
    }

    // TODO(pillow-rs#3ptb): verify single-entry alpha writes, truncation,
    // errors, and active-route receipts after any source materialization.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn putpalettealpha(
        slf: Py<Self>,
        index: &Bound<'_, PyAny>,
        alpha: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<()> {
        let index = index.extract::<i32>()?;
        let alpha = alpha.extract::<i32>()?;
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, move |image, route| match route {
            Some(route) => image.putpalettealpha_with_execution_context(
                index,
                alpha,
                route.pinned_backend,
                route.context,
            ),
            None => image.putpalettealpha(index, alpha),
        })
    }

    // TODO(pillow-rs#3ptb): verify alpha-table prefix semantics, buffer faults,
    // errors, and active-route receipts after any source materialization.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn putpalettealphas(slf: Py<Self>, values: &Bound<'_, PyAny>, py: Python<'_>) -> PyResult<()> {
        let values = palette_alpha_bytes(values)?;
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, move |image, route| match route {
            Some(route) => image.putpalettealphas_with_execution_context(
                &values,
                route.pinned_backend,
                route.context,
            ),
            None => image.putpalettealphas(&values),
        })
    }

    // TODO(pillow-rs#3ptb): verify raw-palette alpha commit output and route
    // receipts for pending decoder and callback-created images.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn commit_palette_alpha(slf: Py<Self>, py: Python<'_>) -> PyResult<()> {
        let route = route_from_context(None, py);
        update_image_with_route(slf, py, route, |image, route| match route {
            Some(route) => image
                .commit_palette_alpha_with_execution_context(route.pinned_backend, route.context),
            None => image.commit_palette_alpha(),
        })
    }
}
