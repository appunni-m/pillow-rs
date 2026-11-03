pub(crate) type Input = crate::raster::GrayImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;
use crate::compute::Architecture;
use crate::compute::registry::support::PaletteInput;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, RgbImage};
use crate::compute::pool_cpu::ops::imageops::helpers::strategy;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategyGuard,
};
use crate::raster::Mode;

// NO_PILLOW_SUPPORT: Pillow's ImageOps.invert rejects P. This Rust strategy
// inverts the native palette indices and preserves the attached palette.
declare_invert_native_mode!(
    ImagePalette8,
    Input,
    1,
    1,
    Some("ImageOps.invert extends Pillow with native P index inversion"),
);

pub(crate) fn execute_quantize(
    image: &GrayImage,
    palette: PaletteInput<'_>,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
    selection: &StrategySelection,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let architecture = super::super::helpers::strategy::selected_quantize_architecture(
        selection,
        Mode::ImagePalette8,
    )?;
    match architecture {
        Architecture::Default => super::default::quantize_impl::execute(
            image, palette, width, height, colors, kmeans, method,
        ),
        _ => Err(PilError::InternalError(
            "P Quantize selected an unimplemented architecture".into(),
        )),
    }
}

const POSTERIZE_INPUTS: &[InputContract] = &[
    InputContract::PosterizeBits,
    InputContract::CheckedByteDimensions {
        bytes_per_pixel: 1,
        allow_empty: true,
    },
];
const AUTOCONTRAST_INPUTS: &[InputContract] = &[
    InputContract::AutocontrastParameters,
    InputContract::CheckedByteDimensions {
        bytes_per_pixel: 1,
        allow_empty: true,
    },
    InputContract::ImageOpsLumaMask,
];
const SOLARIZE_INPUTS: &[InputContract] = &[InputContract::CheckedByteDimensions {
    bytes_per_pixel: 1,
    allow_empty: true,
}];

static AUTOCONTRAST_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::native_byte_point_architectures(OperationId::Autocontrast, Mode::ImagePalette8, 1);
static POSTERIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::native_byte_point_architectures(OperationId::Posterize, Mode::ImagePalette8, 1);
static SOLARIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::native_byte_point_architectures(OperationId::Solarize, Mode::ImagePalette8, 1);

// NO_PILLOW_SUPPORT: Pillow's ImageOps._lut rejects P. This Rust-only
// strategy stretches native palette indices and preserves the attached palette.
pub(crate) const AUTOCONTRAST_SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Autocontrast,
    Mode::ImagePalette8,
    OutputModeRule::Preserve,
    AUTOCONTRAST_INPUTS,
    &AUTOCONTRAST_ARCHITECTURES,
    "Autocontrast applies its byte lookup table directly to native palette indices",
);

// NO_PILLOW_SUPPORT: Pillow's ImageOps._lut rejects P. These native strategies
// transform palette indices and preserve the Rust image's attached palette.
pub(crate) const POSTERIZE_SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Posterize,
    Mode::ImagePalette8,
    OutputModeRule::Preserve,
    POSTERIZE_INPUTS,
    &POSTERIZE_ARCHITECTURES,
    "Posterize applies its byte lookup table directly to native palette indices",
);

// NO_PILLOW_SUPPORT: Pillow's ImageOps._lut rejects P; this native strategy
// solarizes index bytes and preserves the Rust image's attached palette.
pub(crate) const SOLARIZE_SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Solarize,
    Mode::ImagePalette8,
    OutputModeRule::Preserve,
    SOLARIZE_INPUTS,
    &SOLARIZE_ARCHITECTURES,
    "Solarize applies its byte lookup table directly to native palette indices",
);

const EQUALIZE_INPUTS: &[InputContract] = &[
    InputContract::PaletteMetadata,
    InputContract::CheckedByteDimensions { bytes_per_pixel: 1, allow_empty: true },
    InputContract::ImageOpsLumaMask,
];
static EQUALIZE_ARCHITECTURES: [ImplementationSupport; 3] = strategy::cpu_architectures_for(
    OperationId::Equalize,
    Mode::ImagePalette8,
    StrategyGuard::Always,
);
pub(crate) const EQUALIZE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Equalize,
    Mode::ImagePalette8,
    OutputModeRule::Fixed(Mode::ImageRgb8),
    EQUALIZE_INPUTS,
    &EQUALIZE_ARCHITECTURES,
);

