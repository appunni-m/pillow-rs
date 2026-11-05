//! Exact-mode operation families for serial CPU execution.
// Architecture-folder declaration audit (2026-11-04): the source scan found
// every populated architecture folder declared by its mode module or ops file.
// Direct-instruction paths were traced across all families; Transpose's
// per-mode declarations and Analysis's RGB8/CMYK8 GetProjection declarations
// were checked on 2026-11-04. RemapPalette L/P have mode-owned `default`
// entry points that delegate to the shared family byte kernel; their SIMD
// paths remain in matching mode/architecture folders. The owners without a
// populated Default folder are five rejected high-depth Draw modes and eight
// Thumbnail modes whose prepared path composes same-mode Reduce/Resize.
// TODO(pillow-rs#3ptb): finish the family-by-family audit that every execution
// boundary consumes the prepared selection derived from ExecutionContext.
// Resize, Chops, Pad, Crop, Transpose, CompositeModule, Merge, BlendModule,
// AlphaComposite, and supported Draw routes are source-reviewed for
// prepared-selection flow. Composite mode owners consume the selection from
// the family-level assessment; the RGBA AlphaComposite+Mirror fusion retains
// both child selections. Thumbnail's I32 stream consumes its selection;
// ImageOps Quantize threads selection through all 25 modes. Analysis's
// Projection, Histogram/Entropy/Stat, GetData variants,
// GetBBox, GetExtrema variants, GetColors, and GetPixel routes consume
// prepared selections; Stat and Entropy prepare their Histogram child with
// the same context. GetPixelRgba validates its per-mode CoreOnly selection
// before the fixed-RGBA projection. GetChannel and Split declare wrapper-owned
// copy or ExtractBand child routes; the loaded byte-view identity route has no
// kernel selection. Draw's Text ink/context-mode and Bitmap
// load/fill/coordinate/selection ordering now have source review; output and
// fault receipts remain parity work. The direct/eager caller trace now covers
// all 28 non-PipelineOp identities; see docs/CPU_REFACTOR_PLAN.md. This is
// source wiring only, not output, fault-order, callback-order, or receipt proof.
// Transform's public direct-destination path also passes the exact-mode
// prepared selection to execution and reports the selected Default/vector
// path; MESH is guarded to Default. Output, callback, and receipt parity remain
// pending.
// TODO(pillow-rs#3ptb): finish the remaining family-by-family source audit of
// execution boundaries and verify every declared mode/architecture strategy
// reaches its exact implementation without bypassing prepared selection.

pub(crate) mod analysis;
pub(crate) mod chops;
pub(crate) mod color;
pub(crate) mod color3dlut;
pub(crate) mod composite;
pub(crate) mod crop;
pub(crate) mod draw;
pub(crate) mod effects;
pub(crate) mod enhance;
pub(crate) mod eval;
pub(crate) mod filter;
pub(crate) mod geometry;
pub(crate) mod grayscale;
pub(crate) mod imageops;
pub(crate) mod paste;
pub(crate) mod put;
pub(crate) mod reduce;
pub(crate) mod resize;
pub(crate) mod rotate;
pub(crate) mod thumbnail;
pub(crate) mod transform;
pub(crate) mod transpose;
