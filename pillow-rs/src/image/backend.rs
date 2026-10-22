//! Explicit backend selection and recursive pipeline locking for [`Image`].

use std::sync::Arc;

use image_slash_star::{ImageFormat, ImageInfo};

use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

use super::{Image, PipelineOps, image_mode_name, materialization_cache};

impl Image {
    /// Locks this image pipeline to one compute backend.
    ///
    /// The backend choice is applied when the image is materialized. Non-pipeline
    /// images are returned unchanged because there is no deferred work to route.
    pub fn use_backend(mut self, b: crate::compute::Backend) -> Image {
        self.lock_backend_recursive(b);
        self
    }

    fn lock_backend_recursive(&mut self, b: crate::compute::Backend) {
        let Image::Pipeline {
            source,
            ops,
            backend,
            materialized,
            ..
        } = self
        else {
            return;
        };

        Arc::make_mut(source).lock_backend_recursive(b);
        let mut flattened_ops = ops.to_vec();
        for op in &mut flattened_ops {
            Self::lock_op_backend_recursive(op, b);
        }
        *ops = PipelineOps::from_vec(flattened_ops);
        *backend = Some(b);
        // A previously inspected/materialized pipeline must execute again
        // under the newly forced backend, including nested pipeline nodes.
        *materialized = materialization_cache();
    }

    fn lock_op_backend_recursive(op: &mut PipelineOp, b: crate::compute::Backend) {
        match op {
            PipelineOp::Add { other, .. }
            | PipelineOp::Subtract { other, .. }
            | PipelineOp::Multiply { other }
            | PipelineOp::Screen { other }
            | PipelineOp::Darker { other }
            | PipelineOp::Lighter { other }
            | PipelineOp::Difference { other }
            | PipelineOp::Overlay { other }
            | PipelineOp::HardLight { other }
            | PipelineOp::SoftLight { other }
            | PipelineOp::AddModulo { other }
            | PipelineOp::SubtractModulo { other }
            | PipelineOp::LogicalAnd { other }
            | PipelineOp::LogicalOr { other }
            | PipelineOp::LogicalXor { other }
            | PipelineOp::BlendModule { other, .. } => {
                Arc::make_mut(other).lock_backend_recursive(b);
            }
            PipelineOp::CompositeModule { other, mask, .. } => {
                Arc::make_mut(other).lock_backend_recursive(b);
                Arc::make_mut(mask).lock_backend_recursive(b);
            }
            PipelineOp::Paste { source, mask, .. } => {
                Arc::make_mut(source).lock_backend_recursive(b);
                if let Some(mask) = mask {
                    Arc::make_mut(mask).lock_backend_recursive(b);
                }
            }
            PipelineOp::AlphaComposite { source, .. } => {
                Arc::make_mut(source).lock_backend_recursive(b);
            }
            PipelineOp::Merge { bands, .. } => {
                for band in Arc::make_mut(bands).iter_mut() {
                    band.lock_backend_recursive(b);
                }
            }
            PipelineOp::Autocontrast {
                mask: Some(mask), ..
            }
            | PipelineOp::EqualizeMasked { mask } => {
                Arc::make_mut(mask).lock_backend_recursive(b);
            }
            _ => {}
        }
    }

    /// Returns the backend explicitly locked on this pipeline, if any.
    pub fn backend(&self) -> Option<crate::compute::Backend> {
        match self {
            Image::Pipeline { backend, .. } => *backend,
            _ => None,
        }
    }
}

impl Image {
    /// Try applying ImageOps.grayscale immediately for the bounded native-RGB
    /// CPU/SIMD path. Other inputs keep the ordinary lazy pipeline semantics.
    pub(crate) fn try_grayscale_eager_rgb(&self) -> Result<Option<Image>, PilError> {
        if cfg!(feature = "parallel") {
            return Ok(None);
        }
        let Image::Loaded(data) = self else {
            return Ok(None);
        };
        let logical_mode = data
            .explicit_mode
            .as_deref()
            .unwrap_or_else(|| image_mode_name(data.decoded_mode));
        if logical_mode != "RGB" {
            return Ok(None);
        }

        let image = self.materialized_shared()?;
        if !matches!(image.as_ref(), DynamicImage::ImageRgb8(_)) {
            return Ok(None);
        }
        let dims =
            crate::checked_dims::CheckedDims::new_allow_empty(image.width(), image.height(), 3)?;
        let pixels = dims.total_pixels();
        if pixels == 0 || pixels > 256 || image.as_bytes().len() != dims.total_bytes() {
            return Ok(None);
        }

        let Some((backend, _)) = crate::compute::preferred_active_backend()? else {
            return Ok(None);
        };
        let operation = PipelineOp::Grayscale;
        let grayscale = match backend {
            crate::compute::Backend::Cpu => crate::compute::execute_eager_cpu_image_operation(
                "Grayscale",
                image.as_ref(),
                false,
                || crate::compute::registry::execute_cpu(&operation, image.as_ref(), Some("RGB")),
            )?,
            crate::compute::Backend::Simd => crate::compute::execute_eager_simd_image_operation(
                "Grayscale",
                image.as_ref(),
                || {
                    crate::compute::simd_grayscale_with_evidence(
                        image.as_ref(),
                        &operation,
                        Some("RGB"),
                    )
                },
            )?,
            crate::compute::Backend::Gpu => return Ok(None),
        };

        let mut result = Image::from_dynamic(grayscale, None);
        if let Image::Loaded(data) = &mut result {
            data.info = self.image_info();
            data.exif = self.exif_metadata();
        }
        Ok(Some(result))
    }
}

