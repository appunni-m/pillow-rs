use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_quantize_native_mode;

// NO_PILLOW_SUPPORT: Pillow quantizes only L, P, RGB, and RGBA; this adds
// native I;16 sample reduction as a Rust-only strategy.
declare_quantize_native_mode!(u16, "ImageI16", ImageI16);

static CROP_BORDER_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 3] =
    crate::compute::pool_cpu::ops::crop::helpers::strategy::luma16_copy_architectures(
        crate::compute::registry::support::OperationId::CropBorder,
        crate::raster::Mode::ImageI16,
        crate::compute::registry::support::StrategyGuard::NativeCropBorderBytes { channels: 2 },
    );
pub(crate) const CROP_BORDER_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::registry::support::ModeSupport::cpu(
        crate::compute::registry::support::OperationId::CropBorder,
        crate::raster::Mode::ImageI16,
        crate::compute::registry::support::OutputModeRule::Preserve,
        super::super::ops::CROP_BORDER_INPUTS,
        &CROP_BORDER_ARCHITECTURES,
    );
