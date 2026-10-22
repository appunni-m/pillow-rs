//! GPU buffers and resources owned by an in-flight batch.

use super::*;

#[derive(Clone, Copy)]
pub(super) struct BufferRange {
    pub(super) offset: u64,
    pub(super) size: u64,
}

/// Immutable auxiliary resources shared by operations in one execution.
///
/// GPU submissions are split at a bounded operation/resource budget, but the
/// source graph may reuse the same secondary image, mask, or LUT on both sides
/// of that boundary. Keep only repeated resources in this execution-wide
/// cache; unique resources remain chunk-local. The aggregate cache is capped
/// so a long graph cannot turn deduplication into unbounded retention.
#[derive(Default)]
pub(super) struct GpuAuxiliaryCache {
    pub(super) second_ranges: HashMap<usize, BufferRange>,
    pub(super) third_ranges: HashMap<usize, BufferRange>,
    pub(super) lut_ranges: HashMap<[u32; 256], BufferRange>,
    pub(super) img2_values: Vec<u32>,
    pub(super) img3_values: Vec<u32>,
    pub(super) lut_values: Vec<u32>,
}

impl GpuAuxiliaryCache {
    pub(super) fn from_batch(
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        mode: u32,
        capacity: u32,
        storage_alignment: usize,
    ) -> Result<Self, PilError> {
        let mut second_counts = HashMap::<usize, usize>::new();
        let mut third_counts = HashMap::<usize, usize>::new();
        let mut lut_counts = HashMap::<[u32; 256], usize>::new();
        let mut op_modes = Vec::with_capacity(ops.len());
        let mut current_mode = mode;
        for op in ops {
            op_modes.push(current_mode);
            current_mode = gpu_mode_after_op(current_mode, op);
        }
        for ((op, auxiliary), op_mode) in ops.iter().zip(auxiliary_images).zip(&op_modes) {
            if !matches!(op, PipelineOp::PutData { .. }) {
                if let Some(second) = auxiliary.second.as_ref() {
                    *second_counts
                        .entry(Arc::as_ptr(second) as usize)
                        .or_default() += 1;
                }
            }
            if let Some(third) = auxiliary.third.as_ref() {
                *third_counts.entry(Arc::as_ptr(third) as usize).or_default() += 1;
            }
            if let Some(lut) = extract_lut(op, *op_mode) {
                *lut_counts.entry(lut).or_default() += 1;
            }
        }

        let mut cache = Self::default();
        for ((op, auxiliary), op_mode) in ops.iter().zip(auxiliary_images).zip(&op_modes) {
            if !matches!(op, PipelineOp::PutData { .. }) {
                if let Some(second) = auxiliary.second.as_ref() {
                    let key = Arc::as_ptr(second) as usize;
                    if second_counts.get(&key).copied().unwrap_or_default() > 1
                        && !cache.second_ranges.contains_key(&key)
                    {
                        // Typed I;16 Paste sources use a numeric u16 word
                        // rather than the ordinary RGBA byte transport. Do
                        // not place that representation in the shared cache:
                        // the cache key is the image identity and a later
                        // byte-oriented operation could otherwise reuse the
                        // same image with the wrong packing.
                        if !gpu_luma16_paste_source(op, *op_mode, second) {
                            let values = if matches!(op, PipelineOp::ConvertLab { .. }) {
                                crate::lab::gpu_table_words().to_vec()
                            } else {
                                pack_dynamic_rgba(second.as_ref(), capacity)?
                            };
                            if cache.total_bytes().saturating_add(values.len() * 4)
                                <= MAX_GPU_AUXILIARY_CACHE_BYTES
                            {
                                let range = append_arena_slice(
                                    &mut cache.img2_values,
                                    &values,
                                    storage_alignment,
                                );
                                cache.second_ranges.insert(key, range);
                            }
                        }
                    }
                }
            }
            if let Some(third) = auxiliary.third.as_ref() {
                let key = Arc::as_ptr(third) as usize;
                if third_counts.get(&key).copied().unwrap_or_default() > 1
                    && !cache.third_ranges.contains_key(&key)
                {
                    let values = pack_dynamic_rgba(third.as_ref(), capacity)?;
                    if cache.total_bytes().saturating_add(values.len() * 4)
                        <= MAX_GPU_AUXILIARY_CACHE_BYTES
                    {
                        let range =
                            append_arena_slice(&mut cache.img3_values, &values, storage_alignment);
                        cache.third_ranges.insert(key, range);
                    }
                }
            }
            if let Some(lut) = extract_lut(op, *op_mode) {
                if lut_counts.get(&lut).copied().unwrap_or_default() > 1
                    && !cache.lut_ranges.contains_key(&lut)
                {
                    if cache.total_bytes().saturating_add(lut.len() * 4)
                        <= MAX_GPU_AUXILIARY_CACHE_BYTES
                    {
                        let range =
                            append_arena_slice(&mut cache.lut_values, &lut, storage_alignment);
                        cache.lut_ranges.insert(lut, range);
                    }
                }
            }
        }
        Ok(cache)
    }

