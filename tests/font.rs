mod common;

use std::collections::BTreeMap;

use read_fonts::tables::cmap::{Cmap, CmapSubtable};
use read_fonts::tables::glyf::Glyf;
use read_fonts::tables::loca::Loca;
use read_fonts::{FontData, FontRead, FontRef, TableProvider};
use read_fonts::tables::glyf::CurvePoint;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::glyf::{Anchor, Bbox, Component, ComponentFlags, CompositeGlyph, Glyph, SimpleGlyph, Transform};
use write_fonts::tables::maxp::Maxp;
use write_fonts::types::GlyphId16;

use nercone_fonts::font::{capacity, charmap, tags, Font};

use common::{build_font, Specimen};

pub fn simple(contours: &[u16]) -> Vec<u8> {
    let outline = SimpleGlyph {
        bbox: Bbox { x_min: 0, y_min: 0, x_max: 100, y_max: 100 },
        contours: contours
            .iter()
            .enumerate()
            .map(|(index, points)| {
                let base = index as i16 * 10;
                (0..*points)
                    .map(|point| CurvePoint::new(base + point as i16, base + point as i16 * 2, true))
                    .collect::<Vec<CurvePoint>>()
                    .into()
            })
            .collect(),
        instructions: Vec::new(),
        overlaps: false,
    };
    write_fonts::dump_table(&Glyph::Simple(outline)).expect("failed to serialize glyph")
}

pub fn composite(components: &[u16]) -> Vec<u8> {
    let bbox = Bbox { x_min: 0, y_min: 0, x_max: 100, y_max: 100 };
    let entry = |glyph: &u16| Component::new(GlyphId16::new(*glyph), Anchor::Offset { x: 0, y: 0 }, Transform::default(), ComponentFlags::default());
    let mut outline = CompositeGlyph::new(entry(&components[0]), bbox);
    for glyph in &components[1..] {
        outline.add_component(entry(glyph), bbox);
    }
    write_fonts::dump_table(&Glyph::Composite(outline)).expect("failed to serialize glyph")
}

pub fn profile(font: &Font) -> Maxp {
    font.read::<read_fonts::tables::maxp::Maxp>().expect("missing maxp").to_owned_table()
}

pub fn specimen() -> Font {
    let mut font = build_font(&[(0x41, "A"), (0x42, "B"), (0x43, "C"), (0x44, "D")], &Specimen::new());
    font.set_glyphs(&[Vec::new(), simple(&[4, 5, 6]), simple(&[4]), composite(&[1, 2]), composite(&[3, 1])]);
    font
}

#[test]
fn test_profile_states_the_maximums_of_every_glyph() {
    let mut font = specimen();
    font.finalize();
    let maxp = profile(&font);

    assert_eq!(maxp.num_glyphs, 5);
    assert_eq!(maxp.max_points, Some(15));
    assert_eq!(maxp.max_contours, Some(3));
    assert_eq!(maxp.max_composite_points, Some(34));
    assert_eq!(maxp.max_composite_contours, Some(7));
    assert_eq!(maxp.max_component_elements, Some(2));
    assert_eq!(maxp.max_component_depth, Some(2));
    assert_eq!(maxp.max_size_of_instructions, Some(0));
}

#[test]
fn test_profile_is_corrected_whether_stated_low_or_high() {
    for stated in [Some(0u16), Some(4000u16)] {
        let mut font = specimen();
        let mut maxp = profile(&font);
        maxp.max_points = stated;
        maxp.max_contours = stated;
        maxp.max_composite_points = stated;
        maxp.max_composite_contours = stated;
        maxp.max_component_elements = stated;
        maxp.max_component_depth = stated;
        maxp.max_size_of_instructions = stated;
        font.put(tags::MAXP, &maxp);

        font.finalize();
        let corrected = profile(&font);
        assert_eq!(corrected.max_points, Some(15));
        assert_eq!(corrected.max_contours, Some(3));
        assert_eq!(corrected.max_composite_points, Some(34));
        assert_eq!(corrected.max_composite_contours, Some(7));
        assert_eq!(corrected.max_component_elements, Some(2));
        assert_eq!(corrected.max_component_depth, Some(2));
        assert_eq!(corrected.max_size_of_instructions, Some(0));
    }
}

