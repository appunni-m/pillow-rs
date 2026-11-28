use super::PyImage;
use super::arrays::image_info_value_to_python;
use super::execution::{
    ExecutionRoute, PyExecutionContext, route_from_context, route_pending_image,
};
use crate::convert_dither_input_from_python;
use crate::convert_matrix_from_python;
use crate::convert_mode_input_from_python;
use crate::convert_palette_is_adaptive_from_python;
use crate::map_error;
use crate::palette_integer_component_from_python;
use pyo3::PyErr;
use pyo3::exceptions::{PyAssertionError, PyKeyError, PyUserWarning, PyValueError};
use pyo3::prelude::{Bound, Py, PyAny, PyResult, Python};
use pyo3::pyclass;
use pyo3::pymethods;
use pyo3::types::{
    PyAnyMethods, PyBytes, PyBytesMethods, PyDict, PyDictMethods, PyFloat, PyInt, PySlice,
    PyStringMethods, PyTuple, PyTupleMethods, PyTypeMethods,
};

#[pyclass(name = "_ConvertMatrixTransparency", frozen)]
pub(super) struct PyConvertMatrixTransparency {
    target_mode: pillow_rs::Mode,
}

type ConvertMetadataResult = (
    PyImage,
    bool,
    Option<Py<PyAny>>,
    Option<Py<PyAny>>,
    Option<Py<PyConvertMatrixTransparency>>,
    bool,
    bool,
);

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn pyimage_convert_with_metadata(
    slf: Py<PyImage>,
    mode: Option<&Bound<'_, PyAny>>,
    matrix: Option<&Bound<'_, PyAny>>,
    dither: Option<&Bound<'_, PyAny>>,
    palette: Option<&Bound<'_, PyAny>>,
    colors: Option<&Bound<'_, PyAny>>,
    execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
    transparency_info: Option<&Bound<'_, PyDict>>,
    transparency_owner: Option<&Bound<'_, PyAny>>,
    source_mutation_state: Option<&Bound<'_, PyDict>>,
    py: Python<'_>,
) -> PyResult<ConvertMetadataResult> {
    let mode = convert_mode_input_from_python(mode)?;
    let mut inner = slf.bind(py).borrow().inner.clone();
    let preflight_context = execution_context.as_ref().map(|context| context.inner);
    let plan = convert_input_plan_from_python(
        &inner,
        mode,
        matrix,
        palette,
        transparency_info,
        transparency_owner,
        preflight_context,
        py,
    )?;
    let (prepared_inner, plan, clear_transparency) =
        prepare_convert_palette_alpha_before_dither(inner, plan, preflight_context, py)?;
    if clear_transparency {
        // Pillow mutates the source ImagingCore palette before dither
        // extraction; retain that mutation if a later step raises.
        if let Some(state) = source_mutation_state {
            state.set_item("palette_alpha_mutated", true)?;
        }
        slf.bind(py).borrow_mut().inner = prepared_inner.clone();
    }
    inner = prepared_inner;
    let matrix_transparency_target = match &plan {
        pillow_rs::ConvertInputPlan::Matrix {
            target_mode,
            transform_transparency: true,
        } => Some(*target_mode),
        _ => None,
    };
    let matrix_values = if matches!(&plan, pillow_rs::ConvertInputPlan::Matrix { .. }) {
        Some(convert_matrix_from_python(matrix.ok_or_else(|| {
            PyValueError::new_err("Convert matrix plan has no matrix input")
        })?)?)
    } else {
        None
    };
    let dither = if matches!(&plan, pillow_rs::ConvertInputPlan::Convert { .. }) {
        convert_dither_input_from_python(dither)?
    } else {
        pillow_rs::DitherInput::None
    };
    let colors = if plan.consumes_colors() {
        colors
            .map(|value| value.extract::<i32>())
            .transpose()?
            .map(i64::from)
    } else {
        None
    };
    let route = route_from_context(execution_context, py);
    let execution_context = route.map(|route| route.context);
    let output = py
        .detach(move || {
            inner.execute_convert_input_plan_with_route(
                plan,
                matrix_values,
                dither,
                colors,
                execution_context,
                route.and_then(|route| route.requested_backend),
                route.and_then(|route| route.pinned_backend),
            )
        })
        .map_err(map_error)?;
    convert_output_with_metadata(
        output,
        clear_transparency,
        matrix_transparency_target,
        route,
        py,
    )
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn convert_output_with_metadata(
    output: pillow_rs::ConvertInputOutput,
    clear_transparency: bool,
    matrix_transparency_target: Option<pillow_rs::Mode>,
    route: ExecutionRoute,
    py: Python<'_>,
) -> PyResult<ConvertMetadataResult> {
    let matrix_transparency_plan = matrix_transparency_target
        .map(|target_mode| Py::new(py, PyConvertMatrixTransparency { target_mode }))
        .transpose()?;
    if let Some(warning) = output.warning {
        warn_convert_output(warning, py)?;
    }
    let (remove_transparency_delta, transparency_value, palette_color, palette_callback_adaptive) =
        match output.transparency_info {
            pillow_rs::ConvertTransparencyInfoDelta::Preserve => (false, None, None, false),
            pillow_rs::ConvertTransparencyInfoDelta::Remove => (true, None, None, false),
            pillow_rs::ConvertTransparencyInfoDelta::Set(value) => (
                false,
                Some(image_info_value_to_python(py, value)?),
                None,
                false,
            ),
            pillow_rs::ConvertTransparencyInfoDelta::SetPaletteColor {
                rgb,
                adaptive_palette,
            } => (
                false,
                None,
                Some(PyTuple::new(py, rgb)?.into_any().unbind()),
                adaptive_palette,
            ),
        };
    let remove_transparency = remove_transparency_delta || clear_transparency;
    Ok((
        PyImage {
            inner: route_pending_image(output.image, route),
        },
        remove_transparency,
        transparency_value,
        palette_color,
        matrix_transparency_plan,
        clear_transparency,
        palette_callback_adaptive,
    ))
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(super) fn convert_input_plan_from_python(
    image: &pillow_rs::Image,
    mode: pillow_rs::ConvertModeInput,
    matrix: Option<&Bound<'_, PyAny>>,
    palette: Option<&Bound<'_, PyAny>>,
    transparency_info: Option<&Bound<'_, PyDict>>,
    transparency_owner: Option<&Bound<'_, PyAny>>,
    execution_context: Option<pillow_rs::ExecutionContext>,
    py: Python<'_>,
) -> PyResult<pillow_rs::ConvertInputPlan> {
    let mut matrix_truthiness = if matrix.is_some() {
        pillow_rs::ConvertMatrixTruthiness::NotEvaluated
    } else {
        pillow_rs::ConvertMatrixTruthiness::First(false)
    };
    let mut palette_adaptive = None;
    let has_transparency = match transparency_info {
        Some(info) => info.contains("transparency")?,
        // Direct ImagingCore.convert has no Image.info wrapper contract.
        None => false,
    };
    // Pillow snapshots key presence before mode/matrix callbacks, then reads
    // the current owner mapping when the selected conversion consumes a key.
    let mut transparency = None;
    loop {
        let plan = image
            .prepare_convert_input_plan_with_transparency_preflight(
                mode.clone(),
                matrix_truthiness,
                palette_adaptive,
                has_transparency,
                transparency.clone(),
            )
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
            pillow_rs::ConvertInputPlan::NeedsTransparencyValue {
                target_mode,
                input_kind,
            } => {
                let preflight = convert_transparency_preflight_from_python(
                    image,
                    target_mode,
                    input_kind,
                    transparency_info,
                    transparency_owner,
                    execution_context,
                    py,
                )?;
                if preflight == pillow_rs::ConvertTransparencyPreflight::RemoveByteTable {
                    warn_byte_transparency(py)?;
                }
                transparency = Some(preflight);
            }
            pillow_rs::ConvertInputPlan::NeedsPaletteComparison { .. } => {
                palette_adaptive = Some(convert_palette_is_adaptive_from_python(py, palette)?);
            }
            ready => return Ok(ready),
        }
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn convert_transparency_preflight_from_python(
    image: &pillow_rs::Image,
    target_mode: pillow_rs::Mode,
    input_kind: pillow_rs::ConvertTransparencyInputKind,
    transparency_info: Option<&Bound<'_, PyDict>>,
    transparency_owner: Option<&Bound<'_, PyAny>>,
    execution_context: Option<pillow_rs::ExecutionContext>,
    py: Python<'_>,
) -> PyResult<pillow_rs::ConvertTransparencyPreflight> {
    let current_info = match transparency_owner {
        Some(owner) => {
            let info = owner.getattr("info")?;
            Some(info.cast::<PyDict>()?.clone())
        }
        None => None,
    };
    let info_before_callbacks = current_info.as_ref().or(transparency_info);
    let Some(info) = info_before_callbacks else {
        let image = convert_preflight_image(image, execution_context);
        return py
            .detach(move || image.prepare_convert_transparency_from_metadata(target_mode))
            .map_err(map_error);
    };
    let raw_value = info
        .get_item("transparency")?
        .ok_or_else(|| PyKeyError::new_err("transparency"))?;
    if input_kind == pillow_rs::ConvertTransparencyInputKind::PaletteColor
        && raw_value.cast::<PyBytes>().is_err()
    {
        let palette_present = match transparency_owner {
            Some(owner) => !owner.getattr("palette")?.is_none(),
            None => image.has_attached_palette(),
        };
        if !palette_present {
            return Err(PyAssertionError::new_err(""));
        }
    }
    let value =
        convert_transparency_value_for_route(&raw_value, input_kind, transparency_owner, py)?;
    let image = convert_preflight_image(image, execution_context);
    let mut preflight = py
        .detach(move || image.prepare_convert_transparency(target_mode, value, None))
        .map_err(map_error)?;
    if target_mode == pillow_rs::Mode::ImagePalette8 && transparency_owner.is_some() {
        if let pillow_rs::ConvertTransparencyPreflight::RgbKey {
            host_palette_callback,
            ..
        } = &mut preflight
        {
            *host_palette_callback = true;
        }
    }
    Ok(preflight)
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn convert_preflight_image(
    image: &pillow_rs::Image,
    execution_context: Option<pillow_rs::ExecutionContext>,
) -> pillow_rs::Image {
    match execution_context {
        Some(context) => image.clone().with_execution_context(context),
        None => image.clone(),
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn convert_transparency_value_for_route(
    value: &Bound<'_, PyAny>,
    input_kind: pillow_rs::ConvertTransparencyInputKind,
    transparency_owner: Option<&Bound<'_, PyAny>>,
    py: Python<'_>,
) -> PyResult<pillow_rs::ConvertTransparencyValue> {
    if input_kind == pillow_rs::ConvertTransparencyInputKind::ImagingCoreKey {
        return Ok(pillow_rs::ConvertTransparencyValue::ImagingCoreKey(
            crate::inputs::imaging_core_transparency_key_from_python(value)?,
        ));
    }
    if input_kind == pillow_rs::ConvertTransparencyInputKind::PaletteColor
        && value.cast::<PyTuple>().is_ok()
    {
        if let Some(owner) = transparency_owner {
            if let Some(rgb) = palette_tuple_color_from_python(value, owner, py)? {
                return Ok(pillow_rs::ConvertTransparencyValue::PaletteColor(rgb));
            }
        }
    }
    convert_transparency_value_from_python(value, input_kind)
}

// Pillow applies this only after the matrix kernel succeeds and Image._new has
// copied wrapper metadata; keep Python indexing and numeric protocols here.
// TODO(pillow-rs#3ptb): parity-check matrix transparency values and callback
// faults, including malformed scalar keys.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(super) fn transform_matrix_transparency_from_python(
    matrix: &Bound<'_, PyAny>,
    transparency: &Bound<'_, PyAny>,
    target_mode: pillow_rs::Mode,
    py: Python<'_>,
) -> PyResult<Py<PyAny>> {
    match target_mode {
        pillow_rs::Mode::ImageLuma8 => {
            matrix_transparency_pixel(matrix, transparency, py).map(Bound::unbind)
        }
        pillow_rs::Mode::ImageRgb8 => {
            let length = transparency.len()?;
            let mut output = Vec::with_capacity(length.min(3));
            for index in 0..length {
                let start = isize::try_from(index.saturating_mul(4)).unwrap_or(isize::MAX);
                let stop = start.saturating_add(4);
                let row = PySlice::new(py, start, stop, 1);
                let coefficients = matrix.get_item(&row)?;
                output.push(matrix_transparency_pixel(&coefficients, transparency, py)?);
            }
            Ok(PyTuple::new(py, output)?.into_any().unbind())
        }
        _ => Err(PyValueError::new_err("illegal conversion")),
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn matrix_transparency_pixel<'py>(
    matrix: &Bound<'py, PyAny>,
    transparency: &Bound<'py, PyAny>,
    py: Python<'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let mut value = matrix.get_item(0)?.mul(transparency.get_item(0)?)?;
    let second = matrix.get_item(1)?.mul(transparency.get_item(1)?)?;
    value = value.add(second)?;
    let third = matrix.get_item(2)?.mul(transparency.get_item(2)?)?;
    value = value.add(third)?;
    value = value.add(matrix.get_item(3)?.mul(0.5_f64)?)?;
    let value = py.get_type::<PyInt>().call1((value,))?;
    let builtins = py.import("builtins")?;
    let value = builtins.getattr("min")?.call1((255, value))?;
    builtins.getattr("max")?.call1((0, value))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(super) fn prepare_convert_palette_alpha_before_dither(
    image: pillow_rs::Image,
    plan: pillow_rs::ConvertInputPlan,
    context: Option<pillow_rs::ExecutionContext>,
    py: Python<'_>,
) -> PyResult<(pillow_rs::Image, pillow_rs::ConvertInputPlan, bool)> {
    let (target_mode, transparency) = match plan {
        pillow_rs::ConvertInputPlan::Convert {
            target_mode,
            transparency:
                Some(
                    action @ (pillow_rs::ConvertTransparencyPreflight::PaletteAlphaTable(_)
                    | pillow_rs::ConvertTransparencyPreflight::PaletteAlphaIndex(_)),
                ),
        } => (target_mode, action),
        plan => return Ok((image, plan, false)),
    };
    let image = py
        .detach(move || image.convert_source_with_palette_alpha(Some(&transparency), context))
        .map_err(map_error)?;
    let plan = pillow_rs::ConvertInputPlan::Convert {
        target_mode,
        transparency: Some(pillow_rs::ConvertTransparencyPreflight::Preserve),
    };
    Ok((image, plan, true))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn convert_transparency_value_from_python(
    value: &Bound<'_, PyAny>,
    input_kind: pillow_rs::ConvertTransparencyInputKind,
) -> PyResult<pillow_rs::ConvertTransparencyValue> {
    if value.is_none() {
        return Ok(pillow_rs::ConvertTransparencyValue::None);
    }
    if let Ok(bytes) = value.cast::<PyBytes>() {
        return Ok(pillow_rs::ConvertTransparencyValue::Bytes(
            bytes.as_bytes().to_vec(),
        ));
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        if input_kind == pillow_rs::ConvertTransparencyInputKind::PaletteAlpha {
            return Ok(pillow_rs::ConvertTransparencyValue::Other("tuple".into()));
        }
        if matches!(
            input_kind,
            pillow_rs::ConvertTransparencyInputKind::LumaPixelColor
                | pillow_rs::ConvertTransparencyInputKind::RgbPixelColor
        ) {
            if !pixel_color_tuple_length_is_valid(input_kind, tuple.len()) {
                // Pillow validates the tuple arity before asking any component
                // to provide an integer value.
                return Ok(pillow_rs::ConvertTransparencyValue::Tuple(Vec::new()));
            }
            if tuple.len() == 1 {
                let item = tuple.get_item(0)?;
                if !item.is_instance_of::<PyInt>() {
                    // Pillow's scalar-color gate checks PyLong before parsing
                    // a one-element tuple; it does not invoke __index__ here.
                    return Ok(pillow_rs::ConvertTransparencyValue::Tuple(Vec::new()));
                }
            }
            let mut components = Vec::with_capacity(tuple.len());
            for (index, item) in tuple.iter().enumerate() {
                components.push(pixel_color_tuple_component_from_python(
                    &item,
                    input_kind,
                    tuple.len(),
                    index,
                )?);
            }
            return Ok(pillow_rs::ConvertTransparencyValue::Tuple(components));
        }
        let mut components = Vec::with_capacity(tuple.len());
        for item in tuple.iter() {
            if let Some(component) = convert_transparency_component_from_python(&item)? {
                components.push(component);
            } else {
                return Ok(pillow_rs::ConvertTransparencyValue::Other(
                    item.get_type().name()?.to_string(),
                ));
            }
        }
        return Ok(pillow_rs::ConvertTransparencyValue::Tuple(components));
    }
    if let Some(component) = convert_transparency_component_from_python(value)? {
        return Ok(pillow_rs::ConvertTransparencyValue::Scalar(component));
    }
    Ok(pillow_rs::ConvertTransparencyValue::Other(
        value.get_type().name()?.to_string(),
    ))
}

// Keep Pillow's ImagePalette lookup and histogram callback at the Python
// boundary; return the resolved entry in its native channel layout.
// TODO(pillow-rs#3ptb): audit every Pillow 13.0 palette-mode/rawmode pair,
// malformed-buffer error order, and source mutation timing against parity.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_tuple_color_from_python(
    value: &Bound<'_, PyAny>,
    owner: &Bound<'_, PyAny>,
    py: Python<'_>,
) -> PyResult<Option<Option<pillow_rs::ConvertPaletteColor>>> {
    let source_palette = owner.getattr("palette")?;
    let mode = source_palette.getattr("mode")?.extract::<String>()?;
    let palette_model = super::palette_model_from_python(&mode)?;
    let rawmode = source_palette.getattr("rawmode")?;
    let channels = match mode.as_str() {
        "RGB" => 3,
        "RGBA" | "CMYK" => 4,
        _ => return Err(PyValueError::new_err("unsupported palette mode")),
    };
    let palette_data = if rawmode.is_truthy()? {
        // Pillow's Convert path installs the raw palette on a temporary P
        // image before asking ImagePalette.getcolor to allocate the entry.
        // Let the typed Rust palette path decode supported layouts, then keep
        // ImagePalette.getcolor here for its histogram callback and mutation.
        let rawmode = rawmode.extract::<String>()?;
        let raw_data = source_palette.getattr("palette")?.extract::<Vec<u8>>()?;
        let mut palette_image =
            pillow_rs::Image::new(0, 0, pillow_rs::Mode::ImagePalette8, (0, 0, 0, 0))
                .map_err(map_error)?;
        palette_image
            .putpalette_with_model(&raw_data, palette_model, &rawmode)
            .map_err(map_error)?;
        let decoded = palette_image
            .getpalette_with_input(Some(mode.as_str()))
            .map_err(map_error)?
            .ok_or_else(|| PyValueError::new_err("image has no palette"))?;
        PyBytes::new(py, &decoded).into_any()
    } else {
        source_palette.getattr("palette")?
    };
    let image_palette = py
        .import("PIL")?
        .getattr("ImagePalette")?
        .getattr("ImagePalette")?
        .call1((mode.as_str(), palette_data))?;
    let index = match image_palette.call_method1("getcolor", (value, owner)) {
        Ok(index) => index.extract::<usize>()?,
        Err(error) if error.is_instance_of::<PyValueError>(py) => {
            let message = error.value(py).str()?.to_cow()?.into_owned();
            if message == "cannot allocate more than 256 colors" {
                return Ok(Some(None));
            }
            let wrapped =
                PyValueError::new_err("Couldn't allocate a palette color for transparency");
            wrapped.set_cause(py, Some(error));
            return Err(wrapped);
        }
        Err(error) => return Err(error),
    };
    let offset = index.saturating_mul(channels);
    let palette_data = image_palette.getattr("palette")?;
    let palette_len = palette_data.len()?;
    let mut color = [0; 4];
    for (channel, value) in color.iter_mut().take(channels).enumerate() {
        let index = offset.saturating_add(channel);
        *value = palette_byte_at_from_python(&palette_data, index, palette_len)?;
    }
    let palette_color = match mode.as_str() {
        "RGB" => pillow_rs::ConvertPaletteColor::Rgb([color[0], color[1], color[2]]),
        "RGBA" => pillow_rs::ConvertPaletteColor::Rgba(color),
        "CMYK" => pillow_rs::ConvertPaletteColor::Cmyk(color),
        _ => return Err(PyValueError::new_err("unsupported palette mode")),
    };
    Ok(Some(Some(palette_color)))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_byte_at_from_python(
    palette: &Bound<'_, PyAny>,
    index: usize,
    length: usize,
) -> PyResult<u8> {
    if index < length {
        palette.get_item(index)?.extract::<u8>()
    } else {
        Ok(0)
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn pixel_color_tuple_length_is_valid(
    input_kind: pillow_rs::ConvertTransparencyInputKind,
    length: usize,
) -> bool {
    match input_kind {
        pillow_rs::ConvertTransparencyInputKind::LumaPixelColor => length == 1,
        pillow_rs::ConvertTransparencyInputKind::RgbPixelColor => matches!(length, 1 | 3 | 4),
        _ => false,
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn pixel_color_tuple_component_from_python(
    value: &Bound<'_, PyAny>,
    input_kind: pillow_rs::ConvertTransparencyInputKind,
    tuple_len: usize,
    index: usize,
) -> PyResult<pillow_rs::PaletteColorComponent> {
    let value = match input_kind {
        pillow_rs::ConvertTransparencyInputKind::LumaPixelColor => value.extract::<i64>()?,
        pillow_rs::ConvertTransparencyInputKind::RgbPixelColor if tuple_len == 1 || index == 0 => {
            value.extract::<i64>()?
        }
        pillow_rs::ConvertTransparencyInputKind::RgbPixelColor => {
            i64::from(crate::inputs::c_int_from_python(value)?)
        }
        _ => {
            return Err(PyValueError::new_err(
                "non-pixel transparency route reached pixel tuple parser",
            ));
        }
    };
    Ok(pillow_rs::PaletteColorComponent::Integer(value))
}

// Pillow 13's P-source scalar transparency routes accept PyLong instances:
// P-to-alpha checks `isinstance(value, int)`, and P-to-color-key passes the
// scalar to ImagingCore.putpixel/getink, which uses PyLong_Check. Arbitrary
// `__index__` objects are not coerced by either route. Keep the typed carrier
// limited to those accepted numeric forms; verify output and errors in parity.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn convert_transparency_component_from_python(
    value: &Bound<'_, PyAny>,
) -> PyResult<Option<pillow_rs::PaletteColorComponent>> {
    if value.is_instance_of::<PyInt>() {
        return Ok(palette_integer_component_from_python(value));
    }
    if value.is_instance_of::<PyFloat>() {
        return value
            .extract::<f64>()
            .map(pillow_rs::PaletteColorComponent::Float)
            .map(Some);
    }
    Ok(None)
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn warn_byte_transparency(py: Python<'_>) -> PyResult<()> {
    PyErr::warn(
        py,
        &py.get_type::<PyUserWarning>(),
        pyo3::ffi::c_str!(
            "Palette images with Transparency expressed in bytes should be converted to RGBA images"
        ),
        1,
    )
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(super) fn warn_convert_output(
    warning: pillow_rs::ConvertWarning,
    py: Python<'_>,
) -> PyResult<()> {
    match warning {
        pillow_rs::ConvertWarning::TransparencyPaletteAllocation => PyErr::warn(
            py,
            &py.get_type::<PyUserWarning>(),
            pyo3::ffi::c_str!("Couldn't allocate palette entry for transparency"),
            1,
        ),
    }
}

#[pymethods]
impl PyImage {
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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
        Ok(pyimage_convert_with_metadata(
            slf,
            mode,
            matrix,
            dither,
            palette,
            colors,
            execution_context,
            None,
            None,
            None,
            py,
        )?
        .0)
    }

    // TODO(pillow-rs#3ptb): parity-check custom mode/matrix/dither callbacks,
    // P-to-alpha mutation across dither faults, and Image.info replacement.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[pyo3(signature = (mode=None, matrix=None, dither=None, palette=None, colors=None, execution_context=None, transparency_info=None, transparency_owner=None, source_mutation_state=None))]
    fn _convert_with_metadata(
        slf: Py<Self>,
        mode: Option<&Bound<'_, PyAny>>,
        matrix: Option<&Bound<'_, PyAny>>,
        dither: Option<&Bound<'_, PyAny>>,
        palette: Option<&Bound<'_, PyAny>>,
        colors: Option<&Bound<'_, PyAny>>,
        execution_context: Option<pyo3::PyRef<'_, PyExecutionContext>>,
        transparency_info: Option<&Bound<'_, PyDict>>,
        transparency_owner: Option<&Bound<'_, PyAny>>,
        source_mutation_state: Option<&Bound<'_, PyDict>>,
        py: Python<'_>,
    ) -> PyResult<ConvertMetadataResult> {
        pyimage_convert_with_metadata(
            slf,
            mode,
            matrix,
            dither,
            palette,
            colors,
            execution_context,
            transparency_info,
            transparency_owner,
            source_mutation_state,
            py,
        )
    }

    // TODO(pillow-rs#3ptb): verify matrix-transparency value and callback parity.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[pyo3(signature = (matrix, transparency, plan))]
    fn _convert_matrix_transparency(
        &self,
        matrix: &Bound<'_, PyAny>,
        transparency: &Bound<'_, PyAny>,
        plan: pyo3::PyRef<'_, PyConvertMatrixTransparency>,
        py: Python<'_>,
    ) -> PyResult<Py<PyAny>> {
        transform_matrix_transparency_from_python(matrix, transparency, plan.target_mode, py)
    }

    /// Applies Pillow's low-level matrix conversion through the typed core plan.
    // TODO(pillow-rs#3ptb): verify L/12 behavior, deterministic RGB/4 extension,
    // LAB/4 rejection, matrix output, and CPU architecture/fallback receipts.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
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
                    pillow_rs::ConvertInputPlan::Matrix {
                        target_mode,
                        transform_transparency: false,
                    },
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
}