/// GPU result provenance without retaining the input graph or its pixel owners.
#[cfg(feature = "gpu")]
#[derive(Clone)]
pub(crate) struct GpuResultMetadata {
    info: Option<ImageInfo>,
    exif: Option<Vec<u8>>,
    format: Option<ImageFormat>,
}

#[cfg(feature = "gpu")]
impl GpuResultMetadata {
    pub(crate) fn materialize(&self, image: DynamicImage, mode: &str) -> Image {
        let mut result = Image::from_dynamic(image, Some(mode.to_owned()));
        if let Image::Loaded(data) = &mut result {
            data.info = self.info.clone();
            data.exif = self.exif.clone();
            data.source_format = self.format;
        }
        result
    }
}

#[cfg(feature = "gpu")]
pub(crate) struct GpuPendingGraph<'a> {
    pub(crate) source: &'a Image,
    pub(crate) cached: Option<Arc<DynamicImage>>,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) mode: String,
    pub(crate) encoded_bytes: u64,
    pub(crate) ops: Vec<&'a PipelineOp>,
}

#[cfg(feature = "gpu")]
fn gpu_info_bytes(info: &ImageInfo) -> u64 {
    let profile_bytes = |profile: &image_slash_star::RawIccProfile| {
        (profile.keyword.len() + profile.data.len()) as u64
    };
    4096 + info
        .palette
        .as_ref()
        .map_or(0, |p| (p.rgb.len() + p.alpha.len()) as u64)
        + info.source_color.icc_profile().map_or(0, profile_bytes)
        + info
            .source
            .avif_item_icc_profiles()
            .iter()
            .map(|p| profile_bytes(p.profile()))
            .sum::<u64>()
        + std::mem::size_of_val(info.source.avif_item_locations()) as u64
        + std::mem::size_of_val(info.source.avif_item_properties()) as u64
        + std::mem::size_of_val(info.source.avif_item_plane_properties()) as u64
        + std::mem::size_of_val(info.source.avif_item_codec_properties()) as u64
        + std::mem::size_of_val(info.source.avif_item_relationships()) as u64
        + std::mem::size_of_val(info.source.avif_auxiliary_relationships()) as u64
        + std::mem::size_of_val(info.source.avif_premultiplied_relationships()) as u64
        + std::mem::size_of_val(info.source.avif_item_color_properties()) as u64
        + std::mem::size_of_val(info.source.avif_grid_item_ids()) as u64
}

