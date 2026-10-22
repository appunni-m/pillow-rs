//! Bulk pixel data access and mutation methods for [`Image`].

use std::sync::Arc;

use bytes::Bytes;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::{PipelineOp, PixelMode};
use crate::raster::DynamicImage;

use super::{
    FormattedImageData, Image, PutDataInput, PutDataValue, PutDataValueKind, ScalarImageSamples,
    image_mode_name, materialization_cache, select_interleaved_band, select_interleaved_band_into,
    select_lab_band, select_lab_band_into,
};

pub(super) fn decode_integer_samples(raw: &[u8]) -> Vec<i32> {
    raw.chunks_exact(4)
        .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect()
}

pub(super) fn decode_float_samples(raw: &[u8]) -> Vec<f64> {
    raw.chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]) as f64)
        .collect()
}

pub(super) fn validate_scalar_storage(image: &DynamicImage, mode: &str) -> Result<(), PilError> {
    if !matches!(mode, "I" | "F") {
        return Ok(());
    }
    // Pillow permits empty scalar images from `Image.frombytes`. CheckedDims
    // intentionally rejects zero dimensions for allocations, so validate the
    // already-existing empty buffer directly before using it for non-empty
    // images.
    let expected = if image.width() == 0 || image.height() == 0 {
        0
    } else {
        CheckedDims::new(image.width(), image.height(), 4)?.total_bytes()
    };
    if image.as_bytes().len() != expected {
        return Err(PilError::InternalError(format!(
            "{mode} image has invalid scalar storage"
        )));
    }
    Ok(())
}

pub(super) fn putdata_clip_u8(value: f64) -> u8 {
    if value.is_nan() || value >= 255.0 {
        255
    } else if value <= 0.0 {
        0
    } else {
        value as u8
    }
}

fn putdata_clip_component(value: i128) -> u8 {
    value.clamp(0, 255) as u8
}

pub(super) fn is_l16_mode(mode: &str) -> bool {
    matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
}

pub(super) fn l16_uses_big_endian(mode: &str) -> bool {
    match mode {
        "I;16B" => true,
        "I;16L" => false,
        "I;16" | "I;16N" => cfg!(target_endian = "big"),
        _ => false,
    }
}

fn putdata_l16_sample(value: &PutDataValue, scale: f64, offset: f64) -> Result<u16, PilError> {
    match value {
        // _imaging.c:_putdata treats I;16 as an unsigned 16-bit sample
        // destination. Rust's float-to-integer cast provides the same
        // bounded conversion for the public numeric path.
        PutDataValue::Number(number) => Ok((number * scale + offset) as u16),
        // Pillow's I;16 decoder reaches the flattened-sequence guard before
        // numeric coercion for nested/non-numeric values. Keep that public
        // error text exact instead of exposing the generic scalar path.
        _ => Err(PilError::TypeError("sequence must be flattened".to_owned())),
    }
}