#[test]
fn test_no_glyph_exceeds_the_stated_profile() {
    let mut font = specimen();
    font.finalize();
    let maxp = profile(&font);

    let data = font.data();
    let reference = FontRef::new(&data).expect("failed to parse font");
    let glyf: Glyf = reference.glyf().expect("missing glyf");
    let loca: Loca = reference.loca(None).expect("missing loca");

    for index in 0..maxp.num_glyphs as u32 {
        let (points, contours, depth) = Font::count(&glyf, &loca, index);
        let composite = depth > 0;
        let (allowed_points, allowed_contours) = if composite {
            (maxp.max_composite_points.unwrap(), maxp.max_composite_contours.unwrap())
        } else {
            (maxp.max_points.unwrap(), maxp.max_contours.unwrap())
        };
        assert!(points <= allowed_points, "glyph {} holds {} points, more than the {} stated", index, points, allowed_points);
        assert!(contours <= allowed_contours, "glyph {} holds {} contours, more than the {} stated", index, contours, allowed_contours);
        assert!(depth <= maxp.max_component_depth.unwrap(), "glyph {} nests {} levels deep, more than the {} stated", index, depth, maxp.max_component_depth.unwrap());
    }
}

pub fn scattered(codepoints: impl Iterator<Item = u32>) -> BTreeMap<u32, u16> {
    codepoints.enumerate().map(|(index, code)| (code, (1 + (index as u64 * 7919) % 65534) as u16)).collect()
}

pub fn subtables(table: &[u8]) -> Vec<(u16, u16, u16, BTreeMap<u32, u16>)> {
    let cmap = Cmap::read(FontData::new(table)).expect("failed to parse cmap");
    cmap.encoding_records()
        .iter()
        .map(|record| {
            let subtable = record.subtable(cmap.offset_data()).expect("failed to parse subtable");
            let format = match &subtable {
                CmapSubtable::Format4(_) => 4,
                CmapSubtable::Format12(_) => 12,
                _ => panic!("cmap states a subtable of an unwanted format"),
            };
            let mapping: BTreeMap<u32, u16> = subtable.iter().filter(|(_, glyph)| glyph.to_u32() != 0).map(|(code, glyph)| (code, glyph.to_u32() as u16)).collect();
            (record.platform_id() as u16, record.encoding_id(), format, mapping)
        })
        .collect()
}

pub fn assorted() -> BTreeMap<u32, u16> {
    let mut mapping: BTreeMap<u32, u16> = (0x20..0x80u32).map(|code| (code, code as u16 - 0x1F)).collect();
    mapping.extend(scattered(0x3000..0x3100));
    mapping.extend((0..64u32).map(|index| (0xF000 + index * 5, 4000 + index as u16)));
    mapping.extend((0x20000..0x20100u32).map(|code| (code, (code - 0x20000) as u16 + 5000)));
    mapping.extend(scattered(0x1F600..0x1F650));
    mapping
}

#[test]
fn test_charmap_maps_every_character_through_the_subtables_it_states() {
    let mapping = assorted();
    let table = charmap(&mapping);
    let plane: BTreeMap<u32, u16> = mapping.iter().filter(|(code, _)| **code <= 0xFFFF).map(|(code, glyph)| (*code, *glyph)).collect();

    let subtables = subtables(&table);
    assert!(!subtables.is_empty(), "cmap states no subtable at all");

    for (platform, encoding, format, found) in &subtables {
        let wanted = if *format == 4 { &plane } else { &mapping };
        assert_eq!(found, wanted, "the format {} subtable of platform {} encoding {} maps other characters", format, platform, encoding);
    }
}

#[test]
fn test_charmap_states_the_subtables_every_shaper_reads() {
    for (mapping, wanted) in [
        (scattered(0x4E00..0x9FA0), vec![(0, 3, 4), (3, 1, 4)]),
        (scattered(0x20000..0x20100), vec![(3, 10, 12)]),
        (assorted(), vec![(0, 3, 4), (3, 1, 4), (3, 10, 12)]),
    ] {
        let table = charmap(&mapping);
        let found: Vec<(u16, u16, u16)> = subtables(&table).into_iter().map(|(platform, encoding, format, _)| (platform, encoding, format)).collect();
        assert_eq!(found, wanted);
    }
}

#[test]
fn test_charmap_states_a_format_4_subtable_for_a_font_of_many_characters() {
    let mut mapping = scattered(0x3400..0x9FA0);
    mapping.extend(scattered(0x20000..0x20100));

    let table = charmap(&mapping);
    let plane: BTreeMap<u32, u16> = mapping.iter().filter(|(code, _)| **code <= 0xFFFF).map(|(code, glyph)| (*code, *glyph)).collect();
    assert!(plane.len() > 27000, "the mapping under test is too small to prove anything");

    let found = subtables(&table).into_iter().find(|(platform, encoding, _, _)| (*platform, *encoding) == (3, 1));
    let (_, _, format, characters) = found.expect("cmap states no subtable for the Windows BMP encoding");
    assert_eq!(format, 4);
    assert_eq!(characters, plane);
}

