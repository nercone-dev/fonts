import io
from typing import Optional, Iterable, List, Dict, Set

from fontTools import subset
from fontTools.ttLib import TTFont
from fontTools.ttLib.scaleUpem import scale_upem
from fontTools.varLib import instancer

from .design import Axis, Space, Mapping

def features(extra: Iterable[str] = (), without: Iterable[str] = ()) -> List[str]:
    default = subset.Options().layout_features
    return sorted((set(default) | set(extra)) - set(without))

class Features:
    latin = ["*"]
    cjk = features(extra=["fwid", "hwid", "pwid", "palt", "ruby"])
    symbols = features()

    proportional = ["kern", "vkrn", "palt", "halt", "vpal", "vhal", "pwid", "twid", "qwid", "chws", "vchw"]
    ligating = ["liga", "dlig", "clig", "hlig", "rlig", "calt", "rclt"]

class Tables:
    apple = ["morx", "mort", "feat", "prop", "kerx", "kern", "ankr", "bsln", "lcar", "opbd", "trak", "just", "Zapf", "acnt", "fdsc", "fmtx"]

    private = ["DSIG", "PfEd", "FFTM", "TeX ", "Silf", "Glat", "Gloc", "Feat", "Sill", "gasp", "MVAR", "STAT", "cvar", "BASE"]

    dropped = apple + private

class Component:
    def __init__(self, font: TTFont, name: str, prefix: str = "", codepoints: Optional[Set[int]] = None, features: Optional[List[str]] = None):
        self.font = font
        self.name = name
        self.prefix = prefix
        self.codepoints = set(font.getBestCmap()) if codepoints is None else set(codepoints)
        self.features = Features.latin if features is None else features

    @staticmethod
    def load(data: bytes, name: str, **kwargs) -> "Component":
        return Component(TTFont(io.BytesIO(data)), name, **kwargs)

    def prepare(self, axis: Axis, upem: int, scale: float = 1.0, retain: bool = False) -> "Component":
        self.subset()
        self.rebase(axis, retain)
        self.scale(upem, scale)
        self.rename()
        return self

    def space(self) -> Optional[Space]:
        return Space.read(self.font)

    def breakpoints(self, space: Space) -> List[float]:
        return Mapping(self.font, space).breakpoints()

    def retarget(self, space: Space, masters: List[float]):
        Mapping(self.font, space).apply(masters)

    def subset(self):
        options = subset.Options()
        options.layout_features = list(self.features)
        options.layout_scripts = ["*"]
        options.glyph_names = True
        options.hinting = False
        options.notdef_glyph = True
        options.notdef_outline = True
        options.recommended_glyphs = False
        options.passthrough_tables = False
        options.drop_tables = sorted(set(options.drop_tables) | set(Tables.dropped))
        options.prune_unicode_ranges = False
        options.prune_codepage_ranges = False
        options.ignore_missing_unicodes = True

        subsetter = subset.Subsetter(options=options)
        subsetter.populate(unicodes=self.codepoints)
        subsetter.subset(self.font)

        self.codepoints = set(self.font.getBestCmap())

    def rebase(self, axis: Axis, retain: bool = False):
        if "fvar" not in self.font:
            return

        location = {}
        for entry in self.font["fvar"].axes:
            if entry.axisTag != axis.tag:
                if not retain:
                    location[entry.axisTag] = entry.defaultValue
                continue
            location[entry.axisTag] = (max(entry.minValue, axis.minimum), min(max(entry.minValue, axis.default), entry.maxValue), min(entry.maxValue, axis.maximum))

        if location:
            instancer.instantiateVariableFont(self.font, location, inplace=True, updateFontNames=False, optimize=False)

    def scale(self, upem: int, factor: float = 1.0):
        current = self.font["head"].unitsPerEm
        target = int(round(upem * factor))

        if current != target:
            scale_upem(self.font, target)
        self.font["head"].unitsPerEm = upem

    def rename(self):
        if not self.prefix:
            return

        order = self.font.getGlyphOrder()
        renamed = [name if index == 0 else self.prefix + name for index, name in enumerate(order)]

        data = io.BytesIO()
        self.font.save(data)
        data.seek(0)

        self.font = TTFont(data)
        self.font.setGlyphOrder(renamed)
        self.codepoints = set(self.font.getBestCmap())

    def glyphs(self) -> List[str]:
        return [name for name in self.font.getGlyphOrder() if name != ".notdef"]

    def cmap(self) -> Dict[int, str]:
        return dict(self.font.getBestCmap())

    def monospace(self, advance: int):
        metrics = self.font["hmtx"].metrics
        outlines = self.font["glyf"] if "glyf" in self.font else None

        shifts = {}
        for name, (width, bearing) in list(metrics.items()):
            cells = max(1, int(round(width / advance))) if width else 1
            shift = (cells * advance - width) // 2 if width else 0
            shifts[name] = shift
            metrics[name] = (cells * advance, bearing + shift)

        if outlines is not None:
            for name, shift in shifts.items():
                glyph = outlines[name]
                if glyph.isComposite():
                    for component in glyph.components:
                        if hasattr(component, "x"):
                            component.x += shift - shifts.get(component.glyphName, 0)
                elif glyph.numberOfContours > 0 and shift:
                    glyph.coordinates.translate((shift, 0))

        self.freeze()

    def freeze(self):
        if "gvar" not in self.font:
            return

        for variations in self.font["gvar"].variations.values():
            for variation in variations:
                coordinates = variation.coordinates
                for index in (len(coordinates) - 4, len(coordinates) - 3):
                    if index >= 0:
                        coordinates[index] = (0, 0)

        if "HVAR" in self.font:
            del self.font["HVAR"]