fn putdata_bytes(
    mode: crate::pipeline::PixelMode,
    values: &[PutDataValue],
    scale: f64,
    offset: f64,
) -> Result<Vec<u8>, PilError> {
    let capacity = values
        .len()
        .checked_mul(mode.channels())
        .ok_or_else(|| PilError::DimensionError("putdata byte count overflow".into()))?;
    let mut data = Vec::with_capacity(capacity);

    for value in values {
        match (mode, value) {
            (
                crate::pipeline::PixelMode::Mode1
                | crate::pipeline::PixelMode::L
                | crate::pipeline::PixelMode::P,
                PutDataValue::Number(number),
            ) => data.push(putdata_clip_u8(number * scale + offset)),
            (crate::pipeline::PixelMode::I, PutDataValue::Number(number)) => {
                data.extend_from_slice(&((number * scale + offset) as i32).to_le_bytes());
            }
            (crate::pipeline::PixelMode::F, PutDataValue::Number(number)) => {
                data.extend_from_slice(&((number * scale + offset) as f32).to_le_bytes());
            }
            (
                crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA,
                PutDataValue::Packed(packed),
            ) => {
                let bytes = packed.to_le_bytes();
                data.extend_from_slice(&[bytes[0], bytes[3]]);
            }
            (
                crate::pipeline::PixelMode::RGB
                | crate::pipeline::PixelMode::YCbCr
                | crate::pipeline::PixelMode::HSV,
                PutDataValue::Packed(packed),
            ) => data.extend_from_slice(&packed.to_le_bytes()[..3]),
            (
                crate::pipeline::PixelMode::RGBA | crate::pipeline::PixelMode::CMYK,
                PutDataValue::Packed(packed),
            ) => data.extend_from_slice(&packed.to_le_bytes()[..4]),
            (
                crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA,
                PutDataValue::Components(components),
            ) if components.len() == 2 => {
                data.extend(components.iter().copied().map(putdata_clip_component));
            }
            (
                crate::pipeline::PixelMode::RGB
                | crate::pipeline::PixelMode::YCbCr
                | crate::pipeline::PixelMode::HSV,
                PutDataValue::Components(components),
            ) if matches!(components.len(), 3 | 4) => {
                data.extend(components[..3].iter().copied().map(putdata_clip_component));
            }
            (
                crate::pipeline::PixelMode::RGBA | crate::pipeline::PixelMode::CMYK,
                PutDataValue::Components(components),
            ) if matches!(components.len(), 3 | 4) => {
                data.extend(components[..3].iter().copied().map(putdata_clip_component));
                data.push(
                    components
                        .get(3)
                        .copied()
                        .map_or(255, putdata_clip_component),
                );
            }
            (
                crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA,
                PutDataValue::Components(_),
            ) => {
                return Err(PilError::TypeError(
                    "color must be int, or tuple of one or two elements".into(),
                ));
            }
            (
                crate::pipeline::PixelMode::RGB
                | crate::pipeline::PixelMode::RGBA
                | crate::pipeline::PixelMode::CMYK
                | crate::pipeline::PixelMode::YCbCr
                | crate::pipeline::PixelMode::HSV,
                PutDataValue::Components(_),
            ) => {
                return Err(PilError::TypeError(
                    "color must be int, or tuple of one, three or four elements".into(),
                ));
            }
            (
                crate::pipeline::PixelMode::Mode1
                | crate::pipeline::PixelMode::L
                | crate::pipeline::PixelMode::P
                | crate::pipeline::PixelMode::I
                | crate::pipeline::PixelMode::F,
                _,
            ) => {
                return Err(PilError::TypeError("sequence must be flattened".into()));
            }
            _ => {
                return Err(PilError::TypeError("color must be int or tuple".into()));
            }
        }
    }

    Ok(data)
}

pub(super) fn pillow_band_count(mode: &str) -> usize {
    match mode {
        "L" | "1" | "P" | "I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N" => 1,
        "LA" | "La" | "PA" => 2,
        "RGB" | "YCbCr" | "HSV" | "LAB" => 3,
        _ => 4,
    }
}

