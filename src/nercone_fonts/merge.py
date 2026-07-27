import copy
from typing import List

from fontTools.ttLib import TTFont, newTable
from fontTools.fontBuilder import FontBuilder
from fontTools.ttLib.tables import otTables
from fontTools.ttLib.tables._f_v_a_r import Axis as FvarAxis
from fontTools.merge.layout import layoutPreMerge, layoutPostMerge, mergeScriptRecords

from .design import Axis, Space
from .prepare import Component

def empty(tag: str) -> object:
    table = newTable(tag)
    table.table = getattr(otTables, tag)()
    table.table.Version = 0x00010000

    table.table.ScriptList = otTables.ScriptList()
    table.table.ScriptList.ScriptRecord = []
    table.table.ScriptList.ScriptCount = 0

    table.table.FeatureList = otTables.FeatureList()
    table.table.FeatureList.FeatureRecord = []
    table.table.FeatureList.FeatureCount = 0

    table.table.LookupList = otTables.LookupList()
    table.table.LookupList.Lookup = []
    table.table.LookupList.LookupCount = 0

    table.table.FeatureVariations = None

    return table

def definitions() -> object:
    table = newTable("GDEF")
    table.table = otTables.GDEF()
    table.table.Version = 0x00010000
    table.table.GlyphClassDef = None
    table.table.AttachList = None
    table.table.LigCaretList = None
    table.table.MarkAttachClassDef = None
    table.table.MarkGlyphSetsDef = None
    table.table.VarStore = None

    return table

def store() -> otTables.VarStore:
    table = otTables.VarStore()
    table.Format = 1
    table.VarRegionList = otTables.VarRegionList()
    table.VarRegionList.Region = []
    table.VarRegionList.RegionCount = 0
    table.VarRegionList.RegionAxisCount = 0
    table.VarData = []
    table.VarDataCount = 0

    return table

