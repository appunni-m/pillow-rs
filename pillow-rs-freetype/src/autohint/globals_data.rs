// 59 styles, 120 scripts with ranges, 56 blue sets
//! Auto-generated from FreeType afranges.c + afstyles.h — DO NOT EDIT.
use super::blue_strings::*;
#[derive(Debug, Clone, Copy)]
pub struct UniRange {
    pub first: u32,
    pub last: u32,
}
#[derive(Debug, Clone)]
pub struct StyleClass {
    pub description: &'static str,
    pub script_tag: &'static str,
    pub blue_entries: &'static [BlueStringEntry],
    pub uni_ranges: &'static [UniRange],
    pub non_base_ranges: &'static [UniRange],
}

pub static RANGES_ADLM_UNI: &[UniRange] = &[UniRange {
    first: 0x0001E900,
    last: 0x0001E95F,
}];
pub static RANGES_ADLM_NONBASE: &[UniRange] = &[];
pub static RANGES_ADLM_NONBASE_UNI: &[UniRange] = &[UniRange {
    first: 0x0001D944,
    last: 0x0001E94A,
}];
pub static RANGES_ADLM_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_ARAB_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000600,
        last: 0x000006FF,
    },
    UniRange {
        first: 0x00000750,
        last: 0x000007FF,
    },
    UniRange {
        first: 0x00000870,
        last: 0x0000089F,
    },
    UniRange {
        first: 0x000008A0,
        last: 0x000008FF,
    },
    UniRange {
        first: 0x0000FB50,
        last: 0x0000FDFF,
    },
    UniRange {
        first: 0x0000FE70,
        last: 0x0000FEFF,
    },
    UniRange {
        first: 0x00010EC0,
        last: 0x00010EFF,
    },
    UniRange {
        first: 0x0001EE00,
        last: 0x0001EEFF,
    },
];
pub static RANGES_ARAB_NONBASE: &[UniRange] = &[];
pub static RANGES_ARAB_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000600,
        last: 0x00000605,
    },
    UniRange {
        first: 0x00000610,
        last: 0x0000061A,
    },
    UniRange {
        first: 0x0000064B,
        last: 0x0000065F,
    },
    UniRange {
        first: 0x00000670,
        last: 0x00000670,
    },
    UniRange {
        first: 0x000006D6,
        last: 0x000006DC,
    },
    UniRange {
        first: 0x000006DF,
        last: 0x000006E4,
    },
    UniRange {
        first: 0x000006E7,
        last: 0x000006E8,
    },
    UniRange {
        first: 0x000006EA,
        last: 0x000006ED,
    },
    UniRange {
        first: 0x00000897,
        last: 0x0000089F,
    },
    UniRange {
        first: 0x000008CA,
        last: 0x000008E1,
    },
    UniRange {
        first: 0x000008E3,
        last: 0x000008FF,
    },
    UniRange {
        first: 0x0000FBB2,
        last: 0x0000FBC1,
    },
    UniRange {
        first: 0x0000FE70,
        last: 0x0000FE70,
    },
    UniRange {
        first: 0x0000FE72,
        last: 0x0000FE72,
    },
    UniRange {
        first: 0x0000FE74,
        last: 0x0000FE74,
    },
    UniRange {
        first: 0x0000FE76,
        last: 0x0000FE76,
    },
    UniRange {
        first: 0x0000FE78,
        last: 0x0000FE78,
    },
    UniRange {
        first: 0x0000FE7A,
        last: 0x0000FE7A,
    },
    UniRange {
        first: 0x0000FE7C,
        last: 0x0000FE7C,
    },
    UniRange {
        first: 0x0000FE7E,
        last: 0x0000FE7E,
    },
    UniRange {
        first: 0x00010EFD,
        last: 0x00010EFF,
    },
];
pub static RANGES_ARAB_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_ARMN_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000530,
        last: 0x0000058F,
    },
    UniRange {
        first: 0x0000FB13,
        last: 0x0000FB17,
    },
];
pub static RANGES_ARMN_NONBASE: &[UniRange] = &[];
pub static RANGES_ARMN_NONBASE_UNI: &[UniRange] = &[UniRange {
    first: 0x00000559,
    last: 0x0000055F,
}];
pub static RANGES_ARMN_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_AVST_UNI: &[UniRange] = &[UniRange {
    first: 0x00010B00,
    last: 0x00010B3F,
}];
pub static RANGES_AVST_NONBASE: &[UniRange] = &[];
pub static RANGES_AVST_NONBASE_UNI: &[UniRange] = &[UniRange {
    first: 0x00010B39,
    last: 0x00010B3F,
}];
pub static RANGES_AVST_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_BAMU_UNI: &[UniRange] = &[
    UniRange {
        first: 0x0000A6A0,
        last: 0x0000A6FF,
    },
    UniRange {
        first: 0x00016800,
        last: 0x00016A3F,
    },
];
pub static RANGES_BAMU_NONBASE: &[UniRange] = &[];
pub static RANGES_BAMU_NONBASE_UNI: &[UniRange] = &[UniRange {
    first: 0x0000A6F0,
    last: 0x0000A6F1,
}];
pub static RANGES_BAMU_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_BENG_UNI: &[UniRange] = &[UniRange {
    first: 0x00000980,
    last: 0x000009FF,
}];
pub static RANGES_BENG_NONBASE: &[UniRange] = &[];
pub static RANGES_BENG_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000981,
        last: 0x00000981,
    },
    UniRange {
        first: 0x000009BC,
        last: 0x000009BC,
    },
    UniRange {
        first: 0x000009C1,
        last: 0x000009C4,
    },
    UniRange {
        first: 0x000009CD,
        last: 0x000009CD,
    },
    UniRange {
        first: 0x000009E2,
        last: 0x000009E3,
    },
    UniRange {
        first: 0x000009FE,
        last: 0x000009FE,
    },
];
pub static RANGES_BENG_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_BUHD_UNI: &[UniRange] = &[UniRange {
    first: 0x00001740,
    last: 0x0000175F,
}];
pub static RANGES_BUHD_NONBASE: &[UniRange] = &[];
pub static RANGES_BUHD_NONBASE_UNI: &[UniRange] = &[UniRange {
    first: 0x00001752,
    last: 0x00001753,
}];
pub static RANGES_BUHD_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_CAKM_UNI: &[UniRange] = &[UniRange {
    first: 0x00011100,
    last: 0x0001114F,
}];
pub static RANGES_CAKM_NONBASE: &[UniRange] = &[];
pub static RANGES_CAKM_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00011100,
        last: 0x00011102,
    },
    UniRange {
        first: 0x00011127,
        last: 0x00011134,
    },
    UniRange {
        first: 0x00011146,
        last: 0x00011146,
    },
];
pub static RANGES_CAKM_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_CANS_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001400,
        last: 0x0000167F,
    },
    UniRange {
        first: 0x000018B0,
        last: 0x000018FF,
    },
    UniRange {
        first: 0x00011AB0,
        last: 0x00011ABF,
    },
];
pub static RANGES_CANS_NONBASE: &[UniRange] = &[];
pub static RANGES_CANS_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_CANS_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_CARI_UNI: &[UniRange] = &[UniRange {
    first: 0x000102A0,
    last: 0x000102DF,
}];
pub static RANGES_CARI_NONBASE: &[UniRange] = &[];
pub static RANGES_CARI_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_CARI_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_CHER_UNI: &[UniRange] = &[
    UniRange {
        first: 0x000013A0,
        last: 0x000013FF,
    },
    UniRange {
        first: 0x0000AB70,
        last: 0x0000ABBF,
    },
];
pub static RANGES_CHER_NONBASE: &[UniRange] = &[];
pub static RANGES_CHER_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_CHER_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_COPT_UNI: &[UniRange] = &[UniRange {
    first: 0x00002C80,
    last: 0x00002CFF,
}];
pub static RANGES_COPT_NONBASE: &[UniRange] = &[];
pub static RANGES_COPT_NONBASE_UNI: &[UniRange] = &[UniRange {
    first: 0x00002CEF,
    last: 0x00002CF1,
}];
pub static RANGES_COPT_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_CPRT_UNI: &[UniRange] = &[UniRange {
    first: 0x00010800,
    last: 0x0001083F,
}];
pub static RANGES_CPRT_NONBASE: &[UniRange] = &[];
pub static RANGES_CPRT_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_CPRT_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_CYRL_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000400,
        last: 0x000004FF,
    },
    UniRange {
        first: 0x00000500,
        last: 0x0000052F,
    },
    UniRange {
        first: 0x00002DE0,
        last: 0x00002DFF,
    },
    UniRange {
        first: 0x0000A640,
        last: 0x0000A69F,
    },
    UniRange {
        first: 0x00001C80,
        last: 0x00001C8F,
    },
    UniRange {
        first: 0x0001E030,
        last: 0x0001E08F,
    },
];
pub static RANGES_CYRL_NONBASE: &[UniRange] = &[];
pub static RANGES_CYRL_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000483,
        last: 0x00000489,
    },
    UniRange {
        first: 0x00002DE0,
        last: 0x00002DFF,
    },
    UniRange {
        first: 0x0000A66F,
        last: 0x0000A67F,
    },
    UniRange {
        first: 0x0000A69E,
        last: 0x0000A69F,
    },
];
pub static RANGES_CYRL_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_DEVA_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000900,
        last: 0x0000093B,
    },
    UniRange {
        first: 0x0000093D,
        last: 0x00000950,
    },
    UniRange {
        first: 0x00000953,
        last: 0x00000963,
    },
    UniRange {
        first: 0x00000966,
        last: 0x0000097F,
    },
    UniRange {
        first: 0x000020B9,
        last: 0x000020B9,
    },
    UniRange {
        first: 0x0000A8E0,
        last: 0x0000A8FF,
    },
    UniRange {
        first: 0x00011B00,
        last: 0x00011B5F,
    },
];
pub static RANGES_DEVA_NONBASE: &[UniRange] = &[];
pub static RANGES_DEVA_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000900,
        last: 0x00000902,
    },
    UniRange {
        first: 0x0000093A,
        last: 0x0000093A,
    },
    UniRange {
        first: 0x00000941,
        last: 0x00000948,
    },
    UniRange {
        first: 0x0000094D,
        last: 0x0000094D,
    },
    UniRange {
        first: 0x00000953,
        last: 0x00000957,
    },
    UniRange {
        first: 0x00000962,
        last: 0x00000963,
    },
    UniRange {
        first: 0x0000A8E0,
        last: 0x0000A8F1,
    },
    UniRange {
        first: 0x0000A8FF,
        last: 0x0000A8FF,
    },
];
pub static RANGES_DEVA_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_DSRT_UNI: &[UniRange] = &[UniRange {
    first: 0x00010400,
    last: 0x0001044F,
}];
pub static RANGES_DSRT_NONBASE: &[UniRange] = &[];
pub static RANGES_DSRT_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_DSRT_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_ETHI_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001200,
        last: 0x0000137F,
    },
    UniRange {
        first: 0x00001380,
        last: 0x0000139F,
    },
    UniRange {
        first: 0x00002D80,
        last: 0x00002DDF,
    },
    UniRange {
        first: 0x0000AB00,
        last: 0x0000AB2F,
    },
    UniRange {
        first: 0x0001E7E0,
        last: 0x0001E7FF,
    },
];
pub static RANGES_ETHI_NONBASE: &[UniRange] = &[];
pub static RANGES_ETHI_NONBASE_UNI: &[UniRange] = &[UniRange {
    first: 0x0000135D,
    last: 0x0000135F,
}];
pub static RANGES_ETHI_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_GEOK_UNI: &[UniRange] = &[
    UniRange {
        first: 0x000010A0,
        last: 0x000010CD,
    },
    UniRange {
        first: 0x00002D00,
        last: 0x00002D2D,
    },
];
pub static RANGES_GEOK_NONBASE: &[UniRange] = &[];
pub static RANGES_GEOK_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_GEOK_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_GEOR_UNI: &[UniRange] = &[
    UniRange {
        first: 0x000010D0,
        last: 0x000010FF,
    },
    UniRange {
        first: 0x00001C90,
        last: 0x00001CBF,
    },
];
pub static RANGES_GEOR_NONBASE: &[UniRange] = &[];
pub static RANGES_GEOR_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_GEOR_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_GLAG_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00002C00,
        last: 0x00002C5F,
    },
    UniRange {
        first: 0x0001E000,
        last: 0x0001E02F,
    },
];
pub static RANGES_GLAG_NONBASE: &[UniRange] = &[];
pub static RANGES_GLAG_NONBASE_UNI: &[UniRange] = &[UniRange {
    first: 0x0001E000,
    last: 0x0001E02F,
}];
pub static RANGES_GLAG_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_GOTH_UNI: &[UniRange] = &[UniRange {
    first: 0x00010330,
    last: 0x0001034F,
}];
pub static RANGES_GOTH_NONBASE: &[UniRange] = &[];
pub static RANGES_GOTH_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_GOTH_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_GREK_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000370,
        last: 0x000003FF,
    },
    UniRange {
        first: 0x00001F00,
        last: 0x00001FFF,
    },
];
pub static RANGES_GREK_NONBASE: &[UniRange] = &[];
pub static RANGES_GREK_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x0000037A,
        last: 0x0000037A,
    },
    UniRange {
        first: 0x00000384,
        last: 0x00000385,
    },
    UniRange {
        first: 0x00001FBD,
        last: 0x00001FC1,
    },
    UniRange {
        first: 0x00001FCD,
        last: 0x00001FCF,
    },
    UniRange {
        first: 0x00001FDD,
        last: 0x00001FDF,
    },
    UniRange {
        first: 0x00001FED,
        last: 0x00001FEF,
    },
    UniRange {
        first: 0x00001FFD,
        last: 0x00001FFE,
    },
];
pub static RANGES_GREK_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_GUJR_UNI: &[UniRange] = &[UniRange {
    first: 0x00000A80,
    last: 0x00000AFF,
}];
pub static RANGES_GUJR_NONBASE: &[UniRange] = &[];
pub static RANGES_GUJR_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000A81,
        last: 0x00000A82,
    },
    UniRange {
        first: 0x00000ABC,
        last: 0x00000ABC,
    },
    UniRange {
        first: 0x00000AC1,
        last: 0x00000AC8,
    },
    UniRange {
        first: 0x00000ACD,
        last: 0x00000ACD,
    },
    UniRange {
        first: 0x00000AE2,
        last: 0x00000AE3,
    },
    UniRange {
        first: 0x00000AFA,
        last: 0x00000AFF,
    },
];
pub static RANGES_GUJR_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_GURU_UNI: &[UniRange] = &[UniRange {
    first: 0x00000A00,
    last: 0x00000A7F,
}];
pub static RANGES_GURU_NONBASE: &[UniRange] = &[];
pub static RANGES_GURU_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000A01,
        last: 0x00000A02,
    },
    UniRange {
        first: 0x00000A3C,
        last: 0x00000A3C,
    },
    UniRange {
        first: 0x00000A41,
        last: 0x00000A51,
    },
    UniRange {
        first: 0x00000A70,
        last: 0x00000A71,
    },
    UniRange {
        first: 0x00000A75,
        last: 0x00000A75,
    },
];
pub static RANGES_GURU_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_HANI_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001100,
        last: 0x000011FF,
    },
    UniRange {
        first: 0x00002E80,
        last: 0x00002EFF,
    },
    UniRange {
        first: 0x00002F00,
        last: 0x00002FDF,
    },
    UniRange {
        first: 0x00002FF0,
        last: 0x00002FFF,
    },
    UniRange {
        first: 0x00003000,
        last: 0x0000303F,
    },
    UniRange {
        first: 0x00003040,
        last: 0x0000309F,
    },
    UniRange {
        first: 0x000030A0,
        last: 0x000030FF,
    },
    UniRange {
        first: 0x00003100,
        last: 0x0000312F,
    },
    UniRange {
        first: 0x00003130,
        last: 0x0000318F,
    },
    UniRange {
        first: 0x00003190,
        last: 0x0000319F,
    },
    UniRange {
        first: 0x000031A0,
        last: 0x000031BF,
    },
    UniRange {
        first: 0x000031C0,
        last: 0x000031EF,
    },
    UniRange {
        first: 0x000031F0,
        last: 0x000031FF,
    },
    UniRange {
        first: 0x00003300,
        last: 0x000033FF,
    },
    UniRange {
        first: 0x00003400,
        last: 0x00004DBF,
    },
    UniRange {
        first: 0x00004DC0,
        last: 0x00004DFF,
    },
    UniRange {
        first: 0x00004E00,
        last: 0x00009FFF,
    },
    UniRange {
        first: 0x0000A960,
        last: 0x0000A97F,
    },
    UniRange {
        first: 0x0000AC00,
        last: 0x0000D7AF,
    },
    UniRange {
        first: 0x0000D7B0,
        last: 0x0000D7FF,
    },
    UniRange {
        first: 0x0000F900,
        last: 0x0000FAFF,
    },
    UniRange {
        first: 0x0000FE10,
        last: 0x0000FE1F,
    },
    UniRange {
        first: 0x0000FE30,
        last: 0x0000FE4F,
    },
    UniRange {
        first: 0x0000FF00,
        last: 0x0000FFEF,
    },
    UniRange {
        first: 0x0001AFF0,
        last: 0x0001AFFF,
    },
    UniRange {
        first: 0x0001B000,
        last: 0x0001B0FF,
    },
    UniRange {
        first: 0x0001B100,
        last: 0x0001B12F,
    },
    UniRange {
        first: 0x0001B130,
        last: 0x0001B16F,
    },
    UniRange {
        first: 0x0001D300,
        last: 0x0001D35F,
    },
    UniRange {
        first: 0x00020000,
        last: 0x0002A6DF,
    },
    UniRange {
        first: 0x0002A700,
        last: 0x0002B73F,
    },
    UniRange {
        first: 0x0002B740,
        last: 0x0002B81F,
    },
    UniRange {
        first: 0x0002B820,
        last: 0x0002CEAF,
    },
    UniRange {
        first: 0x0002CEB0,
        last: 0x0002EBEF,
    },
    UniRange {
        first: 0x0002EBF0,
        last: 0x0002EE5D,
    },
    UniRange {
        first: 0x0002F800,
        last: 0x0002FA1F,
    },
    UniRange {
        first: 0x00030000,
        last: 0x0003134A,
    },
    UniRange {
        first: 0x00031350,
        last: 0x000323AF,
    },
    UniRange {
        first: 0x000323B0,
        last: 0x00033479,
    },
];
pub static RANGES_HANI_NONBASE: &[UniRange] = &[];
pub static RANGES_HANI_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x0000302A,
        last: 0x0000302F,
    },
    UniRange {
        first: 0x00003190,
        last: 0x0000319F,
    },
];
pub static RANGES_HANI_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_HEBR_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000590,
        last: 0x000005FF,
    },
    UniRange {
        first: 0x0000FB1D,
        last: 0x0000FB4F,
    },
];
pub static RANGES_HEBR_NONBASE: &[UniRange] = &[];
pub static RANGES_HEBR_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000591,
        last: 0x000005BF,
    },
    UniRange {
        first: 0x000005C1,
        last: 0x000005C2,
    },
    UniRange {
        first: 0x000005C4,
        last: 0x000005C5,
    },
    UniRange {
        first: 0x000005C7,
        last: 0x000005C7,
    },
    UniRange {
        first: 0x0000FB1E,
        last: 0x0000FB1E,
    },
];
pub static RANGES_HEBR_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_KALI_UNI: &[UniRange] = &[UniRange {
    first: 0x0000A900,
    last: 0x0000A92F,
}];
pub static RANGES_KALI_NONBASE: &[UniRange] = &[];
pub static RANGES_KALI_NONBASE_UNI: &[UniRange] = &[UniRange {
    first: 0x0000A926,
    last: 0x0000A92D,
}];
pub static RANGES_KALI_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_KHMR_UNI: &[UniRange] = &[UniRange {
    first: 0x00001780,
    last: 0x000017FF,
}];
pub static RANGES_KHMR_NONBASE: &[UniRange] = &[];
pub static RANGES_KHMR_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x000017B7,
        last: 0x000017BD,
    },
    UniRange {
        first: 0x000017C6,
        last: 0x000017C6,
    },
    UniRange {
        first: 0x000017C9,
        last: 0x000017D3,
    },
    UniRange {
        first: 0x000017DD,
        last: 0x000017DD,
    },
];
pub static RANGES_KHMR_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_KHMS_UNI: &[UniRange] = &[UniRange {
    first: 0x000019E0,
    last: 0x000019FF,
}];
pub static RANGES_KHMS_NONBASE: &[UniRange] = &[];
pub static RANGES_KHMS_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_KHMS_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_KNDA_UNI: &[UniRange] = &[UniRange {
    first: 0x00000C80,
    last: 0x00000CFF,
}];
pub static RANGES_KNDA_NONBASE: &[UniRange] = &[];
pub static RANGES_KNDA_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000C81,
        last: 0x00000C81,
    },
    UniRange {
        first: 0x00000CBC,
        last: 0x00000CBC,
    },
    UniRange {
        first: 0x00000CBF,
        last: 0x00000CBF,
    },
    UniRange {
        first: 0x00000CC6,
        last: 0x00000CC6,
    },
    UniRange {
        first: 0x00000CCC,
        last: 0x00000CCD,
    },
    UniRange {
        first: 0x00000CE2,
        last: 0x00000CE3,
    },
];
pub static RANGES_KNDA_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_LAO_UNI: &[UniRange] = &[UniRange {
    first: 0x00000E80,
    last: 0x00000EFF,
}];
pub static RANGES_LAO_NONBASE: &[UniRange] = &[];
pub static RANGES_LAO_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000EB1,
        last: 0x00000EB1,
    },
    UniRange {
        first: 0x00000EB4,
        last: 0x00000EBC,
    },
    UniRange {
        first: 0x00000EC8,
        last: 0x00000ECE,
    },
];
pub static RANGES_LAO_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_LATB_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001D62,
        last: 0x00001D6A,
    },
    UniRange {
        first: 0x00002080,
        last: 0x0000209C,
    },
    UniRange {
        first: 0x00002C7C,
        last: 0x00002C7C,
    },
];
pub static RANGES_LATB_NONBASE: &[UniRange] = &[];
pub static RANGES_LATB_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_LATB_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_LATN_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000020,
        last: 0x0000007F,
    },
    UniRange {
        first: 0x000000A0,
        last: 0x000000A9,
    },
    UniRange {
        first: 0x000000AB,
        last: 0x000000B1,
    },
    UniRange {
        first: 0x000000B4,
        last: 0x000000B8,
    },
    UniRange {
        first: 0x000000BB,
        last: 0x000000FF,
    },
    UniRange {
        first: 0x00000100,
        last: 0x0000017F,
    },
    UniRange {
        first: 0x00000180,
        last: 0x0000024F,
    },
    UniRange {
        first: 0x00000250,
        last: 0x000002AF,
    },
    UniRange {
        first: 0x000002B9,
        last: 0x000002DF,
    },
    UniRange {
        first: 0x000002E5,
        last: 0x000002FF,
    },
    UniRange {
        first: 0x00000300,
        last: 0x0000036F,
    },
    UniRange {
        first: 0x00001AB0,
        last: 0x00001ABE,
    },
    UniRange {
        first: 0x00001D00,
        last: 0x00001D2B,
    },
    UniRange {
        first: 0x00001D6B,
        last: 0x00001D77,
    },
    UniRange {
        first: 0x00001D79,
        last: 0x00001D7F,
    },
    UniRange {
        first: 0x00001D80,
        last: 0x00001D9A,
    },
    UniRange {
        first: 0x00001DC0,
        last: 0x00001DFF,
    },
    UniRange {
        first: 0x00001E00,
        last: 0x00001EFF,
    },
    UniRange {
        first: 0x00002000,
        last: 0x0000206F,
    },
    UniRange {
        first: 0x000020A0,
        last: 0x000020B8,
    },
    UniRange {
        first: 0x000020BA,
        last: 0x000020CF,
    },
    UniRange {
        first: 0x00002150,
        last: 0x0000218F,
    },
    UniRange {
        first: 0x00002C60,
        last: 0x00002C7B,
    },
    UniRange {
        first: 0x00002C7E,
        last: 0x00002C7F,
    },
    UniRange {
        first: 0x00002E00,
        last: 0x00002E7F,
    },
    UniRange {
        first: 0x0000A720,
        last: 0x0000A76F,
    },
    UniRange {
        first: 0x0000A771,
        last: 0x0000A7F0,
    },
    UniRange {
        first: 0x0000A7F2,
        last: 0x0000A7F7,
    },
    UniRange {
        first: 0x0000A7FA,
        last: 0x0000A7FF,
    },
    UniRange {
        first: 0x0000AB30,
        last: 0x0000AB5B,
    },
    UniRange {
        first: 0x0000AB60,
        last: 0x0000AB68,
    },
    UniRange {
        first: 0x0000AB6A,
        last: 0x0000AB6F,
    },
    UniRange {
        first: 0x0000FB00,
        last: 0x0000FB06,
    },
    UniRange {
        first: 0x0001D400,
        last: 0x0001D7FF,
    },
    UniRange {
        first: 0x0001DF00,
        last: 0x0001DFFF,
    },
];
pub static RANGES_LATN_NONBASE: &[UniRange] = &[];
pub static RANGES_LATN_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x0000005E,
        last: 0x00000060,
    },
    UniRange {
        first: 0x0000007E,
        last: 0x0000007E,
    },
    UniRange {
        first: 0x000000A8,
        last: 0x000000A9,
    },
    UniRange {
        first: 0x000000AE,
        last: 0x000000B0,
    },
    UniRange {
        first: 0x000000B4,
        last: 0x000000B4,
    },
    UniRange {
        first: 0x000000B8,
        last: 0x000000B8,
    },
    UniRange {
        first: 0x000000BC,
        last: 0x000000BE,
    },
    UniRange {
        first: 0x000002B9,
        last: 0x000002DF,
    },
    UniRange {
        first: 0x000002E5,
        last: 0x000002FF,
    },
    UniRange {
        first: 0x00000300,
        last: 0x0000036F,
    },
    UniRange {
        first: 0x00001AB0,
        last: 0x00001AEB,
    },
    UniRange {
        first: 0x00001DC0,
        last: 0x00001DFF,
    },
    UniRange {
        first: 0x00002017,
        last: 0x00002017,
    },
    UniRange {
        first: 0x0000203E,
        last: 0x0000203E,
    },
    UniRange {
        first: 0x0000A788,
        last: 0x0000A788,
    },
    UniRange {
        first: 0x0000A7F8,
        last: 0x0000A7FA,
    },
];
pub static RANGES_LATN_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_LATP_UNI: &[UniRange] = &[
    UniRange {
        first: 0x000000AA,
        last: 0x000000AA,
    },
    UniRange {
        first: 0x000000B2,
        last: 0x000000B3,
    },
    UniRange {
        first: 0x000000B9,
        last: 0x000000BA,
    },
    UniRange {
        first: 0x000002B0,
        last: 0x000002B8,
    },
    UniRange {
        first: 0x000002E0,
        last: 0x000002E4,
    },
    UniRange {
        first: 0x00001D2C,
        last: 0x00001D61,
    },
    UniRange {
        first: 0x00001D78,
        last: 0x00001D78,
    },
    UniRange {
        first: 0x00001D9B,
        last: 0x00001DBF,
    },
    UniRange {
        first: 0x00002070,
        last: 0x0000207F,
    },
    UniRange {
        first: 0x00002C7D,
        last: 0x00002C7D,
    },
    UniRange {
        first: 0x0000A770,
        last: 0x0000A770,
    },
    UniRange {
        first: 0x0000A7F1,
        last: 0x0000A7F1,
    },
    UniRange {
        first: 0x0000A7F8,
        last: 0x0000A7F9,
    },
    UniRange {
        first: 0x0000AB5C,
        last: 0x0000AB5F,
    },
    UniRange {
        first: 0x0000AB69,
        last: 0x0000AB69,
    },
    UniRange {
        first: 0x00010780,
        last: 0x000107FB,
    },
];
pub static RANGES_LATP_NONBASE: &[UniRange] = &[];
pub static RANGES_LATP_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_LATP_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_LIMB_UNI: &[UniRange] = &[UniRange {
    first: 0x00001900,
    last: 0x0000194F,
}];
pub static RANGES_LIMB_NONBASE: &[UniRange] = &[];
pub static RANGES_LIMB_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001920,
        last: 0x00001922,
    },
    UniRange {
        first: 0x00001927,
        last: 0x00001934,
    },
    UniRange {
        first: 0x00001937,
        last: 0x0000193B,
    },
];
pub static RANGES_LIMB_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_LISU_UNI: &[UniRange] = &[
    UniRange {
        first: 0x0000A4D0,
        last: 0x0000A4FF,
    },
    UniRange {
        first: 0x00011FB0,
        last: 0x00011FBF,
    },
];
pub static RANGES_LISU_NONBASE: &[UniRange] = &[];
pub static RANGES_LISU_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_LISU_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_MEDF_UNI: &[UniRange] = &[UniRange {
    first: 0x00016E40,
    last: 0x00016E9F,
}];
pub static RANGES_MEDF_NONBASE: &[UniRange] = &[];
pub static RANGES_MEDF_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_MEDF_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_MLYM_UNI: &[UniRange] = &[UniRange {
    first: 0x00000D00,
    last: 0x00000D7F,
}];
pub static RANGES_MLYM_NONBASE: &[UniRange] = &[];
pub static RANGES_MLYM_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000D00,
        last: 0x00000D01,
    },
    UniRange {
        first: 0x00000D3B,
        last: 0x00000D3C,
    },
    UniRange {
        first: 0x00000D4D,
        last: 0x00000D4E,
    },
    UniRange {
        first: 0x00000D62,
        last: 0x00000D63,
    },
];
pub static RANGES_MLYM_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_MONG_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001800,
        last: 0x000018AF,
    },
    UniRange {
        first: 0x00011660,
        last: 0x0001167F,
    },
];
pub static RANGES_MONG_NONBASE: &[UniRange] = &[];
pub static RANGES_MONG_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001885,
        last: 0x00001886,
    },
    UniRange {
        first: 0x000018A9,
        last: 0x000018A9,
    },
];
pub static RANGES_MONG_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_MYMR_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001000,
        last: 0x0000109F,
    },
    UniRange {
        first: 0x0000A9E0,
        last: 0x0000A9FF,
    },
    UniRange {
        first: 0x0000AA60,
        last: 0x0000AA7F,
    },
    UniRange {
        first: 0x000116D0,
        last: 0x000116FF,
    },
];
pub static RANGES_MYMR_NONBASE: &[UniRange] = &[];
pub static RANGES_MYMR_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x0000102D,
        last: 0x00001030,
    },
    UniRange {
        first: 0x00001032,
        last: 0x00001037,
    },
    UniRange {
        first: 0x0000103A,
        last: 0x0000103A,
    },
    UniRange {
        first: 0x0000103D,
        last: 0x0000103E,
    },
    UniRange {
        first: 0x00001058,
        last: 0x00001059,
    },
    UniRange {
        first: 0x0000105E,
        last: 0x00001060,
    },
    UniRange {
        first: 0x00001071,
        last: 0x00001074,
    },
    UniRange {
        first: 0x00001082,
        last: 0x00001082,
    },
    UniRange {
        first: 0x00001085,
        last: 0x00001086,
    },
    UniRange {
        first: 0x0000108D,
        last: 0x0000108D,
    },
    UniRange {
        first: 0x0000A9E5,
        last: 0x0000A9E5,
    },
    UniRange {
        first: 0x0000AA7C,
        last: 0x0000AA7C,
    },
];
pub static RANGES_MYMR_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_NKOO_UNI: &[UniRange] = &[UniRange {
    first: 0x000007C0,
    last: 0x000007FF,
}];
pub static RANGES_NKOO_NONBASE: &[UniRange] = &[];
pub static RANGES_NKOO_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x000007EB,
        last: 0x000007F5,
    },
    UniRange {
        first: 0x000007FD,
        last: 0x000007FD,
    },
];
pub static RANGES_NKOO_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_NONE_UNI: &[UniRange] = &[];
pub static RANGES_NONE_NONBASE: &[UniRange] = &[];
pub static RANGES_NONE_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_NONE_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_OLCK_UNI: &[UniRange] = &[UniRange {
    first: 0x00001C50,
    last: 0x00001C7F,
}];
pub static RANGES_OLCK_NONBASE: &[UniRange] = &[];
pub static RANGES_OLCK_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_OLCK_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_ORKH_UNI: &[UniRange] = &[UniRange {
    first: 0x00010C00,
    last: 0x00010C4F,
}];
pub static RANGES_ORKH_NONBASE: &[UniRange] = &[];
pub static RANGES_ORKH_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_ORKH_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_ORYA_UNI: &[UniRange] = &[UniRange {
    first: 0x00000B00,
    last: 0x00000B7F,
}];
pub static RANGES_ORYA_NONBASE: &[UniRange] = &[];
pub static RANGES_ORYA_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000B01,
        last: 0x00000B02,
    },
    UniRange {
        first: 0x00000B3C,
        last: 0x00000B3C,
    },
    UniRange {
        first: 0x00000B3F,
        last: 0x00000B3F,
    },
    UniRange {
        first: 0x00000B41,
        last: 0x00000B44,
    },
    UniRange {
        first: 0x00000B4D,
        last: 0x00000B56,
    },
    UniRange {
        first: 0x00000B62,
        last: 0x00000B63,
    },
];
pub static RANGES_ORYA_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_OSGE_UNI: &[UniRange] = &[UniRange {
    first: 0x000104B0,
    last: 0x000104FF,
}];
pub static RANGES_OSGE_NONBASE: &[UniRange] = &[];
pub static RANGES_OSGE_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_OSGE_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_OSMA_UNI: &[UniRange] = &[UniRange {
    first: 0x00010480,
    last: 0x000104AF,
}];
pub static RANGES_OSMA_NONBASE: &[UniRange] = &[];
pub static RANGES_OSMA_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_OSMA_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_ROHG_UNI: &[UniRange] = &[UniRange {
    first: 0x00010D00,
    last: 0x00010D3F,
}];
pub static RANGES_ROHG_NONBASE: &[UniRange] = &[];
pub static RANGES_ROHG_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_ROHG_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_SAUR_UNI: &[UniRange] = &[UniRange {
    first: 0x0000A880,
    last: 0x0000A8DF,
}];
pub static RANGES_SAUR_NONBASE: &[UniRange] = &[];
pub static RANGES_SAUR_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x0000A880,
        last: 0x0000A881,
    },
    UniRange {
        first: 0x0000A8B4,
        last: 0x0000A8C5,
    },
];
pub static RANGES_SAUR_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_SHAW_UNI: &[UniRange] = &[UniRange {
    first: 0x00010450,
    last: 0x0001047F,
}];
pub static RANGES_SHAW_NONBASE: &[UniRange] = &[];
pub static RANGES_SHAW_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_SHAW_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_SINH_UNI: &[UniRange] = &[UniRange {
    first: 0x00000D80,
    last: 0x00000DFF,
}];
pub static RANGES_SINH_NONBASE: &[UniRange] = &[];
pub static RANGES_SINH_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000D81,
        last: 0x00000D81,
    },
    UniRange {
        first: 0x00000DCA,
        last: 0x00000DCA,
    },
    UniRange {
        first: 0x00000DD2,
        last: 0x00000DD6,
    },
];
pub static RANGES_SINH_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_SUND_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001B80,
        last: 0x00001BBF,
    },
    UniRange {
        first: 0x00001CC0,
        last: 0x00001CCF,
    },
];
pub static RANGES_SUND_NONBASE: &[UniRange] = &[];
pub static RANGES_SUND_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00001B80,
        last: 0x00001B82,
    },
    UniRange {
        first: 0x00001BA1,
        last: 0x00001BAD,
    },
];
pub static RANGES_SUND_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_SYLO_UNI: &[UniRange] = &[UniRange {
    first: 0x0000A800,
    last: 0x0000A82F,
}];
pub static RANGES_SYLO_NONBASE: &[UniRange] = &[];
pub static RANGES_SYLO_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x0000A802,
        last: 0x0000A802,
    },
    UniRange {
        first: 0x0000A806,
        last: 0x0000A806,
    },
    UniRange {
        first: 0x0000A80B,
        last: 0x0000A80B,
    },
    UniRange {
        first: 0x0000A825,
        last: 0x0000A826,
    },
    UniRange {
        first: 0x0000A82C,
        last: 0x0000A82C,
    },
];
pub static RANGES_SYLO_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_TAML_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000B80,
        last: 0x00000BFF,
    },
    UniRange {
        first: 0x00011FC0,
        last: 0x00011FFF,
    },
];
pub static RANGES_TAML_NONBASE: &[UniRange] = &[];
pub static RANGES_TAML_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000B82,
        last: 0x00000B82,
    },
    UniRange {
        first: 0x00000BC0,
        last: 0x00000BC2,
    },
    UniRange {
        first: 0x00000BCD,
        last: 0x00000BCD,
    },
];
pub static RANGES_TAML_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_TAVT_UNI: &[UniRange] = &[UniRange {
    first: 0x0000AA80,
    last: 0x0000AADF,
}];
pub static RANGES_TAVT_NONBASE: &[UniRange] = &[];
pub static RANGES_TAVT_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x0000AAB0,
        last: 0x0000AAB0,
    },
    UniRange {
        first: 0x0000AAB2,
        last: 0x0000AAB4,
    },
    UniRange {
        first: 0x0000AAB7,
        last: 0x0000AAB8,
    },
    UniRange {
        first: 0x0000AABE,
        last: 0x0000AABF,
    },
    UniRange {
        first: 0x0000AAC1,
        last: 0x0000AAC1,
    },
];
pub static RANGES_TAVT_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_TELU_UNI: &[UniRange] = &[UniRange {
    first: 0x00000C00,
    last: 0x00000C7F,
}];
pub static RANGES_TELU_NONBASE: &[UniRange] = &[];
pub static RANGES_TELU_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000C00,
        last: 0x00000C00,
    },
    UniRange {
        first: 0x00000C04,
        last: 0x00000C04,
    },
    UniRange {
        first: 0x00000C3C,
        last: 0x00000C3C,
    },
    UniRange {
        first: 0x00000C3E,
        last: 0x00000C40,
    },
    UniRange {
        first: 0x00000C46,
        last: 0x00000C56,
    },
    UniRange {
        first: 0x00000C62,
        last: 0x00000C63,
    },
];
pub static RANGES_TELU_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_TFNG_UNI: &[UniRange] = &[UniRange {
    first: 0x00002D30,
    last: 0x00002D7F,
}];
pub static RANGES_TFNG_NONBASE: &[UniRange] = &[];
pub static RANGES_TFNG_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_TFNG_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_THAI_UNI: &[UniRange] = &[UniRange {
    first: 0x00000E00,
    last: 0x00000E7F,
}];
pub static RANGES_THAI_NONBASE: &[UniRange] = &[];
pub static RANGES_THAI_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000E31,
        last: 0x00000E31,
    },
    UniRange {
        first: 0x00000E34,
        last: 0x00000E3A,
    },
    UniRange {
        first: 0x00000E47,
        last: 0x00000E4E,
    },
];
pub static RANGES_THAI_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_TIBT_UNI: &[UniRange] = &[UniRange {
    first: 0x00000F00,
    last: 0x00000FFF,
}];
pub static RANGES_TIBT_NONBASE: &[UniRange] = &[];
pub static RANGES_TIBT_NONBASE_UNI: &[UniRange] = &[
    UniRange {
        first: 0x00000F18,
        last: 0x00000F19,
    },
    UniRange {
        first: 0x00000F35,
        last: 0x00000F35,
    },
    UniRange {
        first: 0x00000F37,
        last: 0x00000F37,
    },
    UniRange {
        first: 0x00000F39,
        last: 0x00000F39,
    },
    UniRange {
        first: 0x00000F3E,
        last: 0x00000F3F,
    },
    UniRange {
        first: 0x00000F71,
        last: 0x00000F7E,
    },
    UniRange {
        first: 0x00000F80,
        last: 0x00000F84,
    },
    UniRange {
        first: 0x00000F86,
        last: 0x00000F87,
    },
    UniRange {
        first: 0x00000F8D,
        last: 0x00000FBC,
    },
];
pub static RANGES_TIBT_NONBASE_NONBASE: &[UniRange] = &[];
pub static RANGES_VAII_UNI: &[UniRange] = &[UniRange {
    first: 0x0000A500,
    last: 0x0000A63F,
}];
pub static RANGES_VAII_NONBASE: &[UniRange] = &[];
pub static RANGES_VAII_NONBASE_UNI: &[UniRange] = &[];
pub static RANGES_VAII_NONBASE_NONBASE: &[UniRange] = &[];
/// Coverage scan order (matches afstyles.h). First match wins.
pub static STYLE_TABLE: &[StyleClass] = &[
    StyleClass {
        description: "Adlam default style",
        script_tag: "adlm",
        blue_entries: SCRIPT_ADLM,
        uni_ranges: RANGES_ADLM_UNI,
        non_base_ranges: RANGES_ADLM_NONBASE_UNI,
    },
    StyleClass {
        description: "Arabic default style",
        script_tag: "arab",
        blue_entries: SCRIPT_ARAB,
        uni_ranges: RANGES_ARAB_UNI,
        non_base_ranges: RANGES_ARAB_NONBASE_UNI,
    },
    StyleClass {
        description: "Armenian default style",
        script_tag: "armn",
        blue_entries: SCRIPT_ARMN,
        uni_ranges: RANGES_ARMN_UNI,
        non_base_ranges: RANGES_ARMN_NONBASE_UNI,
    },
    StyleClass {
        description: "Avestan default style",
        script_tag: "avst",
        blue_entries: SCRIPT_AVST,
        uni_ranges: RANGES_AVST_UNI,
        non_base_ranges: RANGES_AVST_NONBASE_UNI,
    },
    StyleClass {
        description: "Bamum default style",
        script_tag: "bamu",
        blue_entries: SCRIPT_BAMU,
        uni_ranges: RANGES_BAMU_UNI,
        non_base_ranges: RANGES_BAMU_NONBASE_UNI,
    },
    StyleClass {
        description: "Bengali default style",
        script_tag: "beng",
        blue_entries: SCRIPT_BENG,
        uni_ranges: RANGES_BENG_UNI,
        non_base_ranges: RANGES_BENG_NONBASE_UNI,
    },
    StyleClass {
        description: "Buhid default style",
        script_tag: "buhd",
        blue_entries: SCRIPT_BUHD,
        uni_ranges: RANGES_BUHD_UNI,
        non_base_ranges: RANGES_BUHD_NONBASE_UNI,
    },
    StyleClass {
        description: "Chakma default style",
        script_tag: "cakm",
        blue_entries: SCRIPT_CAKM,
        uni_ranges: RANGES_CAKM_UNI,
        non_base_ranges: RANGES_CAKM_NONBASE_UNI,
    },
    StyleClass {
        description: "Canadian Syllabics default style",
        script_tag: "cans",
        blue_entries: SCRIPT_CANS,
        uni_ranges: RANGES_CANS_UNI,
        non_base_ranges: RANGES_CANS_NONBASE_UNI,
    },
    StyleClass {
        description: "Carian default style",
        script_tag: "cari",
        blue_entries: SCRIPT_CARI,
        uni_ranges: RANGES_CARI_UNI,
        non_base_ranges: RANGES_CARI_NONBASE_UNI,
    },
    StyleClass {
        description: "Cherokee default style",
        script_tag: "cher",
        blue_entries: SCRIPT_CHER,
        uni_ranges: RANGES_CHER_UNI,
        non_base_ranges: RANGES_CHER_NONBASE_UNI,
    },
    StyleClass {
        description: "Coptic default style",
        script_tag: "copt",
        blue_entries: SCRIPT_COPT,
        uni_ranges: RANGES_COPT_UNI,
        non_base_ranges: RANGES_COPT_NONBASE_UNI,
    },
    StyleClass {
        description: "Cypriot default style",
        script_tag: "cprt",
        blue_entries: SCRIPT_CPRT,
        uni_ranges: RANGES_CPRT_UNI,
        non_base_ranges: RANGES_CPRT_NONBASE_UNI,
    },
    StyleClass {
        description: "Cyrillic",
        script_tag: "cyrl",
        blue_entries: SCRIPT_CYRL,
        uni_ranges: RANGES_CYRL_UNI,
        non_base_ranges: RANGES_CYRL_NONBASE_UNI,
    },
    StyleClass {
        description: "Devanagari default style",
        script_tag: "deva",
        blue_entries: SCRIPT_DEVA,
        uni_ranges: RANGES_DEVA_UNI,
        non_base_ranges: RANGES_DEVA_NONBASE_UNI,
    },
    StyleClass {
        description: "Deseret default style",
        script_tag: "dsrt",
        blue_entries: SCRIPT_DSRT,
        uni_ranges: RANGES_DSRT_UNI,
        non_base_ranges: RANGES_DSRT_NONBASE_UNI,
    },
    StyleClass {
        description: "Ethiopic default style",
        script_tag: "ethi",
        blue_entries: SCRIPT_ETHI,
        uni_ranges: RANGES_ETHI_UNI,
        non_base_ranges: RANGES_ETHI_NONBASE_UNI,
    },
    StyleClass {
        description: "Georgian (Mkhedruli) default style",
        script_tag: "geor",
        blue_entries: SCRIPT_GEOR,
        uni_ranges: RANGES_GEOR_UNI,
        non_base_ranges: RANGES_GEOR_NONBASE_UNI,
    },
    StyleClass {
        description: "Georgian (Khutsuri) default style",
        script_tag: "geok",
        blue_entries: SCRIPT_GEOK,
        uni_ranges: RANGES_GEOK_UNI,
        non_base_ranges: RANGES_GEOK_NONBASE_UNI,
    },
    StyleClass {
        description: "Glagolitic default style",
        script_tag: "glag",
        blue_entries: SCRIPT_GLAG,
        uni_ranges: RANGES_GLAG_UNI,
        non_base_ranges: RANGES_GLAG_NONBASE_UNI,
    },
    StyleClass {
        description: "Gothic default style",
        script_tag: "goth",
        blue_entries: SCRIPT_GOTH,
        uni_ranges: RANGES_GOTH_UNI,
        non_base_ranges: RANGES_GOTH_NONBASE_UNI,
    },
    StyleClass {
        description: "Greek",
        script_tag: "grek",
        blue_entries: SCRIPT_GREK,
        uni_ranges: RANGES_GREK_UNI,
        non_base_ranges: RANGES_GREK_NONBASE_UNI,
    },
    StyleClass {
        description: "Gujarati default style",
        script_tag: "gujr",
        blue_entries: SCRIPT_GUJR,
        uni_ranges: RANGES_GUJR_UNI,
        non_base_ranges: RANGES_GUJR_NONBASE_UNI,
    },
    StyleClass {
        description: "Gurmukhi default style",
        script_tag: "guru",
        blue_entries: SCRIPT_GURU,
        uni_ranges: RANGES_GURU_UNI,
        non_base_ranges: RANGES_GURU_NONBASE_UNI,
    },
    StyleClass {
        description: "Hebrew default style",
        script_tag: "hebr",
        blue_entries: SCRIPT_HEBR,
        uni_ranges: RANGES_HEBR_UNI,
        non_base_ranges: RANGES_HEBR_NONBASE_UNI,
    },
    StyleClass {
        description: "Kayah Li default style",
        script_tag: "kali",
        blue_entries: SCRIPT_KALI,
        uni_ranges: RANGES_KALI_UNI,
        non_base_ranges: RANGES_KALI_NONBASE_UNI,
    },
    StyleClass {
        description: "Khmer default style",
        script_tag: "khmr",
        blue_entries: SCRIPT_KHMR,
        uni_ranges: RANGES_KHMR_UNI,
        non_base_ranges: RANGES_KHMR_NONBASE_UNI,
    },
    StyleClass {
        description: "Khmer Symbols default style",
        script_tag: "khms",
        blue_entries: SCRIPT_KHMS,
        uni_ranges: RANGES_KHMS_UNI,
        non_base_ranges: RANGES_KHMS_NONBASE_UNI,
    },
    StyleClass {
        description: "Kannada default style",
        script_tag: "knda",
        blue_entries: SCRIPT_KNDA,
        uni_ranges: RANGES_KNDA_UNI,
        non_base_ranges: RANGES_KNDA_NONBASE_UNI,
    },
    StyleClass {
        description: "Lao default style",
        script_tag: "lao",
        blue_entries: SCRIPT_LAO,
        uni_ranges: RANGES_LAO_UNI,
        non_base_ranges: RANGES_LAO_NONBASE_UNI,
    },
    StyleClass {
        description: "Latin subscript",
        script_tag: "latb",
        blue_entries: SCRIPT_LATB,
        uni_ranges: RANGES_LATB_UNI,
        non_base_ranges: RANGES_LATB_NONBASE_UNI,
    },
    StyleClass {
        description: "Latin superscript",
        script_tag: "latp",
        blue_entries: SCRIPT_LATP,
        uni_ranges: RANGES_LATP_UNI,
        non_base_ranges: RANGES_LATP_NONBASE_UNI,
    },
    StyleClass {
        description: "Latin",
        script_tag: "latn",
        blue_entries: SCRIPT_LATN,
        uni_ranges: RANGES_LATN_UNI,
        non_base_ranges: RANGES_LATN_NONBASE_UNI,
    },
    StyleClass {
        description: "Lisu default style",
        script_tag: "lisu",
        blue_entries: SCRIPT_LISU,
        uni_ranges: RANGES_LISU_UNI,
        non_base_ranges: RANGES_LISU_NONBASE_UNI,
    },
    StyleClass {
        description: "Malayalam default style",
        script_tag: "mlym",
        blue_entries: SCRIPT_MLYM,
        uni_ranges: RANGES_MLYM_UNI,
        non_base_ranges: RANGES_MLYM_NONBASE_UNI,
    },
    StyleClass {
        description: "Medefaidrin default style",
        script_tag: "medf",
        blue_entries: SCRIPT_MEDF,
        uni_ranges: RANGES_MEDF_UNI,
        non_base_ranges: RANGES_MEDF_NONBASE_UNI,
    },
    StyleClass {
        description: "Mongolian default style",
        script_tag: "mong",
        blue_entries: SCRIPT_MONG,
        uni_ranges: RANGES_MONG_UNI,
        non_base_ranges: RANGES_MONG_NONBASE_UNI,
    },
    StyleClass {
        description: "Myanmar default style",
        script_tag: "mymr",
        blue_entries: SCRIPT_MYMR,
        uni_ranges: RANGES_MYMR_UNI,
        non_base_ranges: RANGES_MYMR_NONBASE_UNI,
    },
    StyleClass {
        description: "N'Ko default style",
        script_tag: "nkoo",
        blue_entries: SCRIPT_NKOO,
        uni_ranges: RANGES_NKOO_UNI,
        non_base_ranges: RANGES_NKOO_NONBASE_UNI,
    },
    StyleClass {
        description: "Ol Chiki default style",
        script_tag: "olck",
        blue_entries: SCRIPT_OLCK,
        uni_ranges: RANGES_OLCK_UNI,
        non_base_ranges: RANGES_OLCK_NONBASE_UNI,
    },
    StyleClass {
        description: "Old Turkic default style",
        script_tag: "orkh",
        blue_entries: SCRIPT_ORKH,
        uni_ranges: RANGES_ORKH_UNI,
        non_base_ranges: RANGES_ORKH_NONBASE_UNI,
    },
    StyleClass {
        description: "Osage default style",
        script_tag: "osge",
        blue_entries: SCRIPT_OSGE,
        uni_ranges: RANGES_OSGE_UNI,
        non_base_ranges: RANGES_OSGE_NONBASE_UNI,
    },
    StyleClass {
        description: "Osmanya default style",
        script_tag: "osma",
        blue_entries: SCRIPT_OSMA,
        uni_ranges: RANGES_OSMA_UNI,
        non_base_ranges: RANGES_OSMA_NONBASE_UNI,
    },
    StyleClass {
        description: "Hanifi Rohingya default style",
        script_tag: "rohg",
        blue_entries: SCRIPT_ROHG,
        uni_ranges: RANGES_ROHG_UNI,
        non_base_ranges: RANGES_ROHG_NONBASE_UNI,
    },
    StyleClass {
        description: "Saurashtra default style",
        script_tag: "saur",
        blue_entries: SCRIPT_SAUR,
        uni_ranges: RANGES_SAUR_UNI,
        non_base_ranges: RANGES_SAUR_NONBASE_UNI,
    },
    StyleClass {
        description: "Shavian default style",
        script_tag: "shaw",
        blue_entries: SCRIPT_SHAW,
        uni_ranges: RANGES_SHAW_UNI,
        non_base_ranges: RANGES_SHAW_NONBASE_UNI,
    },
    StyleClass {
        description: "Sinhala default style",
        script_tag: "sinh",
        blue_entries: SCRIPT_SINH,
        uni_ranges: RANGES_SINH_UNI,
        non_base_ranges: RANGES_SINH_NONBASE_UNI,
    },
    StyleClass {
        description: "Sundanese default style",
        script_tag: "sund",
        blue_entries: SCRIPT_SUND,
        uni_ranges: RANGES_SUND_UNI,
        non_base_ranges: RANGES_SUND_NONBASE_UNI,
    },
    StyleClass {
        description: "Tamil default style",
        script_tag: "taml",
        blue_entries: SCRIPT_TAML,
        uni_ranges: RANGES_TAML_UNI,
        non_base_ranges: RANGES_TAML_NONBASE_UNI,
    },
    StyleClass {
        description: "Tai Viet default style",
        script_tag: "tavt",
        blue_entries: SCRIPT_TAVT,
        uni_ranges: RANGES_TAVT_UNI,
        non_base_ranges: RANGES_TAVT_NONBASE_UNI,
    },
    StyleClass {
        description: "Telugu default style",
        script_tag: "telu",
        blue_entries: SCRIPT_TELU,
        uni_ranges: RANGES_TELU_UNI,
        non_base_ranges: RANGES_TELU_NONBASE_UNI,
    },
    StyleClass {
        description: "Tifinagh default style",
        script_tag: "tfng",
        blue_entries: SCRIPT_TFNG,
        uni_ranges: RANGES_TFNG_UNI,
        non_base_ranges: RANGES_TFNG_NONBASE_UNI,
    },
    StyleClass {
        description: "Thai default style",
        script_tag: "thai",
        blue_entries: SCRIPT_THAI,
        uni_ranges: RANGES_THAI_UNI,
        non_base_ranges: RANGES_THAI_NONBASE_UNI,
    },
    StyleClass {
        description: "Vai default style",
        script_tag: "vaii",
        blue_entries: SCRIPT_VAII,
        uni_ranges: RANGES_VAII_UNI,
        non_base_ranges: RANGES_VAII_NONBASE_UNI,
    },
    StyleClass {
        description: "Limbu",
        script_tag: "limb",
        blue_entries: SCRIPT_LATN,
        uni_ranges: RANGES_LIMB_UNI,
        non_base_ranges: RANGES_LIMB_NONBASE_UNI,
    },
    StyleClass {
        description: "Oriya",
        script_tag: "orya",
        blue_entries: SCRIPT_LATN,
        uni_ranges: RANGES_ORYA_UNI,
        non_base_ranges: RANGES_ORYA_NONBASE_UNI,
    },
    StyleClass {
        description: "Syloti Nagri",
        script_tag: "sylo",
        blue_entries: SCRIPT_LATN,
        uni_ranges: RANGES_SYLO_UNI,
        non_base_ranges: RANGES_SYLO_NONBASE_UNI,
    },
    StyleClass {
        description: "Tibetan",
        script_tag: "tibt",
        blue_entries: SCRIPT_LATN,
        uni_ranges: RANGES_TIBT_UNI,
        non_base_ranges: RANGES_TIBT_NONBASE_UNI,
    },
    StyleClass {
        description: "CJKV ideographs default style",
        script_tag: "hani",
        blue_entries: SCRIPT_HANI,
        uni_ranges: RANGES_HANI_UNI,
        non_base_ranges: RANGES_HANI_NONBASE_UNI,
    },
];
pub const STYLE_FALLBACK: usize = 58;
pub const STYLE_UNASSIGNED: usize = usize::MAX;
// ── Per-script standard characters (from afscript.h) ──────────

