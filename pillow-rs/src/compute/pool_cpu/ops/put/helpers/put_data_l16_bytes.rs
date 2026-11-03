//! Pillow's row-shaped raw-byte input for the four exact I;16 identities.

use crate::compute::helpers::put_data::clip_u8;

/// Applies Pillow's row-shaped I;16 bytes prefix directly to native u16 samples.
///
/// Each input row updates only its first `width` raw storage bytes. Unwritten
/// bytes keep their existing value; this matters for partial `putdata` calls.
/// If a bytes subclass reports a length beyond its physical storage, missing
/// entries are zero-filled to keep the write within the allocated Rust buffer.
/// Pillow 12.2 reads those entries past the end of `PyBytes_AS_STRING`, so that
/// malformed-input behavior is undefined and cannot be claimed as parity.
// NO_PILLOW_SUPPORT: bytes-subclass length overreads are undefined in Pillow;
// this path defines absent bytes as zero for memory safety.
// NO_PILLOW_SUPPORT: Pillow converts scaled bytes to C int before clipping;
// Rust defines the non-finite and out-of-range cases through clip_u8.
// TODO(pillow-rs#3ptb): freeze those deterministic Rust-only edge values in
// the CPU parity extension lane; they have no defined Pillow oracle result.
pub(crate) fn write_byte_prefix<const BIG_ENDIAN: bool>(
    samples: &mut [u16],
    width: u32,
    data: &[u8],
    entry_count: usize,
    scale: f64,
    offset: f64,
) {
    let width = width as usize;
    if width == 0 {
        return;
    }

    let mut entry_offset = 0;
    while entry_offset < entry_count {
        let row_entries = (entry_count - entry_offset).min(width);
        let available_start = entry_offset.min(data.len());
        let available_end = (entry_offset + row_entries).min(data.len());
        let row_data = &data[available_start..available_end];
        for (column, &byte) in row_data.iter().enumerate() {
            write_sample_lane::<BIG_ENDIAN>(
                samples,
                entry_offset + column / 2,
                column,
                byte,
                scale,
                offset,
            );
        }
        for column in row_data.len()..row_entries {
            write_sample_lane::<BIG_ENDIAN>(
                samples,
                entry_offset + column / 2,
                column,
                0,
                scale,
                offset,
            );
        }
        entry_offset += row_entries;
    }
}

#[inline(always)]
fn write_sample_lane<const BIG_ENDIAN: bool>(
    samples: &mut [u16],
    sample_index: usize,
    column: usize,
    byte: u8,
    scale: f64,
    offset: f64,
) {
    let byte = if scale == 1.0 && offset == 0.0 {
        byte
    } else {
        clip_u8(f64::from(byte) * scale + offset)
    };
    let lane = column & 1;
    let shift = if BIG_ENDIAN { (1 - lane) * 8 } else { lane * 8 };
    let mask = !(u16::from(u8::MAX) << shift);
    samples[sample_index] = (samples[sample_index] & mask) | (u16::from(byte) << shift);
}