#[test]
fn test_format_4_states_the_header_the_specification_defines() {
    let mapping = assorted();
    let table = charmap(&mapping);
    let cmap = Cmap::read(FontData::new(&table)).expect("failed to parse cmap");

    let record = cmap.encoding_records().iter().find(|record| (record.platform_id() as u16, record.encoding_id()) == (3, 1)).expect("no format 4 subtable");
    let start = record.subtable_offset().to_u32() as usize;
    let CmapSubtable::Format4(subtable) = record.subtable(cmap.offset_data()).expect("failed to parse subtable") else {
        panic!("the Windows BMP encoding states a subtable of another format");
    };

    let following = cmap
        .encoding_records()
        .iter()
        .map(|record| record.subtable_offset().to_u32() as usize)
        .filter(|offset| *offset > start)
        .min()
        .unwrap_or(table.len());
    assert_eq!(subtable.length() as usize, following - start);
    assert_eq!(subtable.language(), 0);

    let count = subtable.end_code().len();
    assert_eq!(subtable.seg_count_x2() as usize, count * 2);
    let mut selector = 0u16;
    while 1usize << (selector + 1) <= count {
        selector += 1;
    }
    assert_eq!(subtable.entry_selector(), selector);
    assert_eq!(subtable.search_range(), 2 * (1 << selector));
    assert_eq!(subtable.range_shift() as usize, count * 2 - subtable.search_range() as usize);

    assert_eq!(subtable.start_code().len(), count);
    assert_eq!(subtable.id_delta().len(), count);
    assert_eq!(subtable.id_range_offsets().len(), count);
    assert_eq!(u16::from_be_bytes(table[start + 14 + count * 2..start + 16 + count * 2].try_into().unwrap()), 0);
    assert_eq!(subtable.end_code()[count - 1].get(), 0xFFFF);
    for index in 0..count {
        let (first, last) = (subtable.start_code()[index].get(), subtable.end_code()[index].get());
        assert!(first <= last, "segment {} runs from U+{:04X} down to U+{:04X}", index, first, last);
        if index > 0 {
            assert!(subtable.end_code()[index - 1].get() < first, "segment {} begins at U+{:04X}, within the segment before it", index, first);
        }

        let offset = subtable.id_range_offsets()[index].get() as usize;
        if offset == 0 {
            continue;
        }
        assert_eq!(subtable.id_delta()[index].get(), 0);
        let position = start + 16 + count * 8 + (offset / 2 + (last - first) as usize - (count - index)) * 2;
        assert!(position + 2 <= start + subtable.length() as usize, "segment {} reaches beyond the subtable", index);
    }
}

#[test]
fn test_charmap_keeps_every_character_reachable_when_format_4_overflows() {
    for mapping in [scattered(0xE00..0xFFFF), scattered((0..21000u32).map(|index| index * 3))] {
        let table = charmap(&mapping);
        let subtables = subtables(&table);

        let bmp = subtables.iter().find(|(platform, encoding, _, _)| (*platform, *encoding) == (3, 1));
        let (_, _, format, held) = bmp.expect("cmap states no subtable for the Windows BMP encoding");
        assert_eq!(*format, 4);
        assert!(!held.is_empty());
        assert!(held.len() < mapping.len(), "the mapping under test does not overflow a format 4 subtable");

        let prefix: BTreeMap<u32, u16> = mapping.iter().take(held.len()).map(|(code, glyph)| (*code, *glyph)).collect();
        assert_eq!(held, &prefix, "the format 4 subtable holds other than the lowest characters it has room for");

        let full = subtables.iter().find(|(_, _, format, _)| *format == 12);
        let (_, _, _, reachable) = full.expect("cmap leaves characters out of format 4 without stating a format 12 subtable");
        assert_eq!(reachable, &mapping);
    }
}

#[test]
fn test_glyph_count_states_every_glyph_a_font_holds() {
    let mut font = specimen();
    font.set_glyphs(&vec![Vec::new(); capacity]);
    assert_eq!(profile(&font).num_glyphs as usize, capacity);
    assert_eq!(font.glyph_count(), capacity);
}

#[test]
#[should_panic(expected = "more than the 65535 a font can hold")]
fn test_glyphs_beyond_the_capacity_of_a_font_are_refused() {
    let mut font = specimen();
    font.set_glyphs(&vec![Vec::new(); capacity + 1]);
}
