use super::{
    DynamicImage, FormattedImageData, Image, PilError, PipelineOp, ScalarImageSamples,
    validated_band_index,
};
use crate::compute::registry::support::StrategySelection;
use crate::compute::{Backend, PipelineOperationTelemetryCapture};
use crate::raster::Mode;

fn with_getdata_receipt<T>(
    requested_backend: Option<Backend>,
    selection: StrategySelection,
    operation_receipts: &mut PipelineOperationTelemetryCapture,
    output_bytes: impl FnOnce(&T) -> usize,
    execute: impl FnOnce() -> Result<T, PilError>,
) -> Result<T, PilError> {
    // TODO(pillow-rs#3ptb): aggregate source graph timing/resources; this
    // receipt currently measures only the terminal host read.
    let telemetry =
        crate::compute::begin_eager_cpu_operation_telemetry("GetData", requested_backend);
    operation_receipts.restore();
    match execute() {
        Ok(result) => {
            crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                telemetry,
                output_bytes(&result),
                "GetData",
                selection,
            );
            Ok(result)
        }
        Err(error) => {
            crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
            Err(error)
        }
    }
}

impl Image {
    fn getdata_band(
        &self,
        image: &DynamicImage,
        band: i32,
        operation_receipts: &mut PipelineOperationTelemetryCapture,
    ) -> Result<Vec<u8>, PilError> {
        // Pillow validates the requested logical band before extracting it.
        // Keep this check in the core so every binding observes the same
        // contract instead of silently clamping to RGBA channel 3.
        let mode = image.mode();
        let band_index = validated_band_index(band, mode.band_count())?;
        if crate::compute::pool_cpu::ops::analysis::ops::supports_get_data_band_into(mode) {
            let dimensions = crate::checked_dims::CheckedDims::new_allow_empty(
                image.width(),
                image.height(),
                1,
            )?;
            let mut output = dimensions.alloc_buffer();
            let terminal_mode = self.mode_identity()?;
            let expected = self.getdata_band_len_for_mode(terminal_mode, band)?;
            if output.len() != expected {
                return Err(PilError::InternalError(
                    "getdata band output length mismatch".into(),
                ));
            }
            let terminal_band_index =
                usize::try_from(band).expect("getdata band was validated as nonnegative");
            let backend_band =
                u8::try_from(terminal_band_index).expect("Pillow band indices fit in u8");
            self.write_getdata_band_into(
                terminal_mode,
                terminal_band_index,
                backend_band,
                &mut output,
                operation_receipts,
            )?;
            return Ok(output);
        }

        // Scalar modes intentionally have no caller-owned band buffer API.
        // Select only their direct byte-conversion strategy; byte modes select
        // GetDataBandInto exactly once in the caller-owned path above.
        let (requested_backend, context) = self.selected_getdata_route()?;
        let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_data_band(
            mode, &context,
        )?;
        if selection.implementation.operation
            != crate::compute::registry::support::OperationId::GetDataBand
            || selection.implementation.mode != mode
        {
            return Err(PilError::InternalError(
                "GetDataBand selection did not match its DynamicImage mode".into(),
            ));
        }
        with_getdata_receipt(
            requested_backend,
            selection,
            operation_receipts,
            Vec::len,
            || {
                crate::compute::pool_cpu::ops::analysis::ops::get_data_band(
                    image,
                    band_index,
                    selection,
                )
            },
        )
    }

