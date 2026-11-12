use super::{
    Bound, PyAny, PyBytes, PyErr, PyInt, PyList, PyOverflowError, PyResult, PyTuple, PyTypeError,
    PyValueError, Python, c_int_from_python, chops_offset_from_python,
};
use pyo3::types::{
    PyAnyMethods, PyFloat, PySequence, PySequenceMethods, PyTupleMethods, PyTypeMethods,
};

pub(crate) fn putpixel_value_from_python(
    value: &Bound<'_, PyAny>,
    mode: pillow_rs::Mode,
) -> PyResult<pillow_rs::PutPixelValue> {
    use pillow_rs::PutPixelValue;
    let tuple = value.cast::<PyTuple>().ok();
    let scalar = if let Some(tuple) = tuple.filter(|tuple| tuple.len() == 1) {
        tuple.get_item(0)?
    } else {
        value.clone()
    };
    if mode == pillow_rs::Mode::ImageF32 {
        return scalar.extract::<f64>().map(PutPixelValue::Float);
    }
    if scalar.is_instance_of::<PyInt>() {
        return scalar
            .extract::<i64>()
            .map(PutPixelValue::Integer)
            .map_err(|error| {
                if error.is_instance_of::<PyOverflowError>(value.py()) {
                    PyOverflowError::new_err("int too big to convert")
                } else {
                    error
                }
            });
    }
    let tuple_length = tuple.map(|tuple| tuple.len());
    let (length, palette_color) = putpixel_component_shape_from_python(value, mode, tuple_length)?;
    putpixel_components_from_python(value, length, palette_color)
}

pub(crate) fn putpixel_component_shape_from_python(
    value: &Bound<'_, PyAny>,
    mode: pillow_rs::Mode,
    tuple_length: Option<usize>,
) -> PyResult<(usize, bool)> {
    // Only P/PA's public palette wrapper accepts RGB(A) lists. Ordinary
    // getink accepts tuples, and unwraps a singleton before numeric coercion.
    let palette_mode = matches!(
        mode,
        pillow_rs::Mode::ImagePalette8 | pillow_rs::Mode::ImagePaletteAlpha8
    );
    let palette_list =
        palette_mode && value.is_instance_of::<PyList>() && matches!(value.len()?, 3 | 4);
    let length = if palette_list {
        Some(value.len()?)
    } else {
        tuple_length
    };
    let palette_color = palette_mode && matches!(length, Some(3 | 4));
    let single_band = matches!(
        mode,
        pillow_rs::Mode::One
            | pillow_rs::Mode::ImageLuma8
            | pillow_rs::Mode::ImagePalette8
            | pillow_rs::Mode::ImageI32
            | pillow_rs::Mode::ImageI16
            | pillow_rs::Mode::ImageI16L
            | pillow_rs::Mode::ImageI16B
            | pillow_rs::Mode::ImageI16N
    );
    if single_band && !palette_color {
        return Err(PyTypeError::new_err(
            "color must be int or single-element tuple",
        ));
    }
    let Some(length) = length else {
        return Err(PyTypeError::new_err("color must be int or tuple"));
    };
    let two_band = matches!(
        mode,
        pillow_rs::Mode::ImageLumaA8
            | pillow_rs::Mode::ImageLaPremultiplied8
            | pillow_rs::Mode::ImagePaletteAlpha8
    );
    if !palette_color
        && ((two_band && !matches!(length, 1 | 2)) || (!two_band && !matches!(length, 3 | 4)))
    {
        return Err(PyTypeError::new_err(if two_band {
            "color must be int, or tuple of one or two elements"
        } else {
            "color must be int, or tuple of one, three or four elements"
        }));
    }
    if length == 1 {
        return Err(pyo3::exceptions::PySystemError::new_err(
            "new style getargs format but argument is not a tuple",
        ));
    }
    Ok((length, palette_color))
}

