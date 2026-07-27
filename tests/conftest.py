from typing import Optional, List, Dict, Tuple

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.ttLib import TTFont

def square(upem: int) -> object:
    pen = TTGlyphPen(None)
    pen.moveTo((50, 0))
    pen.lineTo((50, upem // 2))
    pen.lineTo((upem // 2, upem // 2))
    pen.lineTo((upem // 2, 0))
    pen.closePath()
    return pen.glyph()

def build_font(glyphs: Dict[int, str], upem: int = 1000, features: str = "",
               axes: Optional[List[Tuple[str, float, float, float, str]]] = None,
               typo: Tuple[int, int, int] = (800, -200, 0), line: Tuple[int, int, int] = (1000, -250, 0),
               window: Tuple[int, int] = (1000, 250), selection: int = 0x40) -> TTFont:
    order = [".notdef"] + sorted(set(glyphs.values()))

    builder = FontBuilder(upem, isTTF=True)
    builder.setupGlyphOrder(order)
    builder.setupCharacterMap(dict(glyphs))
    builder.setupGlyf({name: square(upem) for name in order})
    builder.setupHorizontalMetrics({name: (upem // 2 + 100, 50) for name in order})
    builder.setupHorizontalHeader(ascent=line[0], descent=line[1], lineGap=line[2])
    builder.setupNameTable({"familyName": "Test", "styleName": "Regular"})
    builder.setupOS2(sTypoAscender=typo[0], sTypoDescender=typo[1], sTypoLineGap=typo[2],
                     usWinAscent=window[0], usWinDescent=window[1], fsSelection=selection,
                     sCapHeight=700, sxHeight=500)
    builder.setupPost()

    if axes:
        builder.setupFvar(axes, [])

    if features:
        addOpenTypeFeaturesFromString(builder.font, features)

    return builder.font
