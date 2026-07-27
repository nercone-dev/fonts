from nercone_fonts.models import Weight, Slope, Style, License, Typeface, Family
from nercone_fonts.prepare import Component
from nercone_fonts.metrics import Metrics

from conftest import build_font

def family() -> Family:
    return Family("Test", "Test", License("Test License", "https://example.com/", filepath="licenses/OFL.txt"), latin=Typeface("Test"))

def components(selection: int) -> list:
    base = Component(build_font({0x41: "A", 0x56: "V"}, typo=(760, -240, 216), line=(980, -236, 0), window=(980, 236), selection=selection), "Base")
    addon = Component(build_font({0x3042: "B"}, window=(1160, 288)), "Addon", prefix="x.")
    return [base, addon]

def test_typo_metrics_flag_preserved_when_base_sets_it():
    base, addon = components(0x40 | 0x80)
    metrics = Metrics.of([base, addon])
    metrics.apply(base.font, family(), Style(Weight.Regular, Slope.Upright), 1.0)

    assert base.font["OS/2"].fsSelection & 0x0080

def test_typo_metrics_flag_omitted_when_base_lacks_it():
    base, addon = components(0x40)
    metrics = Metrics.of([base, addon])
    metrics.apply(base.font, family(), Style(Weight.Regular, Slope.Upright), 1.0)

    assert not base.font["OS/2"].fsSelection & 0x0080

def test_line_metrics_follow_base():
    base, addon = components(0x40)
    metrics = Metrics.of([base, addon])
    metrics.apply(base.font, family(), Style(Weight.Regular, Slope.Upright), 1.0)

    assert base.font["hhea"].ascender == 980
    assert base.font["hhea"].descender == -236
    assert base.font["OS/2"].sTypoAscender == 760
    assert base.font["OS/2"].sTypoDescender == -240
    assert base.font["OS/2"].sTypoLineGap == 216

def test_window_metrics_cover_every_component():
    metrics = Metrics.of(components(0x40))

    assert metrics.window_ascent == 1160
    assert metrics.window_descent == 288
