//! Lazy pipeline state, metadata inference, and construction methods for [`Image`].

use std::sync::{Arc, OnceLock};

use crate::error::PilError;
use crate::pipeline::{ColorMode, PipelineOp, PixelMode};
use crate::raster::{DynamicImage, Mode};

use super::Image;

mod dimensions;
pub(super) use dimensions::known_pipeline_op_dimensions;

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
    origin: Option<crate::PipelineTelemetryOrigin>,
    len: usize,
    mode_preserving: bool,
    flattened: OnceLock<Arc<[PipelineOp]>>,
    flattened_origins: OnceLock<Arc<[Option<crate::PipelineTelemetryOrigin>]>>,
}

impl PipelineOps {
    pub(crate) fn empty() -> Self {
        Self {
            chain: Arc::new(PipelineOpChain {
                parent: None,
                op: None,
                origin: None,
                len: 0,
                mode_preserving: true,
                flattened: OnceLock::new(),
                flattened_origins: OnceLock::new(),
            }),
            prefix_cache: None,
        }
    }

    pub(crate) fn one(op: PipelineOp) -> Self {
        let origin = Self::capture_origin();
        Self::one_with_origin(op, origin)
    }

    fn one_with_origin(op: PipelineOp, origin: Option<crate::PipelineTelemetryOrigin>) -> Self {
        let mode_preserving = op_preserves_mode(&op);
        Self {
            chain: Arc::new(PipelineOpChain {
                parent: None,
                op: Some(op),
                origin,
                len: 1,
                mode_preserving,
                flattened: OnceLock::new(),
                flattened_origins: OnceLock::new(),
            }),
            prefix_cache: None,
        }
    }

    pub(crate) fn append(&self, op: PipelineOp) -> Self {
        let origin = Self::capture_origin();
        self.append_with_origin(op, origin)
    }

    fn append_with_origin(
        &self,
        op: PipelineOp,
        origin: Option<crate::PipelineTelemetryOrigin>,
    ) -> Self {
        Self {
            chain: Arc::new(PipelineOpChain {
                parent: Some(Arc::clone(&self.chain)),
                origin,
                mode_preserving: self.chain.mode_preserving && op_preserves_mode(&op),
                op: Some(op),
                len: self.chain.len + 1,
                flattened: OnceLock::new(),
                flattened_origins: OnceLock::new(),
            }),
            prefix_cache: self.prefix_cache.clone(),
        }
    }

