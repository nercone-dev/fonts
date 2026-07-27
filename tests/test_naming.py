from nercone_fonts.models import Slope, Style, License, Typeface, Family
from nercone_fonts.design import Axis
from nercone_fonts.naming import Names

from conftest import build_font

def named() -> object:
    font = build_font({0x41: "A", 0x56: "V"}, axes=[
        ("opsz", 14.0, 14.0, 32.0, "Optical Size"),
        ("wght", 100.0, 400.0, 900.0, "Weight")
    ])

    family = Family("Test", "Test", License("Test License", "https://example.com/", filepath="licenses/OFL.txt"), latin=Typeface("Test"))
    Names(family, Style(None, Slope.Upright), Axis(100, 400, 900), "1.0").apply(font)
    return font

def test_axis_names_match_their_tags():
    font = named()
    table = font["name"]

    labels = {entry.axisTag: table.getDebugName(entry.axisNameID) for entry in font["fvar"].axes}
    assert labels == {"opsz": "Optical Size", "wght": "Weight"}

def test_instances_declare_a_coordinate_for_every_axis():
    font = named()

    for instance in font["fvar"].instances:
        assert set(instance.coordinates) == {"opsz", "wght"}
        assert instance.coordinates["opsz"] == 14.0

def test_style_attributes_cover_every_axis():
    font = named()

    tags = {record.AxisTag for record in font["STAT"].table.DesignAxisRecord.Axis}
    assert {"opsz", "wght", "ital"} <= tags