pub(crate) fn putpixel_components_from_python(
    value: &Bound<'_, PyAny>,
    length: usize,
    palette_color: bool,
) -> PyResult<pillow_rs::PutPixelValue> {
    use pillow_rs::PutPixelValue;

    if palette_color
        && (0..length).any(|index| {
            value
                .get_item(index)
                .is_ok_and(|component| component.is_instance_of::<PyFloat>())
        })
    {
        return value
            .extract::<Vec<f64>>()
            .map(PutPixelValue::FloatComponents);
    }
    let mut components = Vec::with_capacity(length);
    for index in 0..length {
        let component = value.get_item(index)?;
        let integer = if palette_color && index < 3 {
            component.extract::<i64>().map_err(|error| {
                if error.is_instance_of::<PyOverflowError>(value.py()) {
                    PyValueError::new_err("bytes must be in range(0, 256)")
                } else {
                    error
                }
            })?
        } else if index == 0 || palette_color {
            component.extract::<i64>().map_err(|error| {
                if error.is_instance_of::<PyOverflowError>(value.py()) {
                    PyOverflowError::new_err("int too big to convert")
                } else {
                    error
                }
            })?
        } else {
            chops_offset_from_python(&component)? as i64
        };
        components.push(integer);
    }
    Ok(PutPixelValue::Components(components))
}

