//! Split support and wrapper-route declarations for exact-mode owners.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationModeRoute, OperationRoutePath, OutputModeRule,
};
use crate::raster::Mode;

const NO_INPUTS: &[InputContract] = &[];
const PALETTE_COPY: &[OperationRoutePath] = &[OperationRoutePath::PalettePreservingCopy];
const IDENTITY_COPY: &[OperationRoutePath] = &[OperationRoutePath::IdentityCopy];
const EXTRACT_BANDS: &[OperationRoutePath] = &[OperationRoutePath::PipelineChildrenPerBand(
    OperationId::ExtractBand,
)];

#[derive(Clone, Copy)]
pub(crate) enum SplitPath {
    Palette,
    Identity,
    Bands,
}

pub(crate) fn mode_route(mode: Mode, core_only: bool, path: SplitPath) -> OperationModeRoute {
    let support = if core_only {
        ModeSupport::cpu_core_only(
            OperationId::Split,
            mode,
            OutputModeRule::NoRasterResult,
            NO_INPUTS,
            &[],
            "native high-depth Split extends the Pillow mode contract",
        )
    } else {
        ModeSupport::cpu(
            OperationId::Split,
            mode,
            OutputModeRule::NoRasterResult,
            NO_INPUTS,
            &[],
        )
    };
    let paths = match path {
        SplitPath::Palette => PALETTE_COPY,
        SplitPath::Identity => IDENTITY_COPY,
        SplitPath::Bands => EXTRACT_BANDS,
    };
    OperationModeRoute::WrapperOwned {
        mode,
        support,
        paths,
    }
}

macro_rules! declare_split_support {
    ($mode:ident, pillow, $path:ident $(,)?) => {
        pub(crate) fn split_mode_route() -> $crate::compute::registry::support::OperationModeRoute {
            $crate::compute::pool_cpu::ops::analysis::helpers::split_support::mode_route(
                $crate::raster::Mode::$mode,
                false,
                $crate::compute::pool_cpu::ops::analysis::helpers::split_support::SplitPath::$path,
            )
        }
    };
    ($mode:ident, core_only, $path:ident $(,)?) => {
        pub(crate) fn split_mode_route() -> $crate::compute::registry::support::OperationModeRoute {
            $crate::compute::pool_cpu::ops::analysis::helpers::split_support::mode_route(
                $crate::raster::Mode::$mode,
                true,
                $crate::compute::pool_cpu::ops::analysis::helpers::split_support::SplitPath::$path,
            )
        }
    };
}

pub(crate) use declare_split_support;
