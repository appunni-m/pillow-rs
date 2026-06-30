// 59 styles, 120 scripts with ranges, 56 blue sets
//! Auto-generated from FreeType afranges.c + afstyles.h — DO NOT EDIT.
use super::blue_strings::*;
#[derive(Debug,Clone,Copy)] pub struct UniRange { pub first: u32, pub last: u32 }
#[derive(Debug,Clone)] pub struct StyleClass { pub description: &'static str, pub script_tag: &'static str, pub blue_entries: &'static [BlueStringEntry], pub uni_ranges: &'static [UniRange], pub non_base_ranges: &'static [UniRange] }

pub static RANGES_ADLM_UNI: &[UniRange] = &[
    UniRange { first: 0x0001E900, last: 0x0001E95F },
];
pub static RANGES_ADLM_NONBASE: &[UniRange] = &[
];
pub static RANGES_ADLM_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0001D944, last: 0x0001E94A },
];
pub static RANGES_ADLM_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_ARAB_UNI: &[UniRange] = &[
    UniRange { first: 0x00000600, last: 0x000006FF },
    UniRange { first: 0x00000750, last: 0x000007FF },
    UniRange { first: 0x00000870, last: 0x0000089F },
    UniRange { first: 0x000008A0, last: 0x000008FF },
    UniRange { first: 0x0000FB50, last: 0x0000FDFF },
    UniRange { first: 0x0000FE70, last: 0x0000FEFF },
    UniRange { first: 0x00010EC0, last: 0x00010EFF },
    UniRange { first: 0x0001EE00, last: 0x0001EEFF },
];
pub static RANGES_ARAB_NONBASE: &[UniRange] = &[
];
pub static RANGES_ARAB_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000600, last: 0x00000605 },
    UniRange { first: 0x00000610, last: 0x0000061A },
    UniRange { first: 0x0000064B, last: 0x0000065F },
    UniRange { first: 0x00000670, last: 0x00000670 },
    UniRange { first: 0x000006D6, last: 0x000006DC },
    UniRange { first: 0x000006DF, last: 0x000006E4 },
    UniRange { first: 0x000006E7, last: 0x000006E8 },
    UniRange { first: 0x000006EA, last: 0x000006ED },
    UniRange { first: 0x00000897, last: 0x0000089F },
    UniRange { first: 0x000008CA, last: 0x000008E1 },
    UniRange { first: 0x000008E3, last: 0x000008FF },
    UniRange { first: 0x0000FBB2, last: 0x0000FBC1 },
    UniRange { first: 0x0000FE70, last: 0x0000FE70 },
    UniRange { first: 0x0000FE72, last: 0x0000FE72 },
    UniRange { first: 0x0000FE74, last: 0x0000FE74 },
    UniRange { first: 0x0000FE76, last: 0x0000FE76 },
    UniRange { first: 0x0000FE78, last: 0x0000FE78 },
    UniRange { first: 0x0000FE7A, last: 0x0000FE7A },
    UniRange { first: 0x0000FE7C, last: 0x0000FE7C },
    UniRange { first: 0x0000FE7E, last: 0x0000FE7E },
    UniRange { first: 0x00010EFD, last: 0x00010EFF },
];
pub static RANGES_ARAB_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_ARMN_UNI: &[UniRange] = &[
    UniRange { first: 0x00000530, last: 0x0000058F },
    UniRange { first: 0x0000FB13, last: 0x0000FB17 },
];
pub static RANGES_ARMN_NONBASE: &[UniRange] = &[
];
pub static RANGES_ARMN_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000559, last: 0x0000055F },
];
pub static RANGES_ARMN_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_AVST_UNI: &[UniRange] = &[
    UniRange { first: 0x00010B00, last: 0x00010B3F },
];
pub static RANGES_AVST_NONBASE: &[UniRange] = &[
];
pub static RANGES_AVST_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00010B39, last: 0x00010B3F },
];
pub static RANGES_AVST_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_BAMU_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A6A0, last: 0x0000A6FF },
    UniRange { first: 0x00016800, last: 0x00016A3F },
];
pub static RANGES_BAMU_NONBASE: &[UniRange] = &[
];
pub static RANGES_BAMU_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A6F0, last: 0x0000A6F1 },
];
pub static RANGES_BAMU_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_BENG_UNI: &[UniRange] = &[
    UniRange { first: 0x00000980, last: 0x000009FF },
];
pub static RANGES_BENG_NONBASE: &[UniRange] = &[
];
pub static RANGES_BENG_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000981, last: 0x00000981 },
    UniRange { first: 0x000009BC, last: 0x000009BC },
    UniRange { first: 0x000009C1, last: 0x000009C4 },
    UniRange { first: 0x000009CD, last: 0x000009CD },
    UniRange { first: 0x000009E2, last: 0x000009E3 },
    UniRange { first: 0x000009FE, last: 0x000009FE },
];
pub static RANGES_BENG_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_BUHD_UNI: &[UniRange] = &[
    UniRange { first: 0x00001740, last: 0x0000175F },
];
pub static RANGES_BUHD_NONBASE: &[UniRange] = &[
];
pub static RANGES_BUHD_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00001752, last: 0x00001753 },
];
pub static RANGES_BUHD_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_CAKM_UNI: &[UniRange] = &[
    UniRange { first: 0x00011100, last: 0x0001114F },
];
pub static RANGES_CAKM_NONBASE: &[UniRange] = &[
];
pub static RANGES_CAKM_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00011100, last: 0x00011102 },
    UniRange { first: 0x00011127, last: 0x00011134 },
    UniRange { first: 0x00011146, last: 0x00011146 },
];
pub static RANGES_CAKM_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_CANS_UNI: &[UniRange] = &[
    UniRange { first: 0x00001400, last: 0x0000167F },
    UniRange { first: 0x000018B0, last: 0x000018FF },
    UniRange { first: 0x00011AB0, last: 0x00011ABF },
];
pub static RANGES_CANS_NONBASE: &[UniRange] = &[
];
pub static RANGES_CANS_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_CANS_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_CARI_UNI: &[UniRange] = &[
    UniRange { first: 0x000102A0, last: 0x000102DF },
];
pub static RANGES_CARI_NONBASE: &[UniRange] = &[
];
pub static RANGES_CARI_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_CARI_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_CHER_UNI: &[UniRange] = &[
    UniRange { first: 0x000013A0, last: 0x000013FF },
    UniRange { first: 0x0000AB70, last: 0x0000ABBF },
];
pub static RANGES_CHER_NONBASE: &[UniRange] = &[
];
pub static RANGES_CHER_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_CHER_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_COPT_UNI: &[UniRange] = &[
    UniRange { first: 0x00002C80, last: 0x00002CFF },
];
pub static RANGES_COPT_NONBASE: &[UniRange] = &[
];
pub static RANGES_COPT_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00002CEF, last: 0x00002CF1 },
];
pub static RANGES_COPT_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_CPRT_UNI: &[UniRange] = &[
    UniRange { first: 0x00010800, last: 0x0001083F },
];
pub static RANGES_CPRT_NONBASE: &[UniRange] = &[
];
pub static RANGES_CPRT_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_CPRT_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_CYRL_UNI: &[UniRange] = &[
    UniRange { first: 0x00000400, last: 0x000004FF },
    UniRange { first: 0x00000500, last: 0x0000052F },
    UniRange { first: 0x00002DE0, last: 0x00002DFF },
    UniRange { first: 0x0000A640, last: 0x0000A69F },
    UniRange { first: 0x00001C80, last: 0x00001C8F },
    UniRange { first: 0x0001E030, last: 0x0001E08F },
];
pub static RANGES_CYRL_NONBASE: &[UniRange] = &[
];
pub static RANGES_CYRL_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000483, last: 0x00000489 },
    UniRange { first: 0x00002DE0, last: 0x00002DFF },
    UniRange { first: 0x0000A66F, last: 0x0000A67F },
    UniRange { first: 0x0000A69E, last: 0x0000A69F },
];
pub static RANGES_CYRL_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_DEVA_UNI: &[UniRange] = &[
    UniRange { first: 0x00000900, last: 0x0000093B },
    UniRange { first: 0x0000093D, last: 0x00000950 },
    UniRange { first: 0x00000953, last: 0x00000963 },
    UniRange { first: 0x00000966, last: 0x0000097F },
    UniRange { first: 0x000020B9, last: 0x000020B9 },
    UniRange { first: 0x0000A8E0, last: 0x0000A8FF },
    UniRange { first: 0x00011B00, last: 0x00011B5F },
];
pub static RANGES_DEVA_NONBASE: &[UniRange] = &[
];
pub static RANGES_DEVA_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000900, last: 0x00000902 },
    UniRange { first: 0x0000093A, last: 0x0000093A },
    UniRange { first: 0x00000941, last: 0x00000948 },
    UniRange { first: 0x0000094D, last: 0x0000094D },
    UniRange { first: 0x00000953, last: 0x00000957 },
    UniRange { first: 0x00000962, last: 0x00000963 },
    UniRange { first: 0x0000A8E0, last: 0x0000A8F1 },
    UniRange { first: 0x0000A8FF, last: 0x0000A8FF },
];
pub static RANGES_DEVA_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_DSRT_UNI: &[UniRange] = &[
    UniRange { first: 0x00010400, last: 0x0001044F },
];
pub static RANGES_DSRT_NONBASE: &[UniRange] = &[
];
pub static RANGES_DSRT_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_DSRT_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_ETHI_UNI: &[UniRange] = &[
    UniRange { first: 0x00001200, last: 0x0000137F },
    UniRange { first: 0x00001380, last: 0x0000139F },
    UniRange { first: 0x00002D80, last: 0x00002DDF },
    UniRange { first: 0x0000AB00, last: 0x0000AB2F },
    UniRange { first: 0x0001E7E0, last: 0x0001E7FF },
];
pub static RANGES_ETHI_NONBASE: &[UniRange] = &[
];
pub static RANGES_ETHI_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000135D, last: 0x0000135F },
];
pub static RANGES_ETHI_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_GEOK_UNI: &[UniRange] = &[
    UniRange { first: 0x000010A0, last: 0x000010CD },
    UniRange { first: 0x00002D00, last: 0x00002D2D },
];
pub static RANGES_GEOK_NONBASE: &[UniRange] = &[
];
pub static RANGES_GEOK_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_GEOK_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_GEOR_UNI: &[UniRange] = &[
    UniRange { first: 0x000010D0, last: 0x000010FF },
    UniRange { first: 0x00001C90, last: 0x00001CBF },
];
pub static RANGES_GEOR_NONBASE: &[UniRange] = &[
];
pub static RANGES_GEOR_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_GEOR_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_GLAG_UNI: &[UniRange] = &[
    UniRange { first: 0x00002C00, last: 0x00002C5F },
    UniRange { first: 0x0001E000, last: 0x0001E02F },
];
pub static RANGES_GLAG_NONBASE: &[UniRange] = &[
];
pub static RANGES_GLAG_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0001E000, last: 0x0001E02F },
];
pub static RANGES_GLAG_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_GOTH_UNI: &[UniRange] = &[
    UniRange { first: 0x00010330, last: 0x0001034F },
];
pub static RANGES_GOTH_NONBASE: &[UniRange] = &[
];
pub static RANGES_GOTH_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_GOTH_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_GREK_UNI: &[UniRange] = &[
    UniRange { first: 0x00000370, last: 0x000003FF },
    UniRange { first: 0x00001F00, last: 0x00001FFF },
];
pub static RANGES_GREK_NONBASE: &[UniRange] = &[
];
pub static RANGES_GREK_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000037A, last: 0x0000037A },
    UniRange { first: 0x00000384, last: 0x00000385 },
    UniRange { first: 0x00001FBD, last: 0x00001FC1 },
    UniRange { first: 0x00001FCD, last: 0x00001FCF },
    UniRange { first: 0x00001FDD, last: 0x00001FDF },
    UniRange { first: 0x00001FED, last: 0x00001FEF },
    UniRange { first: 0x00001FFD, last: 0x00001FFE },
];
pub static RANGES_GREK_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_GUJR_UNI: &[UniRange] = &[
    UniRange { first: 0x00000A80, last: 0x00000AFF },
];
pub static RANGES_GUJR_NONBASE: &[UniRange] = &[
];
pub static RANGES_GUJR_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000A81, last: 0x00000A82 },
    UniRange { first: 0x00000ABC, last: 0x00000ABC },
    UniRange { first: 0x00000AC1, last: 0x00000AC8 },
    UniRange { first: 0x00000ACD, last: 0x00000ACD },
    UniRange { first: 0x00000AE2, last: 0x00000AE3 },
    UniRange { first: 0x00000AFA, last: 0x00000AFF },
];
pub static RANGES_GUJR_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_GURU_UNI: &[UniRange] = &[
    UniRange { first: 0x00000A00, last: 0x00000A7F },
];
pub static RANGES_GURU_NONBASE: &[UniRange] = &[
];
pub static RANGES_GURU_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000A01, last: 0x00000A02 },
    UniRange { first: 0x00000A3C, last: 0x00000A3C },
    UniRange { first: 0x00000A41, last: 0x00000A51 },
    UniRange { first: 0x00000A70, last: 0x00000A71 },
    UniRange { first: 0x00000A75, last: 0x00000A75 },
];
pub static RANGES_GURU_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_HANI_UNI: &[UniRange] = &[
    UniRange { first: 0x00001100, last: 0x000011FF },
    UniRange { first: 0x00002E80, last: 0x00002EFF },
    UniRange { first: 0x00002F00, last: 0x00002FDF },
    UniRange { first: 0x00002FF0, last: 0x00002FFF },
    UniRange { first: 0x00003000, last: 0x0000303F },
    UniRange { first: 0x00003040, last: 0x0000309F },
    UniRange { first: 0x000030A0, last: 0x000030FF },
    UniRange { first: 0x00003100, last: 0x0000312F },
    UniRange { first: 0x00003130, last: 0x0000318F },
    UniRange { first: 0x00003190, last: 0x0000319F },
    UniRange { first: 0x000031A0, last: 0x000031BF },
    UniRange { first: 0x000031C0, last: 0x000031EF },
    UniRange { first: 0x000031F0, last: 0x000031FF },
    UniRange { first: 0x00003300, last: 0x000033FF },
    UniRange { first: 0x00003400, last: 0x00004DBF },
    UniRange { first: 0x00004DC0, last: 0x00004DFF },
    UniRange { first: 0x00004E00, last: 0x00009FFF },
    UniRange { first: 0x0000A960, last: 0x0000A97F },
    UniRange { first: 0x0000AC00, last: 0x0000D7AF },
    UniRange { first: 0x0000D7B0, last: 0x0000D7FF },
    UniRange { first: 0x0000F900, last: 0x0000FAFF },
    UniRange { first: 0x0000FE10, last: 0x0000FE1F },
    UniRange { first: 0x0000FE30, last: 0x0000FE4F },
    UniRange { first: 0x0000FF00, last: 0x0000FFEF },
    UniRange { first: 0x0001AFF0, last: 0x0001AFFF },
    UniRange { first: 0x0001B000, last: 0x0001B0FF },
    UniRange { first: 0x0001B100, last: 0x0001B12F },
    UniRange { first: 0x0001B130, last: 0x0001B16F },
    UniRange { first: 0x0001D300, last: 0x0001D35F },
    UniRange { first: 0x00020000, last: 0x0002A6DF },
    UniRange { first: 0x0002A700, last: 0x0002B73F },
    UniRange { first: 0x0002B740, last: 0x0002B81F },
    UniRange { first: 0x0002B820, last: 0x0002CEAF },
    UniRange { first: 0x0002CEB0, last: 0x0002EBEF },
    UniRange { first: 0x0002EBF0, last: 0x0002EE5D },
    UniRange { first: 0x0002F800, last: 0x0002FA1F },
    UniRange { first: 0x00030000, last: 0x0003134A },
    UniRange { first: 0x00031350, last: 0x000323AF },
    UniRange { first: 0x000323B0, last: 0x00033479 },
];
pub static RANGES_HANI_NONBASE: &[UniRange] = &[
];
pub static RANGES_HANI_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000302A, last: 0x0000302F },
    UniRange { first: 0x00003190, last: 0x0000319F },
];
pub static RANGES_HANI_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_HEBR_UNI: &[UniRange] = &[
    UniRange { first: 0x00000590, last: 0x000005FF },
    UniRange { first: 0x0000FB1D, last: 0x0000FB4F },
];
pub static RANGES_HEBR_NONBASE: &[UniRange] = &[
];
pub static RANGES_HEBR_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000591, last: 0x000005BF },
    UniRange { first: 0x000005C1, last: 0x000005C2 },
    UniRange { first: 0x000005C4, last: 0x000005C5 },
    UniRange { first: 0x000005C7, last: 0x000005C7 },
    UniRange { first: 0x0000FB1E, last: 0x0000FB1E },
];
pub static RANGES_HEBR_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_KALI_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A900, last: 0x0000A92F },
];
pub static RANGES_KALI_NONBASE: &[UniRange] = &[
];
pub static RANGES_KALI_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A926, last: 0x0000A92D },
];
pub static RANGES_KALI_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_KHMR_UNI: &[UniRange] = &[
    UniRange { first: 0x00001780, last: 0x000017FF },
];
pub static RANGES_KHMR_NONBASE: &[UniRange] = &[
];
pub static RANGES_KHMR_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x000017B7, last: 0x000017BD },
    UniRange { first: 0x000017C6, last: 0x000017C6 },
    UniRange { first: 0x000017C9, last: 0x000017D3 },
    UniRange { first: 0x000017DD, last: 0x000017DD },
];
pub static RANGES_KHMR_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_KHMS_UNI: &[UniRange] = &[
    UniRange { first: 0x000019E0, last: 0x000019FF },
];
pub static RANGES_KHMS_NONBASE: &[UniRange] = &[
];
pub static RANGES_KHMS_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_KHMS_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_KNDA_UNI: &[UniRange] = &[
    UniRange { first: 0x00000C80, last: 0x00000CFF },
];
pub static RANGES_KNDA_NONBASE: &[UniRange] = &[
];
pub static RANGES_KNDA_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000C81, last: 0x00000C81 },
    UniRange { first: 0x00000CBC, last: 0x00000CBC },
    UniRange { first: 0x00000CBF, last: 0x00000CBF },
    UniRange { first: 0x00000CC6, last: 0x00000CC6 },
    UniRange { first: 0x00000CCC, last: 0x00000CCD },
    UniRange { first: 0x00000CE2, last: 0x00000CE3 },
];
pub static RANGES_KNDA_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_LAO_UNI: &[UniRange] = &[
    UniRange { first: 0x00000E80, last: 0x00000EFF },
];
pub static RANGES_LAO_NONBASE: &[UniRange] = &[
];
pub static RANGES_LAO_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000EB1, last: 0x00000EB1 },
    UniRange { first: 0x00000EB4, last: 0x00000EBC },
    UniRange { first: 0x00000EC8, last: 0x00000ECE },
];
pub static RANGES_LAO_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_LATB_UNI: &[UniRange] = &[
    UniRange { first: 0x00001D62, last: 0x00001D6A },
    UniRange { first: 0x00002080, last: 0x0000209C },
    UniRange { first: 0x00002C7C, last: 0x00002C7C },
];
pub static RANGES_LATB_NONBASE: &[UniRange] = &[
];
pub static RANGES_LATB_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_LATB_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_LATN_UNI: &[UniRange] = &[
    UniRange { first: 0x00000020, last: 0x0000007F },
    UniRange { first: 0x000000A0, last: 0x000000A9 },
    UniRange { first: 0x000000AB, last: 0x000000B1 },
    UniRange { first: 0x000000B4, last: 0x000000B8 },
    UniRange { first: 0x000000BB, last: 0x000000FF },
    UniRange { first: 0x00000100, last: 0x0000017F },
    UniRange { first: 0x00000180, last: 0x0000024F },
    UniRange { first: 0x00000250, last: 0x000002AF },
    UniRange { first: 0x000002B9, last: 0x000002DF },
    UniRange { first: 0x000002E5, last: 0x000002FF },
    UniRange { first: 0x00000300, last: 0x0000036F },
    UniRange { first: 0x00001AB0, last: 0x00001ABE },
    UniRange { first: 0x00001D00, last: 0x00001D2B },
    UniRange { first: 0x00001D6B, last: 0x00001D77 },
    UniRange { first: 0x00001D79, last: 0x00001D7F },
    UniRange { first: 0x00001D80, last: 0x00001D9A },
    UniRange { first: 0x00001DC0, last: 0x00001DFF },
    UniRange { first: 0x00001E00, last: 0x00001EFF },
    UniRange { first: 0x00002000, last: 0x0000206F },
    UniRange { first: 0x000020A0, last: 0x000020B8 },
    UniRange { first: 0x000020BA, last: 0x000020CF },
    UniRange { first: 0x00002150, last: 0x0000218F },
    UniRange { first: 0x00002C60, last: 0x00002C7B },
    UniRange { first: 0x00002C7E, last: 0x00002C7F },
    UniRange { first: 0x00002E00, last: 0x00002E7F },
    UniRange { first: 0x0000A720, last: 0x0000A76F },
    UniRange { first: 0x0000A771, last: 0x0000A7F0 },
    UniRange { first: 0x0000A7F2, last: 0x0000A7F7 },
    UniRange { first: 0x0000A7FA, last: 0x0000A7FF },
    UniRange { first: 0x0000AB30, last: 0x0000AB5B },
    UniRange { first: 0x0000AB60, last: 0x0000AB68 },
    UniRange { first: 0x0000AB6A, last: 0x0000AB6F },
    UniRange { first: 0x0000FB00, last: 0x0000FB06 },
    UniRange { first: 0x0001D400, last: 0x0001D7FF },
    UniRange { first: 0x0001DF00, last: 0x0001DFFF },
];
pub static RANGES_LATN_NONBASE: &[UniRange] = &[
];
pub static RANGES_LATN_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000005E, last: 0x00000060 },
    UniRange { first: 0x0000007E, last: 0x0000007E },
    UniRange { first: 0x000000A8, last: 0x000000A9 },
    UniRange { first: 0x000000AE, last: 0x000000B0 },
    UniRange { first: 0x000000B4, last: 0x000000B4 },
    UniRange { first: 0x000000B8, last: 0x000000B8 },
    UniRange { first: 0x000000BC, last: 0x000000BE },
    UniRange { first: 0x000002B9, last: 0x000002DF },
    UniRange { first: 0x000002E5, last: 0x000002FF },
    UniRange { first: 0x00000300, last: 0x0000036F },
    UniRange { first: 0x00001AB0, last: 0x00001AEB },
    UniRange { first: 0x00001DC0, last: 0x00001DFF },
    UniRange { first: 0x00002017, last: 0x00002017 },
    UniRange { first: 0x0000203E, last: 0x0000203E },
    UniRange { first: 0x0000A788, last: 0x0000A788 },
    UniRange { first: 0x0000A7F8, last: 0x0000A7FA },
];
pub static RANGES_LATN_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_LATP_UNI: &[UniRange] = &[
    UniRange { first: 0x000000AA, last: 0x000000AA },
    UniRange { first: 0x000000B2, last: 0x000000B3 },
    UniRange { first: 0x000000B9, last: 0x000000BA },
    UniRange { first: 0x000002B0, last: 0x000002B8 },
    UniRange { first: 0x000002E0, last: 0x000002E4 },
    UniRange { first: 0x00001D2C, last: 0x00001D61 },
    UniRange { first: 0x00001D78, last: 0x00001D78 },
    UniRange { first: 0x00001D9B, last: 0x00001DBF },
    UniRange { first: 0x00002070, last: 0x0000207F },
    UniRange { first: 0x00002C7D, last: 0x00002C7D },
    UniRange { first: 0x0000A770, last: 0x0000A770 },
    UniRange { first: 0x0000A7F1, last: 0x0000A7F1 },
    UniRange { first: 0x0000A7F8, last: 0x0000A7F9 },
    UniRange { first: 0x0000AB5C, last: 0x0000AB5F },
    UniRange { first: 0x0000AB69, last: 0x0000AB69 },
    UniRange { first: 0x00010780, last: 0x000107FB },
];
pub static RANGES_LATP_NONBASE: &[UniRange] = &[
];
pub static RANGES_LATP_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_LATP_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_LIMB_UNI: &[UniRange] = &[
    UniRange { first: 0x00001900, last: 0x0000194F },
];
pub static RANGES_LIMB_NONBASE: &[UniRange] = &[
];
pub static RANGES_LIMB_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00001920, last: 0x00001922 },
    UniRange { first: 0x00001927, last: 0x00001934 },
    UniRange { first: 0x00001937, last: 0x0000193B },
];
pub static RANGES_LIMB_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_LISU_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A4D0, last: 0x0000A4FF },
    UniRange { first: 0x00011FB0, last: 0x00011FBF },
];
pub static RANGES_LISU_NONBASE: &[UniRange] = &[
];
pub static RANGES_LISU_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_LISU_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_MEDF_UNI: &[UniRange] = &[
    UniRange { first: 0x00016E40, last: 0x00016E9F },
];
pub static RANGES_MEDF_NONBASE: &[UniRange] = &[
];
pub static RANGES_MEDF_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_MEDF_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_MLYM_UNI: &[UniRange] = &[
    UniRange { first: 0x00000D00, last: 0x00000D7F },
];
pub static RANGES_MLYM_NONBASE: &[UniRange] = &[
];
pub static RANGES_MLYM_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000D00, last: 0x00000D01 },
    UniRange { first: 0x00000D3B, last: 0x00000D3C },
    UniRange { first: 0x00000D4D, last: 0x00000D4E },
    UniRange { first: 0x00000D62, last: 0x00000D63 },
];
pub static RANGES_MLYM_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_MONG_UNI: &[UniRange] = &[
    UniRange { first: 0x00001800, last: 0x000018AF },
    UniRange { first: 0x00011660, last: 0x0001167F },
];
pub static RANGES_MONG_NONBASE: &[UniRange] = &[
];
pub static RANGES_MONG_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00001885, last: 0x00001886 },
    UniRange { first: 0x000018A9, last: 0x000018A9 },
];
pub static RANGES_MONG_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_MYMR_UNI: &[UniRange] = &[
    UniRange { first: 0x00001000, last: 0x0000109F },
    UniRange { first: 0x0000A9E0, last: 0x0000A9FF },
    UniRange { first: 0x0000AA60, last: 0x0000AA7F },
    UniRange { first: 0x000116D0, last: 0x000116FF },
];
pub static RANGES_MYMR_NONBASE: &[UniRange] = &[
];
pub static RANGES_MYMR_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000102D, last: 0x00001030 },
    UniRange { first: 0x00001032, last: 0x00001037 },
    UniRange { first: 0x0000103A, last: 0x0000103A },
    UniRange { first: 0x0000103D, last: 0x0000103E },
    UniRange { first: 0x00001058, last: 0x00001059 },
    UniRange { first: 0x0000105E, last: 0x00001060 },
    UniRange { first: 0x00001071, last: 0x00001074 },
    UniRange { first: 0x00001082, last: 0x00001082 },
    UniRange { first: 0x00001085, last: 0x00001086 },
    UniRange { first: 0x0000108D, last: 0x0000108D },
    UniRange { first: 0x0000A9E5, last: 0x0000A9E5 },
    UniRange { first: 0x0000AA7C, last: 0x0000AA7C },
];
pub static RANGES_MYMR_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_NKOO_UNI: &[UniRange] = &[
    UniRange { first: 0x000007C0, last: 0x000007FF },
];
pub static RANGES_NKOO_NONBASE: &[UniRange] = &[
];
pub static RANGES_NKOO_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x000007EB, last: 0x000007F5 },
    UniRange { first: 0x000007FD, last: 0x000007FD },
];
pub static RANGES_NKOO_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_NONE_UNI: &[UniRange] = &[
];
pub static RANGES_NONE_NONBASE: &[UniRange] = &[
];
pub static RANGES_NONE_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_NONE_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_OLCK_UNI: &[UniRange] = &[
    UniRange { first: 0x00001C50, last: 0x00001C7F },
];
pub static RANGES_OLCK_NONBASE: &[UniRange] = &[
];
pub static RANGES_OLCK_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_OLCK_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_ORKH_UNI: &[UniRange] = &[
    UniRange { first: 0x00010C00, last: 0x00010C4F },
];
pub static RANGES_ORKH_NONBASE: &[UniRange] = &[
];
pub static RANGES_ORKH_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_ORKH_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_ORYA_UNI: &[UniRange] = &[
    UniRange { first: 0x00000B00, last: 0x00000B7F },
];
pub static RANGES_ORYA_NONBASE: &[UniRange] = &[
];
pub static RANGES_ORYA_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000B01, last: 0x00000B02 },
    UniRange { first: 0x00000B3C, last: 0x00000B3C },
    UniRange { first: 0x00000B3F, last: 0x00000B3F },
    UniRange { first: 0x00000B41, last: 0x00000B44 },
    UniRange { first: 0x00000B4D, last: 0x00000B56 },
    UniRange { first: 0x00000B62, last: 0x00000B63 },
];
pub static RANGES_ORYA_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_OSGE_UNI: &[UniRange] = &[
    UniRange { first: 0x000104B0, last: 0x000104FF },
];
pub static RANGES_OSGE_NONBASE: &[UniRange] = &[
];
pub static RANGES_OSGE_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_OSGE_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_OSMA_UNI: &[UniRange] = &[
    UniRange { first: 0x00010480, last: 0x000104AF },
];
pub static RANGES_OSMA_NONBASE: &[UniRange] = &[
];
pub static RANGES_OSMA_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_OSMA_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_ROHG_UNI: &[UniRange] = &[
    UniRange { first: 0x00010D00, last: 0x00010D3F },
];
pub static RANGES_ROHG_NONBASE: &[UniRange] = &[
];
pub static RANGES_ROHG_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_ROHG_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_SAUR_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A880, last: 0x0000A8DF },
];
pub static RANGES_SAUR_NONBASE: &[UniRange] = &[
];
pub static RANGES_SAUR_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A880, last: 0x0000A881 },
    UniRange { first: 0x0000A8B4, last: 0x0000A8C5 },
];
pub static RANGES_SAUR_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_SHAW_UNI: &[UniRange] = &[
    UniRange { first: 0x00010450, last: 0x0001047F },
];
pub static RANGES_SHAW_NONBASE: &[UniRange] = &[
];
pub static RANGES_SHAW_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_SHAW_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_SINH_UNI: &[UniRange] = &[
    UniRange { first: 0x00000D80, last: 0x00000DFF },
];
pub static RANGES_SINH_NONBASE: &[UniRange] = &[
];
pub static RANGES_SINH_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000D81, last: 0x00000D81 },
    UniRange { first: 0x00000DCA, last: 0x00000DCA },
    UniRange { first: 0x00000DD2, last: 0x00000DD6 },
];
pub static RANGES_SINH_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_SUND_UNI: &[UniRange] = &[
    UniRange { first: 0x00001B80, last: 0x00001BBF },
    UniRange { first: 0x00001CC0, last: 0x00001CCF },
];
pub static RANGES_SUND_NONBASE: &[UniRange] = &[
];
pub static RANGES_SUND_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00001B80, last: 0x00001B82 },
    UniRange { first: 0x00001BA1, last: 0x00001BAD },
];
pub static RANGES_SUND_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_SYLO_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A800, last: 0x0000A82F },
];
pub static RANGES_SYLO_NONBASE: &[UniRange] = &[
];
pub static RANGES_SYLO_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A802, last: 0x0000A802 },
    UniRange { first: 0x0000A806, last: 0x0000A806 },
    UniRange { first: 0x0000A80B, last: 0x0000A80B },
    UniRange { first: 0x0000A825, last: 0x0000A826 },
    UniRange { first: 0x0000A82C, last: 0x0000A82C },
];
pub static RANGES_SYLO_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_TAML_UNI: &[UniRange] = &[
    UniRange { first: 0x00000B80, last: 0x00000BFF },
    UniRange { first: 0x00011FC0, last: 0x00011FFF },
];
pub static RANGES_TAML_NONBASE: &[UniRange] = &[
];
pub static RANGES_TAML_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000B82, last: 0x00000B82 },
    UniRange { first: 0x00000BC0, last: 0x00000BC2 },
    UniRange { first: 0x00000BCD, last: 0x00000BCD },
];
pub static RANGES_TAML_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_TAVT_UNI: &[UniRange] = &[
    UniRange { first: 0x0000AA80, last: 0x0000AADF },
];
pub static RANGES_TAVT_NONBASE: &[UniRange] = &[
];
pub static RANGES_TAVT_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x0000AAB0, last: 0x0000AAB0 },
    UniRange { first: 0x0000AAB2, last: 0x0000AAB4 },
    UniRange { first: 0x0000AAB7, last: 0x0000AAB8 },
    UniRange { first: 0x0000AABE, last: 0x0000AABF },
    UniRange { first: 0x0000AAC1, last: 0x0000AAC1 },
];
pub static RANGES_TAVT_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_TELU_UNI: &[UniRange] = &[
    UniRange { first: 0x00000C00, last: 0x00000C7F },
];
pub static RANGES_TELU_NONBASE: &[UniRange] = &[
];
pub static RANGES_TELU_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000C00, last: 0x00000C00 },
    UniRange { first: 0x00000C04, last: 0x00000C04 },
    UniRange { first: 0x00000C3C, last: 0x00000C3C },
    UniRange { first: 0x00000C3E, last: 0x00000C40 },
    UniRange { first: 0x00000C46, last: 0x00000C56 },
    UniRange { first: 0x00000C62, last: 0x00000C63 },
];
pub static RANGES_TELU_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_TFNG_UNI: &[UniRange] = &[
    UniRange { first: 0x00002D30, last: 0x00002D7F },
];
pub static RANGES_TFNG_NONBASE: &[UniRange] = &[
];
pub static RANGES_TFNG_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_TFNG_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_THAI_UNI: &[UniRange] = &[
    UniRange { first: 0x00000E00, last: 0x00000E7F },
];
pub static RANGES_THAI_NONBASE: &[UniRange] = &[
];
pub static RANGES_THAI_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000E31, last: 0x00000E31 },
    UniRange { first: 0x00000E34, last: 0x00000E3A },
    UniRange { first: 0x00000E47, last: 0x00000E4E },
];
pub static RANGES_THAI_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_TIBT_UNI: &[UniRange] = &[
    UniRange { first: 0x00000F00, last: 0x00000FFF },
];
pub static RANGES_TIBT_NONBASE: &[UniRange] = &[
];
pub static RANGES_TIBT_NONBASE_UNI: &[UniRange] = &[
    UniRange { first: 0x00000F18, last: 0x00000F19 },
    UniRange { first: 0x00000F35, last: 0x00000F35 },
    UniRange { first: 0x00000F37, last: 0x00000F37 },
    UniRange { first: 0x00000F39, last: 0x00000F39 },
    UniRange { first: 0x00000F3E, last: 0x00000F3F },
    UniRange { first: 0x00000F71, last: 0x00000F7E },
    UniRange { first: 0x00000F80, last: 0x00000F84 },
    UniRange { first: 0x00000F86, last: 0x00000F87 },
    UniRange { first: 0x00000F8D, last: 0x00000FBC },
];
pub static RANGES_TIBT_NONBASE_NONBASE: &[UniRange] = &[
];
pub static RANGES_VAII_UNI: &[UniRange] = &[
    UniRange { first: 0x0000A500, last: 0x0000A63F },
];
pub static RANGES_VAII_NONBASE: &[UniRange] = &[
];
pub static RANGES_VAII_NONBASE_UNI: &[UniRange] = &[
];
pub static RANGES_VAII_NONBASE_NONBASE: &[UniRange] = &[
];
/// Coverage scan order (matches afstyles.h). First match wins.
pub static STYLE_TABLE: &[StyleClass] = &[
    StyleClass { description: "Adlam default style", script_tag: "adlm",
        blue_entries: SCRIPT_ADLM, uni_ranges: RANGES_ADLM_UNI,
        non_base_ranges: RANGES_ADLM_NONBASE },
    StyleClass { description: "Arabic default style", script_tag: "arab",
        blue_entries: SCRIPT_ARAB, uni_ranges: RANGES_ARAB_UNI,
        non_base_ranges: RANGES_ARAB_NONBASE },
    StyleClass { description: "Armenian default style", script_tag: "armn",
        blue_entries: SCRIPT_ARMN, uni_ranges: RANGES_ARMN_UNI,
        non_base_ranges: RANGES_ARMN_NONBASE },
    StyleClass { description: "Avestan default style", script_tag: "avst",
        blue_entries: SCRIPT_AVST, uni_ranges: RANGES_AVST_UNI,
        non_base_ranges: RANGES_AVST_NONBASE },
    StyleClass { description: "Bamum default style", script_tag: "bamu",
        blue_entries: SCRIPT_BAMU, uni_ranges: RANGES_BAMU_UNI,
        non_base_ranges: RANGES_BAMU_NONBASE },
    StyleClass { description: "Bengali default style", script_tag: "beng",
        blue_entries: SCRIPT_BENG, uni_ranges: RANGES_BENG_UNI,
        non_base_ranges: RANGES_BENG_NONBASE },
    StyleClass { description: "Buhid default style", script_tag: "buhd",
        blue_entries: SCRIPT_BUHD, uni_ranges: RANGES_BUHD_UNI,
        non_base_ranges: RANGES_BUHD_NONBASE },
    StyleClass { description: "Chakma default style", script_tag: "cakm",
        blue_entries: SCRIPT_CAKM, uni_ranges: RANGES_CAKM_UNI,
        non_base_ranges: RANGES_CAKM_NONBASE },
    StyleClass { description: "Canadian Syllabics default style", script_tag: "cans",
        blue_entries: SCRIPT_CANS, uni_ranges: RANGES_CANS_UNI,
        non_base_ranges: RANGES_CANS_NONBASE },
    StyleClass { description: "Carian default style", script_tag: "cari",
        blue_entries: SCRIPT_CARI, uni_ranges: RANGES_CARI_UNI,
        non_base_ranges: RANGES_CARI_NONBASE },
    StyleClass { description: "Cherokee default style", script_tag: "cher",
        blue_entries: SCRIPT_CHER, uni_ranges: RANGES_CHER_UNI,
        non_base_ranges: RANGES_CHER_NONBASE },
    StyleClass { description: "Coptic default style", script_tag: "copt",
        blue_entries: SCRIPT_COPT, uni_ranges: RANGES_COPT_UNI,
        non_base_ranges: RANGES_COPT_NONBASE },
    StyleClass { description: "Cypriot default style", script_tag: "cprt",
        blue_entries: SCRIPT_CPRT, uni_ranges: RANGES_CPRT_UNI,
        non_base_ranges: RANGES_CPRT_NONBASE },
    StyleClass { description: "Cyrillic", script_tag: "cyrl",
        blue_entries: SCRIPT_CYRL, uni_ranges: RANGES_CYRL_UNI,
        non_base_ranges: RANGES_CYRL_NONBASE },
    StyleClass { description: "Devanagari default style", script_tag: "deva",
        blue_entries: SCRIPT_DEVA, uni_ranges: RANGES_DEVA_UNI,
        non_base_ranges: RANGES_DEVA_NONBASE },
    StyleClass { description: "Deseret default style", script_tag: "dsrt",
        blue_entries: SCRIPT_DSRT, uni_ranges: RANGES_DSRT_UNI,
        non_base_ranges: RANGES_DSRT_NONBASE },
    StyleClass { description: "Ethiopic default style", script_tag: "ethi",
        blue_entries: SCRIPT_ETHI, uni_ranges: RANGES_ETHI_UNI,
        non_base_ranges: RANGES_ETHI_NONBASE },
    StyleClass { description: "Georgian (Mkhedruli) default style", script_tag: "geor",
        blue_entries: SCRIPT_GEOR, uni_ranges: RANGES_GEOR_UNI,
        non_base_ranges: RANGES_GEOR_NONBASE },
    StyleClass { description: "Georgian (Khutsuri) default style", script_tag: "geok",
        blue_entries: SCRIPT_GEOK, uni_ranges: RANGES_GEOK_UNI,
        non_base_ranges: RANGES_GEOK_NONBASE },
    StyleClass { description: "Glagolitic default style", script_tag: "glag",
        blue_entries: SCRIPT_GLAG, uni_ranges: RANGES_GLAG_UNI,
        non_base_ranges: RANGES_GLAG_NONBASE },
    StyleClass { description: "Gothic default style", script_tag: "goth",
        blue_entries: SCRIPT_GOTH, uni_ranges: RANGES_GOTH_UNI,
        non_base_ranges: RANGES_GOTH_NONBASE },
    StyleClass { description: "Greek", script_tag: "grek",
        blue_entries: SCRIPT_GREK, uni_ranges: RANGES_GREK_UNI,
        non_base_ranges: RANGES_GREK_NONBASE },
    StyleClass { description: "Gujarati default style", script_tag: "gujr",
        blue_entries: SCRIPT_GUJR, uni_ranges: RANGES_GUJR_UNI,
        non_base_ranges: RANGES_GUJR_NONBASE },
    StyleClass { description: "Gurmukhi default style", script_tag: "guru",
        blue_entries: SCRIPT_GURU, uni_ranges: RANGES_GURU_UNI,
        non_base_ranges: RANGES_GURU_NONBASE },
    StyleClass { description: "Hebrew default style", script_tag: "hebr",
        blue_entries: SCRIPT_HEBR, uni_ranges: RANGES_HEBR_UNI,
        non_base_ranges: RANGES_HEBR_NONBASE },
    StyleClass { description: "Kayah Li default style", script_tag: "kali",
        blue_entries: SCRIPT_KALI, uni_ranges: RANGES_KALI_UNI,
        non_base_ranges: RANGES_KALI_NONBASE },
    StyleClass { description: "Khmer default style", script_tag: "khmr",
        blue_entries: SCRIPT_KHMR, uni_ranges: RANGES_KHMR_UNI,
        non_base_ranges: RANGES_KHMR_NONBASE },
    StyleClass { description: "Khmer Symbols default style", script_tag: "khms",
        blue_entries: SCRIPT_KHMS, uni_ranges: RANGES_KHMS_UNI,
        non_base_ranges: RANGES_KHMS_NONBASE },
    StyleClass { description: "Kannada default style", script_tag: "knda",
        blue_entries: SCRIPT_KNDA, uni_ranges: RANGES_KNDA_UNI,
        non_base_ranges: RANGES_KNDA_NONBASE },
    StyleClass { description: "Lao default style", script_tag: "lao",
        blue_entries: SCRIPT_LAO, uni_ranges: RANGES_LAO_UNI,
        non_base_ranges: RANGES_LAO_NONBASE },
    StyleClass { description: "Latin", script_tag: "latn",
        blue_entries: SCRIPT_LATN, uni_ranges: RANGES_LATN_UNI,
        non_base_ranges: RANGES_LATN_NONBASE },
    StyleClass { description: "Latin subscript fallback default style", script_tag: "latb",
        blue_entries: SCRIPT_LATB, uni_ranges: RANGES_LATB_UNI,
        non_base_ranges: RANGES_LATB_NONBASE },
    StyleClass { description: "Latin superscript fallback default style", script_tag: "latp",
        blue_entries: SCRIPT_LATP, uni_ranges: RANGES_LATP_UNI,
        non_base_ranges: RANGES_LATP_NONBASE },
    StyleClass { description: "Lisu default style", script_tag: "lisu",
        blue_entries: SCRIPT_LISU, uni_ranges: RANGES_LISU_UNI,
        non_base_ranges: RANGES_LISU_NONBASE },
    StyleClass { description: "Malayalam default style", script_tag: "mlym",
        blue_entries: SCRIPT_MLYM, uni_ranges: RANGES_MLYM_UNI,
        non_base_ranges: RANGES_MLYM_NONBASE },
    StyleClass { description: "Medefaidrin default style", script_tag: "medf",
        blue_entries: SCRIPT_MEDF, uni_ranges: RANGES_MEDF_UNI,
        non_base_ranges: RANGES_MEDF_NONBASE },
    StyleClass { description: "Mongolian default style", script_tag: "mong",
        blue_entries: SCRIPT_MONG, uni_ranges: RANGES_MONG_UNI,
        non_base_ranges: RANGES_MONG_NONBASE },
    StyleClass { description: "Myanmar default style", script_tag: "mymr",
        blue_entries: SCRIPT_MYMR, uni_ranges: RANGES_MYMR_UNI,
        non_base_ranges: RANGES_MYMR_NONBASE },
    StyleClass { description: "N'Ko default style", script_tag: "nkoo",
        blue_entries: SCRIPT_NKOO, uni_ranges: RANGES_NKOO_UNI,
        non_base_ranges: RANGES_NKOO_NONBASE },
    StyleClass { description: "Ol Chiki default style", script_tag: "olck",
        blue_entries: SCRIPT_OLCK, uni_ranges: RANGES_OLCK_UNI,
        non_base_ranges: RANGES_OLCK_NONBASE },
    StyleClass { description: "Old Turkic default style", script_tag: "orkh",
        blue_entries: SCRIPT_ORKH, uni_ranges: RANGES_ORKH_UNI,
        non_base_ranges: RANGES_ORKH_NONBASE },
    StyleClass { description: "Osage default style", script_tag: "osge",
        blue_entries: SCRIPT_OSGE, uni_ranges: RANGES_OSGE_UNI,
        non_base_ranges: RANGES_OSGE_NONBASE },
    StyleClass { description: "Osmanya default style", script_tag: "osma",
        blue_entries: SCRIPT_OSMA, uni_ranges: RANGES_OSMA_UNI,
        non_base_ranges: RANGES_OSMA_NONBASE },
    StyleClass { description: "Hanifi Rohingya default style", script_tag: "rohg",
        blue_entries: SCRIPT_ROHG, uni_ranges: RANGES_ROHG_UNI,
        non_base_ranges: RANGES_ROHG_NONBASE },
    StyleClass { description: "Saurashtra default style", script_tag: "saur",
        blue_entries: SCRIPT_SAUR, uni_ranges: RANGES_SAUR_UNI,
        non_base_ranges: RANGES_SAUR_NONBASE },
    StyleClass { description: "Shavian default style", script_tag: "shaw",
        blue_entries: SCRIPT_SHAW, uni_ranges: RANGES_SHAW_UNI,
        non_base_ranges: RANGES_SHAW_NONBASE },
    StyleClass { description: "Sinhala default style", script_tag: "sinh",
        blue_entries: SCRIPT_SINH, uni_ranges: RANGES_SINH_UNI,
        non_base_ranges: RANGES_SINH_NONBASE },
    StyleClass { description: "Sundanese default style", script_tag: "sund",
        blue_entries: SCRIPT_SUND, uni_ranges: RANGES_SUND_UNI,
        non_base_ranges: RANGES_SUND_NONBASE },
    StyleClass { description: "Tamil default style", script_tag: "taml",
        blue_entries: SCRIPT_TAML, uni_ranges: RANGES_TAML_UNI,
        non_base_ranges: RANGES_TAML_NONBASE },
    StyleClass { description: "Tai Viet default style", script_tag: "tavt",
        blue_entries: SCRIPT_TAVT, uni_ranges: RANGES_TAVT_UNI,
        non_base_ranges: RANGES_TAVT_NONBASE },
    StyleClass { description: "Telugu default style", script_tag: "telu",
        blue_entries: SCRIPT_TELU, uni_ranges: RANGES_TELU_UNI,
        non_base_ranges: RANGES_TELU_NONBASE },
    StyleClass { description: "Tifinagh default style", script_tag: "tfng",
        blue_entries: SCRIPT_TFNG, uni_ranges: RANGES_TFNG_UNI,
        non_base_ranges: RANGES_TFNG_NONBASE },
    StyleClass { description: "Thai default style", script_tag: "thai",
        blue_entries: SCRIPT_THAI, uni_ranges: RANGES_THAI_UNI,
        non_base_ranges: RANGES_THAI_NONBASE },
    StyleClass { description: "Vai default style", script_tag: "vaii",
        blue_entries: SCRIPT_VAII, uni_ranges: RANGES_VAII_UNI,
        non_base_ranges: RANGES_VAII_NONBASE },
    StyleClass { description: "Limbu", script_tag: "limb",
        blue_entries: SCRIPT_LATN, uni_ranges: RANGES_LIMB_UNI,
        non_base_ranges: RANGES_LIMB_NONBASE },
    StyleClass { description: "Oriya", script_tag: "orya",
        blue_entries: SCRIPT_LATN, uni_ranges: RANGES_ORYA_UNI,
        non_base_ranges: RANGES_ORYA_NONBASE },
    StyleClass { description: "Syloti Nagri", script_tag: "sylo",
        blue_entries: SCRIPT_LATN, uni_ranges: RANGES_SYLO_UNI,
        non_base_ranges: RANGES_SYLO_NONBASE },
    StyleClass { description: "Tibetan", script_tag: "tibt",
        blue_entries: SCRIPT_LATN, uni_ranges: RANGES_TIBT_UNI,
        non_base_ranges: RANGES_TIBT_NONBASE },
    StyleClass { description: "CJKV ideographs default style", script_tag: "hani",
        blue_entries: SCRIPT_HANI, uni_ranges: RANGES_HANI_UNI,
        non_base_ranges: RANGES_HANI_NONBASE },
];
pub const STYLE_FALLBACK: usize = 30;
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

