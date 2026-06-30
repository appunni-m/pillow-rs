# FT Parity Debugging — Session Log

**Start:** 27,154/27,695 (541 failed)  
**Current:** 27,686/27,695 (9 failed)  
**Net:** -532 failures

## Commits

| Commit | Fix | Impact |
|--------|-----|--------|
| `cf19f9e` | getlength from Python hmtx, not C FT_LOAD_DEFAULT | -98 |
| `887070a` | walk_contour conic wrap fix | -130 |
| `cbbdcba` | getmetrics: FT_MulFix + FT_PIX_CEIL | -4 |
| `04975f8` | pp1.x phantom-point translation | -291 |
| `1ecd364` | WEAK_INTERPOLATION classification | -9 |

## 9 Remaining (all same root cause)

| Font | Glyph | Sizes |
|------|-------|-------|
| NotoSerifDisplay-Bold | B | 10-24pt (5) |
| NotoSerifDisplay-Bold | g | 24pt (1) |
| LiberationSerif-Bold | $ | 10pt (1) |
| LiberationMono-Regular | l | 16pt (1) |
| LiberationSansNarrow-BoldItalic | ; | 20pt (1) |

All fail from direction-chain WEAK/STRONG classification producing different
IUP reference pairs. Different contour topologies interact differently with
the near_limit threshold at UPEM=1000 and corner_is_flat delta updates.

Full logic reference: `pillow-rs-freetype/doc/AUTOHINTER_NUANCES.md`