impl Image {
    /// Inspect the primary graph without decoding or executing an image operation.
    #[cfg(feature = "gpu")]
    pub(crate) fn gpu_pending_graph(&self) -> Result<GpuPendingGraph<'_>, PilError> {
        self.gpu_pending_graph_depth(0)
    }

    #[cfg(feature = "gpu")]
    fn gpu_pending_graph_depth(&self, depth: usize) -> Result<GpuPendingGraph<'_>, PilError> {
        if depth > 64 {
            return Err(PilError::ValueError(
                "GPU graph exceeds 64 source levels".into(),
            ));
        }
        match self {
            Self::Pipeline {
                source,
                ops,
                materialized,
                backend,
                explicit_mode,
                palette,
                mode,
                ..
            } => {
                if backend.is_some_and(|backend| backend != crate::Backend::Gpu) {
                    return Err(PilError::ValueError(
                        "GPU batch graph is locked to another backend".into(),
                    ));
                }
                if palette.is_some() || matches!(explicit_mode.as_deref(), Some("P" | "PA")) {
                    return Err(PilError::ValueError(
                        "GPU graph palette boundary is not yet represented by a native plan".into(),
                    ));
                }
                if let Some(pixels) = materialized.get() {
                    let pixels = pixels.clone()?;
                    let pixel_mode = explicit_mode
                        .clone()
                        .or_else(|| {
                            mode.get()
                                .and_then(|m| m.as_ref().ok().and_then(|m| m.clone()))
                        })
                        .unwrap_or_else(|| image_mode_name(pixels.color().into()).to_owned());
                    return Ok(GpuPendingGraph {
                        source: self,
                        width: pixels.width(),
                        height: pixels.height(),
                        mode: pixel_mode,
                        cached: Some(pixels),
                        encoded_bytes: 0,
                        ops: Vec::new(),
                    });
                }
                let (mut graph, skip) = if let Some(prefix) = ops.prefix_cache()
                    && let Some(pixels) = prefix.materialized.get()
                {
                    let pixels = pixels.clone()?;
                    let pixel_mode = explicit_mode
                        .clone()
                        .unwrap_or_else(|| image_mode_name(pixels.color().into()).to_owned());
                    (
                        GpuPendingGraph {
                            source: self,
                            width: pixels.width(),
                            height: pixels.height(),
                            mode: pixel_mode,
                            cached: Some(pixels),
                            encoded_bytes: 0,
                            ops: Vec::new(),
                        },
                        prefix.ops_len,
                    )
                } else {
                    (source.gpu_pending_graph_depth(depth + 1)?, 0)
                };
                if graph
                    .ops
                    .len()
                    .saturating_add(ops.len().saturating_sub(skip))
                    > 256
                {
                    return Err(PilError::ValueError(
                        "GPU batch graph exceeds 256 pending operations".into(),
                    ));
                }
                graph.ops.extend(ops.as_slice()[skip..].iter());
                Ok(graph)
            }
            Self::Loaded(data) => Ok(GpuPendingGraph {
                source: self,
                cached: Some(data.image.clone()),
                width: data.image.width(),
                height: data.image.height(),
                mode: data
                    .explicit_mode
                    .clone()
                    .unwrap_or_else(|| image_mode_name(data.decoded_mode).to_owned()),
                encoded_bytes: 0,
                ops: Vec::new(),
            }),
            Self::Bytes {
                source,
                info: Some(info),
                materialized,
                ..
            } => Ok(GpuPendingGraph {
                source: self,
                cached: materialized.get().map(|image| image.clone()).transpose()?,
                width: info.width,
                height: info.height,
                mode: image_mode_name(info.mode).to_owned(),
                encoded_bytes: source.bytes().len() as u64,
                ops: Vec::new(),
            }),
            _ => Err(PilError::ValueError(
                "GPU source has no proven native byte header/layout".into(),
            )),
        }
    }

    /// Conservative metadata ownership reservation, without cloning payloads.
    #[cfg(feature = "gpu")]
    pub(crate) fn gpu_metadata_bytes(&self) -> u64 {
        let mut leaf = self;
        while let Self::Pipeline { source, .. } = leaf {
            leaf = source;
        }
        let (info, exif) = match leaf {
            Self::Pipeline { .. } => unreachable!(),
            Self::Loaded(data) => (data.info.as_ref(), data.exif.as_ref()),
            Self::Paletted(data) => (data.info.as_ref(), data.exif.as_ref()),
            Self::Bytes { source, info, .. } => {
                return (source.bytes().len() as u64)
                    .saturating_mul(16)
                    .saturating_add(info.as_ref().map_or(0, gpu_info_bytes));
            }
        };
        4096 + exif.map_or(0, |bytes| bytes.len() as u64) + info.map_or(0, gpu_info_bytes)
    }

    /// Publish pixels produced externally, retaining the normal pipeline metadata.
    #[cfg(feature = "gpu")]
    pub(crate) fn gpu_result_metadata(&self) -> GpuResultMetadata {
        GpuResultMetadata {
            info: self.image_info(),
            exif: self.exif_metadata(),
            format: match self {
                Self::Pipeline { format, .. } | Self::Bytes { format, .. } => *format,
                Self::Loaded(source) => source.source_format,
                Self::Paletted(source) => source.source_format,
            },
        }
    }

    /// Return whether two cloned handles refer to the same immutable source
    /// node for execution-resource reuse.
    ///
    /// Public wrappers intentionally create a fresh `Arc<Image>` for each
    /// operation argument, so comparing those outer handles would miss the
    /// fact that the underlying loaded pixels or lazy materialization cache is
    /// shared. This identity is narrower than pixel equality: independently
    /// created images with equal bytes must not be treated as one secondary.
    pub(crate) fn shares_execution_source(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Loaded(left), Self::Loaded(right)) => Arc::ptr_eq(&left.image, &right.image),
            (Self::Paletted(left), Self::Paletted(right)) => {
                Arc::ptr_eq(&left.materialized, &right.materialized)
            }
            (
                Self::Bytes {
                    materialized: left, ..
                },
                Self::Bytes {
                    materialized: right,
                    ..
                },
            )
            | (
                Self::Pipeline {
                    materialized: left, ..
                },
                Self::Pipeline {
                    materialized: right,
                    ..
                },
            ) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }
}