/// Get all blue zone characters for a script tag.
/// Returns all characters that define that script's blue zones.
pub fn blue_chars_for_script(tag: &str) -> &'static [u32] {
    match tag {
        "adlm" => &[0x1E902, 0x1E905, 0x1E908, 0x1E90C, 0x1E90F, 0x1E914, 0x1E916, 0x1E91A, 0x1E924, 0x1E928, 0x1E929, 0x1E92C, 0x1E92D, 0x1E92E, 0x1E934, 0x1E938, 0x1E93A, 0x1E93B, 0x1E93C, 0x1E93E, 0x1E940],
        "arab" => &[0x0625, 0x0627, 0x062A, 0x062B, 0x0637, 0x0638, 0x0640, 0x0643, 0x0644],
        "armn" => &[0x0531, 0x0532, 0x0533, 0x0534, 0x0543, 0x0544, 0x0547, 0x0548, 0x054D, 0x054F, 0x0552, 0x0555, 0x0561, 0x0562, 0x0563, 0x0565, 0x0567, 0x0568, 0x056B, 0x056C, 0x056E, 0x0570, 0x0572, 0x0573, 0x0574, 0x0575, 0x0577, 0x0578, 0x057A, 0x057D, 0x057E, 0x0580, 0x0581, 0x0582, 0x0583, 0x0585, 0x0586],
        "avst" => &[0x10B00, 0x10B01, 0x10B10, 0x10B1B],
        "bamu" => &[0xA6A2, 0xA6A7, 0xA6A8, 0xA6AD, 0xA6B3, 0xA6B6, 0xA6BD, 0xA6C1, 0xA6C8, 0xA6C9, 0xA6DB, 0xA6EB, 0xA6EC, 0xA6EF, 0xA6F2],
        "beng" => &[0x0985, 0x0987, 0x098F, 0x0993, 0x0995, 0x099F, 0x09A0, 0x09A1, 0x09A4, 0x09A8, 0x09AC, 0x09AD, 0x09B2, 0x09BF, 0x09C0, 0x09C8, 0x09D7],
        "buhd" => &[0x1740, 0x1742, 0x1743, 0x1745, 0x1746, 0x1748, 0x1749, 0x174A, 0x174B, 0x174C, 0x174E, 0x174F, 0x1750, 0x1751],
        "cakm" => &[0x11103, 0x11105, 0x11109, 0x11113, 0x11116, 0x11117, 0x11118, 0x11119, 0x1111B, 0x1111D, 0x11124, 0x11125],
        "cans" => &[0x1401, 0x1403, 0x141E, 0x1422, 0x142A, 0x144C, 0x144E, 0x146B, 0x148D, 0x14A1, 0x14A2, 0x14A3, 0x14A7, 0x14BB, 0x14BE, 0x14C0, 0x14C2, 0x14C4, 0x14D1, 0x14D3, 0x14D5, 0x14D7, 0x14DA, 0x1506, 0x1511, 0x1542, 0x1543, 0x1544, 0x1546, 0x15B4, 0x15B5, 0x15DC, 0x15E2, 0x15EE, 0x15F0, 0x15F6, 0x1623, 0x1646, 0x18D7, 0x18D8],
        "cari" => &[0x102A3, 0x102A7, 0x102AB, 0x102AC, 0x102AD, 0x102B1, 0x102B7, 0x102B8, 0x102BA, 0x102BC, 0x102BF, 0x102C0, 0x102C9],
        "cher" => &[0x13A4, 0x13A6, 0x13AC, 0x13BB, 0x13C3, 0x13C6, 0x13D5, 0x13E3, 0x13F8, 0xAB74, 0xAB76, 0xAB79, 0xAB7B, 0xAB7C, 0xAB7E, 0xAB90, 0xAB92, 0xAB93, 0xAB96, 0xAB97, 0xAB9D, 0xABA0, 0xABA4, 0xABA5, 0xABB3, 0xABB6, 0xABBB, 0xABBF],
        "copt" => &[0x2C8C, 0x2C8D, 0x2C8E, 0x2C8F, 0x2C90, 0x2C91, 0x2C9E, 0x2C9F, 0x2CA0, 0x2CA1, 0x2CA4, 0x2CA5, 0x2CB0, 0x2CCA, 0x2CCB, 0x2CD0, 0x2CD1, 0x2CD2, 0x2CD8, 0x2CD9, 0x2CDC, 0x2CDD, 0x2CDE, 0x2CDF],
        "cprt" => &[0x10803, 0x10805, 0x10808, 0x1080A, 0x1080D, 0x1080F, 0x10810, 0x10813, 0x10816, 0x10819, 0x1081B, 0x10823, 0x10826, 0x10831, 0x10833, 0x10835],
        "cyrl" => &[0x0411, 0x0412, 0x0415, 0x0417, 0x041E, 0x041F, 0x0421, 0x0428, 0x042D, 0x0435, 0x0437, 0x043D, 0x043E, 0x043F, 0x0440, 0x0441, 0x0443, 0x0444, 0x0445, 0x0448],
        "deva" => &[0x0905, 0x0906, 0x0908, 0x0909, 0x0910, 0x0913, 0x0914, 0x0915, 0x091B, 0x091F, 0x0920, 0x0921, 0x0925, 0x0927, 0x0928, 0x092D, 0x092E, 0x0936, 0x093F, 0x0940, 0x0941, 0x0943, 0x094B, 0x094C],
        "dsrt" => &[0x10400, 0x10402, 0x10404, 0x1040B, 0x10411, 0x10417, 0x1041B, 0x10428, 0x1042A, 0x1042C, 0x10433, 0x10439, 0x1043F, 0x10443],
        "ethi" => &[0x1200, 0x1203, 0x1208, 0x1210, 0x121B, 0x122A, 0x1260, 0x12CB, 0x12D0, 0x12D8, 0x1328, 0x1350],
        "geok" => &[0x10A4, 0x10A5, 0x10A6, 0x10A7, 0x10A8, 0x10AA, 0x10AB, 0x10B1, 0x10B3, 0x10B9, 0x10BA, 0x10BC, 0x2D01, 0x2D02, 0x2D03, 0x2D04, 0x2D05, 0x2D06, 0x2D07, 0x2D08, 0x2D0B, 0x2D0C, 0x2D0E, 0x2D10, 0x2D11, 0x2D13, 0x2D14, 0x2D15, 0x2D16, 0x2D17, 0x2D18, 0x2D19, 0x2D1B, 0x2D1D, 0x2D21, 0x2D22, 0x2D23],
        "geor" => &[0x10D0, 0x10D2, 0x10D3, 0x10D4, 0x10D5, 0x10D6, 0x10D7, 0x10D8, 0x10DB, 0x10DD, 0x10DE, 0x10DF, 0x10E1, 0x10E2, 0x10E3, 0x10E4, 0x10E5, 0x10E6, 0x10E7, 0x10E8, 0x10E9, 0x10EB, 0x10EC, 0x10EE, 0x1C92, 0x1C94, 0x1C98, 0x1C9B, 0x1C9C, 0x1C9D, 0x1C9F, 0x1CA8, 0x1CA9, 0x1CAF, 0x1CB2, 0x1CB3, 0x1CB4, 0x1CB8, 0x1CBD],
        "glag" => &[0x2C02, 0x2C04, 0x2C05, 0x2C0A, 0x2C0B, 0x2C14, 0x2C1E, 0x2C21, 0x2C2A, 0x2C2B, 0x2C32, 0x2C34, 0x2C35, 0x2C3A, 0x2C3B, 0x2C44, 0x2C4E, 0x2C51, 0x2C5A, 0x2C5B],
        "goth" => &[0x10332, 0x10334, 0x10336, 0x1033E, 0x10340, 0x10343, 0x10344, 0x10348],
        "grek" => &[0x0392, 0x0393, 0x0394, 0x0395, 0x0396, 0x0398, 0x039E, 0x039F, 0x03A9, 0x03B1, 0x03B2, 0x03B3, 0x03B4, 0x03B5, 0x03B6, 0x03B7, 0x03B8, 0x03B9, 0x03BB, 0x03BC, 0x03BE, 0x03BF, 0x03C0, 0x03C1, 0x03C3, 0x03C4, 0x03C6, 0x03C7, 0x03C8, 0x03C9],
        "gujr" => &[0x0A87, 0x0A88, 0x0A8A, 0x0A8B, 0x0A8C, 0x0A96, 0x0A97, 0x0A98, 0x0A9B, 0x0A9C, 0x0A9E, 0x0A9F, 0x0AA0, 0x0AA4, 0x0AA8, 0x0AB0, 0x0AB2, 0x0AB6, 0x0AB8, 0x0ABF, 0x0AC0, 0x0AC1, 0x0AC3, 0x0AC4, 0x0AE6, 0x0AE7, 0x0AE8, 0x0AE9, 0x0AED],
        "guru" => &[0x0A05, 0x0A07, 0x0A08, 0x0A09, 0x0A0F, 0x0A13, 0x0A15, 0x0A17, 0x0A19, 0x0A1A, 0x0A1C, 0x0A20, 0x0A24, 0x0A27, 0x0A30, 0x0A38, 0x0A3F, 0x0A40, 0x0A66, 0x0A67, 0x0A68, 0x0A69, 0x0A6D, 0x0A73],
        "hani" => &[0x007C, 0x4E2A, 0x4E3A, 0x4E3B, 0x4E8B, 0x4E9B, 0x4EBA, 0x4ED6, 0x4EE5, 0x4EEC, 0x4F60, 0x4F86, 0x4F8B, 0x500B, 0x5011, 0x519B, 0x5225, 0x522B, 0x5230, 0x5236, 0x524D, 0x52A8, 0x52D5, 0x5373, 0x540C, 0x5417, 0x5427, 0x542C, 0x5462, 0x548C, 0x54C1, 0x54CD, 0x55CE, 0x56E0, 0x5730, 0x589E, 0x5927, 0x5979, 0x5B78, 0x5B83, 0x5BF9, 0x5C06, 0x5C07, 0x5C0D, 0x5C31, 0x5DF2, 0x5E08, 0x5E2B, 0x5E2D, 0x5E74, 0x5F97, 0x60C5, 0x60F3, 0x610F, 0x613F, 0x6211, 0x6216, 0x6307, 0x6536, 0x653F, 0x65AD, 0x65AF, 0x65B0, 0x65B7, 0x65E2, 0x65F6, 0x660E, 0x661F, 0x662F, 0x6642, 0x666F, 0x6700, 0x6703, 0x6709, 0x671D, 0x671F, 0x6765, 0x6784, 0x6837, 0x6A23, 0x6C11, 0x6C92, 0x6CA1, 0x70BA, 0x7136, 0x7167, 0x7269, 0x7279, 0x73B0, 0x73FE, 0x7403, 0x7406, 0x751F, 0x7528, 0x7576, 0x770B, 0x773C, 0x7740, 0x786E, 0x79CD, 0x7B2C, 0x7D93, 0x7F6E, 0x8005, 0x80FD, 0x81EA, 0x8230, 0x8457, 0x88E1, 0x8981, 0x8AAA, 0x8ABF, 0x8BF4, 0x8C01, 0x8C03, 0x8CBB, 0x8D39, 0x8D77, 0x8ECD, 0x8FC7, 0x8FD8, 0x8FD9, 0x8FDB, 0x9019, 0x901A, 0x9032, 0x904E, 0x9053, 0x9084, 0x90A3, 0x90FD, 0x914D, 0x91CC, 0x958B, 0x9593, 0x95F4, 0x9645, 0x9648, 0x9650, 0x9664, 0x9673, 0x968F, 0x969B, 0x96A8, 0x96F7, 0x9732, 0x9762, 0x987E, 0x9F4A],
        "hebr" => &[0x05D1, 0x05D3, 0x05D4, 0x05D7, 0x05D8, 0x05DA, 0x05DB, 0x05DD, 0x05DF, 0x05E1, 0x05E3, 0x05E5, 0x05E6, 0x05E7],
// Generated: 60 scripts, 55 with blue zone chars

        "kali" => &[0xA900, 0xA901, 0xA905, 0xA908, 0xA90B, 0xA90D, 0xA90F, 0xA911, 0xA914, 0xA916, 0xA918, 0xA91C, 0xA91E, 0xA921, 0xA922],
        "khmr" => &[0x1780, 0x1781, 0x1783, 0x1784, 0x1785, 0x178B, 0x178F, 0x1791, 0x1793, 0x1794, 0x1798, 0x1799, 0x179A, 0x179B, 0x17A2, 0x17A7, 0x17A9, 0x17B2, 0x17B6],
        "khms" => &[0x19E0, 0x19E1, 0x19F6, 0x19F9],
        "knda" => &[0x0C85, 0x0C87, 0x0C89, 0x0C8A, 0x0C8E, 0x0C90, 0x0CA3, 0x0CA6, 0x0CA8, 0x0CB0, 0x0CB2, 0x0CB8, 0x0CE6, 0x0CE8, 0x0CEC, 0x0CED],
        "lao" => &[0x0E87, 0x0E8A, 0x0E8D, 0x0E94, 0x0E96, 0x0E9A, 0x0E9B, 0x0E9D, 0x0E9F, 0x0EA1, 0x0EA2, 0x0EA3, 0x0EA5, 0x0EA7, 0x0EAD, 0x0EAE, 0x0EAF, 0x0EB2, 0x0EBD, 0x0EC2, 0x0EC3, 0x0EC4, 0x0EC6],
        "latn" => &[0x0043, 0x0045, 0x0048, 0x004C, 0x004F, 0x0051, 0x0053, 0x0054, 0x0055, 0x005A, 0x0062, 0x0063, 0x0064, 0x0065, 0x0066, 0x0067, 0x0068, 0x0069, 0x006A, 0x006B, 0x006E, 0x006F, 0x0070, 0x0071, 0x0072, 0x0073, 0x0075, 0x0076, 0x0078, 0x0079, 0x007A],
        "lisu" => &[0xA4D5, 0xA4DA, 0xA4DB, 0xA4DC, 0xA4DE, 0xA4E1, 0xA4E2, 0xA4E7, 0xA4E9, 0xA4F1, 0xA4F3, 0xA4F4, 0xA4F5, 0xA4F6],
        "medf" => &[0x16E40, 0x16E41, 0x16E42, 0x16E43, 0x16E4F, 0x16E52, 0x16E53, 0x16E5A, 0x16E5F, 0x16E60, 0x16E61, 0x16E62, 0x16E64, 0x16E65, 0x16E67, 0x16E68, 0x16E69, 0x16E6C, 0x16E6D, 0x16E6E, 0x16E73, 0x16E74, 0x16E76, 0x16E79, 0x16E7D, 0x16E7E, 0x16E80, 0x16E84, 0x16E85, 0x16E88, 0x16E8D],
        "mlym" => &[0x0D12, 0x0D18, 0x0D1A, 0x0D1F, 0x0D20, 0x0D25, 0x0D27, 0x0D2A, 0x0D31, 0x0D32, 0x0D36],
        "mong" => &[0x1833, 0x1834, 0x1836, 0x183D, 0x1842, 0x1843, 0x184A, 0x200D],
        "mymr" => &[0x1001, 0x1002, 0x1004, 0x1009, 0x100A, 0x100E, 0x1012, 0x1015, 0x1017, 0x101D, 0x1025, 0x1028, 0x1029, 0x102B, 0x102D, 0x103C, 0x1042, 0x1045, 0x1046, 0x1049, 0x104A, 0x104B, 0x104D, 0x104F, 0x1065],
        "nkoo" => &[0x07C0, 0x07C9, 0x07CB, 0x07CE, 0x07CF, 0x07D0, 0x07D2, 0x07D6, 0x07D8, 0x07DB, 0x07DC, 0x07DF, 0x07E0, 0x07E1, 0x07E5],
        "olck" => &[0x1C5B, 0x1C5C, 0x1C5D, 0x1C61, 0x1C62, 0x1C65],
        "orkh" => &[0x10C09, 0x10C17, 0x10C18, 0x10C26, 0x10C27],
        "osge" => &[0x104B0, 0x104B5, 0x104B9, 0x104BB, 0x104BC, 0x104BD, 0x104BE, 0x104BF, 0x104C2, 0x104C6, 0x104CD, 0x104CE, 0x104D2, 0x104D3, 0x104D8, 0x104DA, 0x104DB, 0x104DD, 0x104E1, 0x104E3, 0x104E4, 0x104E5, 0x104E6, 0x104E7, 0x104EA, 0x104EE, 0x104F5, 0x104F6, 0x104F8, 0x104F9, 0x104FA, 0x104FB],
        "osma" => &[0x10480, 0x10482, 0x10486, 0x10488, 0x10489, 0x1048A, 0x10490, 0x10492, 0x10498, 0x1049B, 0x104A0, 0x104A3, 0x104A9],
        "rohg" => &[0x0640, 0x10D00, 0x10D03, 0x10D06, 0x10D10, 0x10D11, 0x10D14, 0x10D15, 0x10D16],
        "saur" => &[0xA882, 0xA88E, 0xA892, 0xA896, 0xA89B, 0xA89C, 0xA89D, 0xA89E, 0xA8A4, 0xA8A8, 0xA8B3, 0xA8BA],
        "shaw" => &[0x10454, 0x10455, 0x10456, 0x10457, 0x10459, 0x1045F, 0x10463, 0x10471, 0x10472, 0x10473, 0x10474, 0x10478, 0x10479, 0x1047A, 0x1047B, 0x1047C],
        "sinh" => &[0x0D89, 0x0D8B, 0x0D91, 0x0D94, 0x0D9A, 0x0D9D, 0x0DA2, 0x0DA7, 0x0DAD, 0x0DAE, 0x0DAF, 0x0DB0, 0x0DB3, 0x0DB4, 0x0DB6, 0x0DBA, 0x0DBB, 0x0DBD, 0x0DC6],
        "sund" => &[0x1B84, 0x1B86, 0x1B88, 0x1B89, 0x1B8B, 0x1B94, 0x1B95, 0x1B97, 0x1B9E, 0x1BAE, 0x1BB0, 0x1BBC, 0x1BBD, 0x1CC4],
        "taml" => &[0x0B88, 0x0B89, 0x0B92, 0x0B93, 0x0B95, 0x0B99, 0x0B9A, 0x0B9F, 0x0BAA, 0x0BB1, 0x0BB2, 0x0BB6],
        "tavt" => &[0xAA86, 0xAA89, 0xAA92, 0xAA94, 0xAA96, 0xAAAB, 0xAAAE],
        "telu" => &[0x0C05, 0x0C07, 0x0C0C, 0x0C15, 0x0C19, 0x0C1A, 0x0C1E, 0x0C23, 0x0C30, 0x0C31, 0x0C3D, 0x0C68, 0x0C6C, 0x0C6F],
        "tfng" => &[0x2D35, 0x2D39, 0x2D3C, 0x2D4E, 0x2D54, 0x2D59, 0x2D5B, 0x2D5E],
        "thai" => &[0x0E01, 0x0E0D, 0x0E0E, 0x0E0F, 0x0E10, 0x0E1A, 0x0E1B, 0x0E1D, 0x0E1F, 0x0E22, 0x0E24, 0x0E26, 0x0E29, 0x0E2D, 0x0E2E, 0x0E2F, 0x0E32, 0x0E40, 0x0E41, 0x0E42, 0x0E43, 0x0E44, 0x0E50, 0x0E51, 0x0E53],
        "vaii" => &[0xA505, 0xA506, 0xA562, 0xA59C, 0xA59D, 0xA5CD, 0xA5DE, 0xA616, 0xA619, 0xA61C],
        _ => &[],
    }
}
