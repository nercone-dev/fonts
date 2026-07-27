mod common;

use nercone_fonts::metrics::Metrics;
use nercone_fonts::models::{Family, License, Slope, Style, Typeface, Weight};
use nercone_fonts::prepare::Component;

use common::{build_font, Specimen};

pub fn family() -> Family {
    Family {
        name: "Test".to_string(),
        filename: "Test".to_string(),
        license: License { name: "Test License", url: "https://example.com/", filepath: "licenses/OFL.txt", filename: "LICENSE" },
        latin: Typeface { name: "Test".to_string(), sources: Vec::new(), prefix: String::new() },
        cjk: Vec::new(),
        symbols: None,
        typeface: "Sans".to_string(),
        region: "CJK".to_string(),
        monospace: false,
    }
}

pub fn components(selection: u16) -> (Component, Component) {
    let mut specimen = Specimen::new();
    specimen.typo = (760, -240, 216);
    specimen.line = (980, -236, 0);
    specimen.window = (980, 236);
    specimen.selection = selection;
    let base = Component::new(build_font(&[(0x41, "A"), (0x56, "V")], &specimen), "Base", None, None);

    let mut specimen = Specimen::new();
    specimen.window = (1160, 288);
    let addon = Component::new(build_font(&[(0x3042, "B")], &specimen), "Addon", None, None);

    (base, addon)
}

pub fn style() -> Style {
    Style { weight: Some(Weight::Regular), slope: Slope::Upright }
}

#[test]
fn test_typo_metrics_flag_preserved_when_base_sets_it() {
    let (mut base, addon) = components(0x40 | 0x80);
    let metrics = Metrics::of(&[&base, &addon], false);
    metrics.apply(&mut base.font, &family(), &style(), 1.0, None);

    let os2 = base.font.read::<read_fonts::tables::os2::Os2>().expect("missing OS/2");
    assert!(os2.fs_selection().bits() & 0x0080 != 0);
}

#[test]
fn test_typo_metrics_flag_omitted_when_base_lacks_it() {
    let (mut base, addon) = components(0x40);
    let metrics = Metrics::of(&[&base, &addon], false);
    metrics.apply(&mut base.font, &family(), &style(), 1.0, None);

    let os2 = base.font.read::<read_fonts::tables::os2::Os2>().expect("missing OS/2");
    assert!(os2.fs_selection().bits() & 0x0080 == 0);
}

#[test]
fn test_line_metrics_follow_base() {
    let (mut base, addon) = components(0x40);
    let metrics = Metrics::of(&[&base, &addon], false);
    metrics.apply(&mut base.font, &family(), &style(), 1.0, None);

    let hhea = base.font.read::<read_fonts::tables::hhea::Hhea>().expect("missing hhea");
    assert_eq!(hhea.ascender().to_i16(), 980);
    assert_eq!(hhea.descender().to_i16(), -236);

    let os2 = base.font.read::<read_fonts::tables::os2::Os2>().expect("missing OS/2");
    assert_eq!(os2.s_typo_ascender(), 760);
    assert_eq!(os2.s_typo_descender(), -240);
    assert_eq!(os2.s_typo_line_gap(), 216);
}

#[test]
fn test_window_metrics_cover_every_component() {
    let (base, addon) = components(0x40);
    let metrics = Metrics::of(&[&base, &addon], false);

    assert_eq!(metrics.window_ascent, 1160);
    assert_eq!(metrics.window_descent, 288);
}
