//! Lazy pipeline state, metadata inference, and construction methods for [`Image`].

use std::sync::{Arc, OnceLock};

use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp, PixelMode, TransposeMethod};
use crate::raster::DynamicImage;

use super::Image;

/// Persistent operation list used by lazy image nodes.
///
/// Appending to an existing `Image::Pipeline` keeps an immutable parent link
/// instead of cloning the complete operation vector. The first execution of a
/// node flattens the chain once and shares that immutable slice with every
/// subsequent route, metadata query, or backend execution. This keeps graph
/// construction linear for long lazy chains while preserving the existing
/// slice-based backend contract at the execution boundary.
#[derive(Debug, Clone)]
pub struct PipelineOps {
    chain: Arc<PipelineOpChain>,
    prefix_cache: Option<PipelinePrefixCache>,
}

#[derive(Debug)]
struct PipelineOpChain {
    parent: Option<Arc<PipelineOpChain>>,
    op: Option<PipelineOp>,
    len: usize,
    mode_preserving: bool,
    flattened: OnceLock<Arc<[PipelineOp]>>,
}

impl PipelineOps {
    pub(crate) fn empty() -> Self {
        Self {
            chain: Arc::new(PipelineOpChain {
                parent: None,
                op: None,
                len: 0,
                mode_preserving: true,
                flattened: OnceLock::new(),
            }),
            prefix_cache: None,
        }
    }

    pub(crate) fn one(op: PipelineOp) -> Self {
        let mode_preserving = op_preserves_mode(&op);
        Self {
            chain: Arc::new(PipelineOpChain {
                parent: None,
                op: Some(op),
                len: 1,
                mode_preserving,
                flattened: OnceLock::new(),
            }),
            prefix_cache: None,
        }
    }

    pub(crate) fn append(&self, op: PipelineOp) -> Self {
        Self {
            chain: Arc::new(PipelineOpChain {
                parent: Some(Arc::clone(&self.chain)),
                mode_preserving: self.chain.mode_preserving && op_preserves_mode(&op),
                op: Some(op),
                len: self.chain.len + 1,
                flattened: OnceLock::new(),
            }),
            prefix_cache: self.prefix_cache.clone(),
        }
    }

    pub(crate) fn from_vec(ops: Vec<PipelineOp>) -> Self {
        ops.into_iter()
            .fold(Self::empty(), |pipeline, op| pipeline.append(op))
    }

    pub(crate) fn as_slice(&self) -> &[PipelineOp] {
        self.chain
            .flattened
            .get_or_init(|| {
                let mut reversed = Vec::with_capacity(self.chain.len);
                let mut cursor = Some(Arc::clone(&self.chain));
                while let Some(node) = cursor {
                    if let Some(op) = node.op.as_ref() {
                        reversed.push(op.clone());
                    }
                    cursor = node.parent.as_ref().map(Arc::clone);
                }
                reversed.reverse();
                Arc::<[PipelineOp]>::from(reversed)
            })
            .as_ref()
    }

    pub(crate) fn to_vec(&self) -> Vec<PipelineOp> {
        self.as_slice().to_vec()
    }

    pub(crate) fn mode_preserving(&self) -> bool {
        self.chain.mode_preserving
    }

    pub(crate) fn len(&self) -> usize {
        self.chain.len
    }

    pub(crate) fn prefix_cache(&self) -> Option<&PipelinePrefixCache> {
        self.prefix_cache.as_ref()
    }

    fn with_prefix_cache(mut self, prefix_cache: Option<PipelinePrefixCache>) -> Self {
        self.prefix_cache = prefix_cache;
        self
    }
}

