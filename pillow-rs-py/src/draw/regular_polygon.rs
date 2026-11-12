use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyIndexError, PyOverflowError, PyTypeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyErr, PyModule, PyResult, Python};
use pyo3::types::{PyAnyMethods, PyFloat, PyInt, PyList, PyTuple};

struct RawBoundingCircle<'py> {
    center_x: Option<Bound<'py, PyAny>>,
    center_y: Option<Bound<'py, PyAny>>,
    radius: Bound<'py, PyAny>,
    nested: bool,
}

// TODO(pillow-rs#3ptb): cover stateful list/tuple subclasses and numeric
// subclasses whose arithmetic overrides affect Pillow's vertex calculation.
// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
pub(super) fn regular_polygon_inputs<'py>(
    py: Python<'py>,
    bounding_circle: &Bound<'py, PyAny>,
    n_sides: &Bound<'py, PyAny>,
    rotation: &Bound<'py, PyAny>,
) -> PyResult<(
    pillow_rs::RegularPolygonCircle,
    pillow_rs::RegularPolygonSides,
    f64,
)> {
    validate_side_count(n_sides)?;
    let circle = parse_bounding_circle(py, bounding_circle)?;
    validate_radius(&circle.radius)?;
    validate_rotation(rotation)?;

    let _side_count_as_float = n_sides.extract::<f64>()?;
    let n_sides = n_sides.extract::<isize>().map_err(|error| {
        if error.is_instance_of::<PyOverflowError>(py) {
            PyOverflowError::new_err("Python int too large to convert to C ssize_t")
        } else {
            error
        }
    })?;
    let n_sides = i64::try_from(n_sides)
        .map_err(|_| PyOverflowError::new_err("Python int too large to convert to C ssize_t"))?;
    let rotation = rotation.extract::<f64>()?;
    let radius = circle.radius.extract::<f64>()?;
    let center_x = circle
        .center_x
        .ok_or_else(|| PyIndexError::new_err("list index out of range"))?
        .extract::<f64>()?;
    let center_y = circle
        .center_y
        .ok_or_else(|| PyIndexError::new_err("list index out of range"))?
        .extract::<f64>()?;
    let circle = if circle.nested {
        pillow_rs::RegularPolygonCircle::Nested(center_x, center_y, radius)
    } else {
        pillow_rs::RegularPolygonCircle::Flat(center_x, center_y, radius)
    };
    Ok((
        circle,
        pillow_rs::RegularPolygonSides::Value(n_sides),
        rotation,
    ))
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
fn validate_side_count(n_sides: &Bound<'_, PyAny>) -> PyResult<()> {
    if !n_sides.is_instance_of::<PyInt>() {
        return Err(PyTypeError::new_err("n_sides should be an int"));
    }
    if n_sides.rich_compare(3, CompareOp::Lt)?.is_truthy()? {
        return Err(PyValueError::new_err("n_sides should be an int > 2"));
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
fn parse_bounding_circle<'py>(
    py: Python<'py>,
    bounding_circle: &Bound<'py, PyAny>,
) -> PyResult<RawBoundingCircle<'py>> {
    if !is_list_or_tuple(bounding_circle) {
        return Err(PyTypeError::new_err("bounding_circle should be a sequence"));
    }
    match bounding_circle.len()? {
        3 => parse_flat_circle(py, bounding_circle),
        2 => parse_nested_circle(py, bounding_circle),
        _ => Err(invalid_circle_shape()),
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
fn parse_flat_circle<'py>(
    py: Python<'py>,
    bounding_circle: &Bound<'py, PyAny>,
) -> PyResult<RawBoundingCircle<'py>> {
    if !all_numeric(bounding_circle)? {
        return Err(invalid_circle_numbers());
    }
    let values = python_list(py, bounding_circle)?;
    let length = values.len()?;
    if length == 0 {
        return Err(PyValueError::new_err(
            "not enough values to unpack (expected at least 1, got 0)",
        ));
    }
    let center_length = length - 1;
    let center_x = (center_length > 0)
        .then(|| values.get_item(0).ok())
        .flatten();
    let center_y = (center_length > 1)
        .then(|| values.get_item(1).ok())
        .flatten();
    let radius = values.get_item(length - 1)?;
    Ok(RawBoundingCircle {
        center_x,
        center_y,
        radius,
        nested: false,
    })
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn parse_nested_circle<'py>(
    py: Python<'py>,
    bounding_circle: &Bound<'py, PyAny>,
) -> PyResult<RawBoundingCircle<'py>> {
    let first = bounding_circle.get_item(0)?;
    if !is_list_or_tuple(&first) {
        return Err(invalid_circle_shape());
    }
    let center_for_numbers = bounding_circle.get_item(0)?;
    if !all_numeric(&center_for_numbers)? || !is_numeric(&bounding_circle.get_item(1)?) {
        return Err(invalid_circle_numbers());
    }
    let center_for_length = bounding_circle.get_item(0)?;
    if center_for_length.len()? != 2 {
        return Err(PyValueError::new_err(
            "bounding_circle centre should contain 2D coordinates (e.g. (x, y))",
        ));
    }
    let center = python_list(py, &bounding_circle.get_item(0)?)?;
    Ok(RawBoundingCircle {
        center_x: center.get_item(0).ok(),
        center_y: center.get_item(1).ok(),
        radius: bounding_circle.get_item(1)?,
        nested: true,
    })
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn all_numeric(sequence: &Bound<'_, PyAny>) -> PyResult<bool> {
    for item in sequence.try_iter()? {
        if !is_numeric(&item?) {
            return Ok(false);
        }
    }
    Ok(true)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
fn is_numeric(value: &Bound<'_, PyAny>) -> bool {
    value.is_instance_of::<PyInt>() || value.is_instance_of::<PyFloat>()
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
fn is_list_or_tuple(value: &Bound<'_, PyAny>) -> bool {
    value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>()
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
fn python_list<'py>(py: Python<'py>, value: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    PyModule::import(py, "builtins")?
        .getattr("list")?
        .call1((value,))
}

// TODO(pillow-rs#3ptb): add parity coverage for zero and negative radii.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
fn validate_radius(radius: &Bound<'_, PyAny>) -> PyResult<()> {
    if radius.rich_compare(0, CompareOp::Le)?.is_truthy()? {
        return Err(PyValueError::new_err(
            "bounding_circle radius should be > 0",
        ));
    }
    Ok(())
}

// VERIFY: source=reviewed@2026-11-12; parity=pass@regular-polygon-prepush-final2-192x9; benchmark=pass@regular-polygon-standard-default-neon/pil-imagedraw-imagedraw.regular-polygon.standard
fn validate_rotation(rotation: &Bound<'_, PyAny>) -> PyResult<()> {
    if is_numeric(rotation) {
        Ok(())
    } else {
        Err(PyValueError::new_err("rotation should be an int or float"))
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
fn invalid_circle_shape() -> PyErr {
    PyValueError::new_err(
        "bounding_circle should contain 2D coordinates and a radius (e.g. (x, y, r) or ((x, y), r) )",
    )
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=n/a
fn invalid_circle_numbers() -> PyErr {
    PyValueError::new_err("bounding_circle should only contain numeric data")
}
