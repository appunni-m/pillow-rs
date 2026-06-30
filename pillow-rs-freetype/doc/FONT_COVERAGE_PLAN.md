# 55-Script Font Coverage — Gap Analysis & Minimum-3 Plan

## Current Status: 1 font per script (55 × 1 = 55 fonts, 990 test entries)

All 55 scripts pass mask dimension tests (±2px). But coverage only has
1 font sample per script — not sufficient to verify the autohinter works
across variable font families (weight, width, italic, serif/sans).

## Gap Categories

### Category A: 24 scripts with 2+ system TTF fonts available (just add them)

| Script | Current | fc-list | Can Add |
|--------|---------|---------|---------|
| adlm | 1 | 4 | NotoSansAdlam-Bold, NotoSansAdlam-Regular |
| arab | 1 | 15 | NotoSansArabic-Bold, NotoNaskhArabic-Bold |
| armn | 1 | 25 | DejaVuSans-Oblique, DejaVuSerif-BoldItalic |
| bamu | 1 | 2 | NotoSansBamum-Regular |
| beng | 1 | 4 | NotoSansBengali-Bold, NotoSerifBengali-Bold |
| cans | 1 | 2 | NotoSansCanadianAboriginal-Bold |
| cher | 1 | 2 | NotoSansCherokee-Regular |
| cyrl | 1 | 64 | LiberationSerif-Regular, DejaVuSans-Bold |
| deva | 1 | 4 | NotoSerifDevanagari-Regular, NotoSansDevanagari-Bold |
| ethi | 1 | 4 | NotoSerifEthiopic-Regular, NotoSansEthiopic-Regular |
| geok | 1 | 20 | DejaVuSans-Oblique, DejaVuSerif-BoldItalic |
| geor | 1 | 25 | DejaVuSans-Oblique, DejaVuSerif-BoldItalic |
| grek | 1 | 68 | LiberationSerif-Regular, DejaVuSans-Bold |
| gujr | 1 | 4 | NotoSansGujarati-Regular, NotoSerifGujarati-Bold |
| guru | 1 | 4 | NotoSansGurmukhi-Regular, NotoSerifGurmukhi-Regular |
| hebr | 1 | 26 | LiberationSerif-Regular, DejaVuSans-Bold |
| kali | 1 | 2 | NotoSansKayahLi-Regular |
| khmr | 1 | 4 | NotoSerifKhmer-Bold, NotoSansKhmer-Bold |
| khms | 1 | 4 | NotoSerifKhmer-Bold, NotoSansKhmer-Bold |
| knda | 1 | 4 | NotoSerifKannada-Bold, NotoSansKannada-Bold |
| lao | 1 | 18 | DejaVuSans-Oblique, NotoSerifLao-Regular |
| latb | 1 | 59 | LiberationSerif-Regular, DejaVuSans-Bold |
| latn | 1 | 69 | LiberationSerif-Regular, DejaVuSans-Bold, P052-Italic |
| latp | 1 | 47 | LiberationSerif-Regular, DejaVuSans-Bold |
| lisu | 1 | 11 | DejaVuSans-Oblique, NotoSansLisu-Regular |
| medf | 1 | 2 | NotoSansMedefaidrin-Bold |
| mlym | 1 | 4 | NotoSansMalayalam-Bold, NotoSerifMalayalam-Bold |
| mymr | 1 | 4 | NotoSerifMyanmar-Bold, NotoSansMyanmar-Regular |
| nkoo | 1 | 5 | NotoSansNKo-Regular, DejaVuSans-Bold |
| olck | 1 | 2 | NotoSansOlChiki-Bold |
| rohg | 1 | 2 | NotoSansHanifiRohingya-Bold |
| sinh | 1 | 4 | NotoSerifSinhala-Bold, NotoSerifSinhala-Regular |
| sund | 1 | 2 | NotoSansSundanese-Regular |
| taml | 1 | 6 | NotoSerifTamil-Regular, NotoSansTamil-Regular |
| telu | 1 | 4 | NotoSerifTelugu-Regular, NotoSerifTelugu-Bold |
| tfng | 1 | 16 | DejaVuSans-Bold, NotoSansTifinaghAPT-Regular |
| thai | 1 | 6 | NotoSerifThai-Bold, NotoSansThai-Regular |

