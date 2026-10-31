use super::{
    DynamicImage, FormattedImageData, Image, PilError, PipelineOp, ScalarImageSamples,
    select_interleaved_band_into, select_lab_band_into, validated_band_index,
};
use crate::raster::{FromPrimitive, Mode};

fn native_sample_count(width: u32, height: u32, channels: u8) -> Result<usize, PilError> {
    Ok(crate::checked_dims::CheckedDims::new_allow_empty(width, height, channels)?.total_bytes())
}

fn mode_carrier_mismatch(mode: Mode) -> PilError {
    PilError::InternalError(format!(
        "GetData mode {mode:?} does not match its native raster carrier"
    ))
}

fn narrow_native_samples<T: Copy>(
    samples: &[T],
    width: u32,
    height: u32,
    channels: u8,
    mode: Mode,
    convert: impl Fn(T) -> u8,
) -> Result<Vec<u8>, PilError> {
    let sample_count = native_sample_count(width, height, channels)?;
    let samples = samples
        .get(..sample_count)
        .ok_or_else(|| mode_carrier_mismatch(mode))?;
    Ok(samples.iter().copied().map(convert).collect())
}

fn getdata_byte_modes(image: &DynamicImage, mode: Mode) -> Result<Vec<u8>, PilError> {
    let output = match (mode, image) {
        (Mode::One, DynamicImage::One(image))
        | (Mode::ImageLuma8, DynamicImage::ImageLuma8(image))
        | (Mode::ImagePalette8, DynamicImage::ImagePalette8(image)) => {
            image.as_raw().clone()
        }
        (Mode::ImageLumaA8, DynamicImage::ImageLumaA8(image))
        | (Mode::ImageLaPremultiplied8, DynamicImage::ImageLaPremultiplied8(image))
        | (Mode::ImagePaletteAlpha8, DynamicImage::ImagePaletteAlpha8(image)) => {
            image.as_raw().clone()
        }
        (Mode::ImageRgb8, DynamicImage::ImageRgb8(image))
        | (Mode::ImageYCbCr8, DynamicImage::ImageYCbCr8(image))
        | (Mode::ImageHsv8, DynamicImage::ImageHsv8(image)) => image.as_raw().clone(),
        (Mode::ImageLab8, DynamicImage::ImageLab8(image)) => {
            let mut output = image.as_raw().clone();
            for pixel in output.chunks_exact_mut(3) {
                pixel[1] = pixel[1].wrapping_sub(128);
                pixel[2] = pixel[2].wrapping_sub(128);
            }
            output
        }
        (Mode::ImageRgba8, DynamicImage::ImageRgba8(image))
        | (Mode::ImageRgbPremultiplied8, DynamicImage::ImageRgbPremultiplied8(image))
        | (Mode::ImageRgbx8, DynamicImage::ImageRgbx8(image))
        | (Mode::ImageCmyk8, DynamicImage::ImageCmyk8(image))
        | (Mode::ImageI32, DynamicImage::ImageI32(image))
        | (Mode::ImageF32, DynamicImage::ImageF32(image)) => image.as_raw().clone(),
        _ => return Err(mode_carrier_mismatch(mode)),
    };
    Ok(output)
}

fn getdata_unsigned_luma16(image: &DynamicImage, mode: Mode) -> Result<Vec<u8>, PilError> {
    let image = match (mode, image) {
        (Mode::ImageI16, DynamicImage::ImageI16(image))
        | (Mode::ImageI16L, DynamicImage::ImageI16L(image))
        | (Mode::ImageI16B, DynamicImage::ImageI16B(image))
        | (Mode::ImageI16N, DynamicImage::ImageI16N(image)) => image,
        _ => return Err(mode_carrier_mismatch(mode)),
    };
    let count = native_sample_count(image.width(), image.height(), 1)?;
    let samples = image
        .as_raw()
        .get(..count)
        .ok_or_else(|| mode_carrier_mismatch(mode))?;
    Ok(samples
        .iter()
        .map(|sample| (*sample).min(u16::from(u8::MAX)) as u8)
        .collect())
}

