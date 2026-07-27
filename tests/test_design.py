import copy

from fontTools.ttLib import newTable
from fontTools.ttLib.tables.TupleVariation import TupleVariation
from fontTools.varLib import instancer

from nercone_fonts.design import Axis, Space
from nercone_fonts.prepare import Component

from conftest import build_font

def variable_font() -> object:
    font = build_font({0x41: "A", 0x56: "V"}, axes=[
        ("opsz", 10.0, 10.0, 20.0, "Optical Size"),
        ("wght", 100.0, 400.0, 900.0, "Weight")
    ])

    avar = newTable("avar")
    avar.segments = {
        "opsz": {-1.0: -1.0, 0.0: 0.0, 1.0: 1.0},
        "wght": {-1.0: -1.0, 0.0: 0.0, 0.5: 0.8, 1.0: 1.0}
    }
    font["avar"] = avar

    gvar = newTable("gvar")
    gvar.version, gvar.reserved = 1, 0
    gvar.variations = {"A": [
        TupleVariation({"wght": (0.0, 1.0, 1.0)}, [(30, 0), (30, 40), (60, 40), (60, 0), (0, 0), (80, 0), (0, 0), (0, 0)]),
        TupleVariation({"opsz": (0.0, 1.0, 1.0)}, [(-10, 0), (-10, -20), (-20, -20), (-20, 0), (0, 0), (-40, 0), (0, 0), (0, 0)]),
        TupleVariation({"opsz": (0.0, 1.0, 1.0), "wght": (0.0, 1.0, 1.0)}, [(5, 5), (5, 5), (5, 5), (5, 5), (0, 0), (10, 0), (0, 0), (0, 0)])
    ]}
    font["gvar"] = gvar

    return font

def outline(font: object, location: dict) -> list:
    static = instancer.instantiateVariableFont(font, location, inplace=False, optimize=False)
    coordinates = static["glyf"]["A"].getCoordinates(static["glyf"])[0]
    return [static["hmtx"]["A"]] + list(coordinates)

def retargeted() -> tuple:
    component = Component(variable_font(), "Base")
    original = copy.deepcopy(component.font)

    space = Space(Axis(100.0, 400.0, 900.0), [(-1.0, -1.0), (0.0, 0.0), (0.5, 0.6), (1.0, 1.0)])
    masters = sorted(set(space.breakpoints()) | set(component.breakpoints(space)))
    component.retarget(space, masters)

    return original, component.font, masters

def test_retarget_preserves_outlines_across_every_axis():
    original, rebuilt, masters = retargeted()

    for opsz in (10.0, 15.0, 20.0):
        for weight in masters:
            location = {"opsz": opsz, "wght": weight}
            before, after = outline(original, dict(location)), outline(rebuilt, dict(location))
            for (x1, y1), (x2, y2) in zip(before, after):
                assert abs(x1 - x2) <= 1 and abs(y1 - y2) <= 1, (location, before, after)

def test_retarget_keeps_foreign_axes_declared():
    original, rebuilt, masters = retargeted()

    assert [entry.axisTag for entry in rebuilt["fvar"].axes] == ["opsz", "wght"]
    assert rebuilt["avar"].segments["opsz"] == {-1.0: -1.0, 0.0: 0.0, 1.0: 1.0}
    assert rebuilt["avar"].segments["wght"] == {-1.0: -1.0, 0.0: 0.0, 0.5: 0.6, 1.0: 1.0}
