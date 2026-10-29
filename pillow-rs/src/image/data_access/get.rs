use super::{
    DynamicImage, FormattedImageData, Image, PilError, PipelineOp, ScalarImageSamples,
    image_mode_name, is_l16_mode, pillow_band_count, select_interleaved_band,
    select_interleaved_band_into, select_lab_band, select_lab_band_into, validated_band_index,
};
impl Image {
    fn getdata_band(&self, image: &DynamicImage, band: i32) -> Result<Vec<u8>, PilError> {
        // Pillow validates the requested logical band before extracting it.
        // Keep this check in the core so every binding observes the same
        // contract instead of silently clamping to RGBA channel 3.
        let mode = self.mode_from_materialized(image);
        let band = validated_band_index(band, pillow_band_count(&mode))?;
        if mode == "LAB" {
            if let DynamicImage::ImageRgb8(image) = image {
                return Ok(select_lab_band(image.as_raw(), band));
            }
            return Ok(select_lab_band(image.to_rgb8().as_raw(), band));
        }
        // Keep ordinary byte layouts narrow for banded reads. The old
        // fallback widened the complete frame to RGBA before selecting a
        // single channel, which made a read-only terminal operation pay
        // for an avoidable full-frame allocation and conversion.
        match image {
            DynamicImage::ImageLuma8(image) if band == 0 => {
                return Ok(image.as_raw().clone());
            }
            DynamicImage::ImageLumaA8(image) if band < 2 => {
                return Ok(select_interleaved_band(image.as_raw(), 2, band));
            }
            DynamicImage::ImageRgb8(image) if band < 3 => {
                return Ok(select_interleaved_band(image.as_raw(), 3, band));
            }
            DynamicImage::ImageRgba8(image) if band < 4 => {
                return Ok(select_interleaved_band(image.as_raw(), 4, band));
            }
            _ => {}
        }
        let rgba = image.to_rgba8();
        Ok(rgba.pixels().map(|pixel| pixel[band]).collect())
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
        let mode = self.mode_from_materialized(&img);
        if mode == "LAB" {
            let mut data = img.to_rgb8().into_raw();
            for pixel in data.chunks_exact_mut(3) {
                pixel[1] = pixel[1].wrapping_sub(128);
                pixel[2] = pixel[2].wrapping_sub(128);
            }
            return Ok(data);
        }
        match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => {
                let gray = img.to_luma8();
                Ok(gray.into_raw())
            }
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => {
                let ga = img.to_luma_alpha8();
                let mut out = Vec::with_capacity((ga.width() * ga.height() * 2) as usize);
                for p in ga.pixels() {
                    out.push(p[0]);
                    out.push(p[1]);
                }
                Ok(out)
            }
            crate::raster::ColorType::Rgb8
            | crate::raster::ColorType::Rgb16
            | crate::raster::ColorType::Rgb32F => {
                let rgb = img.to_rgb8();
                Ok(rgb.into_raw())
            }
            _ => {
                let rgba = img.to_rgba8();
                Ok(rgba.into_raw())
            }
        }
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
        let mode = self.mode()?;
        self.getdata_band_len_for_mode(&mode, band)
    }

    fn getdata_band_len_for_mode(&self, mode: &str, band: i32) -> Result<usize, PilError> {
        if is_l16_mode(mode) || mode == "I" || mode == "F" {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let band_count = pillow_band_count(mode);
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
        match self {
            Image::Paletted(data) => {
                crate::compute::record_eager_cpu_operation_receipt(
                    "GetData",
                    requested_backend,
                    data.indices.as_raw().len(),
                );
                Ok(Some(data.indices.as_raw()))
            }
            // Preserve `mode()` precedence while avoiding its owned String in
            // this eligibility-only check.
            Image::Loaded(data)
                if matches!(
                    data.explicit_mode
                        .as_deref()
                        .unwrap_or_else(|| image_mode_name(data.decoded_mode)),
                    "1" | "L"
                ) =>
            {
                let values: Option<&[u8]> = match data.image.as_ref() {
                    DynamicImage::ImageLuma8(image) => Some(image.as_raw().as_slice()),
                    _ => None,
                };
                if let Some(values) = values {
                    crate::compute::record_eager_cpu_operation_receipt(
                        "GetData",
                        requested_backend,
                        values.len(),
                    );
                }
                Ok(values)
            }
            _ => Ok(None),
        }
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
        let mode_name = self.mode()?;
        let mode = mode_name.as_str();
        let expected = self.getdata_band_len_for_mode(mode, band)?;
        let band_index = usize::try_from(band).expect("getdata band was validated as nonnegative");
        let backend_band = u8::try_from(band_index).expect("Pillow band indices fit in u8");
        if output.len() != expected {
            return Err(PilError::InternalError(
                "getdata band output length mismatch".into(),
            ));
        }
        let selected_backend = match self.backend() {
            Some(backend) => Some(backend),
            None => crate::compute::preferred_active_backend()?.map(|(backend, _)| backend),
        };
        let backend_native = match selected_backend {
            Some(crate::compute::Backend::Simd) => matches!(
                mode,
                "LA" | "PA" | "RGB" | "RGBA" | "RGBa" | "RGBX" | "CMYK" | "HSV" | "YCbCr"
            ),
            Some(crate::compute::Backend::Gpu) => !matches!(mode, "P" | "LAB" | "I" | "F"),
            Some(crate::compute::Backend::Cpu) | None => false,
        };
        if backend_native
            && let Some(backend @ (crate::compute::Backend::Simd | crate::compute::Backend::Gpu)) =
                selected_backend
        {
            if backend == crate::compute::Backend::Simd && matches!(self, Image::Loaded(_)) {
                // A materialized image needs no deferred pipeline boundary.
                // Write SIMD channel extraction into Python's final bytes
                // allocation instead of allocating an intermediate L image
                // and copying that image back into the binding buffer.
                let image = self.materialized_shared()?;
                let context = crate::compute::ExecutionContext::for_backend(backend);
                return crate::compute::execute_eager_simd_bytes(
                    "GetData",
                    image.as_ref(),
                    output,
                    |output| {
                        crate::compute::simd_extract_band_into_in_context(
                            &context,
                            image.as_ref(),
                            backend_band,
                            Some(mode),
                            output,
                        )
                    },
                );
            }

            // Keep terminal projection in the lazy graph so the selected
            // backend can fuse it with pending image work and report its real
            // execution receipt. P and LAB stay on the exact host path below:
            // P must expose raw indices, while LAB's a/b samples use Pillow's
            // signed-byte offset rather than ExtractBand's raw channel value.
            let extracted = Image::push_op(
                self,
                PipelineOp::ExtractBand {
                    index: backend_band,
                },
            )
            .use_backend(backend);
            let image = extracted.materialized_shared()?;
            let values = image.as_bytes();
            if values.len() != output.len() {
                return Err(PilError::InternalError(
                    "getdata band backend output length mismatch".into(),
                ));
            }
            output.copy_from_slice(values);
            return Ok(());
        }

        self.write_getdata_band_cpu(band_index, selected_backend, output)
    }

    fn write_getdata_band_cpu(
        &self,
        band: usize,
        selected_backend: Option<crate::compute::Backend>,
        output: &mut [u8],
    ) -> Result<(), PilError> {
        let telemetry =
            crate::compute::begin_eager_cpu_operation_telemetry("GetData", selected_backend);
        let result = (|| {
            if let Image::Paletted(data) = self {
                output.copy_from_slice(data.indices.as_raw());
                return Ok(());
            }

            let img = self.materialized_shared()?;
            let materialized_mode = self.mode_from_materialized(&img);
            if materialized_mode == "LAB" {
                if let DynamicImage::ImageRgb8(image) = img.as_ref() {
                    select_lab_band_into(image.as_raw(), band, output);
                } else {
                    select_lab_band_into(img.to_rgb8().as_raw(), band, output);
                }
                return Ok(());
            }

            match img.as_ref() {
                DynamicImage::ImageLuma8(image) if band == 0 => {
                    output.copy_from_slice(image.as_raw());
                    return Ok(());
                }
                DynamicImage::ImageLumaA8(image) if band < 2 => {
                    select_interleaved_band_into(image.as_raw(), 2, band, output);
                    return Ok(());
                }
                DynamicImage::ImageRgb8(image) if band < 3 => {
                    select_interleaved_band_into(image.as_raw(), 3, band, output);
                    return Ok(());
                }
                DynamicImage::ImageRgba8(image) if band < 4 => {
                    select_interleaved_band_into(image.as_raw(), 4, band, output);
                    return Ok(());
                }
                _ => {}
            }

            let rgba = img.to_rgba8();
            for (destination, pixel) in output.iter_mut().zip(rgba.pixels()) {
                *destination = pixel[band];
            }
            Ok(())
        })();
        if result.is_ok() {
            crate::compute::finish_eager_cpu_operation_telemetry(telemetry, output.len());
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
        let mode = self.mode()?;
        if is_l16_mode(&mode) {
            if band.is_some() {
                // Pillow's ImagingCore.getdata rejects every explicit band
                // selection for I;16 modes with the mode error, rather than
                // the generic index error used by ordinary multi-band images.
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            return Ok(FormattedImageData::IntegerScalars(
                self.materialized_shared()?
                    .to_luma16()
                    .as_raw()
                    .iter()
                    .copied()
                    .map(i32::from)
                    .collect(),
            ));
        }
        if mode == "I" || mode == "F" {
            if band.is_some() {
                return Err(PilError::ValueError("image has wrong mode".into()));
            }
            return Ok(match self.scalar_samples(&mode)? {
                ScalarImageSamples::Integer(values) => FormattedImageData::IntegerScalars(values),
                ScalarImageSamples::Float(values) => FormattedImageData::FloatScalars(values),
            });
        }
        let raw = self.getdata(band)?;
        if band.is_some() {
            return Ok(FormattedImageData::Scalars(raw));
        }

        let band_count = pillow_band_count(&mode);
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