    /// Returns pixel data as a flat sequence of channel bytes.
    ///
    /// When `band` is `Some`, returns only that channel. Negative bands are not
    /// interpreted here; callers should pass a non-negative band index. Without
    /// `band`, bytes are returned in the current decoded image channel order.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn getdata(&self, band: Option<i32>) -> Result<Vec<u8>, PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let img = self.materialized_shared()?;
        operation_receipts.capture();
        if let Some(band) = band {
            return self.getdata_band(&img, band, &mut operation_receipts);
        }
        let (requested_backend, context) = self.selected_getdata_route()?;
        let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_data(
            img.mode(), &context,
        )?;
        with_getdata_receipt(
            requested_backend,
            selection,
            &mut operation_receipts,
            Vec::len,
            || crate::compute::pool_cpu::ops::analysis::ops::get_data(img.as_ref(), selection),
        )
    }

    /// Return the number of samples produced by Pillow's formatted band read.
    ///
    /// This validates the same mode and band constraints as
    /// [`Image::getdata_formatted`]. For indexed `P` images, it reads the size
    /// from image metadata without materializing an image when its dimensions
    /// are already known. Callers can allocate the final host-owned byte buffer
    /// before asking the core to fill it.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when band selection is unsupported for
    /// the mode or the requested band is out of range. Other errors can occur
    /// while loading or materializing the image.
    pub fn getdata_band_len(&self, band: i32) -> Result<usize, PilError> {
        self.getdata_band_len_for_mode(self.mode_identity()?, band)
    }

    fn getdata_band_len_for_mode(&self, mode: Mode, band: i32) -> Result<usize, PilError> {
        let band_count =
            crate::compute::pool_cpu::ops::analysis::ops::get_data_band_into_count(mode)?;
        validated_band_index(band, band_count)?;
        let (width, height) = self.size()?;
        Ok(crate::checked_dims::CheckedDims::new_allow_empty(width, height, 1)?.total_pixels())
    }

    /// Borrow a directly stored single-band byte plane for a host binding to
    /// copy into its final immutable result object.
    ///
    /// This avoids materialization and a Rust-side copy for owned P, 1, and L
    /// images. The returned slice is a view of the image; bindings must copy
    /// it before returning so later image mutation cannot change a GetData
    /// snapshot.
    pub fn getdata_band_view(&self, band: i32) -> Result<Option<&[u8]>, PilError> {
        if band != 0 {
            return Ok(None);
        }
        let (requested_backend, context) = self.cpu_arch_execution_route()?;
        if requested_backend.is_some_and(|backend| backend != crate::compute::Backend::Cpu) {
            return Ok(None);
        }
        let mode = match self {
            Image::Paletted(_) => Mode::ImagePalette8,
            Image::Loaded(data) => data.image.mode(),
            _ => return Ok(None),
        };
        let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_data(
            mode, &context,
        )?;
        let values = match self {
            Image::Paletted(data) => {
                crate::compute::pool_cpu::ops::analysis::ops::get_data_palette_indices_view(
                    data.indices.as_bytes(),
                    selection,
                )?
            }
            Image::Loaded(data) => {
                let Some(values) =
                    crate::compute::pool_cpu::ops::analysis::ops::get_data_band_byte_view(
                        data.image.as_ref(),
                        selection,
                    )?
                else {
                    return Ok(None);
                };
                values
            }
            _ => return Ok(None),
        };
        crate::compute::record_eager_cpu_operation_receipt_with_selection(
            "GetData",
            requested_backend,
            values.len(),
            "GetData borrows the exact native byte carrier",
            selection,
        );
        Ok(Some(values))
    }

    /// Fill a caller-owned buffer with Pillow's formatted band-read bytes.
    ///
    /// The buffer length must equal [`Image::getdata_band_len`] for the same
    /// band. This form lets host bindings write directly into their final
    /// bytes object without an intermediate Rust `Vec`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when band selection is unsupported for
    /// the mode or the requested band is out of range, and
    /// [`PilError::InternalError`] when the output length is incorrect. Other
    /// errors can occur while loading or materializing the image.
    pub fn getdata_band_into(&self, band: i32, output: &mut [u8]) -> Result<(), PilError> {
        let mode = self.mode_identity()?;
        let expected = self.getdata_band_len_for_mode(mode, band)?;
        let band_index = usize::try_from(band).expect("getdata band was validated as nonnegative");
        let backend_band = u8::try_from(band_index).expect("Pillow band indices fit in u8");
        if output.len() != expected {
            return Err(PilError::InternalError(
                "getdata band output length mismatch".into(),
            ));
        }
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        self.write_getdata_band_into(
            mode,
            band_index,
            backend_band,
            output,
            &mut operation_receipts,
        )
    }

    fn write_getdata_band_into(
        &self,
        mode: Mode,
        band_index: usize,
        backend_band: u8,
        output: &mut [u8],
        operation_receipts: &mut PipelineOperationTelemetryCapture,
    ) -> Result<(), PilError> {
        let (selected_backend, context) = self.selected_getdata_route()?;
        if self.try_getdata_band_simd_sink(backend_band, selected_backend, context, output)? {
            return Ok(());
        }
        if self.try_getdata_band_backend(
            mode,
            backend_band,
            selected_backend,
            context,
            output,
            operation_receipts,
        )? {
            return Ok(());
        }

        let selection =
            crate::compute::pool_cpu::ops::analysis::ops::select_get_data_band_into(mode, &context)?;
        self.write_getdata_band_cpu(
            band_index,
            selected_backend,
            selection,
            output,
            operation_receipts,
        )
    }

    fn selected_getdata_route(
        &self,
    ) -> Result<
        (
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        ),
        PilError,
    > {
        self.cpu_arch_execution_route()
    }

    fn try_getdata_band_simd_sink(
        &self,
        band: u8,
        selected_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
        output: &mut [u8],
    ) -> Result<bool, PilError> {
        if selected_backend != Some(crate::compute::Backend::Simd)
            || !matches!(self, Image::Loaded(_))
        {
            return Ok(false);
        }
        let image = self.materialized_shared()?;
        let operation = PipelineOp::ExtractBand { index: band };
        let handled = crate::compute::execute_eager_cpu_arch_bytes_with_context(
            "GetData",
            image.as_ref(),
            context,
            selected_backend,
            output,
            |context, output| {
                let prepared = crate::compute::registry::prepare_cpu(
                    &operation,
                    crate::compute::registry::support::OperationInputs {
                        image: image.as_ref(),
                        palette: crate::compute::registry::support::PaletteInput::default(),
                    },
                    context,
                )?;
                let selection = *prepared.selection();
                let Some(output_work) = prepared.execute_into(output)? else {
                    return Ok(None);
                };
                Ok(Some((selection, output_work)))
            },
        )?;
        Ok(handled.is_some())
    }

    fn try_getdata_band_backend(
        &self,
        mode: Mode,
        band: u8,
        selected_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
        output: &mut [u8],
        operation_receipts: &mut PipelineOperationTelemetryCapture,
    ) -> Result<bool, PilError> {
        let Some(backend @ (crate::compute::Backend::Simd | crate::compute::Backend::Gpu)) =
            selected_backend
        else {
            return Ok(false);
        };
        // TODO(pillow-rs#3ptb): move GPU eligibility into the GPU support
        // declaration when it can compose this CPU terminal contract.
        let supported =
            crate::compute::pool_cpu::ops::analysis::ops::supports_get_data_band_terminal(
                mode, backend, &context,
            );
        if !supported {
            return Ok(false);
        }

        // Keep terminal projection in the lazy graph so the selected backend
        // can fuse it with pending image work and report its real receipt. P
        // and LAB stay on the host path: P needs raw indices, while LAB's a/b
        // samples need Pillow's signed-byte offset.
        let extracted = Image::push_op(self, PipelineOp::ExtractBand { index: band })
            .use_backend(backend);
        let image = extracted.materialized_shared()?;
        operation_receipts.capture();
        operation_receipts.restore();
        let values = image.as_bytes();
        if values.len() != output.len() {
            return Err(PilError::InternalError(
                "getdata band backend output length mismatch".into(),
            ));
        }
        output.copy_from_slice(values);
        Ok(true)
    }

    fn write_getdata_band_cpu(
        &self,
        band: usize,
        selected_backend: Option<crate::compute::Backend>,
        selection: crate::compute::registry::support::StrategySelection,
        output: &mut [u8],
        operation_receipts: &mut PipelineOperationTelemetryCapture,
    ) -> Result<(), PilError> {
        let image = match self {
            Image::Paletted(_) => None,
            _ => Some(self.materialized_shared()?),
        };
        operation_receipts.capture();
        let telemetry =
            crate::compute::begin_eager_cpu_operation_telemetry("GetData", selected_backend);
        operation_receipts.restore();
        let result = (|| {
            if let Image::Paletted(data) = self {
                return crate::compute::pool_cpu::ops::analysis::ops::get_data_palette_indices_band_into(
                    data.indices.as_bytes(),
                    band,
                    selection,
                    output,
                );
            }

            let Some(img) = image.as_deref() else {
                return Err(PilError::InternalError(
                    "GetData CPU band route omitted its materialized image".into(),
                ));
            };
            crate::compute::pool_cpu::ops::analysis::ops::get_data_band_into(
                img,
                band,
                selection,
                output,
            )
        })();
        if result.is_ok() {
            crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                telemetry,
                output.len(),
                "GetData",
                selection,
            );
        } else {
            crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
        }
        result
    }

    /// Returns `getdata` values with Pillow's scalar/multiband shape applied.
    ///
    /// The byte extraction and logical band count are core behavior. Host
    /// bindings should only convert [`FormattedImageData`] into their native
    /// list/tuple representation.
    pub fn getdata_formatted(&self, band: Option<i32>) -> Result<FormattedImageData, PilError> {
        let mode = self.mode_identity()?;
        if matches!(mode, Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N) {
            if band.is_some() {
                // Pillow's ImagingCore.getdata rejects every explicit band
                // selection for I;16 modes with the mode error, rather than
                // the generic index error used by ordinary multi-band images.
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
            let image = self.materialized_shared()?;
            operation_receipts.capture();
            let (requested_backend, context) = self.selected_getdata_route()?;
            let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_data(
                mode, &context,
            )?;
            let values = with_getdata_receipt(
                requested_backend,
                selection,
                &mut operation_receipts,
                |values: &Vec<i32>| {
                    values.len().saturating_mul(std::mem::size_of::<i32>())
                },
                || {
                    crate::compute::pool_cpu::ops::analysis::ops::get_data_integer_samples(
                        image.as_ref(),
                        selection,
                    )
                },
            )?;
            return Ok(FormattedImageData::IntegerScalars(values));
        }
        if matches!(mode, Mode::ImageI32 | Mode::ImageF32) {
            if band.is_some() {
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
            let image = self.materialized_shared()?;
            operation_receipts.capture();
            let (requested_backend, context) = self.selected_getdata_route()?;
            let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_data(
                mode, &context,
            )?;
            let (formatted, _) = with_getdata_receipt(
                requested_backend,
                selection,
                &mut operation_receipts,
                |result: &(FormattedImageData, usize)| result.1,
                || {
                    let samples =
                        crate::compute::pool_cpu::ops::analysis::ops::get_data_scalar_samples(
                            image.as_ref(),
                            selection,
                        )?;
                    let (formatted, output_bytes) = match samples {
                        ScalarImageSamples::Integer(values) => {
                            let output_bytes =
                                values.len().saturating_mul(std::mem::size_of::<i32>());
                            (FormattedImageData::IntegerScalars(values), output_bytes)
                        }
                        ScalarImageSamples::Float(values) => {
                            let output_bytes =
                                values.len().saturating_mul(std::mem::size_of::<f64>());
                            (FormattedImageData::FloatScalars(values), output_bytes)
                        }
                    };
                    Ok((formatted, output_bytes))
                },
            )?;
            return Ok(formatted);
        }
        let raw = self.getdata(band)?;
        if band.is_some() {
            return Ok(FormattedImageData::Scalars(raw));
        }

        let band_count = mode.band_count();
        if band_count == 1 {
            return Ok(FormattedImageData::Scalars(raw));
        }

        Ok(FormattedImageData::Components(
            raw.chunks_exact(band_count)
                .map(|pixel| pixel.to_vec())
                .collect(),
        ))
    }
}
