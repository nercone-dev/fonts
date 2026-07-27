from nercone_fonts.design import Axis
from nercone_fonts.prepare import Component

from conftest import build_font

def component() -> Component:
    font = build_font({0x41: "A", 0x56: "V"}, axes=[
        ("opsz", 14.0, 14.0, 32.0, "Optical Size"),
        ("wght", 100.0, 400.0, 900.0, "Weight")
    ])
    return Component(font, "Base")

def test_rebase_retains_foreign_axes_when_asked():
    base = component()
    base.rebase(Axis(100, 400, 900), retain=True)

    assert [entry.axisTag for entry in base.font["fvar"].axes] == ["opsz", "wght"]

def test_rebase_pins_foreign_axes_by_default():
    addon = component()
    addon.rebase(Axis(100, 400, 900))

    assert [entry.axisTag for entry in addon.font["fvar"].axes] == ["wght"]

def test_rebase_limits_the_merge_axis():
    base = component()
    base.rebase(Axis(300, 400, 700), retain=True)

    entry = [entry for entry in base.font["fvar"].axes if entry.axisTag == "wght"][0]
    assert (entry.minValue, entry.defaultValue, entry.maxValue) == (300.0, 400.0, 700.0)

def test_monospace_rounds_advances_to_whole_cells():
    addon = Component(build_font({0x41: "A"}), "Addon")
    addon.monospace(350)

    assert addon.font["hmtx"]["A"] == (700, 100)

def test_monospace_centers_outlines_without_scaling_them():
    addon = Component(build_font({0x41: "A"}), "Addon")
    before = list(addon.font["glyf"]["A"].getCoordinates(addon.font["glyf"])[0])
    addon.monospace(350)
    after = list(addon.font["glyf"]["A"].getCoordinates(addon.font["glyf"])[0])

    assert after == [(x + 50, y) for x, y in before]
