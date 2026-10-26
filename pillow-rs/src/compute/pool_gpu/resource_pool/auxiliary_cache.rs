use super::{
    Arc, AuxiliaryImages, BufferRange, HashMap, MAX_GPU_AUXILIARY_CACHE_BYTES, PilError,
    PipelineOp, append_arena_slice, extract_lut, gpu_luma16_paste_source, gpu_mode_after_op,
    pack_dynamic_rgba,
};
/// Immutable auxiliary resources shared by operations in one execution.
///
/// GPU submissions are split at a bounded operation/resource budget, but the
/// source graph may reuse the same secondary image, mask, or LUT on both sides
/// of that boundary. Keep only repeated resources in this execution-wide
/// cache; unique resources remain chunk-local. The aggregate cache is capped
/// so a long graph cannot turn deduplication into unbounded retention.
#[derive(Default)]
pub(in crate::compute::pool_gpu) struct GpuAuxiliaryCache {
    pub(in crate::compute::pool_gpu) second_ranges: HashMap<usize, BufferRange>,
    pub(in crate::compute::pool_gpu) third_ranges: HashMap<usize, BufferRange>,
    pub(in crate::compute::pool_gpu) lut_ranges: HashMap<[u32; 256], BufferRange>,
    pub(in crate::compute::pool_gpu) img2_values: Vec<u32>,
    pub(in crate::compute::pool_gpu) img3_values: Vec<u32>,
    pub(in crate::compute::pool_gpu) lut_values: Vec<u32>,
}

impl GpuAuxiliaryCache {
    pub(in crate::compute::pool_gpu) fn from_batch(
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
            if !matches!(op, PipelineOp::PutData { .. })
                && let Some(second) = auxiliary.second.as_ref()
            {
                *second_counts
                    .entry(Arc::as_ptr(second) as usize)
                    .or_default() += 1;
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
            if !matches!(op, PipelineOp::PutData { .. })
                && let Some(second) = auxiliary.second.as_ref()
            {
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
            if let Some(lut) = extract_lut(op, *op_mode)
                && lut_counts.get(&lut).copied().unwrap_or_default() > 1
                && !cache.lut_ranges.contains_key(&lut)
                && cache.total_bytes().saturating_add(lut.len() * 4)
                    <= MAX_GPU_AUXILIARY_CACHE_BYTES
            {
                let range = append_arena_slice(&mut cache.lut_values, &lut, storage_alignment);
                cache.lut_ranges.insert(lut, range);
            }
        }
        Ok(cache)
    }

    pub(in crate::compute::pool_gpu) fn total_bytes(&self) -> usize {
        (self.img2_values.len() + self.img3_values.len() + self.lut_values.len()) * 4
    }
}
