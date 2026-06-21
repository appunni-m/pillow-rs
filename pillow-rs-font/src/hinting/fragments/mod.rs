//! Opcode dispatch fragments — each range is handled by a dedicated file.
//! Impl blocks are auto-available as ExecContext methods.

mod range_00_0f;
mod range_10_1f;
mod range_20_3f;
mod range_40_5f;
mod range_60_7f;
mod range_80_bf;
