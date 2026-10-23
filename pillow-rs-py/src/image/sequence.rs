use crate::*;
use pyo3::{pyclass, pymethods};

/// Thin host handle for the Rust-owned ImageSequence iterator state.
#[pyclass(name = "Iterator", unsendable)]
pub struct PyImageSequenceIterator {
    image: Py<PyAny>,
    state: pillow_rs::ImageSequenceIterator,
}

#[pymethods]
impl PyImageSequenceIterator {
    #[new]
    fn new(im: Py<PyAny>, py: Python<'_>) -> PyResult<Self> {
        let bound = im.bind(py);
        if !bound.hasattr("seek")? {
            return Err(pyo3::exceptions::PyAttributeError::new_err(
                "im must have seek method",
            ));
        }
        let min_frame = match bound.getattr("_min_frame") {
            Ok(value) => value.extract::<u32>()?,
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyAttributeError>(py) => 0,
            Err(error) => return Err(error),
        };
        Ok(Self {
            image: im,
            state: pillow_rs::ImageSequenceIterator::new(min_frame),
        })
    }

    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let frame = self.state.position();
        match self.image.bind(py).call_method1("seek", (frame,)) {
            Ok(_) => {
                self.state.advance();
                Ok(self.image.clone_ref(py))
            }
            Err(error) if error.is_instance_of::<pyo3::exceptions::PyEOFError>(py) => Err(
                pyo3::exceptions::PyStopIteration::new_err("end of sequence"),
            ),
            Err(error) => Err(error),
        }
    }
}
