import io
import os
import collections
from typing import Optional, List, Dict, Tuple

from fontTools.ttLib import TTFont
from fontTools.varLib import instancer
from fontTools.varLib.hvar import add_HVAR
from fontTools.varLib.featureVars import addFeatureVariations

from .models import Weight, Slope, Style, Format, Typeface, Family
from .constants import Paths, version
from .design import Axis, Space
from .prepare import Component, Features
from .merge import Merger
from .metrics import Metrics
from .naming import Names, Notice
from .outlines import Outlines

emphasis = 600.0
limit = 0xFFFF

class Builder:
    def __init__(self, family: Family, logger=None, formats: Optional[List[Format]] = None, directory: str = Paths.files):
        self.family = family
        self.logger = logger
        self.formats = list(formats) if formats else list(Format)
        self.directory = directory
        self.axis = Axis()
        self.notice = ""

    def note(self, message: str):
        if self.logger:
            self.logger.info(message)

    def build(self) -> List[str]:
        os.makedirs(self.directory, exist_ok=True)

        fonts = [TTFont(io.BytesIO(source.read()), lazy=True) for source in self.family.sources()]
        self.axis = Axis.of(fonts)
        self.notice = Notice.of(fonts)

        written = []
        for slope in Slope:
            self.note("{}: composing {}".format(self.family.name, slope.value.lower()))
            font, metrics, advance = self.compose(slope)

            style = Style(None, slope)
            self.finish(font, style, metrics, advance)
            data = self.write(font, style)
            written += self.paths(style)

            for weight in (Weight.Regular, Weight.Bold):
                style = Style(weight, slope)
                self.note("{}: instancing {}".format(self.family.name, style.name()))
                static = instancer.instantiateVariableFont(
                    TTFont(io.BytesIO(data)), {Space.tag: float(weight.value)},
                    inplace=True, updateFontNames=False)
                self.finish(static, style, metrics, advance)
                self.write(static, style)
                written += self.paths(style)

        return written

    def compose(self, slope: Slope) -> Tuple[TTFont, Metrics, Optional[int]]:
        family = self.family

        base = self.component(family.latin, slope, Weight.Regular, prefix="")
        upem = base.font["head"].unitsPerEm
        base.prepare(self.axis, upem, retain=True)

        advance = self.cell(base) if family.monospace else None
        if advance:
            base.monospace(advance)

        claimed = set(base.codepoints)
        addons, reference = [], None

        for typeface in family.cjk:
            component = self.component(typeface, slope, Weight.Regular, typeface.prefix)
            component.codepoints -= claimed
            component.prepare(self.axis, upem)
            if advance:
                component.monospace(advance)
            claimed |= component.codepoints
            addons.append(component)
            reference = reference or component

        if family.symbols:
            component = self.component(family.symbols, slope, Weight.Regular, family.symbols.prefix)
            component.codepoints -= claimed
            component.prepare(self.axis, upem, self.ratio(component, upem, advance))
            if advance:
                component.monospace(advance)
            claimed |= component.codepoints
            addons.append(component)

        bold = None
        if not family.latin.variable():
            bold = self.component(family.latin, slope, Weight.Bold, prefix="bold.")
            bold.prepare(self.axis, upem)
            if advance:
                bold.monospace(advance)
            addons.append(bold)

        components = [base] + addons
        metrics = Metrics.of(components, italic=slope.italic())
        substitutions = self.emphasise(base, bold)

        space = self.space(reference or base)
        masters = self.masters(components, space)
        self.note("{}: {} masters at {}".format(
            self.family.name, len(masters), ", ".join(str(int(weight)) for weight in masters)))

        for component in components:
            component.retarget(space, masters)

        font = Merger(base, addons, space).build()

        if len(font.getGlyphOrder()) > limit:
            raise ValueError("{} needs {} glyphs, more than the {} a font can hold".format(
                self.family.name, len(font.getGlyphOrder()), limit))

        if substitutions:
            addFeatureVariations(font, [([{Space.tag: (space.normalize(emphasis), 1.0)}], substitutions)])

        return font, metrics, advance

    def space(self, reference: Component) -> Space:
        found = reference.space()
        return Space(self.axis, found.segments if found else None)

    def masters(self, components: List[Component], space: Space) -> List[float]:
        found = set(space.breakpoints())
        for component in components:
            found.update(component.breakpoints(space))

        return sorted({float(round(weight)) for weight in found if self.axis.minimum <= weight <= self.axis.maximum} | {self.axis.default})

    def component(self, typeface: Typeface, slope: Slope, weight: Weight, prefix: str) -> Component:
        source = typeface.source(slope, weight)
        return Component.load(source.read(), typeface.name, prefix=prefix, features=self.features(typeface))

    def features(self, typeface: Typeface) -> List[str]:
        if typeface is self.family.latin:
            default = Features.latin
        elif typeface is self.family.symbols:
            default = Features.symbols
        else:
            default = Features.cjk

        if not self.family.monospace:
            return default

        dropped = set(Features.proportional) | set(Features.ligating)
        return [tag for tag in (Features.cjk if "*" in default else default) if tag not in dropped]

    def cell(self, base: Component) -> int:
        widths = collections.Counter(width for width, _ in base.font["hmtx"].metrics.values() if width > 0)
        return widths.most_common(1)[0][0]

    def ratio(self, component: Component, upem: int, advance: Optional[int]) -> float:
        if not advance:
            return 1.0

        widths = collections.Counter(width for width, _ in component.font["hmtx"].metrics.values() if width > 0)
        return advance * component.font["head"].unitsPerEm / (widths.most_common(1)[0][0] * upem)

    def emphasise(self, base: Component, bold: Optional[Component]) -> Dict[str, str]:
        if bold is None:
            return {}

        heavy = bold.cmap()
        return {name: heavy[code] for code, name in base.cmap().items()
                if code in heavy and heavy[code] != name}

    def finish(self, font: TTFont, style: Style, metrics: Metrics, advance: Optional[int]):
        if not style.variable():
            self.flatten(font)

        metrics.apply(font, self.family, style, float(version), advance)
        Names(self.family, style, self.axis, version, self.notice).apply(font)

        if "fvar" in font and "gvar" in font:
            add_HVAR(font)

    def flatten(self, font: TTFont):
        if "GSUB" not in font:
            return

        table = font["GSUB"].table
        substitutions = {}
        for record in table.FeatureList.FeatureRecord:
            if record.FeatureTag != "rvrn":
                continue
            for index in record.Feature.LookupListIndex:
                for subtable in table.LookupList.Lookup[index].SubTable:
                    substitutions.update(getattr(subtable, "mapping", None) or {})

        if not substitutions:
            return

        for subtable in font["cmap"].tables:
            subtable.cmap = {code: substitutions.get(name, name) for code, name in subtable.cmap.items()}

    def path(self, style: Style, format: Format) -> str:
        return os.path.join(self.directory, "{}-{}.{}".format(
            self.family.filename, style.name(), format.extension()))

    def paths(self, style: Style) -> List[str]:
        return [self.path(style, format) for format in self.formats]

    def write(self, font: TTFont, style: Style) -> bytes:
        buffer = io.BytesIO()
        font.save(buffer)
        data = buffer.getvalue()

        for format in self.formats:
            path = self.path(style, format)
            if format is Format.TTF:
                with open(path, "wb") as f:
                    f.write(data)
            elif format is Format.OTF:
                with open(path, "wb") as f:
                    f.write(self.compact(data, style))
            else:
                other = TTFont(io.BytesIO(data))
                other.flavor = format.flavor()
                other.save(path)

        self.note("{}: wrote {} {}".format(self.family.name, style.name(), "/".join(format.directory() for format in self.formats)))
        return data

    def compact(self, data: bytes, style: Style) -> bytes:
        if style.variable():
            return data

        font = TTFont(io.BytesIO(data))
        if "fvar" in font:
            location = {entry.axisTag: entry.defaultValue for entry in font["fvar"].axes}
            instancer.instantiateVariableFont(font, location, inplace=True, updateFontNames=False)

        buffer = io.BytesIO()
        Outlines.compact(font, self.family, style, version).save(buffer)
        return buffer.getvalue()