fn getdata_internal_high_depth(image: &DynamicImage, mode: Mode) -> Result<Vec<u8>, PilError> {
    let output = match (mode, image) {
        // NO_PILLOW_SUPPORT: internal high-depth identities are Rust-only;
        // GetData narrows their native samples at this byte-return boundary.
        (Mode::ImageLumaA16, DynamicImage::ImageLumaA16(image)) => narrow_native_samples(
            image.as_raw(),
            image.width(),
            image.height(),
            2,
            mode,
            u8::from_primitive,
        )?,
        (Mode::ImageRgb16, DynamicImage::ImageRgb16(image)) => narrow_native_samples(
            image.as_raw(),
            image.width(),
            image.height(),
            3,
            mode,
            u8::from_primitive,
        )?,
        (Mode::ImageRgba16, DynamicImage::ImageRgba16(image)) => narrow_native_samples(
            image.as_raw(),
            image.width(),
            image.height(),
            4,
            mode,
            u8::from_primitive,
        )?,
        (Mode::ImageRgb32F, DynamicImage::ImageRgb32F(image)) => narrow_native_samples(
            image.as_raw(),
            image.width(),
            image.height(),
            3,
            mode,
            u8::from_primitive,
        )?,
        (Mode::ImageRgba32F, DynamicImage::ImageRgba32F(image)) => narrow_native_samples(
            image.as_raw(),
            image.width(),
            image.height(),
            4,
            mode,
            u8::from_primitive,
        )?,
        _ => return Err(mode_carrier_mismatch(mode)),
    };
    Ok(output)
}

fn getdata_mode_bytes(
    image: &DynamicImage,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<Vec<u8>, PilError> {
    // TODO(pillow-rs#3ptb): move these exact-mode byte/sample implementations
    // into analysis/<mode>/default and keep this API as selection orchestration.
    let mode = selection.implementation.mode;
    if selection.implementation.operation != crate::compute::registry::support::OperationId::GetData
        || image.mode() != mode
    {
        return Err(PilError::InternalError(
            "GetData selection did not match its mode-bearing raster".into(),
        ));
    }
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32 => getdata_byte_modes(image, mode),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            getdata_unsigned_luma16(image, mode)
        }
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => getdata_internal_high_depth(image, mode),
    }
}

fn validate_getdata_band_output(image: &DynamicImage, output: &[u8]) -> Result<(), PilError> {
    let expected = crate::checked_dims::CheckedDims::new_allow_empty(
        image.width(),
        image.height(),
        1,
    )?
    .total_pixels();
    if output.len() != expected {
        return Err(PilError::InternalError(
            "getdata band output length mismatch".into(),
        ));
    }
    Ok(())
}

fn write_getdata_byte_band(
    image: &DynamicImage,
    mode: Mode,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    match (mode, image) {
        (Mode::One, DynamicImage::One(image))
        | (Mode::ImageLuma8, DynamicImage::ImageLuma8(image))
        | (Mode::ImagePalette8, DynamicImage::ImagePalette8(image)) if band == 0 => {
            output.copy_from_slice(image.as_raw());
        }
        (Mode::ImageLumaA8, DynamicImage::ImageLumaA8(image))
        | (Mode::ImageLaPremultiplied8, DynamicImage::ImageLaPremultiplied8(image))
        | (Mode::ImagePaletteAlpha8, DynamicImage::ImagePaletteAlpha8(image)) => {
            select_interleaved_band_into(image.as_raw(), 2, band, output);
        }
        (Mode::ImageRgb8, DynamicImage::ImageRgb8(image))
        | (Mode::ImageYCbCr8, DynamicImage::ImageYCbCr8(image))
        | (Mode::ImageHsv8, DynamicImage::ImageHsv8(image)) => {
            select_interleaved_band_into(image.as_raw(), 3, band, output);
        }
        (Mode::ImageLab8, DynamicImage::ImageLab8(image)) => {
            select_lab_band_into(image.as_raw(), band, output);
        }
        (Mode::ImageRgba8, DynamicImage::ImageRgba8(image))
        | (Mode::ImageRgbPremultiplied8, DynamicImage::ImageRgbPremultiplied8(image))
        | (Mode::ImageRgbx8, DynamicImage::ImageRgbx8(image))
        | (Mode::ImageCmyk8, DynamicImage::ImageCmyk8(image)) => {
            select_interleaved_band_into(image.as_raw(), 4, band, output);
        }
        (Mode::ImageI32, DynamicImage::ImageI32(image))
        | (Mode::ImageF32, DynamicImage::ImageF32(image)) => {
            select_interleaved_band_into(image.as_raw(), 4, 0, output);
        }
        _ => return Err(mode_carrier_mismatch(mode)),
    }
    Ok(())
}

