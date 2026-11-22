use super::{
    Arc, Bytes, Image, PilError, PipelineOp, PutDataInput, PutDataValue, PutDataValueKind,
    is_l16_identity, materialization_cache, putdata_byte_width, putdata_bytes,
    putdata_hidden_lane_bytes, putdata_hidden_lanes, unsupported_putdata_mode,
    uses_native_value_at_putdata,
};
use crate::compute::PipelineOperationTelemetryCapture;
use crate::image::PutDataExecutionContext;
use crate::raster::Mode;

impl Image {
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
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
        self.putdata_with_execution_context(data, None)
    }

    /// Queues replacement bytes under a route captured by a host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_with_execution_context(
        &mut self,
        data: &[u8],
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<(), PilError> {
        let payload_mode = self.mode_identity()?;
        putdata_byte_width(payload_mode).ok_or_else(|| unsupported_putdata_mode(payload_mode))?;
        let mut new_self = Image::push_op(
            self,
            PipelineOp::PutData {
                data: Bytes::copy_from_slice(data),
                hidden_lanes: None,
                payload_mode,
            },
        );
        if let Some((_, context)) = route {
            new_self = new_self.with_execution_context(context);
        }
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
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putdata_shared_bytes_fast_path(
        &mut self,
        data: Bytes,
        entry_count: usize,
        scale: f64,
        offset: f64,
    ) -> Result<bool, PilError> {
        self.putdata_shared_bytes_fast_path_with_context(data, entry_count, scale, offset, None)
    }

    /// Queues shared immutable bytes with the wrapper's captured execution route.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_shared_bytes_fast_path_with_context(
        &mut self,
        data: Bytes,
        entry_count: usize,
        scale: f64,
        offset: f64,
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<bool, PilError> {
        if scale != 1.0 || offset != 0.0 || data.len() != entry_count {
            return Ok(false);
        }
        let payload_mode = self.mode_identity()?;
        if !matches!(
            payload_mode,
            Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8
        ) {
            return Ok(false);
        }
        self.validate_putdata_length(entry_count)?;
        let mut new_self = Image::push_op(
            self,
            PipelineOp::PutData {
                data,
                hidden_lanes: None,
                payload_mode,
            },
        );
        if let Some((_, context)) = route {
            new_self = new_self.with_execution_context(context);
        }
        *self = new_self;
        Ok(true)
    }

    /// Normalizes one value per pixel through the mode's PutData strategy.
    ///
    /// Pillow 12.2's `_imaging.c:_putdata` applies `scale` and `offset` only to
    /// single-band, signed-integer, and float images. Its multiband `getink`
    /// path instead treats integers as little-endian packed pixels and ignores
    /// scaling. CoreOnly high-depth modes write clamped `u16` or cast `f32`
    /// components directly into native storage, without a byte projection;
    /// these tuple writes also leave `scale` and `offset` unapplied. The
    /// target-output matrix records this `NO_PILLOW_SUPPORT` extension.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::TypeError`] for incompatible value shapes or too
    /// many pixels, and propagates image mode or dimension errors.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putdata_values(
        &mut self,
        values: &[PutDataValue],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        self.putdata_values_with_execution_context(values, scale, offset, None)
    }

    /// Normalizes values under a route captured by the host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_values_with_execution_context(
        &mut self,
        values: &[PutDataValue],
        scale: f64,
        offset: f64,
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<(), PilError> {
        self.validate_putdata_length(values.len())?;

        let image_mode = self.mode_identity()?;
        if uses_native_value_at_putdata(image_mode) {
            if is_l16_identity(image_mode) {
                let mut context = self.putdata_context(route);
                for (pixel_index, value) in values.iter().enumerate() {
                    self.putdata_value_at_with_context(
                        pixel_index,
                        value,
                        scale,
                        offset,
                        &mut context,
                    )?;
                }
                return Ok(());
            }
            return self.putdata_core_only_native_values(image_mode, values, route);
        }
        let data = putdata_bytes(image_mode, values, scale, offset)?;
        let hidden_lanes = putdata_hidden_lane_bytes(image_mode, values)?;
        let mut new_self = Image::push_op(
            self,
            PipelineOp::PutData {
                data: Bytes::from(data),
                hidden_lanes,
                payload_mode: image_mode,
            },
        );
        if let Some((_, context)) = route {
            new_self = new_self.with_execution_context(context);
        }
        *self = new_self;
        Ok(())
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putdata_core_only_native_values(
        &mut self,
        mode: Mode,
        values: &[PutDataValue],
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<(), PilError> {
        // TODO(pillow-rs#3ptb): verify all five native bulk outputs, ordered
        // prefix writes on invalid values, and fallback receipts in the frozen
        // CPU parity matrix.
        if values.is_empty() {
            return Ok(());
        }

        let mut context = self.putdata_context(route);
        context.operation_receipts.capture();
        let first =
            crate::compute::pool_cpu::ops::put::helpers::put_data_value_at::native_value_at_input(
                mode, &values[0],
            )?;
        let requested_backend = context.requested_backend;
        let selection = self.putdata_value_at_selection(mode, &mut context)?;
        self.load()?;
        context.operation_receipts.capture();

        let mut completed = 0;
        let result = match self {
            Image::Loaded(data) => {
                let result =
                    crate::compute::pool_cpu::ops::put::ops::execute_core_only_put_data_values(
                        Arc::make_mut(&mut data.image),
                        values,
                        first,
                        selection,
                        &mut completed,
                    );
                if completed != 0 {
                    data.known_zero_fill = false;
                }
                result
            }
            _ => Err(PilError::InternalError(
                "native putdata did not materialize writable storage".into(),
            )),
        };

        if completed != 0 {
            crate::compute::record_eager_cpu_operation_receipt_with_selection_and_receipts(
                "PutData",
                requested_backend,
                0,
                "native CoreOnly PutData writes update host storage synchronously",
                selection,
                context.operation_receipts,
            );
        }
        result
    }

    /// Returns the host-language value shape required by the current mode.
    ///
    /// This is the core-owned mode policy for `putdata`. A binding may still
    /// need to inspect host-language protocol behavior, but it does not need
    /// to duplicate the mode's scalar or component shape.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putdata_value_kind(&self) -> Result<PutDataValueKind, PilError> {
        let image_mode = self.mode_identity()?;
        if matches!(
            image_mode,
            Mode::One
                | Mode::ImageLuma8
                | Mode::ImagePalette8
                | Mode::ImageI32
                | Mode::ImageI16
                | Mode::ImageI16L
                | Mode::ImageI16B
                | Mode::ImageI16N
                | Mode::ImageF32
        ) {
            return Ok(PutDataValueKind::Numeric);
        }
        match image_mode {
            Mode::ImageLumaA16 => {
                return Ok(PutDataValueKind::NativeU16Components { channels: 2 });
            }
            Mode::ImageRgb16 => {
                return Ok(PutDataValueKind::NativeU16Components { channels: 3 });
            }
            Mode::ImageRgba16 => {
                return Ok(PutDataValueKind::NativeU16Components { channels: 4 });
            }
            Mode::ImageRgb32F => {
                return Ok(PutDataValueKind::FloatComponents { channels: 3 });
            }
            Mode::ImageRgba32F => {
                return Ok(PutDataValueKind::FloatComponents { channels: 4 });
            }
            _ => {}
        }
        let channels =
            putdata_byte_width(image_mode).ok_or_else(|| unsupported_putdata_mode(image_mode))?;
        Ok(PutDataValueKind::Components { channels })
    }

    /// Applies a callback-free bulk input through the canonical `putdata`
    /// normalization path.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putdata_input(
        &mut self,
        input: PutDataInput<'_>,
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        self.putdata_input_with_execution_context(input, scale, offset, None)
    }

    /// Applies extracted values under the route captured by the host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_input_with_execution_context(
        &mut self,
        input: PutDataInput<'_>,
        scale: f64,
        offset: f64,
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<(), PilError> {
        match input {
            PutDataInput::Integers(values) => {
                self.putdata_integer_values_with_execution_context(values, scale, offset, route)
            }
            PutDataInput::Numbers(values) => {
                self.putdata_numeric_values_with_execution_context(values, scale, offset, route)
            }
        }
    }

    /// Replaces numeric samples through the shared `putdata` normalization path.
    ///
    /// Bindings use this callback-free entry point for exact built-in numeric
    /// sequences. Keeping the conversion in core reaches the same bulk path
    /// for every binding while custom Python values can still use
    /// [`Image::putdata_value_at`] to preserve re-entrant coercion order.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putdata_numeric_values(
        &mut self,
        values: &[f64],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        self.putdata_numeric_values_with_execution_context(values, scale, offset, None)
    }

    /// Writes numeric samples under the route captured by the host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_numeric_values_with_execution_context(
        &mut self,
        values: &[f64],
        scale: f64,
        offset: f64,
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<(), PilError> {
        let values = values
            .iter()
            .copied()
            .map(PutDataValue::Number)
            .collect::<Vec<_>>();
        self.putdata_values_with_execution_context(&values, scale, offset, route)
    }

    /// Replaces exact integer samples through Pillow's numeric-or-packed
    /// `putdata` path.
    ///
    /// Pillow interprets an integer as a scaled numeric sample for
    /// single-band modes, but as a little-endian packed pixel for multiband
    /// modes. Bindings only need to extract the host integer sequence; this
    /// mode distinction belongs here so every backend receives the same
    /// canonical operation.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putdata_integer_values(
        &mut self,
        values: &[i64],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        self.putdata_integer_values_with_execution_context(values, scale, offset, None)
    }

    /// Writes integer samples under the route captured by the host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_integer_values_with_execution_context(
        &mut self,
        values: &[i64],
        scale: f64,
        offset: f64,
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<(), PilError> {
        match self.putdata_value_kind()? {
            PutDataValueKind::Numeric => {
                let values = values
                    .iter()
                    .copied()
                    .map(|value| PutDataValue::Number(value as f64))
                    .collect::<Vec<_>>();
                self.putdata_values_with_execution_context(&values, scale, offset, route)
            }
            PutDataValueKind::Components { .. } => {
                let values = values
                    .iter()
                    .copied()
                    .map(PutDataValue::Packed)
                    .collect::<Vec<_>>();
                self.putdata_values_with_execution_context(&values, scale, offset, route)
            }
            PutDataValueKind::NativeU16Components { .. } => {
                let values = values
                    .iter()
                    .copied()
                    .map(PutDataValue::Packed)
                    .collect::<Vec<_>>();
                self.putdata_values_with_execution_context(&values, scale, offset, route)
            }
            PutDataValueKind::FloatComponents { .. } => Err(PilError::TypeError(
                "floating multiband putdata requires component tuples".into(),
            )),
        }
    }

    /// Validates the sequence length before host-language value coercion.
    ///
    /// Pillow rejects an oversized `putdata` sequence before converting any
    /// element, so bindings must delegate this check before they iterate a
    /// Python sequence. Keeping it here also gives every binding the same
    /// dimension and error behavior.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
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
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putdata_bytes_fast_path(
        &mut self,
        data: &[u8],
        entry_count: usize,
        scale: f64,
        offset: f64,
    ) -> Result<bool, PilError> {
        self.putdata_bytes_fast_path_with_execution_context(data, entry_count, scale, offset, None)
    }

    /// Applies byte-oriented fast paths under the wrapper-captured route.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_bytes_fast_path_with_execution_context(
        &mut self,
        data: &[u8],
        entry_count: usize,
        scale: f64,
        offset: f64,
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<bool, PilError> {
        let image_mode = self.mode_identity()?;
        if is_l16_identity(image_mode) {
            self.putdata_l16_bytes_with_count(data, entry_count, scale, offset, route)?;
            return Ok(true);
        }
        if matches!(
            image_mode,
            Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8
        ) {
            if scale == 1.0 && offset == 0.0 && data.len() == entry_count {
                // Bytes already contain the exact one-byte samples Pillow
                // stores for these modes. Queue them directly instead of
                // widening every byte to PutDataValue and encoding it back.
                self.putdata_with_execution_context(data, route)?;
                return Ok(true);
            }
            let values = data
                .iter()
                .copied()
                // Rust callers may provide more logical entries than the
                // physical byte slice. Zero-fill those missing values; the
                // Pillow image8 wrapper always derives this count from the
                // physical bytes payload.
                .chain(std::iter::repeat(0))
                .take(entry_count)
                .map(|value| PutDataValue::Number(f64::from(value)))
                .collect::<Vec<_>>();
            self.putdata_values_with_execution_context(&values, scale, offset, route)?;
            return Ok(true);
        }
        Ok(false)
    }

    /// Replaces raw I;16 samples from Pillow's bytes fast path.
    ///
    /// Pillow treats a `bytes` argument to `putdata` as byte-oriented storage
    /// for I;16 modes, rather than as one numeric sample per byte. Its
    /// `_imaging.c:_putdata` image8 fast path copies at most one logical row's
    /// worth of input bytes at a time. It updates only those raw bytes and
    /// preserves all other existing sample bytes. With a scale or offset it
    /// clips each input byte before placing it in that row layout.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putdata_l16_bytes(
        &mut self,
        data: &[u8],
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        self.putdata_l16_bytes_with_count(data, data.len(), scale, offset, None)
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putdata_l16_bytes_with_count(
        &mut self,
        data: &[u8],
        entry_count: usize,
        scale: f64,
        offset: f64,
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> Result<(), PilError> {
        self.validate_putdata_length(entry_count)?;
        let mode = self.mode_identity()?;
        let (requested_backend, context) = match route {
            Some(route) => route,
            None => self.cpu_arch_execution_route_advisory(),
        };
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        self.load()?;
        operation_receipts.capture();
        if let Image::Loaded(image_data) = self {
            let selection =
                crate::compute::pool_cpu::ops::put::ops::select_put_data_l16_bytes(mode, &context)?;
            crate::compute::pool_cpu::ops::put::ops::execute_put_data_l16_bytes(
                Arc::make_mut(&mut image_data.image),
                data,
                entry_count,
                scale,
                offset,
                selection,
            )?;
            image_data.known_zero_fill = false;
            crate::compute::record_eager_cpu_operation_receipt_with_selection_and_receipts(
                "PutData",
                requested_backend,
                0,
                "I;16 byte writes update host storage synchronously",
                selection,
                operation_receipts,
            );
            return Ok(());
        }
        Err(PilError::InternalError(
            "putdata I;16 did not materialize writable storage".into(),
        ))
    }

    /// Creates the routed CPU context for synchronous value writes.
    ///
    /// Bindings retain this value across callback-driven sequence processing
    /// and pass it to [`Image::putdata_value_at_with_context`]. SIMD requests
    /// retain their detected architecture so exact-mode selection can use a
    /// matching strategy or report the Default fallback. A pinned GPU request
    /// uses CPU Default; an unpinned GPU preference can use an active SIMD
    /// architecture for this synchronous host-write path.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_execution_context(&self) -> PutDataExecutionContext {
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        self.putdata_execution_context_for_route(requested_backend, context)
    }

    /// Creates callback-write state from a route selected by the host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_execution_context_for_route(
        &self,
        requested_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
    ) -> PutDataExecutionContext {
        PutDataExecutionContext {
            context,
            requested_backend,
            selection: None,
            operation_receipts: PipelineOperationTelemetryCapture::begin_preserving_current(),
        }
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putdata_context(
        &self,
        route: Option<(
            Option<crate::compute::Backend>,
            crate::compute::ExecutionContext,
        )>,
    ) -> PutDataExecutionContext {
        match route {
            Some((requested_backend, context)) => {
                self.putdata_execution_context_for_route(requested_backend, context)
            }
            None => self.putdata_execution_context(),
        }
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn putdata_value_at_selection(
        &self,
        mode: Mode,
        context: &mut PutDataExecutionContext,
    ) -> Result<crate::compute::registry::support::StrategySelection, PilError> {
        if let Some(selection) = context.selection
            && selection.implementation.mode == mode
        {
            return Ok(selection);
        }
        let selection = crate::compute::pool_cpu::ops::put::ops::select_put_data_value_at(
            mode,
            &context.context,
        )?;
        context.selection = Some(selection);
        Ok(selection)
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
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn putdata_value_at(
        &mut self,
        pixel_index: usize,
        value: &PutDataValue,
        scale: f64,
        offset: f64,
    ) -> Result<(), PilError> {
        let mut context = self.putdata_execution_context();
        self.putdata_value_at_with_context(pixel_index, value, scale, offset, &mut context)
    }

    /// Applies one normalized value using the shared callback-sequence context.
    ///
    /// The binding creates one context per host sequence and passes it through
    /// each synchronous write after coercing that value without holding the
    /// image borrow. A changed exact mode refreshes the cached mode strategy.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn putdata_value_at_with_context(
        &mut self,
        pixel_index: usize,
        value: &PutDataValue,
        scale: f64,
        offset: f64,
        context: &mut PutDataExecutionContext,
    ) -> Result<(), PilError> {
        // Keep receipts from earlier writes and callback-driven operations
        // outside the per-write telemetry reset below. The context restores
        // them together when the whole synchronous sequence ends.
        context.operation_receipts.capture();

        // This public helper is the callback-preserving path: writes are
        // visible before the binding coerces the next value, so they execute
        // directly on host storage even when SIMD/GPU is requested. Retain the
        // request captured by the route context before `load()` consumes a
        // locked pipeline so telemetry reports the actual CPU fallback.
        let requested_backend = context.requested_backend;

        let (width, height) = self.size()?;
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PilError::DimensionError("pixel count overflow".into()))?;
        if pixel_index >= pixel_count {
            return Err(PilError::TypeError("too many data entries".into()));
        }

        let image_mode = self.mode_identity()?;
        let input = if is_l16_identity(image_mode) {
            crate::compute::pool_cpu::ops::put::helpers::put_data_value_at::ValueAtInput::Luma16(
                crate::compute::pool_cpu::ops::put::helpers::put_data_value_at::luma16_sample(
                    value, scale, offset,
                )?,
            )
        } else if uses_native_value_at_putdata(image_mode) {
            crate::compute::pool_cpu::ops::put::helpers::put_data_value_at::native_value_at_input(
                image_mode, value,
            )?
        } else {
            putdata_byte_width(image_mode).ok_or_else(|| unsupported_putdata_mode(image_mode))?;
            let bytes = putdata_bytes(image_mode, std::slice::from_ref(value), scale, offset)?;
            crate::compute::pool_cpu::ops::put::helpers::put_data_value_at::ValueAtInput::Bytes(
                bytes,
                putdata_hidden_lanes(image_mode, value)?,
            )
        };
        let selection = self.putdata_value_at_selection(image_mode, context)?;

        self.load()?;
        context.operation_receipts.capture();
        match self {
            Image::Paletted(data) if image_mode == Mode::ImagePalette8 => {
                crate::compute::pool_cpu::ops::put::ops::execute_put_data_value_at(
                    &mut data.indices,
                    pixel_index,
                    input,
                    selection,
                )?;
                data.materialized = materialization_cache();
            }
            Image::Loaded(data) => {
                let image = Arc::make_mut(&mut data.image);
                crate::compute::pool_cpu::ops::put::ops::execute_put_data_value_at(
                    image,
                    pixel_index,
                    input,
                    selection,
                )?;
                data.known_zero_fill = false;
            }
            _ => {
                return Err(PilError::InternalError(
                    "putdata did not materialize writable storage".into(),
                ));
            }
        }
        crate::compute::record_eager_cpu_operation_receipt_with_selection(
            "PutData",
            requested_backend,
            0,
            "per-item writes must be visible before the next sequence value is coerced",
            selection,
        );
        context.operation_receipts.capture();
        Ok(())
    }
}