/// Whether an operation leaves the logical Pillow mode unchanged.
///
/// This metadata is deliberately conservative.  Returning `false` only
/// means that a later mode query may materialize; it never changes execution
/// semantics.  The fast path is important for lazy native-mode chains such as
/// repeated `ImageOps.invert`, whose validation needs the source mode but not
/// the pixels.
fn op_preserves_mode(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Resize { .. }
            | PipelineOp::ResizeBoxed { .. }
            | PipelineOp::Crop { .. }
            | PipelineOp::Rotate { .. }
            | PipelineOp::Transpose { .. }
            | PipelineOp::Thumbnail { .. }
            | PipelineOp::Reduce { .. }
            | PipelineOp::Filter3x3 { .. }
            | PipelineOp::Filter5x5 { .. }
            | PipelineOp::GaussianBlur { .. }
            | PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::MaxFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::RankFilter { .. }
            | PipelineOp::Autocontrast { .. }
            | PipelineOp::Invert
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Posterize { .. }
            | PipelineOp::Solarize { .. }
            | PipelineOp::Contain { .. }
            | PipelineOp::Cover { .. }
            | PipelineOp::Fit { .. }
            | PipelineOp::Pad { .. }
            | PipelineOp::Scale { .. }
            | PipelineOp::Expand { .. }
            | PipelineOp::CropBorder { .. }
            | PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
            | PipelineOp::Multiply { .. }
            | PipelineOp::Screen { .. }
            | PipelineOp::Darker { .. }
            | PipelineOp::Lighter { .. }
            | PipelineOp::Difference { .. }
            | PipelineOp::Overlay { .. }
            | PipelineOp::HardLight { .. }
            | PipelineOp::SoftLight { .. }
            | PipelineOp::AddModulo { .. }
            | PipelineOp::SubtractModulo { .. }
            | PipelineOp::LogicalAnd { .. }
            | PipelineOp::LogicalOr { .. }
            | PipelineOp::LogicalXor { .. }
            | PipelineOp::Offset { .. }
            | PipelineOp::Duplicate
            | PipelineOp::InvertChops
            | PipelineOp::Brightness { .. }
            | PipelineOp::Contrast { .. }
            | PipelineOp::ColorSaturation { .. }
            | PipelineOp::Sharpness { .. }
            | PipelineOp::EffectSpread { .. }
            | PipelineOp::Paste { .. }
            | PipelineOp::DrawLine { .. }
            | PipelineOp::DrawRectangle { .. }
            | PipelineOp::DrawRoundedRect { .. }
            | PipelineOp::DrawEllipse { .. }
            | PipelineOp::DrawCircle { .. }
            | PipelineOp::DrawPolygon { .. }
            | PipelineOp::DrawArc { .. }
            | PipelineOp::DrawChord { .. }
            | PipelineOp::DrawPieslice { .. }
            | PipelineOp::DrawPoint { .. }
            | PipelineOp::PutPixel { .. }
            // PutData records the image's current mode when it is queued, so
            // the generic mode-preserving planner path is authoritative.
            | PipelineOp::PutData { .. }
            | PipelineOp::Eval { .. }
    )
}

fn known_putalpha_mode(mode: PixelMode) -> Option<&'static str> {
    match mode {
        PixelMode::YCbCr | PixelMode::HSV | PixelMode::RGB => Some("RGBA"),
        PixelMode::L | PixelMode::LA => Some("LA"),
        PixelMode::RGBA => Some("RGBA"),
        PixelMode::P | PixelMode::PA => Some("PA"),
        PixelMode::CMYK => Some("RGBA"),
        // Palette putalpha is tagged as PA in push_op before this planner is
        // reached. Public putalpha also rejects 1/I/F, YCbCr, and HSV before
        // queueing; keep this fallback for malformed/internal descriptors.
        _ => None,
    }
}

/// Computes a statically known logical mode for one pipeline operation.
///
/// This is intentionally conservative: `None` retains the existing
/// materialization fallback and therefore cannot change public behavior. The
/// mode cache is separate from the pixel cache because metadata reads must not
/// publish or expose a partially executed image.
pub(crate) fn known_pipeline_op_mode(op: &PipelineOp, current: &str) -> Option<String> {
    if op_preserves_mode(op) {
        return Some(current.to_owned());
    }
    Some(
        match op {
            PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }
                if matches!(current, "P" | "PA") =>
            {
                "RGB"
            }
            PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. } => current,
            // Public convert and merge constructors attach `explicit_mode`
            // for non-raster-native targets (including CMYK, YCbCr, HSV, I,
            // F, P, and 1). `Image::mode` returns that tag before reaching
            // this planner, so only these four raster-native modes are live
            // here. Keep the fallback conservative for malformed/internal
            // descriptors instead of manufacturing metadata.
            PipelineOp::Convert { mode, .. } | PipelineOp::Merge { mode, .. } => match mode {
                ColorMode::L => "L",
                ColorMode::LA => "LA",
                ColorMode::RGB => "RGB",
                ColorMode::RGBA => "RGBA",
                _ => return None,
            },
            PipelineOp::ConvertLab { .. } => "LAB",
            PipelineOp::Grayscale => "L",
            PipelineOp::Colorize { .. } => "RGB",
            PipelineOp::Constant { .. } => "L",
            PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => {
                known_putalpha_mode(*mode)?
            }
            PipelineOp::ExtractBand { .. } => "L",
            PipelineOp::EffectNoise { .. } => "L",
            _ => return None,
        }
        .to_owned(),
    )
}