    pub(crate) fn from_vec_with_origins(
        ops: Vec<PipelineOp>,
        origins: Vec<Option<crate::PipelineTelemetryOrigin>>,
    ) -> Self {
        ops.into_iter()
            .zip(origins.into_iter().chain(std::iter::repeat(None)))
            .fold(Self::empty(), |pipeline, (op, origin)| {
                pipeline.append_with_origin(op, origin)
            })
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

    pub(crate) fn operation_origins(&self) -> &[Option<crate::PipelineTelemetryOrigin>] {
        self.chain
            .flattened_origins
            .get_or_init(|| {
                let mut reversed = Vec::with_capacity(self.chain.len);
                let mut cursor = Some(Arc::clone(&self.chain));
                while let Some(node) = cursor {
                    if node.op.is_some() {
                        reversed.push(node.origin.clone());
                    }
                    cursor = node.parent.as_ref().map(Arc::clone);
                }
                reversed.reverse();
                Arc::<[Option<crate::PipelineTelemetryOrigin>]>::from(reversed)
            })
            .as_ref()
    }

    pub(crate) fn has_operation_origins(&self) -> bool {
        let mut cursor = Some(Arc::clone(&self.chain));
        while let Some(node) = cursor {
            if node.origin.is_some() {
                return true;
            }
            cursor = node.parent.as_ref().map(Arc::clone);
        }
        false
    }

    pub(crate) fn telemetry_origin_scope(
        &self,
    ) -> Option<crate::compute::RestorePipelineOperationOrigins> {
        if !crate::compute::Backend::pipeline_telemetry_enabled() {
            return None;
        }
        Some(crate::compute::enter_operation_origins(
            self.as_slice()
                .iter()
                .zip(self.operation_origins())
                .map(|(op, origin)| (crate::compute::registry::variant_key(op), origin.clone())),
        ))
    }

    fn capture_origin() -> Option<crate::PipelineTelemetryOrigin> {
        crate::compute::Backend::pipeline_telemetry_enabled()
            .then(crate::compute::capture_active_origin)
            .flatten()
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

fn known_putalpha_mode(mode: PixelMode) -> Option<Mode> {
    match mode {
        PixelMode::YCbCr | PixelMode::HSV | PixelMode::RGB => Some(Mode::ImageRgba8),
        PixelMode::L | PixelMode::LA => Some(Mode::ImageLumaA8),
        PixelMode::RGBA => Some(Mode::ImageRgba8),
        PixelMode::P | PixelMode::PA => Some(Mode::ImagePaletteAlpha8),
        PixelMode::CMYK => Some(Mode::ImageRgba8),
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
pub(crate) fn known_pipeline_op_mode(op: &PipelineOp, current: Mode) -> Option<Mode> {
    if op_preserves_mode(op) {
        return Some(current);
    }
    match op {
        PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }
            if matches!(current, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) =>
        {
            Some(Mode::ImageRgb8)
        }
        PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. } => Some(current),
        // The public wrapper records explicit target modes for conversions
        // whose identity is not recoverable from a generic raster carrier.
        PipelineOp::Convert { mode, .. } | PipelineOp::Merge { mode, .. } => match mode {
            ColorMode::L => Some(Mode::ImageLuma8),
            ColorMode::LA => Some(Mode::ImageLumaA8),
            ColorMode::RGB => Some(Mode::ImageRgb8),
            ColorMode::RGBA => Some(Mode::ImageRgba8),
            _ => None,
        },
        PipelineOp::ConvertLab { .. } => Some(Mode::ImageLab8),
        PipelineOp::Grayscale => Some(Mode::ImageLuma8),
        PipelineOp::Colorize { .. } => Some(Mode::ImageRgb8),
        PipelineOp::Constant { .. } => Some(Mode::ImageLuma8),
        PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => {
            known_putalpha_mode(*mode)
        }
        PipelineOp::ExtractBand { .. } | PipelineOp::EffectNoise { .. } => {
            Some(Mode::ImageLuma8)
        }
        _ => None,
    }
}

/// Rounds a non-negative dimension using Python's ties-to-even rule.
///
/// Metadata propagation must never manufacture a dimension for an invalid or
/// overflowing operation.  Returning `None` lets the normal materialization
/// path preserve the operation's existing error behavior.
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

/// Shared once-initialized Pillow-owned output identity for a lazy pipeline node.
///
/// `None` means that the operation list contains a mode transition whose
/// representation cannot be described without materializing pixels.
pub type PipelineModeCache = Arc<OnceLock<Result<Option<Mode>, PilError>>>;

pub(crate) fn materialization_cache() -> MaterializationCache {
    Arc::new(OnceLock::new())
}

pub(crate) fn pipeline_shape_cache() -> PipelineShapeCache {
    Arc::new(OnceLock::new())
}

pub(crate) fn pipeline_mode_cache() -> PipelineModeCache {
    Arc::new(OnceLock::new())
}

struct PipelineOutputMetadata {
    source_is_paletted: bool,
    palette_safe: bool,
    promotes_p_to_pa: bool,
    explicit_mode: Option<Mode>,
    palette: Option<Vec<u8>>,
    palette_alpha: Option<Vec<u8>>,
}

fn pipeline_output_metadata(source: &Image, op: &PipelineOp) -> PipelineOutputMetadata {
    let source_is_paletted = source.has_palette_samples();
    let palette_safe = source_is_paletted && Image::is_palette_safe_op_for_source(source, op);
    let indexed_chops_result_without_palette =
        source_is_paletted && Image::indexed_chops_result_without_palette(op);
    let putalpha_mode = match op {
        PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => Some(*mode),
        _ => None,
    };
    let promotes_p_to_pa = source.has_palette_mode()
        && matches!(
            op,
            PipelineOp::PutAlpha {
                mode: PixelMode::P,
                ..
            } | PipelineOp::PutAlphaData {
                mode: PixelMode::P,
                ..
            }
        );
    let explicit_mode =
        pipeline_output_mode(source, op, source_is_paletted, palette_safe, putalpha_mode);
    let (palette, palette_alpha) = pipeline_output_palette(
        source,
        op,
        palette_safe,
        indexed_chops_result_without_palette,
    );
    PipelineOutputMetadata {
        source_is_paletted,
        palette_safe,
        promotes_p_to_pa,
        explicit_mode,
        palette,
        palette_alpha,
    }
}

fn new_pipeline_node(
    source: Arc<Image>,
    ops: PipelineOps,
    backend: Option<crate::compute::Backend>,
    explicit_mode: Option<Mode>,
    palette: Option<Vec<u8>>,
    palette_alpha: Option<Vec<u8>>,
) -> Image {
    Image::Pipeline {
        source,
        ops,
        format: None,
        explicit_mode,
        backend,
        palette,
        palette_alpha,
        materialized: materialization_cache(),
        shape: pipeline_shape_cache(),
        mode: pipeline_mode_cache(),
    }
}

fn append_pipeline_op(
    source: &Arc<Image>,
    ops: &PipelineOps,
    backend: Option<crate::compute::Backend>,
    materialized: &MaterializationCache,
    op: PipelineOp,
    source_is_paletted: bool,
    explicit_mode: Option<Mode>,
    palette: Option<Vec<u8>>,
    palette_alpha: Option<Vec<u8>>,
) -> Image {
    let can_reuse_prefix =
        !source_is_paletted && !ops.is_empty() && ops.mode_preserving() && op_preserves_mode(&op);
    let ancestor_cache = if can_reuse_prefix {
        if materialized.get().is_some() {
            Some(PipelinePrefixCache {
                ops_len: ops.len(),
                materialized: Arc::clone(materialized),
            })
        } else {
            // An unmaterialized prefix must stay part of the flattened batch.
            // Marking its empty cache here makes evaluate_pipeline split every
            // lazy chain into a prefix and suffix submission, defeating GPU
            // batching and adding an avoidable frame copy.
            ops.prefix_cache().cloned()
        }
    } else {
        None
    };
    let new_ops = ops.append(op).with_prefix_cache(ancestor_cache);
    new_pipeline_node(
        Arc::clone(source),
        new_ops,
        backend,
        explicit_mode,
        palette,
        palette_alpha,
    )
}

fn pipeline_output_mode(
    source: &Image,
    op: &PipelineOp,
    source_is_paletted: bool,
    palette_safe: bool,
    putalpha_mode: Option<PixelMode>,
) -> Option<Mode> {
    let source_mode = source.known_mode_identity();
    let explicit_mode = if matches!(
        op,
        PipelineOp::ExtractBand { .. } | PipelineOp::Merge { .. }
    ) {
        None
    } else if source_is_paletted {
        if source_mode == Some(Mode::ImagePaletteAlpha8)
            && matches!(op, PipelineOp::PutPixel { .. })
        {
            // PA writes keep the raw index/alpha sample layout even when no
            // RGB palette is attached, so later conversion still sees PA.
            Some(Mode::ImagePaletteAlpha8)
        } else {
            palette_safe.then(|| match source_mode {
                Some(Mode::ImagePaletteAlpha8) => Mode::ImagePaletteAlpha8,
                _ => Mode::ImagePalette8,
            })
        }
    } else {
        match op {
            // Pillow's constant constructor always allocates L, even when the
            // source has an explicit CMYK, YCbCr, HSV, I, F, or 1 mode tag.
            PipelineOp::Grayscale
            | PipelineOp::Constant { .. }
            | PipelineOp::Convert { .. }
            | PipelineOp::ConvertLab { .. }
            | PipelineOp::ExtractBand { .. } => None,
            _ => source_mode,
        }
    };

    // Keep putalpha's output-mode promotion here so P becomes PA without
    // expanding its indices, while CMYK becomes ordinary RGBA without a tag.
    match putalpha_mode {
        Some(PixelMode::P | PixelMode::PA) => Some(Mode::ImagePaletteAlpha8),
        Some(PixelMode::CMYK) => None,
        _ => explicit_mode,
    }
}

fn pipeline_output_palette(
    source: &Image,
    op: &PipelineOp,
    palette_safe: bool,
    indexed_chops_result_without_palette: bool,
) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    let palette_dropped_by_pa_operation =
        source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
        && match op {
            PipelineOp::Resize { filter, .. }
            | PipelineOp::ResizeBoxed { filter, .. }
            | PipelineOp::Thumbnail { filter, .. }
            | PipelineOp::Fit { filter, .. }
            | PipelineOp::Contain { filter, .. }
            | PipelineOp::Cover { filter, .. } => {
                !matches!(filter, crate::pipeline::ResampleFilter::Nearest)
            }
            PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. } => true,
            _ => false,
        };
    let preserve_palette = palette_safe
        && !matches!(op, PipelineOp::ExtractBand { .. })
        && !palette_dropped_by_pa_operation;
    let palette = if indexed_chops_result_without_palette {
        Some(Vec::new())
    } else if preserve_palette {
        source.extract_palette()
    } else {
        None
    };
    let palette_alpha = if indexed_chops_result_without_palette {
        None
    } else if preserve_palette {
        source.palette_alpha()
    } else {
        None
    };
    (palette, palette_alpha)
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
        let PipelineOutputMetadata {
            source_is_paletted,
            palette_safe,
            promotes_p_to_pa,
            explicit_mode,
            palette,
            palette_alpha,
        } = pipeline_output_metadata(source, &op);
        match source {
            // Keep a concrete sample-layout boundary around palette expansion
            // and P↔PA transitions. A flattened batch has only one external
            // mode tag, so it cannot correctly run P operations before
            // putalpha or PA operations after it (especially on the GPU).
            Image::Pipeline { .. }
                if source_is_paletted
                    && (!palette_safe
                        || promotes_p_to_pa
                        || source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)) =>
            {
                new_pipeline_node(
                    Arc::new(source.clone()),
                    PipelineOps::one(op),
                    source.backend(),
                    explicit_mode,
                    palette,
                    palette_alpha,
                )
            }
            Image::Pipeline {
                source: pipeline_source,
                ops,
                backend,
                materialized: pipeline_materialized,
                ..
            } => append_pipeline_op(
                pipeline_source,
                ops,
                *backend,
                pipeline_materialized,
                op,
                source_is_paletted,
                explicit_mode,
                palette,
                palette_alpha,
            ),
            other => new_pipeline_node(
                Arc::new(other.clone()),
                PipelineOps::one(op),
                other.backend(),
                explicit_mode,
                palette,
                palette_alpha,
            ),
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
        explicit_mode: Mode,
    ) -> Image {
        Image::Pipeline {
            source: Arc::new(source.clone()),
            ops: PipelineOps::one(op),
            format: None,
            explicit_mode: Some(explicit_mode),
            backend: source.backend(),
            palette: None,
            palette_alpha: None,
            materialized: materialization_cache(),
            shape: pipeline_shape_cache(),
            mode: pipeline_mode_cache(),
        }
    }
}