fn write_getdata_unsigned_luma16_band(
    image: &DynamicImage,
    mode: Mode,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    let image = match (mode, image) {
        (Mode::ImageI16, DynamicImage::ImageI16(image))
        | (Mode::ImageI16L, DynamicImage::ImageI16L(image))
        | (Mode::ImageI16B, DynamicImage::ImageI16B(image))
        | (Mode::ImageI16N, DynamicImage::ImageI16N(image)) => image,
        _ => return Err(mode_carrier_mismatch(mode)),
    };
    write_native_sample_band(
        image.as_raw(),
        image.width(),
        image.height(),
        1,
        band,
        mode,
        output,
        |sample| sample.min(u16::from(u8::MAX)) as u8,
    )
}

fn write_native_sample_band<T: Copy>(
    samples: &[T],
    width: u32,
    height: u32,
    channels: u8,
    band: usize,
    mode: Mode,
    output: &mut [u8],
    convert: impl Fn(T) -> u8,
) -> Result<(), PilError> {
    let count = native_sample_count(width, height, channels)?;
    let samples = samples
        .get(..count)
        .ok_or_else(|| mode_carrier_mismatch(mode))?;
    for (destination, sample) in output
        .iter_mut()
        .zip(samples.iter().skip(band).step_by(usize::from(channels)))
    {
        *destination = convert(*sample);
    }
    Ok(())
}

fn write_getdata_internal_band(
    image: &DynamicImage,
    mode: Mode,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    // NO_PILLOW_SUPPORT: these internal high-depth modes expose native bands
    // through this byte API without changing or retagging their raster.
    match (mode, image) {
        (Mode::ImageLumaA16, DynamicImage::ImageLumaA16(image)) => write_native_sample_band(
            image.as_raw(), image.width(), image.height(), 2, band, mode, output, u8::from_primitive,
        ),
        (Mode::ImageRgb16, DynamicImage::ImageRgb16(image)) => write_native_sample_band(
            image.as_raw(), image.width(), image.height(), 3, band, mode, output, u8::from_primitive,
        ),
        (Mode::ImageRgba16, DynamicImage::ImageRgba16(image)) => write_native_sample_band(
            image.as_raw(), image.width(), image.height(), 4, band, mode, output, u8::from_primitive,
        ),
        (Mode::ImageRgb32F, DynamicImage::ImageRgb32F(image)) => write_native_sample_band(
            image.as_raw(), image.width(), image.height(), 3, band, mode, output, u8::from_primitive,
        ),
        (Mode::ImageRgba32F, DynamicImage::ImageRgba32F(image)) => write_native_sample_band(
            image.as_raw(), image.width(), image.height(), 4, band, mode, output, u8::from_primitive,
        ),
        _ => Err(mode_carrier_mismatch(mode)),
    }
}

// TODO(pillow-rs#3ptb): move the exact-mode band sample writers into
// analysis/<mode>/default and leave the Image API responsible for selection.
fn write_getdata_band_samples(
    image: &DynamicImage,
    mode: Mode,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    validate_getdata_band_output(image, output)?;
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32 => write_getdata_byte_band(image, mode, band, output),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            write_getdata_unsigned_luma16_band(image, mode, band, output)
        }
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => write_getdata_internal_band(image, mode, band, output),
    }
}