/// Prepared P-mode Autocontrast strategy over native palette indices.
pub(crate) struct PreparedAutocontrast<'a> {
    input: &'a GrayImage,
    lut: Vec<u8>,
    selection: StrategySelection,
}

impl<'a> PreparedAutocontrast<'a> {
    pub(crate) fn new(input: &'a GrayImage, lut: Vec<u8>, selection: StrategySelection) -> Self {
        Self { input, lut, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.selection.implementation.architecture {
            Architecture::Default => {
                super::default::autocontrast_impl::execute(self.input, &self.lut)
            }
            Architecture::Sse2 => super::sse2::autocontrast_impl::execute(self.input, &self.lut),
            Architecture::Neon => super::neon::autocontrast_impl::execute(self.input, &self.lut),
            Architecture::WasmSimd128 => {
                super::wasm_simd128::autocontrast_impl::execute(self.input, &self.lut)
            }
            _ => Err(PilError::InternalError(
                "native-byte point selection has no matching CPU kernel".into(),
            )),
        }
    }
}

/// Prepared P-mode Equalize strategy over its palette-expanded RGB samples.
pub(crate) struct PreparedEqualize {
    input: RgbImage,
    lut: Vec<u8>,
    selection: StrategySelection,
}

impl PreparedEqualize {
    pub(crate) fn new(input: RgbImage, lut: Vec<u8>, selection: StrategySelection) -> Self {
        Self { input, lut, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.selection.implementation.architecture {
            Architecture::Default => {
                super::default::equalize_impl::execute_rgb(&self.input, &self.lut)
            }
            Architecture::Sse2 => super::sse2::equalize_impl::execute(&self.input, &self.lut),
            Architecture::Neon => super::neon::equalize_impl::execute(&self.input, &self.lut),
            Architecture::WasmSimd128 => {
                super::wasm_simd128::equalize_impl::execute(&self.input, &self.lut)
            }
            _ => Err(PilError::InternalError(
                "native-byte point selection has no matching CPU kernel".into(),
            )),
        }
    }
}

/// Prepared P-mode Posterize strategy with its selected architecture.
pub(crate) struct PreparedPosterize<'a> {
    input: &'a GrayImage,
    bits: u8,
    selection: StrategySelection,
}

impl<'a> PreparedPosterize<'a> {
    pub(crate) fn new(input: &'a GrayImage, bits: u8, selection: StrategySelection) -> Self {
        Self { input, bits, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.selection.implementation.architecture {
            Architecture::Default => super::default::posterize_impl::execute(self.input, self.bits),
            Architecture::Sse2 => super::sse2::posterize_impl::execute(self.input, self.bits),
            Architecture::Neon => super::neon::posterize_impl::execute(self.input, self.bits),
            Architecture::WasmSimd128 => {
                super::wasm_simd128::posterize_impl::execute(self.input, self.bits)
            }
            _ => Err(PilError::InternalError(
                "native-byte point selection has no matching CPU kernel".into(),
            )),
        }
    }
}

/// Prepared P-mode Solarize strategy with its selected architecture.
pub(crate) struct PreparedSolarize<'a> {
    input: &'a GrayImage,
    threshold: u8,
    selection: StrategySelection,
}

impl<'a> PreparedSolarize<'a> {
    pub(crate) fn new(input: &'a GrayImage, threshold: u8, selection: StrategySelection) -> Self {
        Self { input, threshold, selection }
    }

    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        match self.selection.implementation.architecture {
            Architecture::Default => {
                super::default::solarize_impl::execute(self.input, self.threshold)
            }
            Architecture::Sse2 => super::sse2::solarize_impl::execute(self.input, self.threshold),
            Architecture::Neon => super::neon::solarize_impl::execute(self.input, self.threshold),
            Architecture::WasmSimd128 => {
                super::wasm_simd128::solarize_impl::execute(self.input, self.threshold)
            }
            _ => Err(PilError::InternalError(
                "native-byte point selection has no matching CPU kernel".into(),
            )),
        }
    }
}
