use super::*;

pub(super) struct GpuBatchAuxiliaryRanges {
    pub(super) second: Option<BufferRange>,
    pub(super) third: Option<BufferRange>,
    pub(super) lut: Option<BufferRange>,
}

pub(super) struct GpuBatchAuxiliaryPreparation<'a> {
    pub(super) op: &'a PipelineOp,
    pub(super) auxiliary: &'a AuxiliaryImages,
    pub(super) index: usize,
    pub(super) op_mode: u32,
    pub(super) logical_mode: Option<&'a str>,
    pub(super) source_dimensions: (u32, u32),
    pub(super) cached_pipeline: &'a CachedPipeline,
    pub(super) packed_luma_putdata: bool,
    pub(super) direct_alpha_composite_rgba: Option<&'a [u8]>,
    pub(super) capacity: u32,
    pub(super) storage_alignment: usize,
    pub(super) auxiliary_cache: &'a GpuAuxiliaryCache,
    pub(super) img2_arena: &'a mut Vec<u32>,
    pub(super) img3_arena: &'a mut Vec<u32>,
    pub(super) lut_arena: &'a mut Vec<u32>,
    pub(super) second_cache: &'a mut HashMap<usize, BufferRange>,
    pub(super) third_cache: &'a mut HashMap<usize, BufferRange>,
    pub(super) lut_cache: &'a mut HashMap<[u32; 256], BufferRange>,
}

impl GpuBatchAuxiliaryPreparation<'_> {
    pub(super) fn prepare(mut self) -> Result<GpuBatchAuxiliaryRanges, PilError> {
        let second = self.second_range()?;
        let third = self.third_range()?;
        let lut = self.lut_range()?;
        Ok(GpuBatchAuxiliaryRanges { second, third, lut })
    }

    fn second_range(&mut self) -> Result<Option<BufferRange>, PilError> {
        let dimensions = self.source_dimensions;
        if gpu_transform_uses_geometry_table(self.op, self.logical_mode, dimensions) {
            let values = gpu_transform_geometry_words(self.op, dimensions, self.logical_mode)?;
            let mut range =
                append_arena_slice(&mut *self.img2_arena, &values, self.storage_alignment);
            range.offset += (self.auxiliary_cache.img2_values.len() * 4) as u64;
            return Ok(Some(range));
        }
        if let PipelineOp::PutData { data, mode } = self.op {
            let values = if self.packed_luma_putdata {
                pack_luma8_putdata(data, self.capacity)?
            } else {
                pack_put_data(data, *mode, self.capacity)?
            };
            if values.is_empty() {
                return Ok(None);
            }
            let mut range =
                append_arena_slice(&mut *self.img2_arena, &values, self.storage_alignment);
            range.offset += (self.auxiliary_cache.img2_values.len() * 4) as u64;
            return Ok(Some(range));
        }

        let Some(second) = self.auxiliary.second.as_ref() else {
            return Ok(None);
        };
        let key = Arc::as_ptr(second) as usize;
        if matches!(self.op, PipelineOp::ConvertLab { .. }) {
            if let Some(range) = self.auxiliary_cache.second_ranges.get(&key).copied() {
                return Ok(Some(range));
            }
            if let Some(range) = self.second_cache.get(&key).copied() {
                return Ok(Some(range));
            }
            let mut range = append_arena_slice(
                &mut *self.img2_arena,
                crate::lab::gpu_table_words(),
                self.storage_alignment,
            );
            range.offset += (self.auxiliary_cache.img2_values.len() * 4) as u64;
            self.second_cache.insert(key, range);
            return Ok(Some(range));
        }
        if gpu_luma16_paste_source(self.op, self.op_mode, second) {
            let DynamicImage::ImageLuma16(source) = second.as_ref() else {
                return Err(PilError::InternalError(
                    "GPU typed Paste source was admitted without an ImageLuma16 buffer".into(),
                ));
            };
            let values = pack_luma16_numeric(source, self.capacity)?;
            let mut range =
                append_arena_slice(&mut *self.img2_arena, &values, self.storage_alignment);
            range.offset += (self.auxiliary_cache.img2_values.len() * 4) as u64;
            return Ok(Some(range));
        }
        if self.index == 0
            && let Some(bytes) = self.direct_alpha_composite_rgba
        {
            return Ok(Some(BufferRange {
                offset: 0,
                size: bytes.len() as u64,
            }));
        }
        if let Some(range) = self.auxiliary_cache.second_ranges.get(&key).copied() {
            return Ok(Some(range));
        }
        if let Some(range) = self.second_cache.get(&key).copied() {
            return Ok(Some(range));
        }
        let mut range = append_dynamic_rgba(
            &mut *self.img2_arena,
            second.as_ref(),
            self.capacity,
            self.storage_alignment,
        )?;
        range.offset += (self.auxiliary_cache.img2_values.len() * 4) as u64;
        self.second_cache.insert(key, range);
        Ok(Some(range))
    }

    fn third_range(&mut self) -> Result<Option<BufferRange>, PilError> {
        let Some(third) = self.auxiliary.third.as_ref() else {
            return Ok(None);
        };
        let key = Arc::as_ptr(third) as usize;
        if let Some(range) = self.auxiliary_cache.third_ranges.get(&key).copied() {
            return Ok(Some(range));
        }
        if let Some(range) = self.third_cache.get(&key).copied() {
            return Ok(Some(range));
        }
        let mut range = append_dynamic_rgba(
            &mut *self.img3_arena,
            third.as_ref(),
            self.capacity,
            self.storage_alignment,
        )?;
        range.offset += (self.auxiliary_cache.img3_values.len() * 4) as u64;
        self.third_cache.insert(key, range);
        Ok(Some(range))
    }

    fn lut_range(&mut self) -> Result<Option<BufferRange>, PilError> {
        if !self.cached_pipeline.is_lut
            || matches!(
                self.cached_pipeline.variant_name,
                "__internal_resize_h"
                    | "__internal_resize_v"
                    | "__internal_resize_v_pad_rgbx"
                    | "__internal_resize_h_pad_luma_packed"
                    | "__internal_resize_v_pad_luma_packed"
            )
        {
            return Ok(None);
        }
        let lut = extract_lut(self.op, self.op_mode).ok_or_else(|| {
            PilError::ValueError(format!(
                "GPU LUT length does not match source mode for '{}'",
                registry::variant_key(self.op)
            ))
        })?;
        if let Some(range) = self.auxiliary_cache.lut_ranges.get(&lut).copied() {
            return Ok(Some(range));
        }
        if let Some(range) = self.lut_cache.get(&lut).copied() {
            return Ok(Some(range));
        }
        let mut range = append_arena_slice(&mut *self.lut_arena, &lut, self.storage_alignment);
        range.offset += (self.auxiliary_cache.lut_values.len() * 4) as u64;
        self.lut_cache.insert(lut, range);
        Ok(Some(range))
    }
}
