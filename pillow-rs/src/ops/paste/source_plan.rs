use super::input::{
    invalid_component_error, paste_float_source_error, paste_invalid_type_error,
    paste_list_source_error, paste_scalar_source, paste_source_from_color_string,
    paste_tuple_source, wide_integer_scalar_source,
};
use super::source::resolve_image_source;
use super::types::{PasteSource, PasteSourceInput, ResolvedPasteColor};
use crate::compute::CpuExecutionRoute;
use crate::error::PilError;
use crate::image::Image;
use crate::raster::Mode;

/// Source captured before destination and mask preparation complete.
/// Python conversion overrides may intentionally supply a different mode.
#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct PasteSourcePlan {
    pub(super) destination_mode: Mode,
    pub(super) source: PasteSource,
    pub(super) resolved_color: Option<ResolvedPasteColor>,
    pub(super) execution_route: Option<CpuExecutionRoute>,
}

impl Image {
    /// Applies base-image Paste conversion rules and resolves solid inputs.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_paste_source(
        &self,
        source_input: PasteSourceInput,
    ) -> Result<PasteSourcePlan, PilError> {
        let destination_mode = self.mode_identity()?;
        Self::prepare_paste_source_for_mode_with_execution_route(
            destination_mode,
            source_input,
            self.cpu_execution_route_advisory(),
        )
    }

    /// Prepares a non-image Paste source after the host captured destination mode.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_paste_source_for_mode(
        destination_mode: Mode,
        source_input: PasteSourceInput,
    ) -> Result<PasteSourcePlan, PilError> {
        Self::prepare_paste_source_for_mode_inner(destination_mode, source_input, None)
    }

    /// Prepares a source under the already selected destination route.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_paste_source_for_mode_with_execution_route(
        destination_mode: Mode,
        source_input: PasteSourceInput,
        route: CpuExecutionRoute,
    ) -> Result<PasteSourcePlan, PilError> {
        Self::prepare_paste_source_for_mode_inner(destination_mode, source_input, Some(route))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn prepare_paste_source_for_mode_inner(
        destination_mode: Mode,
        source_input: PasteSourceInput,
        execution_route: Option<CpuExecutionRoute>,
    ) -> Result<PasteSourcePlan, PilError> {
        if matches!(&source_input, PasteSourceInput::Image(_)) && execution_route.is_none() {
            return Err(PilError::InternalError(
                "image Paste preparation requires the destination execution route".into(),
            ));
        }
        let source = match source_input {
            PasteSourceInput::Image(image) => PasteSource::from_parts(Some(*image), 0, 0, 0, 255),
            PasteSourceInput::Scalar(value) => paste_scalar_source(value, destination_mode),
            PasteSourceInput::WideInteger { float_value, .. } => {
                wide_integer_scalar_source(destination_mode, float_value)?
            }
            PasteSourceInput::Float(value) => {
                if destination_mode == Mode::ImageF32 {
                    PasteSource::RawFloat(value)
                } else {
                    return Err(paste_float_source_error(destination_mode));
                }
            }
            PasteSourceInput::TupleComponents(values) => {
                paste_tuple_source(&values, destination_mode)?
            }
            PasteSourceInput::List => return Err(paste_list_source_error(destination_mode)),
            PasteSourceInput::InvalidType(type_name) => {
                return Err(paste_invalid_type_error(destination_mode, &type_name));
            }
            PasteSourceInput::Components(values) => match values.as_slice() {
                [value] => paste_scalar_source(*value, destination_mode),
                [luma, alpha] => match (u8::try_from(*luma), u8::try_from(*alpha)) {
                    (Ok(luma), Ok(alpha)) => PasteSource::LumaAlpha(luma, alpha),
                    _ => PasteSource::RawLumaAlpha(*luma, *alpha),
                },
                [r, g, b] => match (u8::try_from(*r), u8::try_from(*g), u8::try_from(*b)) {
                    (Ok(r), Ok(g), Ok(b)) => PasteSource::Rgb(r, g, b),
                    _ => PasteSource::RawRgb(*r, *g, *b),
                },
                [r, g, b, a] => match (
                    u8::try_from(*r),
                    u8::try_from(*g),
                    u8::try_from(*b),
                    u8::try_from(*a),
                ) {
                    (Ok(r), Ok(g), Ok(b), Ok(a)) => PasteSource::from_parts(None, r, g, b, a),
                    _ => PasteSource::RawRgba(*r, *g, *b, *a),
                },
                _ => return Err(invalid_component_error(destination_mode)),
            },
            PasteSourceInput::String(value) => {
                paste_source_from_color_string(&value, destination_mode)?
            }
            PasteSourceInput::Invalid => return Err(invalid_component_error(destination_mode)),
        };
        let (source, resolved_color) = match source {
            PasteSource::Image(image) => (
                PasteSource::Image(Box::new(resolve_image_source(
                    image,
                    destination_mode,
                    execution_route.ok_or_else(|| {
                        PilError::InternalError(
                            "image Paste preparation lost the destination execution route".into(),
                        )
                    })?,
                )?)),
                None,
            ),
            solid => {
                let color = solid.solid_color(destination_mode)?;
                (solid, Some(color))
            }
        };
        Ok(PasteSourcePlan {
            destination_mode,
            source,
            resolved_color,
            execution_route,
        })
    }

    /// Captures an image source whose Pillow mode conversion has already run.
    ///
    /// The Python adapter uses this after virtual `Image.convert` or when it
    /// keeps the original core. Direct ImagingCore Paste also uses it to keep
    /// the supplied source mode without high-level conversion. Both routes
    /// capture the exact source before typed pair admission.
    /// The returned source mode is retained as-is, including a mode-changing
    /// override result. This reproduces Pillow 12.3.0's observed, undocumented
    /// compatibility behavior: Paste admits matching physical pixel sizes;
    /// unmasked paths copy raw bytes and masked paths blend physical lanes.
    /// The parity input builder models an instance override that returns this
    /// same image and emits the RGBA→CMYK, RGB→RGBA, L→P, RGBA→L, and masked
    /// LA→RGBA cases. The active shared oracle is Pillow 12.2.0; the user has
    /// independently reproduced the same behavior on Pillow 12.3.0. Runtime
    /// parity remains gated on CPU source completion.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_resolved_paste_image_source(
        &self,
        image: Image,
    ) -> Result<PasteSourcePlan, PilError> {
        Ok(PasteSourcePlan {
            destination_mode: self.mode_identity()?,
            source: PasteSource::Image(Box::new(image)),
            resolved_color: None,
            execution_route: None,
        })
    }
}