**36 scripts × 2 additional fonts = 72 new font references needed**
**These exist as TTF files on the system — just need to copy and regenerate references.**

### Category B: 12 scripts with only 1 system TTF (need synthetic or download)

| Script | Notes |
|--------|-------|
| avst | 1 TTF: NotoSansAvestan-Regular. No Bold/Italic variants exist. Need synthetic weight variant + italic simulation, or download from notofonts GitHub. |
| buhd | 1 TTF: NotoSansBuhid-Regular |
| cakm | 1 TTF: NotoSansChakma-Regular |
| cari | 1 TTF: NotoSansCarian-Regular |
| copt | 1 TTF: NotoSansCoptic-Regular |
| cprt | 1 TTF: NotoSansCypriot-Regular |
| dsrt | 1 TTF: NotoSansDeseret-Regular |
| glag | 1 TTF: NotoSansGlagolitic-Regular |
| goth | 1 TTF: NotoSansGothic-Regular |
| mong | 1 TTF: NotoSansMongolian-Regular |
| orkh | 1 TTF: NotoSansOldTurkic-Regular |
| osge | 1 TTF: NotoSansOsage-Regular |
| osma | 1 TTF: NotoSansOsmanya-Regular |
| saur | 1 TTF: NotoSansSaurashtra-Regular |
| shaw | 1 TTF: NotoSansShavian-Regular |
| tavt | 1 TTF: NotoSansTaiViet-Regular |
| vaii | 1 TTF: NotoSansVai-Regular |

**17 scripts × 2 additional references needed.**

These scripts' fonts are Google Noto — single-weight, single-style.
No Bold/Italic variants from Google.
Options:
- **A) Download from notofonts GitHub** — some may have updated variants
- **B) Generate synthetic Bold/Italic via fontTools** — scale-stem, slant outlines
- **C) Accept 1-font coverage** — these are rare scripts where coverage with 1 font is sufficient for verification

### Category C: 2 scripts with only 1 font on system (but CJK has alternatives)

| Script | Notes |
|--------|-------|
| hani | 1 TTF: DroidSansFallbackFull. CJK TTC fonts exist but our parser doesn't support .ttc. Could extract first face, or use fontTools to convert. |
| khms | Shares font with khmr (NotoSansKhmer). Khmer Symbols are a subset. Need separate font or accept shared. |

## Implementation Plan

### Phase 1: Category A — Add system fonts (36 scripts × 2 fonts)
- Modify `generate_full_coverage.py` to collect up to 3 fonts per script
- Select fonts to maximize variety: prefer sans+serif, regular+bold, upright+italic
- Run FreeType references, regenerate `coverage_matrix_full.json`
- Expected: 55 × 3 × 3 chars × 3 sizes × 2 ops = 2,970 entries

### Phase 2: Category B — Synthetic variants for single-font scripts (17 scripts)
- Use fontTools to create Bold (stem-scale) and Italic (slant + shift) variants
- Each: {Regular, Bold, Italic} → 3 variants
- Expected: 17 × 3 × 3 chars × 3 sizes × 2 ops = 918 entries

### Phase 3: Category C — CJK and Khmer Symbols
- hani: extract .ttc → .ttf via fontTools, or use fontTools to find additional CJK .ttf
- khms: use khmr fonts + add filter for Khmer Symbols codepoint range

### Phase 4: Regenerate & Verify
- Full `coverage_matrix_full.json` with 2,970 + 918 + 162 = ~4,050 entries
- Dimension test for all entries
- Update doc/52_SCRIPT_COVERAGE_PLAN.md

## Success Criteria

- All 55 scripts have ≥ 3 font samples
- All fonts pass mask dimension check (±2px)
- No regression on existing 27695 Latin/Greek matrix
- Bbox check (±2px) for scripts with system font variety (22 scripts from multi-script test)
- Dimension check for all 55 scripts
