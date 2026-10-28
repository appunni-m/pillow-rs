use super::{
    Arc, Bytes, CheckedDims, Image, PilError, PipelineOp, PixelMode, PutDataInput, PutDataValue,
    PutDataValueKind, is_l16_mode, l16_uses_big_endian, materialization_cache, putdata_bytes,
    putdata_clip_u8, putdata_l16_sample,
};
impl Image {
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

    fn putdata_requested_backend(&self) -> Option<crate::compute::Backend> {
        if !crate::compute::Backend::pipeline_telemetry_enabled() {
            return None;
        }
        self.backend().or_else(|| {
            crate::compute::preferred_active_backend()
                .ok()
                .flatten()
                .and_then(|(backend, only_active)| only_active.then_some(backend))
        })
    }

    fn putdata_l16_value_at(
        &mut self,
        pixel_index: usize,
        value: &PutDataValue,
        scale: f64,
        offset: f64,
        requested_backend: Option<crate::compute::Backend>,
    ) -> Result<(), PilError> {
        let sample = putdata_l16_sample(value, scale, offset)?;
        self.load()?;
        if let Image::Loaded(data) = self {
            let image = Arc::make_mut(&mut data.image);
            let destination = image
                .as_mut_luma16()
                .ok_or_else(|| PilError::InternalError("putdata I;16 storage mismatch".into()))?;
            let pixel = destination.as_mut().get_mut(pixel_index).ok_or_else(|| {
                PilError::InternalError("putdata I;16 offset out of bounds".into())
            })?;
            *pixel = sample;
            data.known_zero_fill = false;
            crate::compute::record_eager_cpu_operation_receipt_with_reason(
                "PutData",
                requested_backend,
                0,
                "per-item writes must be visible before the next sequence value is coerced",
            );
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
        // This public helper is the callback-preserving path: writes are
        // visible before the binding coerces the next value, so they execute
        // directly on host storage even when SIMD/GPU is requested. Capture
        // the request before `load()` consumes a locked pipeline so telemetry
        // can report the actual CPU route as a backend gap.
        let requested_backend = self.putdata_requested_backend();

        let (width, height) = self.size()?;
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PilError::DimensionError("pixel count overflow".into()))?;
        if pixel_index >= pixel_count {
            return Err(PilError::TypeError("too many data entries".into()));
        }

        let mode_name = self.mode()?;
        if is_l16_mode(&mode_name) {
            return self.putdata_l16_value_at(pixel_index, value, scale, offset, requested_backend);
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
        crate::compute::record_eager_cpu_operation_receipt_with_reason(
            "PutData",
            requested_backend,
            0,
            "per-item writes must be visible before the next sequence value is coerced",
        );
        Ok(())
    }
}