class Merger:
    layout = ["GSUB", "GPOS"]

    def __init__(self, base: Component, addons: List[Component], space: Space, variable: bool = True):
        self.base = base
        self.addons = list(addons)
        self.space = space
        self.variable = variable

        self.font = base.font
        self.assignments = base.cmap()

    def build(self) -> TTFont:
        self.open()
        for addon in self.addons:
            self.append(addon)
        self.close()
        return self.font

    def components(self) -> List[Component]:
        return [self.base] + self.addons

    def open(self):
        if self.variable and "fvar" not in self.font:
            self.font["fvar"] = self.axes()
            table = self.space.avar()
            if table is not None:
                self.font["avar"] = table

        if self.variable and "gvar" not in self.font:
            variations = newTable("gvar")
            variations.version, variations.reserved, variations.variations = 1, 0, {}
            self.font["gvar"] = variations

        for tag in Merger.layout:
            if tag not in self.font and any(tag in addon.font for addon in self.addons):
                self.font[tag] = empty(tag)
        if "GDEF" not in self.font and any("GDEF" in addon.font for addon in self.addons):
            self.font["GDEF"] = definitions()

        if "vmtx" not in self.font:
            self.vertical()

        for tag in Merger.layout:
            self.defaults(tag)

        for component in self.components():
            layoutPreMerge(component.font)

    def axes(self) -> object:
        axis = self.space.axis
        table = newTable("fvar")
        entry = FvarAxis()
        entry.axisTag = Space.tag
        entry.minValue, entry.defaultValue, entry.maxValue = axis.minimum, axis.default, axis.maximum
        entry.flags = 0
        entry.axisNameID = 256
        table.axes = [entry]
        table.instances = []

        return table

    def vertical(self):
        for addon in self.addons:
            if "vhea" not in addon.font or "vmtx" not in addon.font:
                continue
            self.font["vhea"] = addon.font["vhea"]
            metrics = newTable("vmtx")
            metrics.metrics = {}
            self.font["vmtx"] = metrics
            return

    def defaults(self, tag: str):
        tables = [component.font[tag].table for component in self.components() if tag in component.font]
        names = {record.ScriptTag for table in tables for record in table.ScriptList.ScriptRecord}

        for table in tables:
            records = {record.ScriptTag: record for record in table.ScriptList.ScriptRecord}
            default = records.get("DFLT")
            if default is None:
                continue

            for name in sorted(names - set(records)):
                record = otTables.ScriptRecord()
                record.ScriptTag = name
                record.Script = copy.deepcopy(default.Script)
                table.ScriptList.ScriptRecord.append(record)

            table.ScriptList.ScriptRecord.sort(key=lambda record: record.ScriptTag)
            table.ScriptList.ScriptCount = len(table.ScriptList.ScriptRecord)

    def append(self, addon: Component):
        names = addon.glyphs()
        self.font.setGlyphOrder(list(self.font.getGlyphOrder()) + names)

        outlines, other = self.font["glyf"], addon.font["glyf"]
        for name in names:
            outlines[name] = other[name]

        metrics, source = self.font["hmtx"].metrics, addon.font["hmtx"].metrics
        for name in names:
            metrics[name] = source[name]

        if "vmtx" in self.font and "vmtx" in addon.font:
            vertical, source = self.font["vmtx"].metrics, addon.font["vmtx"].metrics
            for name in names:
                if name in source:
                    vertical[name] = source[name]

        if self.variable and "gvar" in addon.font:
            variations, source = self.font["gvar"].variations, addon.font["gvar"].variations
            for name in names:
                if name in source and source[name]:
                    variations[name] = source[name]

        for codepoint, name in addon.cmap().items():
            self.assignments.setdefault(codepoint, name)

        self.variations(addon)
        for tag in Merger.layout:
            self.substitutions(addon, tag)
        self.classes(addon)

    def variations(self, addon: Component):
        if "GDEF" not in addon.font or getattr(addon.font["GDEF"].table, "VarStore", None) is None:
            return

        source = addon.font["GDEF"].table.VarStore

        tags = [entry.axisTag for entry in self.font["fvar"].axes] if "fvar" in self.font else [Space.tag]
        others = [entry.axisTag for entry in addon.font["fvar"].axes] if "fvar" in addon.font else [Space.tag]
        if tags != others:
            for region in source.VarRegionList.Region:
                entries = dict(zip(others, region.VarRegionAxis))
                rebuilt = []
                for tag in tags:
                    entry = entries.get(tag)
                    if entry is None:
                        entry = otTables.VarRegionAxis()
                        entry.StartCoord, entry.PeakCoord, entry.EndCoord = 0.0, 0.0, 0.0
                    rebuilt.append(entry)
                region.VarRegionAxis = rebuilt
            source.VarRegionList.RegionAxisCount = len(tags)

        target = getattr(self.font["GDEF"].table, "VarStore", None)
        if target is None:
            target = self.font["GDEF"].table.VarStore = store()
            target.VarRegionList.RegionAxisCount = source.VarRegionList.RegionAxisCount

        regions = len(target.VarRegionList.Region)
        outer = len(target.VarData)

        indices = set()
        addon.font["GDEF"].table.collect_device_varidxes(indices)
        if "GPOS" in addon.font:
            addon.font["GPOS"].table.collect_device_varidxes(indices)

        mapping = {index: index if index == 0xFFFFFFFF else (((index >> 16) + outer) << 16) | (index & 0xFFFF) for index in indices}
        addon.font["GDEF"].table.remap_device_varidxes(mapping)
        if "GPOS" in addon.font:
            addon.font["GPOS"].table.remap_device_varidxes(mapping)

        for data in source.VarData:
            data.VarRegionIndex = [index + regions for index in data.VarRegionIndex]

        target.VarRegionList.Region += source.VarRegionList.Region
        target.VarRegionList.RegionCount = len(target.VarRegionList.Region)
        target.VarData += source.VarData
        target.VarDataCount = len(target.VarData)

        addon.font["GDEF"].table.VarStore = None

    def substitutions(self, addon: Component, tag: str):
        if tag not in addon.font:
            return

        base, source = self.font[tag].table, addon.font[tag].table

        base.LookupList.Lookup += source.LookupList.Lookup
        base.LookupList.LookupCount = len(base.LookupList.Lookup)

        base.FeatureList.FeatureRecord += source.FeatureList.FeatureRecord
        base.FeatureList.FeatureCount = len(base.FeatureList.FeatureRecord)

        base.ScriptList.ScriptRecord = mergeScriptRecords([base.ScriptList.ScriptRecord, source.ScriptList.ScriptRecord])
        base.ScriptList.ScriptCount = len(base.ScriptList.ScriptRecord)

        base.Version = 0x00010001 if getattr(base, "FeatureVariations", None) is not None else 0x00010000

    def classes(self, addon: Component):
        if "GDEF" not in addon.font:
            return

        base, source = self.font["GDEF"].table, addon.font["GDEF"].table

        for name in ("GlyphClassDef", "MarkAttachClassDef"):
            other = getattr(source, name, None)
            if other is None:
                continue
            own = getattr(base, name, None)
            if own is None:
                setattr(base, name, other)
            else:
                own.classDefs.update(other.classDefs)

        for name, count, records in (("AttachList", "GlyphCount", "AttachPoint"), ("LigCaretList", "LigGlyphCount", "LigGlyph")):
            other = getattr(source, name, None)
            if other is None:
                continue
            own = getattr(base, name, None)
            if own is None:
                setattr(base, name, other)
                continue
            own.Coverage.glyphs += other.Coverage.glyphs
            setattr(own, records, getattr(own, records) + getattr(other, records))
            setattr(own, count, len(getattr(own, records)))

        if getattr(source, "MarkGlyphSetsDef", None) is not None:
            if getattr(base, "MarkGlyphSetsDef", None) is None:
                base.MarkGlyphSetsDef = source.MarkGlyphSetsDef
            else:
                base.MarkGlyphSetsDef.Coverage += source.MarkGlyphSetsDef.Coverage
                base.MarkGlyphSetsDef.MarkSetCount = len(base.MarkGlyphSetsDef.Coverage)

        base.Version = 0x00010002 if getattr(base, "MarkGlyphSetsDef", None) is not None else 0x00010000

    def close(self):
        self.characters()

        layoutPostMerge(self.font)

        if "GDEF" in self.font and getattr(self.font["GDEF"].table, "VarStore", None) is not None:
            self.font["GDEF"].table.Version = 0x00010003

        self.font["maxp"].numGlyphs = len(self.font.getGlyphOrder())

        if "vmtx" in self.font:
            self.heights()

        for tag in Merger.layout:
            if tag in self.font and not self.font[tag].table.LookupList.Lookup:
                del self.font[tag]

    def characters(self) -> object:
        builder = FontBuilder(font=self.font)
        builder.setupCharacterMap(dict(self.assignments), allowFallback=True)

        return self.font["cmap"]

    def heights(self):
        metrics = self.font["vmtx"].metrics
        outlines = self.font["glyf"]
        upem = self.font["head"].unitsPerEm
        ascent = self.font["vhea"].ascent if "vhea" in self.font else upem // 2

        for name in self.font.getGlyphOrder():
            if name in metrics:
                continue
            glyph = outlines[name]
            top = glyph.yMax if glyph.numberOfContours != 0 else 0
            metrics[name] = (upem, ascent - top)
