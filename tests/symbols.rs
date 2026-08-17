mod common;

use read_fonts::tables::glyf::CurvePoint;
use write_fonts::tables::glyf::{Bbox, Glyph, SimpleGlyph};

use nercone_fonts::font::{tags, Extent};
use nercone_fonts::prepare::Component;
use nercone_fonts::symbols::{Align, Cell, Placement, Stretch, Symbols};

use common::{build_font, Specimen};

pub fn outline(bounds: (i16, i16, i16, i16)) -> SimpleGlyph {
    let (x_min, y_min, x_max, y_max) = bounds;
    let contour: Vec<CurvePoint> = vec![
        CurvePoint::new(x_min, y_min, true),
        CurvePoint::new(x_min, y_max, true),
        CurvePoint::new(x_max, y_max, true),
        CurvePoint::new(x_max, y_min, true),
    ];
    SimpleGlyph { bbox: Bbox { x_min, y_min, x_max, y_max }, contours: vec![contour.into()], instructions: Vec::new(), overlaps: false }
}

pub fn component(entries: &[(u32, &'static str, (i16, i16, i16, i16))]) -> Component {
    let names: Vec<(u32, &str)> = entries.iter().map(|(codepoint, name, _)| (*codepoint, *name)).collect();
    let font = build_font(&names, &Specimen::new());
    let mut component = Component::new(font, "Symbols", None, None);

    let cmap = component.cmap();
    let mut glyphs = component.font.glyphs();
    for (codepoint, _, bounds) in entries {
        let index = *cmap.get(codepoint).expect("codepoint missing") as usize;
        glyphs[index] = write_fonts::dump_table(&Glyph::Simple(outline(*bounds))).expect("failed to serialize glyph");
    }
    component.font.set_glyphs(&glyphs);
    component
}

pub fn bounds(component: &Component, codepoint: u32) -> (i16, i16, i16, i16) {
    let index = *component.cmap().get(&codepoint).expect("codepoint missing") as usize;
    let data = component.font.glyphs()[index].clone();
    (
        i16::from_be_bytes(data[2..4].try_into().unwrap()),
        i16::from_be_bytes(data[4..6].try_into().unwrap()),
        i16::from_be_bytes(data[6..8].try_into().unwrap()),
        i16::from_be_bytes(data[8..10].try_into().unwrap()),
    )
}

pub fn close(found: f64, wanted: f64) {
    assert!((found - wanted).abs() < 1e-6, "{} is not {}", found, wanted);
}

pub fn extent(bounds: (f64, f64, f64, f64)) -> Extent {
    let mut found = Extent::new();
    found.include(bounds.0 as f32, bounds.1 as f32);
    found.include(bounds.2 as f32, bounds.3 as f32);
    found
}

#[test]
fn test_cell_reads_the_line_box_and_the_capitals() {
    let font = build_font(&[(0x41, "A")], &Specimen::new());
    let cell = Cell::of(&font, 600);

    assert_eq!(cell.width, 0.6);
    assert_eq!(cell.ascent, 1.0);
    assert_eq!(cell.descent, -0.25);
    assert_eq!(cell.cap, 0.7);
    assert_eq!(cell.height(), 1.25);
    assert_eq!(cell.middle(), 0.375);
}

#[test]
fn test_cell_splits_the_line_gap_between_top_and_bottom() {
    let mut specimen = Specimen::new();
    specimen.line = (1000, -250, 101);
    let font = build_font(&[(0x41, "A")], &specimen);
    let cell = Cell::of(&font, 600);

    assert_eq!(cell.ascent, 1.05);
    assert_eq!(cell.descent, -0.301);
    assert_eq!(cell.height(), 1.351);
}

#[test]
fn test_icon_height_stays_below_the_line_box_when_capitals_are_known() {
    let known = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    assert_eq!(known.icon(), (0.7 * 2.0 + 1.25) / 3.0);
    assert!(known.icon() < known.height());
    assert!(known.icon() > known.cap);

    let unknown = Cell { width: 1.0, ascent: 0.8, descent: -0.2, cap: 0.0 };
    assert_eq!(unknown.icon(), unknown.height());
}

#[test]
fn test_the_ratio_fits_a_cell_into_another_by_its_tightest_side() {
    let symbols = Cell { width: 1.0, ascent: 0.8, descent: -0.2, cap: 0.0 };

    let narrow = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    assert_eq!(narrow.ratio(&symbols), 0.6);

    let shallow = Cell { width: 1.2, ascent: 0.5, descent: -0.1, cap: 0.3 };
    assert_eq!(shallow.ratio(&symbols), shallow.icon());
}

#[test]
fn test_the_shift_moves_one_cell_middle_onto_another() {
    let into = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    let from = Cell { width: 1.0, ascent: 0.8, descent: -0.2, cap: 0.0 };

    close(into.shift(&from), 0.375 - 0.3);
    close(from.shift(&into), -(0.375 - 0.3));
}

#[test]
fn test_icons_keep_their_aspect_and_stay_within_the_icon_height() {
    let cell = Cell { width: 1.2, ascent: 0.8, descent: -0.2, cap: 0.7 };
    let placement = Symbols::placement(0xF0001);
    assert_eq!(placement, Placement::new(Align::Middle, Stretch::Aspect, 0.0, None, false));

    let (horizontal, vertical) = placement.scale(&cell, 1000.0, &extent((0.0, 0.0, 500.0, 500.0)));
    assert_eq!(horizontal, vertical);
    assert_eq!(horizontal, cell.icon() * 1000.0 / 500.0);
}

#[test]
fn test_powerline_symbols_fill_the_whole_line_box() {
    let cell = Cell { width: 1.2, ascent: 0.8, descent: -0.2, cap: 0.7 };
    let placement = Symbols::placement(0xE0A0);
    assert_eq!(placement, Placement::new(Align::Middle, Stretch::Aspect, 0.0, None, true));

    let (horizontal, vertical) = placement.scale(&cell, 1000.0, &extent((0.0, 0.0, 500.0, 500.0)));
    assert_eq!(horizontal, vertical);
    assert_eq!(horizontal, cell.height() * 1000.0 / 500.0);
}

#[test]
fn test_stretched_symbols_take_the_cell_in_both_directions() {
    let cell = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    let placement = Placement::fill(Align::Left, 0.0, None);

    let (horizontal, vertical) = placement.scale(&cell, 1000.0, &extent((0.0, 0.0, 400.0, 1000.0)));
    assert_eq!(horizontal, 600.0 / 400.0);
    assert_eq!(vertical, 1250.0 / 1000.0);
}

#[test]
fn test_overlap_widens_the_cell_but_barely_deepens_it() {
    let cell = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    let placement = Placement::fill(Align::Left, 0.06, None);

    let (horizontal, vertical) = placement.scale(&cell, 1000.0, &extent((0.0, 0.0, 400.0, 1000.0)));
    assert_eq!(horizontal, 600.0 * 1.06 / 400.0);
    assert_eq!(vertical, 1250.0 * 1.01 / 1000.0);
}

#[test]
fn test_negative_overlap_shrinks_both_directions() {
    let cell = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    let placement = Placement::fill(Align::Left, -0.03, None);

    let (horizontal, vertical) = placement.scale(&cell, 1000.0, &extent((0.0, 0.0, 400.0, 1000.0)));
    assert_eq!(horizontal, 600.0 * 0.97 / 400.0);
    assert_eq!(vertical, 1250.0 * 0.97 / 1000.0);
}

#[test]
fn test_the_ratio_limit_keeps_symbols_from_growing_too_wide() {
    let cell = Cell { width: 1.2, ascent: 1.0, descent: -0.25, cap: 0.7 };
    let bounds = extent((0.0, 0.0, 400.0, 500.0));

    let free = Placement::fill(Align::Left, 0.0, None);
    let (horizontal, vertical) = free.scale(&cell, 1000.0, &bounds);
    assert!(400.0 * horizontal / (500.0 * vertical) > 0.7);

    let limited = Placement::fill(Align::Left, 0.0, Some(0.7));
    let (horizontal, vertical) = limited.scale(&cell, 1000.0, &bounds);
    assert_eq!(vertical, 1250.0 / 500.0);
    close(400.0 * horizontal / (500.0 * vertical), 0.7);
}

#[test]
fn test_symbols_sit_in_the_middle_of_the_cell() {
    let cell = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    let bounds = extent((100.0, 0.0, 500.0, 400.0));
    let (dx, dy) = Placement::icon(Align::Middle).offset(&cell, 1000.0, &bounds, (1.0, 1.0));

    assert_eq!(100.0 + dx, (600.0 - 400.0) / 2.0);
    assert_eq!((0.0 + 400.0) / 2.0 + dy, cell.middle() * 1000.0);
}

#[test]
fn test_left_and_right_alignment_touch_the_cell_edges() {
    let cell = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    let bounds = extent((100.0, 0.0, 500.0, 400.0));

    let (dx, _) = Placement::aspect(Align::Left, 0.0, None).offset(&cell, 1000.0, &bounds, (1.0, 1.0));
    assert_eq!(100.0 + dx, 0.0);

    let (dx, _) = Placement::aspect(Align::Right, 0.0, None).offset(&cell, 1000.0, &bounds, (1.0, 1.0));
    assert_eq!(500.0 + dx, 600.0);
}

#[test]
fn test_overlapping_symbols_reach_past_the_cell_edges() {
    let cell = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    let bounds = extent((100.0, 0.0, 500.0, 400.0));

    let (dx, _) = Placement::fill(Align::Left, 0.06, None).offset(&cell, 1000.0, &bounds, (1.0, 1.0));
    assert_eq!(100.0 + dx, -600.0 * 0.06);

    let (dx, _) = Placement::fill(Align::Right, 0.06, None).offset(&cell, 1000.0, &bounds, (1.0, 1.0));
    assert_eq!(500.0 + dx, 600.0 * 1.06);
}

#[test]
fn test_wide_symbols_are_left_aligned_instead_of_overflowing_both_edges() {
    let cell = Cell { width: 0.6, ascent: 1.0, descent: -0.25, cap: 0.7 };
    let bounds = extent((100.0, 0.0, 900.0, 400.0));
    let (dx, _) = Placement::icon(Align::Middle).offset(&cell, 1000.0, &bounds, (1.0, 1.0));

    assert_eq!(100.0 + dx, 0.0);
}

#[test]
fn test_every_powerline_and_progress_symbol_fills_the_line_box() {
    let full: Vec<u32> = (0xE0A0..=0xE0A3).chain(0xE0B0..=0xE0D7).chain(0xEE00..=0xEE0B).collect();
    for codepoint in full {
        assert!(Symbols::placement(codepoint).full, "U+{:04X} does not fill the cell", codepoint);
    }

    for codepoint in [0x2665, 0xE000, 0xE0A4, 0xE0AF, 0xE0D8, 0xEDFF, 0xEE0C, 0xF0001] {
        assert!(!Symbols::placement(codepoint).full, "U+{:04X} fills the cell", codepoint);
    }
}

#[test]
fn test_arrow_tips_are_stretched_towards_their_own_side() {
    assert_eq!(Symbols::placement(0xE0B0), Placement::fill(Align::Left, 0.06, Some(0.70)));
    assert_eq!(Symbols::placement(0xE0B1), Placement::fill(Align::Left, 0.0, Some(0.70)));
    assert_eq!(Symbols::placement(0xE0B2), Placement::fill(Align::Right, 0.06, Some(0.70)));
    assert_eq!(Symbols::placement(0xE0B3), Placement::fill(Align::Right, 0.0, Some(0.70)));
    assert_eq!(Symbols::placement(0xE0A0), Placement::aspect(Align::Middle, 0.0, None));
}

#[test]
fn test_scale_groups_only_hold_codepoints_the_font_carries() {
    let component = component(&[(0xEB89, "one", (0, 0, 100, 100)), (0xEC07, "two", (0, 0, 100, 100)), (0xF0001, "icon", (0, 0, 100, 100))]);
    let cmap = component.cmap();
    let groups = Symbols::grouped(&cmap);

    assert_eq!(groups.len(), 1);
    let mut wanted = vec![cmap[&0xEB89], cmap[&0xEC07]];
    wanted.sort();
    let mut found = groups[0].clone();
    found.sort();
    assert_eq!(found, wanted);
}

#[test]
fn test_fitting_centers_icons_within_the_icon_height() {
    let cell = Cell { width: 1.2, ascent: 0.8, descent: -0.2, cap: 0.7 };
    let mut component = component(&[(0xF0001, "icon", (50, 0, 500, 500))]);
    component.fit(&cell);

    let scale = cell.icon() * 1000.0 / 500.0;
    let dx = -50.0 * scale + (1200.0 - 450.0 * scale) / 2.0;
    let dy = cell.middle() * 1000.0 - 250.0 * scale;
    let round = |value: f64| value.round_ties_even() as i16;
    assert_eq!(bounds(&component, 0xF0001), (round(50.0 * scale + dx), round(dy), round(500.0 * scale + dx), round(500.0 * scale + dy)));
}

#[test]
fn test_fitting_lets_powerline_symbols_reach_the_whole_line_box() {
    let cell = Cell { width: 1.2, ascent: 0.8, descent: -0.2, cap: 0.7 };
    let mut component = component(&[(0xE0A0, "branch", (50, 0, 500, 500))]);
    component.fit(&cell);

    let (_, y_min, _, y_max) = bounds(&component, 0xE0A0);
    close(y_max as f64 - y_min as f64, cell.height() * 1000.0);
    close((y_min as f64 + y_max as f64) / 2.0, cell.middle() * 1000.0);
}

#[test]
fn test_fitting_keeps_grouped_symbols_in_proportion() {
    let cell = Cell { width: 1.2, ascent: 0.8, descent: -0.2, cap: 0.7 };
    let mut component = component(&[(0xEB89, "small", (100, 100, 200, 200)), (0xEB8A, "large", (0, 0, 500, 500))]);
    component.fit(&cell);

    let (small, large) = (bounds(&component, 0xEB89), bounds(&component, 0xEB8A));
    let scale = cell.icon() * 1000.0 / 500.0;
    close((large.2 - large.0) as f64, 500.0 * scale);
    close((small.2 - small.0) as f64, 100.0 * scale);
    close(small.0 as f64 - large.0 as f64, 100.0 * scale);
}

#[test]
fn test_fitting_leaves_the_advances_alone() {
    let cell = Cell { width: 1.2, ascent: 0.8, descent: -0.2, cap: 0.7 };
    let mut component = component(&[(0xF0001, "icon", (50, 0, 500, 500))]);
    let before = component.font.metrics(tags::HHEA, tags::HMTX);
    component.fit(&cell);
    let after = component.font.metrics(tags::HHEA, tags::HMTX);

    let index = component.cmap()[&0xF0001] as usize;
    assert_eq!(after[index].advance, before[index].advance);
    assert_eq!(after[index].bearing, bounds(&component, 0xF0001).0);
}

#[test]
fn test_outline_bounds_follow_the_curve_and_not_its_controls() {
    let mut found = Extent::new();
    found.contour([CurvePoint::new(0, 0, true), CurvePoint::new(500, 2000, false), CurvePoint::new(1000, 0, true)].iter());

    assert_eq!(found.minimum_x, 0.0);
    assert_eq!(found.maximum_x, 1000.0);
    assert_eq!(found.minimum_y, 0.0);
    assert_eq!(found.maximum_y, 1000.0);
}

#[test]
fn test_outline_bounds_read_contours_without_an_on_curve_point() {
    let mut found = Extent::new();
    found.contour(
        [
            CurvePoint::new(0, 500, false),
            CurvePoint::new(500, 1000, false),
            CurvePoint::new(1000, 500, false),
            CurvePoint::new(500, 0, false),
        ]
        .iter(),
    );

    assert_eq!((found.minimum_x, found.maximum_x), (125.0, 875.0));
    assert_eq!((found.minimum_y, found.maximum_y), (125.0, 875.0));
}