impl Image {
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
            // Pillow validates the requested logical band before extracting
            // it. Keep this check in the core so every binding observes the
            // same contract instead of silently clamping to RGBA channel 3.
            let mode = self.mode_from_materialized(&img);
            let band_count = pillow_band_count(&mode);
            if band < 0 || band as usize >= band_count {
                return Err(PilError::ValueError("band index out of range".into()));
            }
            let band = band as usize;
            if mode == "LAB" {
                if let DynamicImage::ImageRgb8(image) = img.as_ref() {
                    return Ok(select_lab_band(image.as_raw(), band));
                }
                return Ok(select_lab_band(img.to_rgb8().as_raw(), band));
            }
            // Keep ordinary byte layouts narrow for banded reads. The old
            // fallback widened the complete frame to RGBA before selecting a
            // single channel, which made a read-only terminal operation pay
            // for an avoidable full-frame allocation and conversion.
            match img.as_ref() {
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
            let rgba = img.to_rgba8();
            return Ok(rgba.pixels().map(|p| p[band]).collect());
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
        if band < 0 || band as usize >= band_count {
            return Err(PilError::ValueError("band index out of range".into()));
        }
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
                return crate::compute::execute_eager_simd_bytes(
                    "GetData",
                    image.as_ref(),
                    output,
                    |output| {
                        crate::compute::simd_extract_band_into(
                            image.as_ref(),
                            band as u8,
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
            let extracted = Image::push_op(self, PipelineOp::ExtractBand { index: band as u8 })
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

        let telemetry =
            crate::compute::begin_eager_cpu_operation_telemetry("GetData", selected_backend);
        let result = (|| {
            if let Image::Paletted(data) = self {
                output.copy_from_slice(data.indices.as_raw());
                return Ok(());
            }

            let img = self.materialized_shared()?;
            let materialized_mode = self.mode_from_materialized(&img);
            let band = band as usize;
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

    /// Queues replacement pixel data from a flat byte sequence.
    ///
    /// `data` must match the active image mode and dimensions expected by the
    /// pipeline operation. Validation happens when the pipeline is materialized.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(())`; materialization reports invalid byte length
    /// or mode mismatches.
    pub fn putdata(&mut self, data: &[u8]) -> Result<(), PilError> {
        let mode_name = self.mode()?;
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        let new_self = Image::push_op(
            self,
            PipelineOp::PutData {
                data: Bytes::copy_from_slice(data),
                mode,
            },
        );
        *self = new_self;
        Ok(())
    }

    /// Queues immutable byte data without copying its backing allocation when
    /// the input already matches a complete native sample layout.
    ///
    /// This binding-oriented fast path accepts only mode 1, L, or P and only
    /// unscaled byte data with one byte per sequence entry. It preserves
    /// partial-image `putdata` semantics when the payload is shorter than the
    /// image, while retaining the immutable owner until deferred execution.
    /// Other layouts must continue through [`Image::putdata_bytes_fast_path`]
    /// so Pillow's per-entry coercion and packed-color rules remain intact.
    pub fn putdata_shared_bytes_fast_path(
        &mut self,
        data: Bytes,
        entry_count: usize,
        scale: f64,
        offset: f64,
    ) -> Result<bool, PilError> {
        if scale != 1.0 || offset != 0.0 || data.len() != entry_count {
            return Ok(false);
        }
        let mode_name = self.mode()?;
        if !matches!(mode_name.as_str(), "1" | "L" | "P") {
            return Ok(false);
        }
        self.validate_putdata_length(entry_count)?;
        let mode = PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        let new_self = Image::push_op(self, PipelineOp::PutData { data, mode });
        *self = new_self;
        Ok(true)
    }

    /// Normalizes one value per pixel and queues exact Pillow `putdata` bytes.
    ///
    /// Pillow 12.2's `_imaging.c:_putdata` applies `scale` and `offset` only to
    /// single-band, signed-integer, and float images. Its multiband `getink`
    /// path instead treats integers as little-endian packed pixels and ignores
    /// scaling. Keeping that distinction here gives CPU, SIMD, and GPU the
    /// same canonical byte payload.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::TypeError`] for incompatible value shapes or too
    /// many pixels, and propagates image mode or dimension errors.
    pub fn putdata_values(
        &mut self,
        values: &[PutDataValue],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        self.validate_putdata_length(values.len())?;

        let mode_name = self.mode()?;
        if is_l16_mode(&mode_name) {
            // Keep I;16 out of PixelMode's byte-oriented deferred path. Its
            // generic fallback widens to RGBA8 and loses the 16-bit sample.
            for (pixel_index, value) in values.iter().enumerate() {
                self.putdata_value_at(pixel_index, value, scale, offset)?;
            }
            return Ok(());
        }
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        let data = putdata_bytes(mode, values, scale, offset)?;
        self.putdata(&data)
    }

    /// Returns the host-language value shape required by the current mode.
    ///
    /// This is the core-owned mode policy for `putdata`. A binding may still
    /// need to inspect Python protocol behavior, but it does not need to
    /// duplicate the list of numeric and multiband mode names.
    pub fn putdata_value_kind(&self) -> Result<PutDataValueKind, PilError> {
        let mode_name = self.mode()?;
        if matches!(
            mode_name.as_str(),
            "1" | "L" | "P" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" | "F"
        ) {
            return Ok(PutDataValueKind::Numeric);
        }
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        Ok(PutDataValueKind::Components {
            channels: mode.channels(),
        })
    }

    /// Applies a callback-free bulk input through the canonical `putdata`
    /// normalization path.
    pub fn putdata_input(
        &mut self,
        input: PutDataInput<'_>,
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        match input {
            PutDataInput::Integers(values) => self.putdata_integer_values(values, scale, offset),
            PutDataInput::Numbers(values) => self.putdata_numeric_values(values, scale, offset),
        }
    }

    /// Replaces numeric samples through the shared `putdata` normalization path.
    ///
    /// Bindings use this callback-free entry point for exact built-in numeric
    /// sequences. Keeping the conversion in core reaches the same bulk path
    /// for every binding while custom Python values can still use
    /// [`Image::putdata_value_at`] to preserve re-entrant coercion order.
    pub fn putdata_numeric_values(
        &mut self,
        values: &[f64],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        let values = values
            .iter()
            .copied()
            .map(PutDataValue::Number)
            .collect::<Vec<_>>();
        self.putdata_values(&values, scale, offset)
    }

    /// Replaces exact integer samples through Pillow's numeric-or-packed
    /// `putdata` path.
    ///
    /// Pillow interprets an integer as a scaled numeric sample for
    /// single-band modes, but as a little-endian packed pixel for multiband
    /// modes. Bindings only need to extract the host integer sequence; this
    /// mode distinction belongs here so every backend receives the same
    /// canonical operation.
    pub fn putdata_integer_values(
        &mut self,
        values: &[i64],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        match self.putdata_value_kind()? {
            PutDataValueKind::Numeric => {
                let values = values
                    .iter()
                    .copied()
                    .map(|value| PutDataValue::Number(value as f64))
                    .collect::<Vec<_>>();
                self.putdata_values(&values, scale, offset)
            }
            PutDataValueKind::Components { .. } => {
                let values = values
                    .iter()
                    .copied()
                    .map(PutDataValue::Packed)
                    .collect::<Vec<_>>();
                self.putdata_values(&values, scale, offset)
            }
        }
    }

    /// Validates the sequence length before host-language value coercion.
    ///
    /// Pillow rejects an oversized `putdata` sequence before converting any
    /// element, so bindings must delegate this check before they iterate a
    /// Python sequence. Keeping it here also gives every binding the same
    /// dimension and error behavior.
    pub fn validate_putdata_length(&self, entry_count: usize) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PilError::DimensionError("pixel count overflow".into()))?;
        if entry_count > pixel_count {
            return Err(PilError::TypeError("too many data entries".into()));
        }
        Ok(())
    }

    /// Applies the byte-oriented `putdata` fast paths.
    ///
    /// Returns `Ok(true)` when the current mode consumed the bytes. Other
    /// modes return `Ok(false)` so the binding can preserve Python's generic
    /// sequence and callback behavior.
    pub fn putdata_bytes_fast_path(
        &mut self,
        data: &[u8],
        entry_count: usize,
        scale: f64,
        offset: f64,
    ) -> Result<bool, PilError> {
        let mode_name = self.mode()?;
        if is_l16_mode(&mode_name) {
            self.putdata_l16_bytes(data, scale, offset)?;
            return Ok(true);
        }
        if matches!(mode_name.as_str(), "1" | "L" | "P") {
            if scale == 1.0 && offset == 0.0 && data.len() == entry_count {
                // Bytes already contain the exact one-byte samples Pillow
                // stores for these modes. Queue them directly instead of
                // widening every byte to PutDataValue and encoding it back.
                self.putdata(data)?;
                return Ok(true);
            }
            let values = data
                .iter()
                .copied()
                // Pillow reads the terminating NUL when a bytes subtype
                // reports one more item than its physical payload. Extend
                // that behavior safely for every missing item instead of
                // following its unchecked C pointer read out of bounds.
                .chain(std::iter::repeat(0))
                .take(entry_count)
                .map(|value| PutDataValue::Number(f64::from(value)))
                .collect::<Vec<_>>();
            self.putdata_values(&values, scale, offset)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Replaces raw I;16 samples from Pillow's bytes fast path.
    ///
    /// Pillow treats a `bytes` argument to `putdata` as byte-oriented storage
    /// for I;16 modes, rather than as one numeric sample per byte. Its
    /// `_imaging.c:_putdata` image8 fast path copies at most one logical row's
    /// worth of input bytes at a time, leaving the remaining byte half of each
    /// 16-bit row zero-filled. With a scale or offset it applies the same
    /// clipping to each input byte before placing it in that row layout.
    pub fn putdata_l16_bytes(
        &mut self,
        data: &[u8],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        self.validate_putdata_length(data.len())?;
        let (width, height) = self.size()?;
        let dimensions = CheckedDims::new(width, height, 2)?;
        let mode = self.mode()?;
        let mut raw = dimensions.alloc_buffer();
        let width = width as usize;
        let row_stride = width
            .checked_mul(2)
            .ok_or_else(|| PilError::DimensionError("putdata row stride overflow".into()))?;
        for (index, byte) in data.iter().copied().enumerate() {
            let row = index / width;
            let column = index % width;
            raw[row * row_stride + column] = if scale == 1.0 && offset == 0.0 {
                byte
            } else {
                putdata_clip_u8(f64::from(byte) * scale + offset)
            };
        }
        self.load()?;
        if let Image::Loaded(image_data) = self {
            let image = Arc::make_mut(&mut image_data.image);
            let destination = image
                .as_mut_luma16()
                .ok_or_else(|| PilError::InternalError("putdata I;16 storage mismatch".into()))?;
            for (pixel, sample) in destination.as_mut().iter_mut().zip(raw.chunks_exact(2)) {
                *pixel = if l16_uses_big_endian(&mode) {
                    u16::from_be_bytes([sample[0], sample[1]])
                } else {
                    u16::from_le_bytes([sample[0], sample[1]])
                };
            }
            image_data.known_zero_fill = false;
            return Ok(());
        }
        Err(PilError::InternalError(
            "putdata I;16 did not materialize writable storage".into(),
        ))
    }

    /// Writes one normalized `putdata` value immediately at a pixel offset.
    ///
    /// Host-language bindings use this form while coercing values because
    /// Pillow exposes each completed pixel to callbacks triggered by coercing
    /// the following value. The same canonical byte normalization as
    /// [`Image::putdata_values`] is retained, while the already materialized
    /// storage is updated without building one deferred pipeline per pixel.
    /// Bulk, callback-free replacement remains backend-dispatched through
    /// [`Image::putdata_values`]. This immediate host-memory mutation is the
    /// CPU path used by public bindings because observable callback order takes
    /// precedence over deferred SIMD/GPU execution.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::TypeError`] when `value` is incompatible with the
    /// image mode or `pixel_index` is outside the image.
    pub fn putdata_value_at(
        &mut self,
        pixel_index: usize,
        value: &PutDataValue,
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        let (width, height) = self.size()?;
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PilError::DimensionError("pixel count overflow".into()))?;
        if pixel_index >= pixel_count {
            return Err(PilError::TypeError("too many data entries".into()));
        }

        let mode_name = self.mode()?;
        if is_l16_mode(&mode_name) {
            let sample = putdata_l16_sample(value, scale, offset)?;
            self.load()?;
            if let Image::Loaded(data) = self {
                let image = Arc::make_mut(&mut data.image);
                let destination = image.as_mut_luma16().ok_or_else(|| {
                    PilError::InternalError("putdata I;16 storage mismatch".into())
                })?;
                let pixel = destination.as_mut().get_mut(pixel_index).ok_or_else(|| {
                    PilError::InternalError("putdata I;16 offset out of bounds".into())
                })?;
                *pixel = sample;
                data.known_zero_fill = false;
                return Ok(());
            }
            return Err(PilError::InternalError(
                "putdata I;16 did not materialize writable storage".into(),
            ));
        }
        let mode = crate::pipeline::PixelMode::from_name(&mode_name).ok_or_else(|| {
            PilError::ValueError(format!("unsupported putdata mode: {mode_name}"))
        })?;
        let bytes = putdata_bytes(mode, std::slice::from_ref(value), scale, offset)?;
        let start = pixel_index
            .checked_mul(mode.channels())
            .ok_or_else(|| PilError::DimensionError("putdata byte offset overflow".into()))?;
        let end = start
            .checked_add(bytes.len())
            .ok_or_else(|| PilError::DimensionError("putdata byte offset overflow".into()))?;

        self.load()?;
        match self {
            Image::Paletted(data) if mode == crate::pipeline::PixelMode::P => {
                data.indices.as_mut()[pixel_index] = bytes[0];
                data.materialized = materialization_cache();
            }
            Image::Loaded(data) => {
                let image = Arc::make_mut(&mut data.image);
                let samples: &mut [u8] = match mode {
                    crate::pipeline::PixelMode::Mode1
                    | crate::pipeline::PixelMode::L
                    | crate::pipeline::PixelMode::P => image
                        .as_mut_luma8()
                        .ok_or_else(|| {
                            PilError::InternalError("putdata L storage mismatch".into())
                        })?
                        .as_mut(),
                    crate::pipeline::PixelMode::LA | crate::pipeline::PixelMode::PA => image
                        .as_mut_luma_alpha8()
                        .ok_or_else(|| {
                            PilError::InternalError("putdata LA storage mismatch".into())
                        })?
                        .as_mut(),
                    crate::pipeline::PixelMode::RGB
                    | crate::pipeline::PixelMode::YCbCr
                    | crate::pipeline::PixelMode::HSV => image
                        .as_mut_rgb8()
                        .ok_or_else(|| {
                            PilError::InternalError("putdata RGB storage mismatch".into())
                        })?
                        .as_mut(),
                    crate::pipeline::PixelMode::RGBA
                    | crate::pipeline::PixelMode::CMYK
                    | crate::pipeline::PixelMode::I
                    | crate::pipeline::PixelMode::F => image
                        .as_mut_rgba8()
                        .ok_or_else(|| {
                            PilError::InternalError("putdata RGBA storage mismatch".into())
                        })?
                        .as_mut(),
                };
                let destination = samples.get_mut(start..end).ok_or_else(|| {
                    PilError::InternalError("putdata storage offset out of bounds".into())
                })?;
                destination.copy_from_slice(&bytes);
                data.known_zero_fill = false;
            }
            _ => {
                return Err(PilError::InternalError(
                    "putdata did not materialize writable storage".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "data_access/putdata_shared_bytes_tests.rs"]
mod putdata_shared_bytes_tests;
