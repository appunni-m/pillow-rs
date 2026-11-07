use pyo3::prelude::{Bound, PyAny, PyErr, PyRef, PyResult, Python};
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::types::{
    PyAnyMethods, PyInt, PyIterator, PySet, PySetMethods, PyTuple,
};

use crate::PyImage;

#[derive(Clone)]
struct Point<'py> {
    x: Bound<'py, PyAny>,
    y: Bound<'py, PyAny>,
    key: Option<Bound<'py, PyTuple>>,
}

struct PythonFloodFillAccess<'py, 'a> {
    py: Python<'py>,
    pixel: &'a Bound<'py, PyAny>,
}

impl<'py> Point<'py> {
    fn as_tuple(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        if let Some(key) = &self.key {
            return Ok(key.clone());
        }
        PyTuple::new(py, [self.x.clone(), self.y.clone()])
    }

    fn with_key(
        py: Python<'py>,
        x: Bound<'py, PyAny>,
        y: Bound<'py, PyAny>,
    ) -> PyResult<Self> {
        let key = PyTuple::new(py, [x.clone(), y.clone()])?;
        Ok(Self { x, y, key: Some(key) })
    }
}

impl<'py, 'a> pillow_rs::FloodFillAccess for PythonFloodFillAccess<'py, 'a> {
    type Point = Point<'py>;
    type Pixel = Bound<'py, PyAny>;
    type Threshold = Bound<'py, PyAny>;
    type ColorVector = Bound<'py, PyAny>;
    type ColorDifference = Bound<'py, PyAny>;
    type ColorSum = Bound<'py, PyAny>;
    type Frontier = Bound<'py, PySet>;
    type FrontierIter = Bound<'py, PyIterator>;
    type Error = PyErr;

    fn new_frontier(&mut self) -> Result<Self::Frontier, Self::Error> {
        PySet::empty(self.py).map_err(Into::into)
    }

    fn frontier_is_empty(&mut self, frontier: &Self::Frontier) -> Result<bool, Self::Error> {
        Ok(frontier.is_empty())
    }

    fn frontier_iter(
        &mut self,
        frontier: &Self::Frontier,
    ) -> Result<Self::FrontierIter, Self::Error> {
        PyIterator::from_object(frontier.as_any()).map_err(Into::into)
    }

    fn frontier_next(
        &mut self,
        iter: &mut Self::FrontierIter,
    ) -> Result<Option<Self::Point>, Self::Error> {
        let Some(coordinates) = iter.next().transpose()? else {
            return Ok(None);
        };
        Ok(Some(Point {
            x: coordinates.get_item(0)?,
            y: coordinates.get_item(1)?,
            key: None,
        }))
    }

    fn frontier_contains(
        &mut self,
        frontier: &Self::Frontier,
        point: &Self::Point,
    ) -> Result<bool, Self::Error> {
        frontier
            .contains(point.as_tuple(self.py)?.as_any())
            .map_err(Into::into)
    }

    fn frontier_insert(
        &mut self,
        frontier: &mut Self::Frontier,
        point: Self::Point,
    ) -> Result<(), Self::Error> {
        frontier.add(point.as_tuple(self.py)?).map_err(Into::into)
    }

    fn neighbors(&mut self, point: &Self::Point) -> Result<[Self::Point; 4], Self::Error> {
        let right = point.x.add(1)?;
        let right = Point::with_key(self.py, right, point.y.clone())?;
        let left = point.x.sub(1)?;
        let left = Point::with_key(self.py, left, point.y.clone())?;
        let down = point.y.add(1)?;
        let down = Point::with_key(self.py, point.x.clone(), down)?;
        let up = point.y.sub(1)?;
        let up = Point::with_key(self.py, point.x.clone(), up)?;
        Ok([
            right,
            left,
            down,
            up,
        ])
    }

    fn point_is_nonnegative(&mut self, point: &Self::Point) -> Result<bool, Self::Error> {
        if point.x.lt(0)? {
            return Ok(false);
        }
        Ok(!point.y.lt(0)?)
    }

    fn get_pixel(&mut self, point: &Self::Point) -> Result<Self::Pixel, Self::Error> {
        self.pixel.get_item(point.as_tuple(self.py)?).map_err(Into::into)
    }

