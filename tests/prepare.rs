mod common;

use write_fonts::types::Tag;

use nercone_fonts::design::Axis;
use nercone_fonts::font::tags;
use nercone_fonts::prepare::Component;

use common::{build_font, coordinates, Specimen};

pub fn component() -> Component {
    let mut specimen = Specimen::new();
    specimen.axes = vec![("opsz", 14.0, 14.0, 32.0), ("wght", 100.0, 400.0, 900.0)];
    let font = build_font(&[(0x41, "A"), (0x56, "V")], &specimen);
    Component::new(font, "Base", None, None)
}

pub fn axes(component: &Component) -> Vec<(Tag, f64, f64, f64)> {
    let fvar = component.font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar");
    fvar.axes()
        .expect("failed to parse fvar axes")
        .iter()
        .map(|entry| (entry.axis_tag(), entry.min_value().to_f64(), entry.default_value().to_f64(), entry.max_value().to_f64()))
        .collect()
}

#[test]
fn test_rebase_retains_foreign_axes_when_asked() {
    let mut base = component();
    base.rebase(&Axis::new(100.0, 400.0, 900.0), true);

    let tags: Vec<Tag> = axes(&base).into_iter().map(|(tag, _, _, _)| tag).collect();
    assert_eq!(tags, vec![Tag::new(b"opsz"), Tag::new(b"wght")]);
}

#[test]
fn test_rebase_pins_foreign_axes_by_default() {
    let mut addon = component();
    addon.rebase(&Axis::new(100.0, 400.0, 900.0), false);

    let tags: Vec<Tag> = axes(&addon).into_iter().map(|(tag, _, _, _)| tag).collect();
    assert_eq!(tags, vec![Tag::new(b"wght")]);
}

#[test]
fn test_rebase_limits_the_merge_axis() {
    let mut base = component();
    base.rebase(&Axis::new(300.0, 400.0, 700.0), true);

    let entry = axes(&base).into_iter().find(|(tag, _, _, _)| *tag == Tag::new(b"wght")).expect("missing wght axis");
    assert_eq!((entry.1, entry.2, entry.3), (300.0, 400.0, 700.0));
}

#[test]
fn test_monospace_rounds_advances_to_whole_cells() {
    let mut addon = Component::new(build_font(&[(0x41, "A")], &Specimen::new()), "Addon", None, None);
    addon.monospace(350);

    let metrics = addon.font.metrics(tags::HHEA, tags::HMTX);
    assert_eq!((metrics[1].advance, metrics[1].bearing), (700, 100));
}

#[test]
fn test_monospace_centers_outlines_without_scaling_them() {
    let mut addon = Component::new(build_font(&[(0x41, "A")], &Specimen::new()), "Addon", None, None);
    let before = coordinates(&addon.font, 1);
    addon.monospace(350);
    let after = coordinates(&addon.font, 1);

    let shifted: Vec<(i16, i16)> = before.iter().map(|(x, y)| (*x + 50, *y)).collect();
    assert_eq!(after, shifted);
}
