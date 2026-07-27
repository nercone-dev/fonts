from nercone_fonts.design import Axis, Space
from nercone_fonts.prepare import Component
from nercone_fonts.merge import Merger

from conftest import build_font

def kerning(table: object, script: str) -> list:
    for record in table.ScriptList.ScriptRecord:
        if record.ScriptTag != script:
            continue
        found = []
        for index in record.Script.DefaultLangSys.FeatureIndex:
            feature = table.FeatureList.FeatureRecord[index]
            if feature.FeatureTag == "kern":
                found += feature.Feature.LookupListIndex
        return found
    return []

def covered(table: object, indices: list) -> set:
    glyphs = set()
    for index in indices:
        for subtable in table.LookupList.Lookup[index].SubTable:
            if getattr(subtable, "Coverage", None) is not None:
                glyphs.update(subtable.Coverage.glyphs)
    return glyphs

def merged() -> object:
    base = Component(build_font({0x41: "A", 0x56: "V"}, features="""
        languagesystem DFLT dflt;
        feature kern { pos A V -40; } kern;
    """), "Base")

    addon = Component(build_font({0x3042: "B", 0x3044: "C"}, features="""
        languagesystem latn dflt;
        feature kern { pos B C -30; } kern;
    """), "Addon")

    return Merger(base, [addon], Space(Axis(400, 400, 400)), variable=False).build()

def test_default_script_kerning_reaches_scripts_added_by_addons():
    font = merged()
    table = font["GPOS"].table

    assert covered(table, kerning(table, "latn")) >= {"A", "B"}

def test_default_script_keeps_kerning_from_every_component():
    font = merged()
    table = font["GPOS"].table

    assert "A" in covered(table, kerning(table, "DFLT"))