/// Rounds a non-negative dimension using Python's ties-to-even rule.
///
/// Metadata propagation must never manufacture a dimension for an invalid or
/// overflowing operation.  Returning `None` lets the normal materialization
/// path preserve the operation's existing error behavior.
fn round_dimension(value: f64) -> Option<u32> {
    if !value.is_finite() || value < 0.0 || value > f64::from(u32::MAX) {
        return None;
    }
    let lower = value.floor();
    let fraction = value - lower;
    let rounded = if fraction < 0.5 {
        lower
    } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
        lower + 1.0
    } else {
        lower
    };
    (rounded <= f64::from(u32::MAX)).then_some(rounded as u32)
}

/// Computes the output shape for a lazy operation without touching pixels.
///
/// The match is intentionally conservative: operations with secondary images,
/// validation that currently occurs during execution, or mode-specific storage
/// transitions return `None` and retain the existing materialization fallback.
pub(super) fn known_pipeline_op_dimensions(
    op: &PipelineOp,
    (width, height): (u32, u32),
) -> Option<(u32, u32)> {
    match op {
        PipelineOp::Resize { w, h, .. }
        | PipelineOp::ResizeBoxed { w, h, .. }
        | PipelineOp::Thumbnail { w, h, .. }
        | PipelineOp::Pad { w, h, .. }
        | PipelineOp::Transform { w, h, .. } => Some((*w, *h)),
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => Some((right.checked_sub(*left)?, bottom.checked_sub(*top)?)),
        PipelineOp::Expand { border, .. } => {
            let border = border.checked_mul(2)?;
            Some((width.checked_add(border)?, height.checked_add(border)?))
        }
        PipelineOp::CropBorder { border } => {
            let border = border.checked_mul(2)?;
            Some((width.checked_sub(border)?, height.checked_sub(border)?))
        }
        PipelineOp::Transpose { method } => Some(match method {
            TransposeMethod::Rotate90
            | TransposeMethod::Rotate270
            | TransposeMethod::Transpose
            | TransposeMethod::Transverse => (height, width),
            TransposeMethod::FlipLeftRight
            | TransposeMethod::FlipTopBottom
            | TransposeMethod::Rotate180 => (width, height),
        }),
        PipelineOp::Rotate {
            angle,
            expand,
            center,
            translate,
            ..
        } => {
            if !expand {
                return Some((width, height));
            }
            // Pillow takes an exact transpose fast path for right-angle
            // rotations before constructing the affine matrix, regardless
            // of the requested resampling filter. Reflect that public shape
            // here so lazy `size` agrees with the materialized result.
            let normalized = angle.rem_euclid(360.0);
            if center.is_none()
                && translate.is_none()
                && [90.0, 270.0]
                    .into_iter()
                    .any(|right_angle| (normalized - right_angle).abs() <= f64::EPSILON)
            {
                return Some((height, width));
            }
            let sw = f64::from(width);
            let sh = f64::from(height);
            let radians = -angle.to_radians();
            let affine_a = crate::ops::rotate::round_rotate_coefficient(radians.cos());
            let affine_b = crate::ops::rotate::round_rotate_coefficient(radians.sin());
            let affine_d = crate::ops::rotate::round_rotate_coefficient(-radians.sin());
            let affine_e = affine_a;
            let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
            let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
            let affine_c = affine_a * (-center_x - translate_x)
                + affine_b * (-center_y - translate_y)
                + center_x;
            let affine_f = affine_d * (-center_x - translate_x)
                + affine_e * (-center_y - translate_y)
                + center_y;
            let transform = |x: f64, y: f64| {
                (
                    affine_a * x + affine_b * y + affine_c,
                    affine_d * x + affine_e * y + affine_f,
                )
            };
            let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
            let (mut min_x, mut min_y, mut max_x, mut max_y) =
                (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for &(x, y) in &corners {
                let (rotated_x, rotated_y) = transform(x, y);
                min_x = min_x.min(rotated_x);
                max_x = max_x.max(rotated_x);
                min_y = min_y.min(rotated_y);
                max_y = max_y.max(rotated_y);
            }
            Some((
                round_dimension(max_x.ceil() - min_x.floor())?,
                round_dimension(max_y.ceil() - min_y.floor())?,
            ))
        }
        PipelineOp::Reduce { x_factor, y_factor } => Some((
            width.div_ceil((*x_factor).max(1)),
            height.div_ceil((*y_factor).max(1)),
        )),
        PipelineOp::Scale { factor, .. } => Some((
            round_dimension(f64::from(width) * *factor)?.max(1),
            round_dimension(f64::from(height) * *factor)?.max(1),
        )),
        PipelineOp::Contain { w, h, .. } | PipelineOp::Cover { w, h, .. } => {
            if width == 0 || height == 0 || *w == 0 || *h == 0 {
                return None;
            }
            let image_ratio = f64::from(width) / f64::from(height);
            let destination_ratio = f64::from(*w) / f64::from(*h);
            if (image_ratio - destination_ratio).abs() < 1e-10 {
                return Some((*w, *h));
            }
            let cover = matches!(op, PipelineOp::Cover { .. });
            if (image_ratio > destination_ratio) != cover {
                let new_height =
                    round_dimension(f64::from(height) / f64::from(width) * f64::from(*w))?;
                Some((*w, if cover { new_height.max(1) } else { new_height }))
            } else {
                let new_width =
                    round_dimension(f64::from(width) / f64::from(height) * f64::from(*h))?;
                Some((if cover { new_width.max(1) } else { new_width }, *h))
            }
        }
        // These operations are unary and have no deferred dimension or
        // secondary-image validation.  Keeping them here is what makes long
        // point/filter/draw chains answer `size()` without replaying pixels.
        PipelineOp::Convert { .. }
        | PipelineOp::ConvertLab { .. }
        | PipelineOp::RemapPalette { .. }
        | PipelineOp::Color3DLut { .. }
        | PipelineOp::ExtractBand { .. }
        | PipelineOp::Filter3x3 { .. }
        | PipelineOp::Filter5x5 { .. }
        | PipelineOp::GaussianBlur { .. }
        | PipelineOp::BoxBlur { .. }
        | PipelineOp::BoxBlurXY { .. }
        | PipelineOp::MedianFilter { .. }
        | PipelineOp::MaxFilter { .. }
        | PipelineOp::MinFilter { .. }
        | PipelineOp::RankFilter { .. }
        | PipelineOp::Autocontrast { .. }
        | PipelineOp::Equalize
        | PipelineOp::EqualizeMasked { .. }
        | PipelineOp::Invert
        | PipelineOp::Flip
        | PipelineOp::Mirror
        | PipelineOp::Posterize { .. }
        | PipelineOp::Solarize { .. }
        | PipelineOp::Grayscale
        | PipelineOp::Colorize { .. }
        | PipelineOp::Constant { .. }
        // Public putpixel validates coordinates before queuing and cannot
        // change the image dimensions. Keeping its shape known prevents a
        // later write from materializing the earlier writes just to check
        // bounds.
        | PipelineOp::PutPixel { .. }
        | PipelineOp::Offset { .. }
        | PipelineOp::Duplicate
        | PipelineOp::InvertChops
        | PipelineOp::Brightness { .. }
        | PipelineOp::Contrast { .. }
        | PipelineOp::ColorSaturation { .. }
        | PipelineOp::Sharpness { .. }
        | PipelineOp::EffectSpread { .. }
        | PipelineOp::Eval { .. }
        | PipelineOp::EffectNoise { .. }
        | PipelineOp::DrawLine { .. }
        | PipelineOp::DrawRectangle { .. }
        | PipelineOp::DrawRoundedRect { .. }
        | PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawPolygon { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. }
        | PipelineOp::DrawPoint { .. } => Some((width, height)),
        _ => None,
    }
}

impl std::ops::Deref for PipelineOps {
    type Target = [PipelineOp];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

/// Shared once-initialized operation-ready pixel result for lazy image nodes.
pub type MaterializationCache = Arc<OnceLock<Result<Arc<DynamicImage>, PilError>>>;

pub(super) fn take_cached_materialization(
    cache: MaterializationCache,
) -> Result<Arc<DynamicImage>, PilError> {
    match Arc::try_unwrap(cache) {
        Ok(cache) => cache.into_inner(),
        Err(cache) => cache.get().cloned(),
    }
    .ok_or_else(|| {
        PilError::InternalError("materialized image cache was unexpectedly empty".to_owned())
    })?
}

/// A materialized prefix that can be shared by flattened sibling pipelines.
///
/// This cache is intentionally limited to a prefix of a mode-preserving,
/// non-palette pipeline. Palette transitions and mode-changing operations
/// retain the ordinary full-chain evaluator because their representation and
/// metadata are part of the operation boundary.
#[derive(Debug, Clone)]
pub struct PipelinePrefixCache {
    /// Number of operations represented by the cached image.
    pub(super) ops_len: usize,
    /// Shared result for the prefix node.
    pub(super) materialized: MaterializationCache,
}

/// Shared once-initialized shape metadata for a lazy pipeline node.
///
/// `None` means that the operation list is valid but contains a dimension or
/// validation transition that cannot be determined without materialization.
/// The error case is cached as well so repeated metadata queries do not repeat
/// a failing source-header lookup.
pub type PipelineShapeCache = Arc<OnceLock<Result<Option<(u32, u32)>, PilError>>>;

/// Shared once-initialized logical output mode for a lazy pipeline node.
///
/// `None` means that the operation list contains a mode transition whose
/// representation cannot be described without materializing pixels.
pub type PipelineModeCache = Arc<OnceLock<Result<Option<String>, PilError>>>;

pub(crate) fn materialization_cache() -> MaterializationCache {
    Arc::new(OnceLock::new())
}

pub(crate) fn pipeline_shape_cache() -> PipelineShapeCache {
    Arc::new(OnceLock::new())
}

pub(crate) fn pipeline_mode_cache() -> PipelineModeCache {
    Arc::new(OnceLock::new())
}

impl Image {
    // ── Pipeline ops ──

    /// Returns a lazy image with `op` appended to its pipeline.
    ///
    /// Existing pipelines are extended in order. Non-pipeline images become the
    /// source of a new pipeline. Mode metadata and palettes are carried forward
    /// when the operation can preserve them; operations that fundamentally
    /// change mode clear or replace the explicit mode tag.
    pub fn push_op(source: &Image, op: PipelineOp) -> Image {
        let source_is_paletted = source.has_palette_samples();
        let palette_safe = source_is_paletted && Self::is_palette_safe_op_for_source(source, &op);
        let indexed_chops_result_without_palette =
            source_is_paletted && Self::indexed_chops_result_without_palette(&op);
        let putalpha_mode = match &op {
            PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => {
                Some(*mode)
            }
            _ => None,
        };
        let promotes_p_to_pa = source.has_palette_mode()
            && matches!(
                &op,
                PipelineOp::PutAlpha {
                    mode: crate::pipeline::PixelMode::P,
                    ..
                } | PipelineOp::PutAlphaData {
                    mode: crate::pipeline::PixelMode::P,
                    ..
                }
            );
        let explicit_mode = if matches!(
            &op,
            PipelineOp::ExtractBand { .. } | PipelineOp::Merge { .. }
        ) {
            None
        } else if source_is_paletted {
            if source.explicit_mode() == Some("PA") && matches!(&op, PipelineOp::PutPixel { .. }) {
                // PA writes keep the raw index/alpha sample layout even when
                // no RGB palette is attached, so later conversion still sees
                // the source as PA rather than generic LA.
                Some("PA".to_owned())
            } else {
                palette_safe.then(|| {
                    if source.explicit_mode() == Some("PA") {
                        "PA".to_owned()
                    } else {
                        "P".to_owned()
                    }
                })
            }
        } else {
            match &op {
                // Pillow's libImaging/Chops.c constant constructor always
                // allocates an L image, even when the source has an explicit
                // CMYK, YCbCr, HSV, I, F, or 1 mode tag. Do not carry that
                // source tag across the mode-changing operation.
                PipelineOp::Grayscale
                | PipelineOp::Constant { .. }
                | PipelineOp::Convert { .. }
                | PipelineOp::ConvertLab { .. }
                | PipelineOp::ExtractBand { .. } => None,
                _ => source.explicit_mode().map(str::to_owned),
            }
        };
        // `push_op` always constructs an Image::Pipeline. Keep putalpha's
        // output-mode promotion here so P becomes PA without expanding its
        // indices, while CMYK becomes ordinary RGBA without a CMYK tag;
        // callers do not need an impossible fallback match after queuing it.
        let explicit_mode = match putalpha_mode {
            Some(crate::pipeline::PixelMode::P | crate::pipeline::PixelMode::PA) => {
                Some("PA".to_owned())
            }
            Some(crate::pipeline::PixelMode::CMYK) => None,
            _ => explicit_mode,
        };
        let preserve_palette = palette_safe
            && !matches!(
                &op,
                PipelineOp::ExtractBand { .. } | PipelineOp::Merge { .. }
            );
        let source_palette = if indexed_chops_result_without_palette {
            Some(Vec::new())
        } else if preserve_palette {
            source.extract_palette()
        } else {
            None
        };
        let source_palette_alpha = if indexed_chops_result_without_palette {
            None
        } else if preserve_palette {
            source.palette_alpha()
        } else {
            None
        };
        match source {
            // Keep a concrete sample-layout boundary around palette expansion
            // and P↔PA transitions. A flattened batch has only one external
            // mode tag, so it cannot correctly run P operations before
            // putalpha or PA operations after it (especially on the GPU).
            Image::Pipeline { .. }
                if source_is_paletted
                    && (!palette_safe
                        || promotes_p_to_pa
                        || source.explicit_mode() == Some("PA")) =>
            {
                Image::Pipeline {
                    source: Arc::new(source.clone()),
                    ops: PipelineOps::one(op),
                    // Pillow derived images (resize, crop, getchannel,
                    // convert, rotate, ...) report format None; only the
                    // originally opened image carries the container format.
                    format: None,
                    explicit_mode,
                    backend: source.backend(),
                    palette: source_palette,
                    palette_alpha: source_palette_alpha,
                    materialized: materialization_cache(),
                    shape: pipeline_shape_cache(),
                    mode: pipeline_mode_cache(),
                }
            }
            Image::Pipeline {
                source: pipeline_source,
                ops,
                backend,
                materialized: pipeline_materialized,
                ..
            } => {
                let can_reuse_prefix = !source_is_paletted
                    && ops.len() > 0
                    && ops.mode_preserving()
                    && op_preserves_mode(&op);
                let ancestor_cache = if can_reuse_prefix {
                    if pipeline_materialized.get().is_some() {
                        Some(PipelinePrefixCache {
                            ops_len: ops.len(),
                            materialized: Arc::clone(pipeline_materialized),
                        })
                    } else {
                        // An unmaterialized prefix must stay part of the
                        // flattened batch. Marking its empty cache here
                        // makes evaluate_pipeline split every lazy chain
                        // into a prefix and suffix submission, defeating
                        // GPU batching and adding an avoidable frame copy.
                        ops.prefix_cache().cloned()
                    }
                } else {
                    None
                };
                let new_ops = ops.append(op).with_prefix_cache(ancestor_cache);
                Image::Pipeline {
                    source: Arc::clone(pipeline_source),
                    ops: new_ops,
                    format: None,
                    explicit_mode,
                    backend: *backend,
                    palette: source_palette,
                    palette_alpha: source_palette_alpha,
                    materialized: materialization_cache(),
                    shape: pipeline_shape_cache(),
                    mode: pipeline_mode_cache(),
                }
            }
            other => Image::Pipeline {
                source: Arc::new(other.clone()),
                ops: PipelineOps::one(op),
                format: None,
                explicit_mode,
                backend: other.backend(),
                palette: source_palette,
                palette_alpha: source_palette_alpha,
                materialized: materialization_cache(),
                shape: pipeline_shape_cache(),
                mode: pipeline_mode_cache(),
            },
        }
    }

    /// Returns a lazy one-operation pipeline with an explicit output mode.
    ///
    /// A separate node is required for mode-changing operations: flattening
    /// into an existing pipeline would pass the final mode tag to operations
    /// that execute before the mode transition.
    pub(crate) fn push_mode_changing_op(
        source: &Image,
        op: PipelineOp,
        output_mode: &str,
    ) -> Image {
        Image::Pipeline {
            source: Arc::new(source.clone()),
            ops: PipelineOps::one(op),
            format: None,
            explicit_mode: Some(output_mode.to_owned()),
            backend: source.backend(),
            palette: None,
            palette_alpha: None,
            materialized: materialization_cache(),
            shape: pipeline_shape_cache(),
            mode: pipeline_mode_cache(),
        }
    }
}