pub(crate) fn exact_rgb_tuple_from_python(value: &Bound<'_, PyAny>) -> Option<[u8; 3]> {
    let tuple = value.cast_exact::<PyTuple>().ok()?;
    if tuple.len() != 3 {
        return None;
    }
    let mut rgb = [0u8; 3];
    for (index, channel) in rgb.iter_mut().enumerate() {
        let value = tuple.get_item(index).ok()?;
        if value.cast_exact::<PyInt>().is_err() {
            return None;
        }
        *channel = value.extract::<u8>().ok()?;
    }
    Some(rgb)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn new_color_input_from_python(
    mode: pillow_rs::Mode,
    color: Option<&Bound<'_, PyAny>>,
) -> PyResult<pillow_rs::NewColorInput> {
    // TODO(pillow-rs#3ptb): freeze custom numeric, wide-tuple, P palette, and
    // size-callback output/fault cases; allocation-before-fill fault order is
    // still a source-level difference when construction fails for memory.
    let Some(value) = color else {
        return Ok(new_color_input(
            None, None, None, None, None, None, None, false,
        ));
    };
    if let Ok(string) = value.extract::<String>() {
        return Ok(new_color_input(
            Some(string),
            None,
            None,
            None,
            None,
            None,
            None,
            true,
        ));
    }

    if mode == pillow_rs::Mode::ImageF32 {
        let tuple = value.cast::<PyTuple>().ok();
        let scalar = match tuple.as_ref().filter(|tuple| tuple.len() == 1) {
            Some(tuple) => tuple.get_item(0)?,
            None => value.clone(),
        };
        return Ok(new_color_input(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(scalar.extract::<f64>()?),
            true,
        ));
    }

    let tuple = value.cast::<PyTuple>().ok();
    let tuple_length = tuple.as_ref().map(|tuple| tuple.len());
    let scalar = match tuple.as_ref().filter(|tuple| tuple.len() == 1) {
        Some(tuple) => tuple.get_item(0)?,
        None => value.clone(),
    };
    if scalar.is_instance_of::<PyInt>() {
        return Ok(new_color_integer_input(mode, new_color_integer(&scalar)?));
    }
    if mode.band_count() == 1 {
        return Err(new_color_type_error(mode));
    }
    if tuple_length == Some(1) && mode.band_count() == 2 {
        return Err(pyo3::exceptions::PySystemError::new_err(
            "new style getargs format but argument is not a tuple",
        ));
    }
    let Some(tuple) = tuple else {
        return Err(new_color_type_error(mode));
    };
    new_color_tuple_input(mode, tuple)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
#[allow(
    clippy::too_many_arguments,
    reason = "keeps the mode-classified constructor payload at one boundary"
)]
fn new_color_input(
    string: Option<String>,
    single: Option<u8>,
    rgb: Option<(u8, u8, u8)>,
    rgba: Option<(u8, u8, u8, u8)>,
    la: Option<(u8, u8)>,
    integer: Option<i32>,
    float: Option<f64>,
    provided: bool,
) -> pillow_rs::NewColorInput {
    pillow_rs::NewColorInput::from_parts(string, single, rgb, rgba, la, integer, float, provided)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn new_color_type_error(mode: pillow_rs::Mode) -> PyErr {
    if mode.band_count() == 1 {
        PyTypeError::new_err("color must be int or single-element tuple")
    } else {
        PyTypeError::new_err("color must be int or tuple")
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn new_color_integer(value: &Bound<'_, PyAny>) -> PyResult<i64> {
    value.extract::<i64>()
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
#[allow(
    clippy::cast_possible_truncation,
    reason = "the value is clamped to the u8 range before conversion"
)]
fn new_color_byte(value: i64) -> u8 {
    value.clamp(0, i64::from(u8::MAX)) as u8
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn new_color_integer_input(mode: pillow_rs::Mode, value: i64) -> pillow_rs::NewColorInput {
    if matches!(
        mode,
        pillow_rs::Mode::ImagePalette8 | pillow_rs::Mode::ImageLuma8 | pillow_rs::Mode::One
    ) {
        return new_color_input(
            None,
            Some(new_color_byte(value)),
            None,
            None,
            None,
            None,
            None,
            true,
        );
    }
    if matches!(
        mode,
        pillow_rs::Mode::ImageI32
            | pillow_rs::Mode::ImageI16
            | pillow_rs::Mode::ImageI16L
            | pillow_rs::Mode::ImageI16B
            | pillow_rs::Mode::ImageI16N
    ) {
        let [byte0, byte1, byte2, byte3, ..] = value.to_le_bytes();
        let integer = i32::from_le_bytes([byte0, byte1, byte2, byte3]);
        return new_color_input(None, None, None, None, None, Some(integer), None, true);
    }

    let [red, green, blue, alpha, ..] = value.to_le_bytes();
    new_color_input(
        None,
        None,
        None,
        Some((red, green, blue, alpha)),
        None,
        None,
        None,
        true,
    )
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn new_color_tuple_input(
    mode: pillow_rs::Mode,
    values: &Bound<'_, PyTuple>,
) -> PyResult<pillow_rs::NewColorInput> {
    let length = values.len();
    let bands = mode.band_count();
    if bands == 2 && !matches!(length, 1 | 2) {
        return Err(PyTypeError::new_err(
            "color must be int, or tuple of one or two elements",
        ));
    }
    if bands >= 3 && !matches!(length, 3 | 4) {
        return Err(PyTypeError::new_err(
            "color must be int, or tuple of one, three or four elements",
        ));
    }

    let first = new_color_byte(new_color_integer(&values.get_item(0)?)?);
    if bands == 1 {
        return Ok(new_color_input(
            None,
            Some(first),
            None,
            None,
            None,
            None,
            None,
            true,
        ));
    }
    let second = new_color_byte(i64::from(c_int_from_python(&values.get_item(1)?)?));
    if bands == 2 {
        return Ok(new_color_input(
            None,
            None,
            None,
            None,
            Some((first, second)),
            None,
            None,
            true,
        ));
    }
    let third = new_color_byte(i64::from(c_int_from_python(&values.get_item(2)?)?));
    if length == 3 {
        return Ok(new_color_input(
            None,
            None,
            Some((first, second, third)),
            None,
            None,
            None,
            None,
            true,
        ));
    }
    let alpha = new_color_byte(i64::from(c_int_from_python(&values.get_item(3)?)?));
    Ok(new_color_input(
        None,
        None,
        None,
        Some((first, second, third, alpha)),
        None,
        None,
        None,
        true,
    ))
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn new_size_from_python(
    value: &Bound<'_, PyAny>,
    _py: Python<'_>,
) -> PyResult<(u32, u32)> {
    let Ok(sequence) = value.cast::<PySequence>() else {
        let type_name = value.get_type().name()?.to_string();
        return Err(PyTypeError::new_err(format!(
            "argument 2 must be 2-item sequence, not {type_name}"
        )));
    };
    if value.is_instance_of::<PyBytes>() {
        let type_name = value.get_type().name()?.to_string();
        return Err(PyTypeError::new_err(format!(
            "argument 2 must be 2-item sequence, not {type_name}"
        )));
    }

    // Match Pillow's nested C-int size conversion after Python _check_size.
    let length = sequence.len()?;
    if length != 2 {
        return Err(PyTypeError::new_err(format!(
            "argument 2 must be sequence of length 2, not {length}"
        )));
    }

    let width = new_size_component(sequence, 0)?;
    let height = new_size_component(sequence, 1)?;
    if width < 0 || height < 0 {
        return Err(PyValueError::new_err("Width and height must be >= 0"));
    }
    let width =
        u32::try_from(width).map_err(|_| PyValueError::new_err("Width and height must be >= 0"))?;
    let height = u32::try_from(height)
        .map_err(|_| PyValueError::new_err("Width and height must be >= 0"))?;
    Ok((width, height))
}

pub(crate) fn frombytes_size_from_python(
    width: &Bound<'_, PyAny>,
    height: &Bound<'_, PyAny>,
) -> PyResult<(u32, u32)> {
    let width = c_int_from_python(width)?;
    let height = c_int_from_python(height)?;
    let width =
        u32::try_from(width).map_err(|_| PyValueError::new_err("Width and height must be >= 0"))?;
    let height = u32::try_from(height)
        .map_err(|_| PyValueError::new_err("Width and height must be >= 0"))?;
    Ok((width, height))
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn new_size_component(value: &Bound<'_, PySequence>, index: usize) -> PyResult<i32> {
    let item = match value.get_item(index) {
        Ok(item) => item,
        Err(_sequence_error) => {
            return Err(PyTypeError::new_err(format!(
                "argument 2, item {index} is not retrievable"
            )));
        }
    };
    c_int_from_python(&item)
}

pub(crate) fn putpixel_coordinates_from_python(xy: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
    let sequence = xy.cast::<pyo3::types::PySequence>().map_err(|_| {
        let name = if xy.is_none() {
            "None".into()
        } else {
            xy.get_type()
                .name()
                .map(|name| name.to_string())
                .unwrap_or_default()
        };
        PyTypeError::new_err(format!("argument 1 must be 2-item sequence, not {name}"))
    })?;
    let length = sequence.len()?;
    if length != 2 {
        return Err(PyTypeError::new_err(format!(
            "argument 1 must be sequence of length 2, not {length}"
        )));
    }
    Ok((
        c_int_from_python(&sequence.get_item(0)?)?,
        c_int_from_python(&sequence.get_item(1)?)?,
    ))
}

pub(crate) fn pixel_access_coordinates_from_python(xy: &Bound<'_, PyAny>) -> PyResult<(i32, i32)> {
    // Pillow's PixelAccess accepts tuples only; its C-long coordinates narrow
    // to C int, and float coordinates truncate toward zero.
    let tuple = xy
        .cast::<PyTuple>()
        .map_err(|_| PyTypeError::new_err("argument must be sequence of length 2"))?;
    if tuple.len() != 2 {
        return Err(PyTypeError::new_err(
            "argument must be sequence of length 2",
        ));
    }
    Ok((
        pixel_access_coordinate(&tuple.get_item(0)?)?,
        pixel_access_coordinate(&tuple.get_item(1)?)?,
    ))
}

fn pixel_access_coordinate(value: &Bound<'_, PyAny>) -> PyResult<i32> {
    if value.is_instance_of::<PyInt>() {
        return value
            .extract::<std::os::raw::c_long>()
            .map(|coordinate| coordinate as i32);
    }
    if value.is_instance_of::<PyFloat>() {
        return value.extract::<f64>().map(pixel_access_float_coordinate);
    }
    let integer = value
        .call_method0("__int__")
        .map_err(|_| PyTypeError::new_err("an integer is required"))?;
    if !integer.is_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err("an integer is required"));
    }
    integer
        .extract::<std::os::raw::c_long>()
        .map(|coordinate| coordinate as i32)
}

/// Gives float coordinates outside Pillow's C-int cast range stable Rust
/// semantics: in-range values truncate toward zero, overflow saturates, and
/// NaN maps to zero. Pillow's C cast has no portable result for those inputs.
fn pixel_access_float_coordinate(coordinate: f64) -> i32 {
    if coordinate.is_nan() {
        return 0;
    }
    if coordinate <= f64::from(i32::MIN) {
        return i32::MIN;
    }
    if coordinate >= f64::from(i32::MAX) {
        return i32::MAX;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the finite coordinate is within i32 bounds and Pillow truncates toward zero"
    )]
    {
        coordinate as i32
    }
}

pub(crate) fn transpose_input_from_python(
    method: &Bound<'_, PyAny>,
) -> PyResult<pillow_rs::TransposeInput> {
    if let Ok(value) = method.extract::<i64>() {
        return Ok(pillow_rs::TransposeInput::Index(value));
    }
    if let Ok(value) = method.extract::<String>() {
        return Ok(pillow_rs::TransposeInput::Name(value));
    }
    Ok(pillow_rs::TransposeInput::Invalid(
        method.get_type().name()?.to_string(),
    ))
}
