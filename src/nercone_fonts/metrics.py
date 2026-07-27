import math
from typing import Optional, List, Tuple

from fontTools.ttLib import TTFont, newTable
from fontTools.otlLib.maxContextCalc import maxCtxFont

from .models import Style, Family
from .prepare import Component

epsilon = 1e-6

class Metrics:
    def __init__(self, upem: int, ascender: int, descender: int, gap: int, line_ascender: int, line_descender: int, line_gap: int, window_ascent: int, window_descent: int, cap_height: int, x_height: int, italic_angle: float, underline_position: int, underline_thickness: int, typo_metrics: bool = False):
        self.upem = upem
        self.ascender = ascender
        self.descender = descender
        self.gap = gap
        self.line_ascender = line_ascender
        self.line_descender = line_descender
        self.line_gap = line_gap
        self.window_ascent = window_ascent
        self.window_descent = window_descent
        self.typo_metrics = typo_metrics
        self.cap_height = cap_height
        self.x_height = x_height
        self.italic_angle = italic_angle
        self.underline_position = underline_position
        self.underline_thickness = underline_thickness

    def slanted(self) -> bool:
        return abs(self.italic_angle) > epsilon

    def caret(self) -> Tuple[int, int]:
        if not self.slanted():
            return (self.upem, 0)
        return (self.upem, round(self.upem * math.tan(math.radians(-self.italic_angle))))

    @staticmethod
    def of(components: List[Component], italic: bool = False) -> "Metrics":
        base = components[0].font
        os2, hhea, post = base["OS/2"], base["hhea"], base["post"]
        upem = base["head"].unitsPerEm

        ascents, descents = [], []
        for component in components:
            other = component.font["OS/2"]
            ascents.append(abs(other.usWinAscent))
            descents.append(abs(other.usWinDescent))

        return Metrics(
            upem=upem,
            ascender=abs(os2.sTypoAscender), descender=-abs(os2.sTypoDescender), gap=abs(os2.sTypoLineGap),
            line_ascender=abs(hhea.ascender), line_descender=-abs(hhea.descender), line_gap=abs(hhea.lineGap),
            window_ascent=max(ascents), window_descent=max(descents),
            typo_metrics=bool(os2.fsSelection & 0x0080),
            cap_height=getattr(os2, "sCapHeight", None) or round(upem * 0.70),
            x_height=getattr(os2, "sxHeight", None) or round(upem * 0.52),
            italic_angle=-abs(post.italicAngle) if italic else 0.0,
            underline_position=post.underlinePosition,
            underline_thickness=post.underlineThickness
        )

    def apply(self, font: TTFont, family: Family, style: Style, revision: float, advance: Optional[int] = None):
        self.header(font, style, revision)
        self.horizontal(font)
        self.selection(font, family, style, advance)
        self.outline(font, family)
        self.smoothing(font)

    def header(self, font: TTFont, style: Style, revision: float):
        head = font["head"]
        head.fontRevision = revision
        head.macStyle = (0x01 if style.bold() else 0) | (0x02 if style.italic() else 0)
        head.lowestRecPPEM = 8
        head.fontDirectionHint = 2

    def horizontal(self, font: TTFont):
        rise, run = self.caret()

        hhea = font["hhea"]
        hhea.ascender = self.line_ascender
        hhea.descender = self.line_descender
        hhea.lineGap = self.line_gap
        hhea.caretSlopeRise = rise
        hhea.caretSlopeRun = run
        hhea.caretOffset = 0

        if "vhea" in font:
            vhea = font["vhea"]
            vhea.lineGap = 0
            vhea.caretSlopeRise = 0
            vhea.caretSlopeRun = 1
            vhea.caretOffset = 0

    def selection(self, font: TTFont, family: Family, style: Style, advance: Optional[int]):
        os2 = font["OS/2"]
        if os2.version < 4:
            os2.version = 4

        os2.usWeightClass = int(style.value())
        os2.usWidthClass = 5
        os2.fsType = 0

        os2.sTypoAscender = self.ascender
        os2.sTypoDescender = self.descender
        os2.sTypoLineGap = self.gap
        os2.usWinAscent = self.window_ascent
        os2.usWinDescent = self.window_descent
        os2.sCapHeight = self.cap_height
        os2.sxHeight = self.x_height

        os2.ySubscriptXSize = round(self.upem * 0.65)
        os2.ySubscriptYSize = round(self.upem * 0.60)
        os2.ySubscriptXOffset = 0
        os2.ySubscriptYOffset = round(self.upem * 0.075)
        os2.ySuperscriptXSize = round(self.upem * 0.65)
        os2.ySuperscriptYSize = round(self.upem * 0.60)
        os2.ySuperscriptXOffset = 0
        os2.ySuperscriptYOffset = round(self.upem * 0.35)
        os2.yStrikeoutSize = max(1, round(self.upem * 0.05))
        os2.yStrikeoutPosition = round(self.x_height * 0.55)

        os2.fsSelection = 0x0080 if self.typo_metrics else 0  # USE_TYPO_METRICS
        if style.italic():
            os2.fsSelection |= 0x0001
        if style.bold():
            os2.fsSelection |= 0x0020
        if not style.italic() and not style.bold():
            os2.fsSelection |= 0x0040

        os2.panose.bFamilyType = 2
        os2.panose.bSerifStyle = 2 if family.typeface == "Serif" else 11
        os2.panose.bProportion = 9 if family.monospace else 3

        os2.achVendID = "NRCN"
        os2.usDefaultChar = 0
        os2.usBreakChar = 32
        os2.usMaxContext = maxCtxFont(font)

        os2.recalcUnicodeRanges(font)
        if hasattr(os2, "recalcCodePageRanges"):
            os2.recalcCodePageRanges(font)

        codes = sorted(font.getBestCmap())
        os2.usFirstCharIndex = min(0xFFFF, codes[0]) if codes else 0xFFFF
        os2.usLastCharIndex = min(0xFFFF, codes[-1]) if codes else 0xFFFF

        os2.xAvgCharWidth = advance if advance else os2.recalcAvgCharWidth(font)

    def outline(self, font: TTFont, family: Family):
        post = font["post"]
        post.formatType = 3.0
        post.italicAngle = self.italic_angle
        post.underlinePosition = self.underline_position
        post.underlineThickness = self.underline_thickness
        post.isFixedPitch = 1 if family.monospace else 0
        post.glyphOrder = None

    def smoothing(self, font: TTFont):
        table = newTable("gasp")
        table.version = 1
        table.gaspRange = {0xFFFF: 0x000A}
        font["gasp"] = table
