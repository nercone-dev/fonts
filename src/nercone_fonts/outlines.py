from typing import Optional, List, Dict

from fontTools.ttLib import TTFont
from fontTools.pens.qu2cuPen import Qu2CuPen
from fontTools.pens.filterPen import FilterPen, DecomposingFilterPen
from fontTools.pens.t2CharStringPen import T2CharStringPen
from fontTools.fontBuilder import FontBuilder

from .models import Style, Family

error = 1 / 2000

class StartedPen(FilterPen):
    def qCurveTo(self, *points):
        if points and points[-1] is None:
            first, last = points[0], points[-2]
            start = (0.5 * (first[0] + last[0]), 0.5 * (first[1] + last[1]))
            self.moveTo(start)
            points = points[:-1] + (start,)

        FilterPen.qCurveTo(self, *points)

class Outlines:
    truetype = ["glyf", "loca", "gvar", "cvt ", "fpgm", "prep", "cvar", "hdmx", "LTSH", "VDMX", "gasp"]

    @staticmethod
    def compact(font: TTFont, family: Family, style: Style, version: str) -> TTFont:
        glyphs = font.getGlyphSet()
        metrics = font["hmtx"].metrics
        tolerance = font["head"].unitsPerEm * error
        charstrings = {}

        for name in font.getGlyphOrder():
            pen = T2CharStringPen(metrics[name][0], glyphs)
            cubic = Qu2CuPen(pen, tolerance, all_cubic=True)
            glyphs[name].draw(DecomposingFilterPen(StartedPen(cubic), glyphs))
            charstrings[name] = pen.getCharString()

        builder = FontBuilder(font=font)
        builder.isTTF = False
        builder.setupCFF("{}-{}".format(family.filename, style.name()), Outlines.information(family, style, version), charstrings, Outlines.private(font))

        for tag in Outlines.truetype:
            if tag in font:
                del font[tag]

        font["maxp"].tableVersion = 0x00005000
        font["post"].formatType = 3.0
        font["head"].indexToLocFormat = 0

        return font

    @staticmethod
    def information(family: Family, style: Style, version: str) -> Dict[str, object]:
        return {
            "FullName": "{} {}".format(family.name, style.name()),
            "FamilyName": family.name,
            "Weight": style.name(),
            "version": version,
            "Notice": family.license.name,
            "isFixedPitch": bool(family.monospace)
        }

    @staticmethod
    def private(font: TTFont) -> Dict[str, object]:
        upem = font["head"].unitsPerEm

        return {
            "BlueValues": [],
            "OtherBlues": [],
            "FamilyBlues": [],
            "FamilyOtherBlues": [],
            "StemSnapH": [],
            "StemSnapV": [],
            "StdHW": max(1, round(upem * 0.04)),
            "StdVW": max(1, round(upem * 0.05)),
            "nominalWidthX": 0,
            "defaultWidthX": 0
        }
