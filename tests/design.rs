mod common;

use write_fonts::tables::avar::{Avar, AxisValueMap, SegmentMaps};
use write_fonts::tables::gvar::{GlyphDelta, GlyphDeltas, GlyphVariations, Gvar, Tent};
use write_fonts::types::{F2Dot14, GlyphId, Tag};

use nercone_fonts::design::{Axis, Space};
use nercone_fonts::font::{tags, Font};
use nercone_fonts::prepare::Component;

use common::{build_font, outline, Specimen};

pub fn segment(pairs: &[(f32, f32)]) -> SegmentMaps {
    SegmentMaps::new(pairs.iter().map(|(plain, mapped)| AxisValueMap::new(F2Dot14::from_f32(*plain), F2Dot14::from_f32(*mapped))).collect())
}

pub fn deltas(values: &[(i16, i16)]) -> Vec<GlyphDelta> {
    values.iter().map(|(x, y)| GlyphDelta::required(*x, *y)).collect()
}

pub fn tents(opsz: f32, wght: f32) -> Vec<Tent> {
    vec![Tent::new(F2Dot14::from_f32(opsz), None), Tent::new(F2Dot14::from_f32(wght), None)]
}

pub fn variable_font() -> Font {
    let mut specimen = Specimen::new();
    specimen.axes = vec![("opsz", 10.0, 10.0, 20.0), ("wght", 100.0, 400.0, 900.0)];
    let mut font = build_font(&[(0x41, "A"), (0x56, "V")], &specimen);

    font.put(tags::AVAR, &Avar::new(vec![
        segment(&[(-1.0, -1.0), (0.0, 0.0), (1.0, 1.0)]),
        segment(&[(-1.0, -1.0), (0.0, 0.0), (0.5, 0.8), (1.0, 1.0)]),
    ]));

    let variations = vec![
        GlyphDeltas::new(tents(0.0, 1.0), deltas(&[(30, 0), (30, 40), (60, 40), (60, 0), (0, 0), (80, 0), (0, 0), (0, 0)])),
        GlyphDeltas::new(tents(1.0, 0.0), deltas(&[(-10, 0), (-10, -20), (-20, -20), (-20, 0), (0, 0), (-40, 0), (0, 0), (0, 0)])),
        GlyphDeltas::new(tents(1.0, 1.0), deltas(&[(5, 5), (5, 5), (5, 5), (5, 5), (0, 0), (10, 0), (0, 0), (0, 0)])),
    ];
    let gvar = Gvar::new(vec![
        GlyphVariations::new(GlyphId::new(0), Vec::new()),
        GlyphVariations::new(GlyphId::new(1), variations),
        GlyphVariations::new(GlyphId::new(2), Vec::new()),
    ], 2).expect("failed to build gvar");
    font.put(tags::GVAR, &gvar);

    font
}

pub fn retargeted() -> (Font, Font, Vec<f64>) {
    let mut component = Component::new(variable_font(), "Base", None, None);
    let original = Font::new(&component.font.data());

    let space = Space::new(Axis::new(100.0, 400.0, 900.0), Some(vec![(-1.0, -1.0), (0.0, 0.0), (0.5, 0.6), (1.0, 1.0)]));
    let mut masters = space.breakpoints();
    masters.extend(component.breakpoints(&space));
    masters.sort_by(f64::total_cmp);
    masters.dedup();
    component.retarget(&space, &masters);

    (original, component.font, masters)
}

pub fn agrees(found: &[(f64, f64)], expected: &[(f64, f64)]) {
    let tolerance = 1.0 / 16384.0;
    assert_eq!(found.len(), expected.len(), "{:?} != {:?}", found, expected);
    for ((plain, mapped), (wanted_plain, wanted_mapped)) in found.iter().zip(expected) {
        assert!((plain - wanted_plain).abs() <= tolerance && (mapped - wanted_mapped).abs() <= tolerance, "{:?} != {:?}", found, expected);
    }
}

#[test]
fn test_retarget_preserves_outlines_across_every_axis() {
    let (original, rebuilt, masters) = retargeted();

    for opsz in [10.0, 15.0, 20.0] {
        for weight in &masters {
            let position = [(Tag::new(b"opsz"), opsz), (Tag::new(b"wght"), *weight)];
            let before = outline(&original, 1, &position);
            let after = outline(&rebuilt, 1, &position);
            assert_eq!(before.len(), after.len(), "{:?} {:?} {:?}", position, before, after);
            for ((x1, y1), (x2, y2)) in before.iter().zip(&after) {
                assert!((x1 - x2).abs() <= 1 && (y1 - y2).abs() <= 1, "{:?} {:?} {:?}", position, before, after);
            }
        }
    }
}

#[test]
fn test_retarget_keeps_foreign_axes_declared() {
    let (_, rebuilt, _) = retargeted();

    let fvar = rebuilt.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar");
    let axes: Vec<Tag> = fvar.axes().expect("failed to parse fvar axes").iter().map(|entry| entry.axis_tag()).collect();
    assert_eq!(axes, vec![Tag::new(b"opsz"), Tag::new(b"wght")]);

    let mappings = Space::mappings(&rebuilt);
    assert_eq!(mappings.len(), 2, "{:?}", mappings);
    agrees(&mappings[0], &[(-1.0, -1.0), (0.0, 0.0), (1.0, 1.0)]);
    agrees(&mappings[1], &[(-1.0, -1.0), (0.0, 0.0), (0.5, 0.6), (1.0, 1.0)]);
}