    pub(super) fn total_bytes(&self) -> usize {
        (self.img2_values.len() + self.img3_values.len() + self.lut_values.len()) * 4
    }
}

/// A batch-owned arena allocation that grows to the largest plan seen by the
/// working-buffer pool and is reused by later plans. The buffer is never
/// shrunk: keeping the capacity with the already-pooled image buffers avoids
/// recreating parameter and auxiliary resources for every materialization.
pub(super) struct ReusableGpuBuffer {
    pub(super) buffer: wgpu::Buffer,
    pub(super) capacity_bytes: u64,
}

impl ReusableGpuBuffer {
    pub(super) fn new(
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        initial_bytes: usize,
        alignment_bytes: usize,
    ) -> Self {
        let capacity_bytes = aligned_bytes(initial_bytes, alignment_bytes) as u64;
        Self {
            buffer: create_sized_buffer(
                device,
                label,
                usage,
                capacity_bytes as usize,
                alignment_bytes,
            ),
            capacity_bytes,
        }
    }

    pub(super) fn ensure_capacity(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        usage: wgpu::BufferUsages,
        required_bytes: usize,
        alignment_bytes: usize,
    ) {
        let required_bytes = aligned_bytes(required_bytes, alignment_bytes) as u64;
        if required_bytes <= self.capacity_bytes {
            return;
        }
        self.buffer = create_sized_buffer(
            device,
            label,
            usage,
            required_bytes as usize,
            alignment_bytes,
        );
        self.capacity_bytes = required_bytes;
    }
}

// ─── BufferPool ────────────────────────────────────────────────────────────

/// GPU storage owned by one in-flight pipeline batch and returned to the
/// bounded working-set pool after readback completes.
///
/// The device, queue, and compiled pipelines are shared, but mutable image
/// storage must not be shared across batches. Keeping these buffers local to
/// an execution removes the need for a process-wide execution mutex. Arena
/// writes remain ordered with their corresponding queue submissions.
pub(super) struct BufferPool {
    pub(super) buf_a: wgpu::Buffer,
    pub(super) buf_b: wgpu::Buffer,
    pub(super) buf_img2: wgpu::Buffer, // Second image for dual-input ops
    pub(super) buf_img3: wgpu::Buffer, // Third image for 3-input ops (Composite/Paste mask)
    pub(super) lut_buf: wgpu::Buffer,  // LUT storage buffer for Eval/PointOp (1024 bytes)
    pub(super) histogram_buf: wgpu::Buffer, // Histogram control storage (1024 u32 words)
    pub(super) params_arena: ReusableGpuBuffer,
    pub(super) img2_arena: ReusableGpuBuffer,
    pub(super) img3_arena: ReusableGpuBuffer,
    pub(super) lut_arena: ReusableGpuBuffer,
    // Native transposes and selected native byte filters may upload directly
    // to shared Metal storage. Keep this write-combined input separate from
    // cached readback buffers.
    #[cfg(target_endian = "little")]
    pub(super) mapped_native_input: Option<ReusableGpuBuffer>,
    /// The zero-source projection shader has stable bindings for a reused
    /// working set. Cache its bind group so timed calls do not rebuild one.
    pub(super) projection_zero_bind_group: Option<(u64, wgpu::BindGroup)>,
    pub(super) capacity: u32,
}

/// One reusable map-readable destination for the final device-to-host copy.
///
/// A staging buffer is returned to the pool only after `map_async` completes
/// and the buffer has been unmapped, so no queued command or host mapping can
/// still own it when a later materialization acquires it.
pub(super) struct StagingBuffer {
    pub(super) buffer: wgpu::Buffer,
    pub(super) capacity_bytes: u64,
}