fn getdata_band_bytes(
    image: &DynamicImage,
    mode: Mode,
    band: usize,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<Vec<u8>, PilError> {
    if selection.implementation.operation
        != crate::compute::registry::support::OperationId::GetDataBand
        || selection.implementation.mode != mode
        || image.mode() != mode
    {
        return Err(PilError::InternalError(
            "GetDataBand selection did not match its mode-bearing raster".into(),
        ));
    }
    let dimensions = crate::checked_dims::CheckedDims::new_allow_empty(
        image.width(),
        image.height(),
        1,
    )?;
    let mut output = dimensions.alloc_buffer();
    write_getdata_band_samples(image, mode, band, &mut output)?;
    Ok(output)
}

fn getdata_unsigned_luma16_values(image: &DynamicImage, mode: Mode) -> Result<Vec<i32>, PilError> {
    let samples = match (mode, image) {
        (Mode::ImageI16, DynamicImage::ImageI16(image))
        | (Mode::ImageI16L, DynamicImage::ImageI16L(image))
        | (Mode::ImageI16B, DynamicImage::ImageI16B(image))
        | (Mode::ImageI16N, DynamicImage::ImageI16N(image)) => image.as_raw(),
        _ => return Err(mode_carrier_mismatch(mode)),
    };
    Ok(samples.iter().copied().map(i32::from).collect())
}

impl Image {
    fn getdata_band(&self, image: &DynamicImage, band: i32) -> Result<Vec<u8>, PilError> {
        // Pillow validates the requested logical band before extracting it.
        // Keep this check in the core so every binding observes the same
        // contract instead of silently clamping to RGBA channel 3.
        let mode = image.mode();
        let band_index = validated_band_index(
            band,
            crate::compute::pool_cpu::ops::analysis::ops::get_data_band_count(mode),
        )?;
        let context = crate::compute::ExecutionContext::for_routed_backend(
            crate::compute::Backend::Cpu,
        );
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
        if !crate::compute::pool_cpu::ops::analysis::ops::supports_get_data_band_into(mode) {
            // These scalar modes intentionally have no caller-owned band
            // buffer API. Keep their established direct byte conversion.
            return getdata_band_bytes(image, mode, band_index, selection);
        }

        let dimensions = crate::checked_dims::CheckedDims::new_allow_empty(
            image.width(),
            image.height(),
            1,
        )?;
        let mut output = dimensions.alloc_buffer();
        self.getdata_band_into(band, &mut output)?;
        Ok(output)
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
        let img = self.materialized_shared()?;
        if let Some(band) = band {
            return self.getdata_band(&img, band);
        }
        let context = crate::compute::ExecutionContext::for_routed_backend(
            crate::compute::Backend::Cpu,
        );
        let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_data(
            img.mode(), &context,
        )?;
        getdata_mode_bytes(img.as_ref(), selection)
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
        self.getdata_band_len_for_mode(self.mode_selector()?, band)
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
        let requested_backend = match self.backend() {
            Some(backend) => Some(backend),
            None => crate::compute::preferred_active_backend()?.map(|(backend, _)| backend),
        };
        if requested_backend.is_some_and(|backend| backend != crate::compute::Backend::Cpu) {
            return Ok(None);
        }
        let (mode, values) = match self {
            Image::Paletted(data) => (Mode::ImagePalette8, data.indices.as_bytes()),
            Image::Loaded(data) => match data.image.as_ref() {
                // Preserve exact mode identity without formatting a Pillow spelling.
                DynamicImage::One(image) => (Mode::One, image.as_raw().as_slice()),
                DynamicImage::ImageLuma8(image) => {
                    (Mode::ImageLuma8, image.as_raw().as_slice())
                }
                _ => return Ok(None),
            },
            _ => return Ok(None),
        };
        let context = crate::compute::ExecutionContext::for_routed_backend(
            crate::compute::Backend::Cpu,
        );
        let selection = crate::compute::pool_cpu::ops::analysis::ops::select_get_data(
            mode, &context,
        )?;
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
        let mode = self.mode_selector()?;
        let expected = self.getdata_band_len_for_mode(mode, band)?;
        let band_index = usize::try_from(band).expect("getdata band was validated as nonnegative");
        let backend_band = u8::try_from(band_index).expect("Pillow band indices fit in u8");
        if output.len() != expected {
            return Err(PilError::InternalError(
                "getdata band output length mismatch".into(),
            ));
        }
        let selected_backend = self.selected_getdata_backend()?;
        if self.try_getdata_band_simd_sink(backend_band, selected_backend, output)? {
            return Ok(());
        }
        if self.try_getdata_band_backend(mode, backend_band, selected_backend, output)? {
            return Ok(());
        }

        let context_backend = if selected_backend == Some(crate::compute::Backend::Simd) {
            crate::compute::Backend::Simd
        } else {
            crate::compute::Backend::Cpu
        };
        let context = crate::compute::ExecutionContext::for_routed_backend(context_backend);
        let selection =
            crate::compute::pool_cpu::ops::analysis::ops::select_get_data_band_into(mode, &context)?;
        self.write_getdata_band_cpu(band_index, selected_backend, selection, output)
    }

    fn selected_getdata_backend(&self) -> Result<Option<crate::compute::Backend>, PilError> {
        match self.backend() {
            Some(backend) => Ok(Some(backend)),
            None => Ok(crate::compute::preferred_active_backend()?.map(|(backend, _)| backend)),
        }
    }

    fn try_getdata_band_simd_sink(
        &self,
        band: u8,
        selected_backend: Option<crate::compute::Backend>,
        output: &mut [u8],
    ) -> Result<bool, PilError> {
        if selected_backend != Some(crate::compute::Backend::Simd)
            || !matches!(self, Image::Loaded(_))
        {
            return Ok(false);
        }
        let image = self.materialized_shared()?;
        let operation = PipelineOp::ExtractBand { index: band };
        let handled = crate::compute::execute_eager_cpu_arch_bytes(
            "GetData",
            image.as_ref(),
            crate::compute::Backend::Simd,
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
        output: &mut [u8],
    ) -> Result<bool, PilError> {
        let Some(backend @ (crate::compute::Backend::Simd | crate::compute::Backend::Gpu)) =
            selected_backend
        else {
            return Ok(false);
        };
        // TODO(pillow-rs#3ptb): move GPU terminal eligibility into the combined
        // CPU/GPU GetDataBandInto support declaration.
        if !crate::compute::pool_cpu::ops::analysis::ops::supports_get_data_band_terminal(
            mode, backend,
        ) {
            return Ok(false);
        }

        // Keep terminal projection in the lazy graph so the selected backend
        // can fuse it with pending image work and report its real receipt. P
        // and LAB stay on the host path: P needs raw indices, while LAB's a/b
        // samples need Pillow's signed-byte offset.
        let extracted = Image::push_op(self, PipelineOp::ExtractBand { index: band })
            .use_backend(backend);
        let image = extracted.materialized_shared()?;
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
    ) -> Result<(), PilError> {
        let telemetry =
            crate::compute::begin_eager_cpu_operation_telemetry("GetData", selected_backend);
        let result = (|| {
            if let Image::Paletted(data) = self {
                output.copy_from_slice(data.indices.as_bytes());
                return Ok(());
            }

            let img = self.materialized_shared()?;
            write_getdata_band_samples(img.as_ref(), img.mode(), band, output)
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
        let mode = self.mode_selector()?;
        if matches!(mode, Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N) {
            if band.is_some() {
                // Pillow's ImagingCore.getdata rejects every explicit band
                // selection for I;16 modes with the mode error, rather than
                // the generic index error used by ordinary multi-band images.
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            let image = self.materialized_shared()?;
            return Ok(FormattedImageData::IntegerScalars(
                getdata_unsigned_luma16_values(image.as_ref(), mode)?,
            ));
        }
        if matches!(mode, Mode::ImageI32 | Mode::ImageF32) {
            if band.is_some() {
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            return Ok(match self.scalar_samples(mode)? {
                ScalarImageSamples::Integer(values) => FormattedImageData::IntegerScalars(values),
                ScalarImageSamples::Float(values) => FormattedImageData::FloatScalars(values),
            });
        }
        let raw = self.getdata(band)?;
        if band.is_some() {
            return Ok(FormattedImageData::Scalars(raw));
        }

        let band_count = crate::compute::pool_cpu::ops::analysis::ops::get_data_band_count(mode);
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
