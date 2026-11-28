use crate::compute::CpuExecutionRoute;
use crate::error::PilError;
use crate::image::Image;
use crate::raster::Mode;

/// Resolves the captured source mode under the destination-selected Convert route.
// TODO(pillow-rs#3ptb): parity-check each eager source Convert under a distinct
// destination route and verify the queued Paste selection/fallback receipt.
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
pub(super) fn resolve_image_source(
    image: Box<Image>,
    destination_mode: Mode,
    route: CpuExecutionRoute,
) -> Result<Image, PilError> {
    let source_mode = image.mode_identity()?;
    // Pillow skips conversion for LA, RGBA, and RGBa pasted into RGB because
    // those modes already use the destination's four-byte physical pixel. Its
    // low-level Paste copies or blends these lanes and leaves RGBa premultiplied.
    if source_mode == destination_mode
        || (destination_mode == Mode::ImageRgb8
            && matches!(
                source_mode,
                Mode::ImageLumaA8 | Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8
            ))
    {
        Ok(*image)
    } else if destination_mode == Mode::ImagePaletteAlpha8 && source_mode == Mode::ImagePalette8 {
        // Pillow promotes P sources to opaque PA samples and retains each index.
        let mut promoted = *image;
        promoted.putalpha(255)?;
        Ok(promoted)
    } else {
        // Pillow converts every other mismatched base-image source here,
        // including RGB to RGBA regardless of whether a mask is present.
        image.convert_with_execution_route(destination_mode, None, None, route)
    }
}