impl StagingBuffer {
    pub(super) fn new(device: &wgpu::Device, capacity_bytes: u64) -> Self {
        Self {
            buffer: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu_readback_staging"),
                size: capacity_bytes.max(4),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            capacity_bytes: capacity_bytes.max(4),
        }
    }
}

/// The result stays in its batch-owned primary buffer on supported integrated
/// Metal devices. Other devices retain the explicit device-to-staging copy.
pub(super) enum ReadbackTarget {
    Primary,
    Staging(StagingBuffer),
}

impl ReadbackTarget {
    pub(super) fn buffer<'a>(
        &'a self,
        buffers: &'a BufferPool,
        final_is_a: bool,
    ) -> &'a wgpu::Buffer {
        match self {
            Self::Primary if final_is_a => &buffers.buf_a,
            Self::Primary => &buffers.buf_b,
            Self::Staging(staging) => &staging.buffer,
        }
    }

    pub(super) fn full_frame_copy_count(&self, input_uploaded: bool) -> u64 {
        // The staged path adds a full-frame copy from the final storage
        // buffer. Output-only generators do not count a source upload.
        u64::from(input_uploaded) + u64::from(matches!(self, Self::Staging(_)))
    }
}

impl BufferPool {
    pub(super) fn new(device: &wgpu::Device, capacity: u32, direct_primary_readback: bool) -> Self {
        Self::new_with_mapped_input(device, capacity, direct_primary_readback, false)
    }

