#!/usr/bin/env python3
"""Build compact SFNT fonts that exercise face metric fallback branches."""

from __future__ import annotations

from pathlib import Path

from fontTools.ttLib import TTFont


ROOT = Path(__file__).resolve().parents[1]
BASE_FONT = ROOT / "tests" / "fixtures" / "fonts" / "glyf" / "hinter-control-matrix.ttf"
OUT_DIR = ROOT / "tests" / "fixtures" / "fonts" / "metrics"
USE_TYPO_METRICS = 1 << 7


def save_font(name: str, font: TTFont) -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    out = OUT_DIR / name
    if out.exists() or out.is_symlink():
        out.unlink()
    font.save(out, reorderTables=True)


def base_font() -> TTFont:
    return TTFont(BASE_FONT, recalcTimestamp=False)


def clear_hhea_metrics(font: TTFont) -> None:
    hhea = font["hhea"]
    hhea.ascent = 0
    hhea.descent = 0
    hhea.lineGap = 0


def write_hhea_zero_typo_fallback() -> None:
    font = base_font()
    clear_hhea_metrics(font)
    os2 = font["OS/2"]
    os2.fsSelection &= ~USE_TYPO_METRICS
    os2.sTypoAscender = 700
    os2.sTypoDescender = -210
    os2.sTypoLineGap = 50
    os2.usWinAscent = 900
    os2.usWinDescent = 260
    save_font("hhea-zero-typo-fallback.ttf", font)


def write_hhea_zero_win_fallback() -> None:
    font = base_font()
    clear_hhea_metrics(font)
    os2 = font["OS/2"]
    os2.fsSelection &= ~USE_TYPO_METRICS
    os2.sTypoAscender = 0
    os2.sTypoDescender = 0
    os2.sTypoLineGap = 0
    os2.usWinAscent = 777
    os2.usWinDescent = 222
    save_font("hhea-zero-win-fallback.ttf", font)


def main() -> None:
    write_hhea_zero_typo_fallback()
    write_hhea_zero_win_fallback()


if __name__ == "__main__":
    main()
