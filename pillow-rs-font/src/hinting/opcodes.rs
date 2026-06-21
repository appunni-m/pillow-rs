//! TrueType opcode constants.

#![allow(dead_code)]
#![allow(missing_docs)]

pub const SVTCA: u8 = 0x00;
pub const SPVTCA: u8 = 0x02;
pub const SFVTCA: u8 = 0x04;
pub const SPVTL: u8 = 0x06;
pub const SFVTL: u8 = 0x07;
pub const SPVFS: u8 = 0x08;
pub const SFVFS: u8 = 0x09;
pub const GPV: u8 = 0x0A;
pub const GFV: u8 = 0x0B;
pub const SFVTPV: u8 = 0x0E;
pub const ISECT: u8 = 0x0F;
pub const SRP0: u8 = 0x10;
pub const SRP1: u8 = 0x11;
pub const SRP2: u8 = 0x12;
pub const SZP0: u8 = 0x13;
pub const SZP1: u8 = 0x14;
pub const SZP2: u8 = 0x15;
pub const SZPS: u8 = 0x16;
pub const SLOOP: u8 = 0x17;
pub const SMD: u8 = 0x18;
pub const SCVTCI: u8 = 0x19;
pub const SSWCI: u8 = 0x1A;
pub const SSW: u8 = 0x1B;
pub const DUP: u8 = 0x20;
pub const POP: u8 = 0x21;
pub const CLEAR: u8 = 0x22;
pub const SWAP: u8 = 0x23;
pub const DEPTH: u8 = 0x24;
pub const CINDEX: u8 = 0x25;
pub const MINDEX: u8 = 0x26;
pub const ALIGNPTS: u8 = 0x27;
pub const LOOPCALL: u8 = 0x2A;
pub const CALL: u8 = 0x2B;
pub const FDEF: u8 = 0x2C;
pub const ENDF: u8 = 0x2D;
pub const MDAP: u8 = 0x2E;
pub const MDAP2: u8 = 0x2F;
pub const IUP: u8 = 0x30;
pub const IUP2: u8 = 0x31;
pub const SHP: u8 = 0x32;
pub const SHC: u8 = 0x34;
pub const SHZ: u8 = 0x36;
pub const IP: u8 = 0x39;
pub const MSIRP: u8 = 0x3A;
pub const ALIGNRP: u8 = 0x3C;
pub const RTDG: u8 = 0x3D;
pub const MIAP: u8 = 0x3E;
pub const MIAP2: u8 = 0x3F;
pub const NPUSHB: u8 = 0x40;
pub const NPUSHW: u8 = 0x41;
pub const WS: u8 = 0x42;
pub const RS: u8 = 0x43;
pub const WCVTP: u8 = 0x44;
pub const RCVT: u8 = 0x45;
pub const GC: u8 = 0x46;
pub const SCFS: u8 = 0x48;
pub const MD: u8 = 0x49;
pub const MPPEM: u8 = 0x4B;
pub const MPS: u8 = 0x4C;
pub const FLIPON: u8 = 0x4D;
pub const FLIPOFF: u8 = 0x4E;
pub const DEBUG: u8 = 0x4F;
pub const LT: u8 = 0x50;
pub const LTEQ: u8 = 0x51;
pub const GT: u8 = 0x52;
pub const GTEQ: u8 = 0x53;
pub const EQ: u8 = 0x54;
pub const NEQ: u8 = 0x55;
pub const AND: u8 = 0x56;
pub const OR: u8 = 0x57;
pub const NOT: u8 = 0x58;
pub const DELTAP1: u8 = 0x5D;
pub const DELTAP2: u8 = 0x5E;
pub const DELTAP3: u8 = 0x5F;
pub const ADD: u8 = 0x62;
pub const SUB: u8 = 0x63;
pub const DIV: u8 = 0x64;
pub const MUL: u8 = 0x65;
pub const ABS: u8 = 0x66;
pub const NEG: u8 = 0x67;
pub const FLOOR: u8 = 0x68;
pub const CEILING: u8 = 0x69;
pub const WCVTF: u8 = 0x70;
pub const SROUND: u8 = 0x76;
pub const S45ROUND: u8 = 0x77;
pub const JROT: u8 = 0x78;
pub const JROF: u8 = 0x79;
pub const JMPR: u8 = 0x7A;
pub const ODD: u8 = 0x7B;
pub const EVEN: u8 = 0x7C;
pub const GETINFO: u8 = 0x88;
pub const IF: u8 = 0x58;
pub const ELSE: u8 = 0x5B;
pub const EIF: u8 = 0x5C;

pub const PUSHB: [u8; 8] = [0xB0, 0xB1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7];
pub const PUSHW: [u8; 8] = [0xB8, 0xB9, 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF];

pub const MDRP_BASE: u8 = 0xC0;
pub const MIRP_BASE: u8 = 0xE0;

#[derive(Copy, Clone, Debug, Default)]
pub struct MirpFlags {
    pub round: bool,
    pub without_set: bool,
    pub set_round_state: bool,
}

pub fn decode_mirp_flags(opcode: u8) -> MirpFlags {
    MirpFlags {
        round: (opcode & 0x01) != 0,
        without_set: (opcode & 0x02) != 0,
        set_round_state: (opcode & 0x04) != 0,
    }
}
