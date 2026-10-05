#[pyclass(name = "BatchExecutor")]
/// Python wrapper for an explicitly queued group of image operations.
pub struct PyBatchExecutor {
    inner: pillow_rs::BatchExecutor,
}

#[pyclass(name = "BatchColor3DLUT")]
/// Immutable, shared LUT parameters accepted by the explicit batch API.
pub struct PyBatchColor3DLut {
    size: (u32, u32, u32),
    table: Arc<[f64]>,
    channels: u32,
    target_mode: Option<String>,
}

#[pymethods]
impl PyBatchColor3DLut {
    #[new]
    fn new(filter: &Bound<'_, PyAny>) -> PyResult<Self> {
        let size = filter.getattr("size")?.extract::<(u32, u32, u32)>()?;
        let channels = filter.getattr("channels")?.extract::<u32>()?;
        let values = filter.getattr("table")?.extract::<Vec<f64>>()?;
        let prepared = pillow_rs::prepare_color3dlut(values, size, channels).map_err(map_error)?;
        let target_mode = filter.getattr("mode")?.extract::<Option<String>>()?;
        Ok(Self {
            size,
            table: Arc::from(prepared.table),
            channels,
            target_mode,
        })
    }
}

#[pymethods]
impl PyBatchExecutor {
    #[new]
    #[pyo3(signature = (queue=false, backend=None))]
    fn new(queue: bool, backend: Option<&str>) -> PyResult<Self> {
        let backend = match backend {
            Some(name) => Some(pillow_rs::Backend::parse(name).ok_or_else(|| {
                PyValueError::new_err("backend must be one of 'cpu', 'simd', or 'gpu'")
            })?),
            None => None,
        };
        Ok(Self {
            inner: pillow_rs::BatchExecutor::new(queue, backend),
        })
    }

    fn submit_grouped(
        &mut self,
        image: &Bound<'_, PyImage>,
        operation: &Bound<'_, PyAny>,
        py: Python<'_>,
    ) -> PyResult<usize> {
        let operation_type = operation.get_type().name()?.to_string();
        let operation = match operation_type.as_str() {
            "MedianFilter" => {
                let size = operation.getattr("size")?.extract::<i64>()?;
                let size = filter_size_from_python(size, false)?;
                pillow_rs::BatchOperation::MedianFilter { size }
            }
            "MaxFilter" => {
                let size = operation.getattr("size")?.extract::<i64>()?;
                let size = filter_size_from_python(size, false)?;
                pillow_rs::BatchOperation::MaxFilter { size }
            }
            "RankFilter" => {
                let size = operation.getattr("size")?.extract::<i64>()?;
                let size = filter_size_from_python(size, false)?;
                let rank = operation.getattr("rank")?.extract::<i64>()?;
                let rank = filter_rank_from_python(rank)?;
                pillow_rs::BatchOperation::RankFilter { size, rank }
            }
            "ExtractBand" => pillow_rs::BatchOperation::ExtractBand {
                channel: operation.getattr("channel")?.extract::<i32>()?,
            },
            "Grayscale" => pillow_rs::BatchOperation::Grayscale,
            "Invert" => pillow_rs::BatchOperation::Invert,
            "Brightness" => pillow_rs::BatchOperation::Brightness {
                factor: operation.getattr("factor")?.extract::<f64>()?,
            },
            "Multiply" => {
                let other = operation.getattr("image")?;
                let other = image_from_python(&other).ok_or_else(|| {
                    PyTypeError::new_err("ImageBatch.Multiply requires a PIL.Image.Image operand")
                })?;
                pillow_rs::BatchOperation::Multiply {
                    other: Box::new(other),
                }
            }
            "Paste" => {
                let source = operation.getattr("source")?;
                let source = image_from_python(&source).ok_or_else(|| {
                    PyTypeError::new_err("ImageBatch.Paste requires a PIL.Image.Image source")
                })?;
                let mask = operation.getattr("mask")?;
                let mask = image_from_python(&mask).ok_or_else(|| {
                    PyTypeError::new_err("ImageBatch.Paste requires a PIL.Image.Image mask")
                })?;
                pillow_rs::BatchOperation::Paste {
                    source: Box::new(source),
                    mask: Box::new(mask),
                }
            }
            "Composite" => {
                let background = operation.getattr("background")?;
                let background = image_from_python(&background).ok_or_else(|| {
                    PyTypeError::new_err(
                        "ImageBatch.Composite requires a PIL.Image.Image background",
                    )
                })?;
                let mask = operation.getattr("mask")?;
                let mask = image_from_python(&mask).ok_or_else(|| {
                    PyTypeError::new_err("ImageBatch.Composite requires a PIL.Image.Image mask")
                })?;
                pillow_rs::BatchOperation::Composite {
                    background: Box::new(background),
                    mask: Box::new(mask),
                }
            }
            "Expand" => {
                let border = operation.getattr("border")?.extract::<i64>()?;
                let border = filter_size_from_python(border, false)?;
                let fill = imageops_color_from_python(Some(&operation.getattr("fill")?));
                pillow_rs::BatchOperation::Expand { border, fill }
            }
            "BatchColor3DLUT" => {
                let lut = operation.extract::<PyRef<'_, PyBatchColor3DLut>>()?;
                pillow_rs::BatchOperation::Color3DLut {
                    size: lut.size,
                    table: Arc::clone(&lut.table),
                    channels: lut.channels,
                    target_mode: lut.target_mode.clone(),
                }
            }
            _ => {
                return Err(PyTypeError::new_err(
                    "batch operation must be an ImageFilter.MedianFilter, ImageFilter.MaxFilter, or ImageFilter.RankFilter, ImageBatch.ExtractBand, ImageBatch.Grayscale, ImageBatch.Invert, ImageBatch.Brightness, ImageBatch.Multiply, ImageBatch.Paste, ImageBatch.Composite, ImageBatch.Expand, or ImageBatch.Color3DLUT instance",
                ));
            }
        };
        let source = image.borrow().inner.clone();
        py.detach(|| self.inner.submit_grouped(source, operation))
            .map_err(map_error)
    }

    fn submit_prepared_pipeline(
        &mut self,
        image: &Bound<'_, PyImage>,
        py: Python<'_>,
    ) -> PyResult<usize> {
        let pipeline = image.borrow().inner.clone();
        py.detach(|| self.inner.submit_prepared_pipeline(pipeline))
            .map_err(map_error)
    }

    fn join(&mut self, py: Python<'_>) -> PyResult<Vec<PyImage>> {
        py.detach(|| self.inner.join())
            .map(|images| images.into_iter().map(|inner| PyImage { inner }).collect())
            .map_err(map_error)
    }
}
