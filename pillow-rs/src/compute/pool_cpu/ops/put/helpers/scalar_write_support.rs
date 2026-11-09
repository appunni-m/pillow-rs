//! Synchronous scalar-write support contracts for exact-mode owners.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, RejectionKind,
};
use crate::raster::Mode;

const VALUE_AT_INPUTS: &[InputContract] = &[InputContract::CallerValidatedPixelIndex];

pub(crate) const fn put_data(mode: Mode, supported: bool) -> ModeSupport {
    if supported {
        ModeSupport::cpu(
            OperationId::PutDataValueAt,
            mode,
            OutputModeRule::Preserve,
            VALUE_AT_INPUTS,
            &[],
        )
    } else {
        ModeSupport::Rejected {
            mode,
            reason: "unsupported putdata mode: unknown",
            kind: RejectionKind::ValueError,
        }
    }
}

pub(crate) const fn put_pixel(mode: Mode, supported: bool) -> ModeSupport {
    if supported {
        ModeSupport::serial(OperationId::PutPixelValueAt, mode, VALUE_AT_INPUTS)
    } else {
        ModeSupport::Rejected {
            mode,
            reason: "scalar putpixel value-at requires I, F, or I;16 mode",
            kind: RejectionKind::NotImplementedError,
        }
    }
}

const I16_BYTES_INPUTS: &[InputContract] = &[InputContract::CallerValidatedRawI16ByteRows];

pub(crate) const fn put_data_l16_bytes(mode: Mode, supported: bool) -> ModeSupport {
    if supported {
        ModeSupport::serial(OperationId::PutDataL16Bytes, mode, I16_BYTES_INPUTS)
    } else {
        ModeSupport::Rejected {
            mode,
            reason: "putdata I;16 storage mismatch",
            kind: RejectionKind::InternalError,
        }
    }
}

macro_rules! declare_scalar_write_support {
    ($mode:ident, $put_data:literal, $put_pixel:literal, $i16_bytes:literal $(,)?) => {
        pub(crate) const PUT_DATA_VALUE_AT_SUPPORT:
            $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::put::helpers::scalar_write_support::put_data(
                $crate::raster::Mode::$mode,
                $put_data,
            );
        pub(crate) const PUT_PIXEL_VALUE_AT_SUPPORT:
            $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::put::helpers::scalar_write_support::put_pixel(
                $crate::raster::Mode::$mode,
                $put_pixel,
            );
        pub(crate) const PUT_DATA_L16_BYTES_SUPPORT:
            $crate::compute::registry::support::ModeSupport =
            $crate::compute::pool_cpu::ops::put::helpers::scalar_write_support::put_data_l16_bytes(
                $crate::raster::Mode::$mode,
                $i16_bytes,
            );
    };
}

pub(crate) use declare_scalar_write_support;