/// Get the standard character for stem width detection for a script tag.
/// Falls back to 'o' for scripts not in the table.
pub fn standard_char_for_script(tag: &str) -> char {
    match tag {
        "adlm" => '\u{1E90C}',
        "arab" => '\u{0644}',
        "armn" => '\u{057D}',
        "avst" => '\u{10B1A}',
        "bamu" => '\u{A6C1}',
        "beng" => '\u{09E6}',
        "buhd" => '\u{174B}',
        "cakm" => '\u{11124}',
        "cans" => '\u{144C}',
        "cari" => '\u{102AB}',
        "cher" => '\u{13A4}',
        "copt" => '\u{2C9E}',
        "cprt" => '\u{10805}',
        "cyrl" => '\u{043E}',
        "deva" => '\u{0920}',
        "dsrt" => '\u{10404}',
        "ethi" => '\u{12D0}',
        "geok" => '\u{10B6}',
        "geor" => '\u{10D8}',
        "glag" => '\u{2C15}',
        "goth" => '\u{10334}',
        "grek" => '\u{03BF}',
        "gujr" => '\u{0A9F}',
        "guru" => '\u{0A20}',
        "hani" => '\u{7530}',
        "hebr" => '\u{05DD}',
        "kali" => '\u{A90D}',
        "khmr" => '\u{17E0}',
        "khms" => '\u{19E1}',
        "knda" => '\u{0CE6}',
        "lao" => '\u{0ED0}',
        "latb" => '\u{2092}',
        "latn" => '\u{006F}',
        "latp" => '\u{1D52}',
        "limb" => '\u{006F}',
        "lisu" => '\u{A4F3}',
        "medf" => '\u{16E61}',
        "mlym" => '\u{0D20}',
        "mong" => '\u{1842}',
        "mymr" => '\u{101D}',
        "nkoo" => '\u{07CB}',
        "none" => '\u{006F}',
        "olck" => '\u{1C5B}',
        "orkh" => '\u{10C17}',
        "orya" => '\u{006F}',
        "osge" => '\u{104C2}',
        "osma" => '\u{10486}',
        "rohg" => '\u{10D30}',
        "saur" => '\u{A89D}',
        "shaw" => '\u{10474}',
        "sinh" => '\u{0DA7}',
        "sund" => '\u{1BB0}',
        "sylo" => '\u{006F}',
        "taml" => '\u{0BE6}',
        "tavt" => '\u{AA92}',
        "telu" => '\u{0C66}',
        "tfng" => '\u{2D54}',
        "thai" => '\u{0E32}',
        "tibt" => '\u{006F}',
        "vaii" => '\u{A613}',
        _ => 'o',
    }
}
