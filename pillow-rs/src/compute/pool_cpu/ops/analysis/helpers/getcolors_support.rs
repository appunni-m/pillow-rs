//! GetColors support constructors used by exact-mode owners.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, RejectionKind,
};
use crate::raster::Mode;

const NO_INPUTS: &[InputContract] = &[];

pub(crate) const fn pillow_mode(mode: Mode) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::GetColors,
        mode,
        OutputModeRule::NoRasterResult,
        NO_INPUTS,
        &[],
    )
}

pub(crate) const fn core_only_mode(mode: Mode) -> ModeSupport {
    ModeSupport::cpu_core_only(
        OperationId::GetColors,
        mode,
        OutputModeRule::NoRasterResult,
        NO_INPUTS,
        &[],
        "native high-depth GetColors extends Pillow's mode contract",
    )
}

pub(crate) const fn rejected_mode(mode: Mode) -> ModeSupport {
    ModeSupport::Rejected {
        mode,
        reason: "image has wrong mode",
        kind: RejectionKind::ValueError,
    }
}

macro_rules! declare_getcolors_support {
    ($mode:ident, pillow $(,)?) => {
        pub(crate) const GET_COLORS_SUPPORT: $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::pillow_mode(
                $crate::raster::Mode::$mode,
            );
    };
    ($mode:ident, core_only $(,)?) => {
        pub(crate) const GET_COLORS_SUPPORT: $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::core_only_mode(
                $crate::raster::Mode::$mode,
            );
    };
    ($mode:ident, rejected $(,)?) => {
        pub(crate) const GET_COLORS_SUPPORT: $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::rejected_mode(
                $crate::raster::Mode::$mode,
            );
    };
}

pub(crate) use declare_getcolors_support;