    fn set_pixel(
        &mut self,
        point: &Self::Point,
        value: &Self::Pixel,
    ) -> Result<(), Self::Error> {
        self.pixel
            .set_item(point.as_tuple(self.py)?, value.clone())
            .map_err(Into::into)
    }

    fn color_vectors(
        &mut self,
        first: &Self::Pixel,
        second: &Self::Pixel,
    ) -> Result<(Self::ColorVector, Self::ColorVector), Self::Error> {
        Ok((
            tuple_or_scalar(self.py, first)?,
            tuple_or_scalar(self.py, second)?,
        ))
    }

    fn color_vector_len(&mut self, vector: &Self::ColorVector) -> Result<usize, Self::Error> {
        vector.len().map_err(Into::into)
    }

    fn color_difference_at(
        &mut self,
        first: &Self::ColorVector,
        second: &Self::ColorVector,
        index: usize,
    ) -> Result<Self::ColorDifference, Self::Error> {
        let index = isize::try_from(index).map_err(|_| {
            pyo3::exceptions::PyOverflowError::new_err("color tuple is too large")
        })?;
        let first_sample = first.get_item(index)?;
        let second_sample = second.get_item(index)?;
        first_sample.sub(&second_sample)?.abs().map_err(Into::into)
    }

    fn color_sum_start(&mut self) -> Result<Self::ColorSum, Self::Error> {
        Ok(PyInt::new(self.py, 0).into_any())
    }

    fn add_color_difference(
        &mut self,
        sum: Self::ColorSum,
        difference: Self::ColorDifference,
    ) -> Result<Self::ColorSum, Self::Error> {
        sum.add(difference).map_err(Into::into)
    }

    fn color_sum_is_within_threshold(
        &mut self,
        sum: &Self::ColorSum,
        threshold: &Self::Threshold,
    ) -> Result<bool, Self::Error> {
        sum.le(threshold).map_err(Into::into)
    }

    fn differs_from_fill_and_border(
        &mut self,
        pixel: &Self::Pixel,
        value: &Self::Pixel,
        border: &Self::Pixel,
    ) -> Result<bool, Self::Error> {
        let excluded = PyTuple::new(self.py, [value.clone(), border.clone()])?;
        excluded.contains(pixel).map(|contains| !contains).map_err(Into::into)
    }

    fn is_swallowed_error(&self, error: &Self::Error) -> bool {
        error.is_instance_of::<PyValueError>(self.py)
            || error.is_instance_of::<PyIndexError>(self.py)
    }
}

fn tuple_or_scalar<'py>(
    py: Python<'py>,
    value: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    if value.is_instance(&py.get_type::<PyTuple>())? {
        return Ok(value.clone());
    }
    PyTuple::new(py, [value.clone()]).map(|tuple| tuple.into_any())
}

fn prepare_native_image(
    native_image: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<pillow_rs::PreparedFloodFill>> {
    let Some(native_image) = native_image else {
        return Ok(None);
    };
    let Ok(image) = native_image.extract::<PyRef<'_, PyImage>>() else {
        return Ok(None);
    };
    pillow_rs::prepare_floodfill(&image.inner)
        .map(Some)
        .map_err(super::map_error)
}

pub(super) fn execute<'py>(
    py: Python<'py>,
    pixel: &Bound<'py, PyAny>,
    x: &Bound<'py, PyAny>,
    y: &Bound<'py, PyAny>,
    value: &Bound<'py, PyAny>,
    border: Option<&Bound<'py, PyAny>>,
    threshold: &Bound<'py, PyAny>,
    native_image: Option<&Bound<'py, PyAny>>,
) -> PyResult<()> {
    let prepared = prepare_native_image(native_image)?;
    let point = Point { x: x.clone(), y: y.clone(), key: None };
    let border = border.filter(|value| !value.is_none());
    let mut access = PythonFloodFillAccess { py, pixel };
    match pillow_rs::execute_floodfill(
        prepared.as_ref(),
        &mut access,
        point,
        value.clone(),
        border.cloned(),
        threshold.clone(),
    ) {
        Ok(()) => Ok(()),
        Err(pillow_rs::FloodFillError::Strategy(error)) => Err(super::map_error(error)),
        Err(pillow_rs::FloodFillError::Callback(error)) => Err(error),
    }
}
