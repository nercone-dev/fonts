from typing import Optional, List, Dict

from fontTools.ttLib import TTFont, newTable
from fontTools.ttLib.tables._f_v_a_r import NamedInstance
from fontTools.otlLib.builder import buildStatTable

from .models import Weight, Style, Family
from .design import Axis

windows = (3, 1, 0x409)

titles = {"wght": "Weight", "wdth": "Width", "opsz": "Optical Size", "ital": "Italic", "slnt": "Slant"}

class Names:
    copyright = 0
    family = 1
    variant = 2
    identifier = 3
    full = 4
    version = 5
    postscript = 6
    description = 10
    license = 13
    license_url = 14

    def __init__(self, family: Family, style: Style, axis: Axis, version: str, notice: str = ""):
        self.subject = family
        self.style = style
        self.axis = axis
        self.release = version
        self.notice = notice

    def records(self) -> Dict[int, str]:
        family, style = self.subject, self.style

        return {
            Names.copyright:   self.notice,
            Names.family:      family.name,
            Names.variant:     style.name(),
            Names.identifier:  family.filename,
            Names.full:        "{} {}".format(family.name, style.name()),
            Names.version:     "Version {}".format(self.release),
            Names.postscript:  "{}-{}".format(family.filename, style.name()),
            Names.description: family.description(),
            Names.license:     family.license.name,
            Names.license_url: family.license.url
        }

    def apply(self, font: TTFont):
        table = newTable("name")
        table.names = []
        font["name"] = table

        for identifier, value in sorted(self.records().items()):
            if value:
                table.setName(value, identifier, *windows)

        if "fvar" in font:
            self.instances(font)
        self.axes(font)

    def instances(self, font: TTFont):
        table, fvar = font["name"], font["fvar"]

        for entry in fvar.axes:
            entry.axisNameID = table.addName(titles.get(entry.axisTag, entry.axisTag), minNameID=255)

        fvar.instances = []
        if not any(entry.axisTag == self.axis.tag for entry in fvar.axes):
            return

        for weight in self.axis.weights():
            instance = NamedInstance()
            instance.subfamilyNameID = table.addName(weight.name + self.style.slope.suffix(), minNameID=255)
            instance.postscriptNameID = 0xFFFF
            instance.coordinates = {entry.axisTag: entry.defaultValue for entry in fvar.axes}
            instance.coordinates[self.axis.tag] = float(weight.value)
            fvar.instances.append(instance)

    def axes(self, font: TTFont):
        weights = self.axis.weights() if self.style.variable() else [self.style.weight]
        italic = self.style.italic()

        values = []
        for weight in weights:
            value = {"value": float(weight.value), "name": weight.name}
            if weight is Weight.Regular:
                value["flags"] = 0x2
                value["linkedValue"] = float(Weight.Bold.value)
            values.append(value)

        entries = [
            {"tag": "wght", "name": "Weight", "values": values},
            {"tag": "ital", "name": "Italic", "values": [{"value": 1.0, "name": "Italic"} if italic else {"value": 0.0, "name": "Roman", "flags": 0x2, "linkedValue": 1.0}]}
        ]

        if "fvar" in font:
            declared = {entry["tag"] for entry in entries}
            entries += [{"tag": entry.axisTag, "name": titles.get(entry.axisTag, entry.axisTag)} for entry in font["fvar"].axes if entry.axisTag not in declared]

        for ordering, entry in enumerate(entries):
            entry["ordering"] = ordering

        buildStatTable(font, entries, elidedFallbackName="Regular", macNames=False)

class Notice:
    @staticmethod
    def of(fonts: List[TTFont]) -> str:
        notices = []
        for font in fonts:
            if "name" not in font:
                continue
            value = font["name"].getDebugName(Names.copyright)
            if value:
                value = " ".join(value.split())
                if value not in notices:
                    notices.append(value)

        return "\n".join(notices)