    pub(super) fn new_with_mapped_input(
        device: &wgpu::Device,
        capacity: u32,
        direct_primary_readback: bool,
        mapped_input: bool,
    ) -> Self {
        let size = (capacity.max(1) as u64) * 4;
        let mut image_usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        if direct_primary_readback {
            // MAP_READ selects shared memory with normal CPU caching on Metal.
            // MAP_WRITE would select write-combined caching and penalize reads.
            image_usage |= wgpu::BufferUsages::MAP_READ;
        }
        let input_usage = if mapped_input {
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::MAP_WRITE
        } else {
            image_usage
        };
        let buf_a = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_a"),
            size,
            usage: input_usage,
            mapped_at_creation: false,
        });
        let buf_b = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_b"),
            size,
            usage: image_usage,
            mapped_at_creation: false,
        });
        let buf_img2 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_img2"),
            // Missing optional auxiliary inputs are never read by the
            // shader. A single storage element is sufficient as the
            // fallback binding and avoids allocating another full image.
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let buf_img3 = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_buf_img3"),
            size: 4,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lut_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_lut"),
            size: 1024, // 256 entries * 4 bytes each
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let histogram_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu_histogram"),
            size: GPU_HISTOGRAM_BYTES as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            4,
            4,
        );
        let img2_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_img2",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            4,
            4,
        );
        let img3_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_img3",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            4,
            4,
        );
        let lut_arena = ReusableGpuBuffer::new(
            device,
            "gpu_batch_lut",
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            1024,
            4,
        );
        BufferPool {
            buf_a,
            buf_b,
            buf_img2,
            buf_img3,
            lut_buf,
            histogram_buf,
            params_arena,
            img2_arena,
            img3_arena,
            lut_arena,
            #[cfg(target_endian = "little")]
            mapped_native_input: None,
            projection_zero_bind_group: None,
            capacity,
        }
    }

    pub(super) fn upload_rgba(
        &self,
        queue: &wgpu::Queue,
        rgba: &RgbaImage,
    ) -> Result<(), PilError> {
        let (w, h) = rgba.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixel_count > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < image size {}",
                self.capacity, pixel_count
            )));
        }

        // WGSL packs R, G, B, and A into the low-to-high bytes of one u32.
        // Little-endian RGBA storage already has exactly that byte layout, so
        // upload it directly instead of allocating and filling a second host
        // vector. Big-endian builds retain the explicit portable packer.
        #[cfg(target_endian = "little")]
        queue.write_buffer(&self.buf_a, 0, rgba.as_raw());
        #[cfg(target_endian = "big")]
        {
            let packed = pack_rgba(rgba, self.capacity)?;
            queue.write_buffer(&self.buf_a, 0, bytemuck::cast_slice(&packed));
        }

        // The first ordinary dispatch writes buf_b completely. Uploading the
        // source to both ping-pong buffers doubled host-to-device traffic and
        // did not provide input to any dispatch.
        Ok(())
    }

    #[cfg(target_endian = "little")]
    pub(super) fn upload_rgb(&self, queue: &wgpu::Queue, rgb: &RgbImage) -> Result<(), PilError> {
        let (w, h) = rgb.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixel_count > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < image size {}",
                self.capacity, pixel_count
            )));
        }
        let byte_count = CheckedDims::new(w, h, 4)?.total_bytes();
        let byte_count = u64::try_from(byte_count)
            .map_err(|_| PilError::InternalError("GPU RGB upload size overflow".into()))?;
        let Some(size) = NonZeroU64::new(byte_count) else {
            return Ok(());
        };
        let mut upload = queue
            .write_buffer_with(&self.buf_a, 0, size)
            .ok_or_else(|| PilError::InternalError("GPU RGB staging allocation failed".into()))?;
        expand_rgb_into_rgba(rgb.as_raw(), &mut upload)?;
        drop(upload);
        Ok(())
    }

    pub(super) fn upload_standard_image(
        &self,
        queue: &wgpu::Queue,
        image: &DynamicImage,
    ) -> Result<(), PilError> {
        #[cfg(target_endian = "little")]
        if let DynamicImage::ImageRgb8(rgb) = image {
            return self.upload_rgb(queue, rgb);
        }
        if let DynamicImage::ImageRgba8(rgba) = image {
            return self.upload_rgba(queue, rgba);
        }

        let rgba = image.to_rgba8();
        self.upload_rgba(queue, &rgba)
    }

    /// Upload four `L` samples per storage word. Packed-luma shaders extract
    /// each byte by pixel index, avoiding an expanded RGBA host image and a
    /// four-byte transfer per sample.
    pub(super) fn upload_packed_luma8(
        &self,
        queue: &wgpu::Queue,
        image: &crate::raster::GrayImage,
    ) -> Result<(), PilError> {
        let (w, h) = image.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        let transfer_bytes = pixel_count
            .div_ceil(4)
            .checked_mul(4)
            .ok_or_else(|| PilError::ValueError("GPU packed-luma input is too large".into()))?;
        let capacity_bytes = (self.capacity as usize)
            .checked_mul(std::mem::size_of::<u32>())
            .ok_or_else(|| PilError::ValueError("GPU packed-luma buffer is too large".into()))?;
        if transfer_bytes > capacity_bytes || image.as_raw().len() != pixel_count {
            return Err(PilError::ValueError(
                "GPU packed-luma input does not fit its checked buffer".into(),
            ));
        }
        let size = u64::try_from(transfer_bytes)
            .ok()
            .and_then(NonZeroU64::new)
            .ok_or_else(|| PilError::ValueError("GPU packed-luma input is empty".into()))?;
        let mut upload = queue
            .write_buffer_with(&self.buf_a, 0, size)
            .ok_or_else(|| {
                PilError::InternalError("GPU packed-luma staging allocation failed".into())
            })?;
        upload.fill(0);
        upload[..pixel_count].copy_from_slice(image.as_raw());
        Ok(())
    }

    #[cfg(target_endian = "little")]
    pub(super) fn upload_native_channel_bytes(
        &self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        channels: u8,
        raw: &[u8],
    ) -> Result<(), PilError> {
        self.upload_native_channel_bytes_with_la_patch(queue, width, height, channels, raw, None)
    }

    #[cfg(target_endian = "little")]
    pub(super) fn upload_native_channel_bytes_with_la_patch(
        &self,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        channels: u8,
        raw: &[u8],
        la_patch: Option<(usize, [u8; 2])>,
    ) -> Result<(), PilError> {
        let expected_bytes = CheckedDims::new(width, height, channels)?.total_bytes();
        if raw.len() != expected_bytes {
            return Err(PilError::ValueError(
                "GPU native channel input does not match its checked dimensions".into(),
            ));
        }
        if let Some((offset, _)) = la_patch {
            if channels != 2 || offset.checked_add(2).is_none_or(|end| end > expected_bytes) {
                return Err(PilError::ValueError(
                    "GPU native LA patch does not match its checked dimensions".into(),
                ));
            }
        }
        let transfer_bytes = expected_bytes
            .div_ceil(std::mem::size_of::<u32>())
            .checked_mul(std::mem::size_of::<u32>())
            .ok_or_else(|| PilError::ValueError("GPU channel input is too large".into()))?;
        if transfer_bytes / std::mem::size_of::<u32>() > self.capacity as usize {
            return Err(PilError::ValueError(
                "GPU native channel input exceeds its checked buffer".into(),
            ));
        }
        let size = u64::try_from(transfer_bytes)
            .ok()
            .and_then(NonZeroU64::new)
            .ok_or_else(|| PilError::ValueError("GPU channel input is empty".into()))?;
        let mut upload = queue
            .write_buffer_with(&self.buf_a, 0, size)
            .ok_or_else(|| {
                PilError::InternalError("GPU native channel staging allocation failed".into())
            })?;
        upload.fill(0);
        upload[..expected_bytes].copy_from_slice(raw);
        if let Some((offset, pixel)) = la_patch {
            upload[offset..offset + 2].copy_from_slice(&pixel);
        }
        Ok(())
    }

    #[cfg(target_endian = "little")]
    pub(super) fn upload_packed_rgb(
        &self,
        queue: &wgpu::Queue,
        image: &RgbImage,
        layout: &PackedRgbTransposeLayout,
    ) -> Result<(), PilError> {
        if image.as_raw().len() != layout.native_bytes
            || layout.transfer_bytes > u64::from(self.capacity) * 4
        {
            return Err(PilError::InternalError(
                "GPU packed RGB upload does not fit its checked working set".into(),
            ));
        }
        let size = NonZeroU64::new(layout.transfer_bytes)
            .ok_or_else(|| PilError::InternalError("GPU packed RGB upload is empty".into()))?;
        let mut upload = queue
            .write_buffer_with(&self.buf_a, 0, size)
            .ok_or_else(|| PilError::InternalError("GPU RGB staging allocation failed".into()))?;
        upload[..layout.native_bytes].copy_from_slice(image.as_raw());
        upload[layout.native_bytes..].fill(0);
        Ok(())
    }

    /// Upload an `I;16*` image as one zero-extended sample per packed storage
    /// word. The admitted GPU operations for this representation only move
    /// complete words, so no shader is allowed to interpret the sample as
    /// four independent RGBA bytes. Keeping the high bytes zero also makes
    /// the readback contract explicit instead of narrowing through `to_rgba8`.
    pub(super) fn upload_luma16(
        &self,
        queue: &wgpu::Queue,
        image: &ImageBuffer<Luma<u16>, Vec<u16>>,
        mode: Option<&str>,
    ) -> Result<(), PilError> {
        let (w, h) = image.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixel_count > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < image size {}",
                self.capacity, pixel_count
            )));
        }
        let mut packed = Vec::with_capacity(pixel_count * std::mem::size_of::<u32>());
        for &sample in image.as_raw() {
            let sample_bytes = if matches!(mode, Some("I;16B" | "I;16N")) {
                sample.to_be_bytes()
            } else {
                sample.to_ne_bytes()
            };
            packed.extend_from_slice(&[sample_bytes[0], sample_bytes[1], 0, 0]);
        }
        queue.write_buffer(&self.buf_a, 0, &packed);
        Ok(())
    }

    /// Upload `I;16*` samples as numeric unsigned values for a conversion.
    /// Geometry keeps the public byte order opaque, but Convert needs the
    /// decoded sample value to apply Pillow's clamp-to-255 rule. WGSL storage
    /// words are little-endian, so write the host-native `u16` value into the
    /// low two bytes independently of the source mode's declared byte order.
    pub(super) fn upload_luma16_numeric(
        &self,
        queue: &wgpu::Queue,
        image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    ) -> Result<(), PilError> {
        let (w, h) = image.dimensions();
        let pixel_count = CheckedDims::new(w, h, 1)?.total_pixels();
        if pixel_count > self.capacity as usize {
            return Err(PilError::ValueError(format!(
                "GPU buffer capacity {} < image size {}",
                self.capacity, pixel_count
            )));
        }
        let packed = pack_luma16_numeric(image, self.capacity)?;
        queue.write_buffer(&self.buf_a, 0, bytemuck::cast_slice(&packed));
        Ok(())
    }

    pub(super) fn retained_bytes(&self) -> u64 {
        let bytes = gpu_working_set_bytes(self.capacity)
            .saturating_add(self.params_arena.capacity_bytes)
            .saturating_add(self.img2_arena.capacity_bytes)
            .saturating_add(self.img3_arena.capacity_bytes)
            .saturating_add(self.lut_arena.capacity_bytes);
        #[cfg(target_endian = "little")]
        let bytes = bytes.saturating_add(
            self.mapped_native_input
                .as_ref()
                .map_or(0, |input| input.capacity_bytes),
        );
        bytes
    }
}
