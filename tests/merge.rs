mod common;

use std::collections::BTreeSet;

use read_fonts::FontRead;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::gpos::{Gpos, PairPos, PairSet, PairValueRecord, PositionLookup, PositionLookupList, ValueRecord};
use write_fonts::tables::layout::{CoverageTable, Feature, FeatureList, FeatureRecord, LangSys, Lookup, LookupFlag, Script, ScriptList, ScriptRecord};
use write_fonts::types::{GlyphId16, Tag};

use nercone_fonts::design::{Axis, Space};
use nercone_fonts::font::{tags, Font};
use nercone_fonts::merge::Merger;
use nercone_fonts::prepare::Component;

use common::{build_font, Specimen};

pub fn positioning(first: u16, second: u16, advance: i16, script: &[u8; 4]) -> Gpos {
    let coverage: CoverageTable = [GlyphId16::new(first)].into_iter().collect();
    let pairs = PairSet::new(vec![PairValueRecord::new(GlyphId16::new(second), ValueRecord::new().with_x_advance(advance), ValueRecord::new())]);
    let lookup = PositionLookup::Pair(Lookup::new(LookupFlag::empty(), vec![PairPos::format_1(coverage, vec![pairs])]));
    let record = FeatureRecord::new(Tag::new(b"kern"), Feature::new(None, vec![0]));
    let entry = Script::new(Some(LangSys::new(vec![0])), Vec::new());
    Gpos::new(
        ScriptList::new(vec![ScriptRecord::new(Tag::new(script), entry)]),
        FeatureList::new(vec![record]),
        PositionLookupList::new(vec![lookup]),
    )
}

pub fn merged() -> Font {
    let mut font = build_font(&[(0x41, "A"), (0x56, "V")], &Specimen::new());
    font.put(tags::GPOS, &positioning(1, 2, -40, b"DFLT"));
    let base = Component::new(font, "Base", None, None);

    let mut font = build_font(&[(0x3042, "B"), (0x3044, "C")], &Specimen::new());
    font.put(tags::GPOS, &positioning(1, 2, -30, b"latn"));
    let addon = Component::new(font, "Addon", None, None);

    Merger::new(base, vec![addon], Space::new(Axis::new(400.0, 400.0, 400.0), None), false).build()
}

pub fn table(font: &Font) -> Gpos {
    let data = font.get(tags::GPOS).expect("missing GPOS");
    read_fonts::tables::gpos::Gpos::read(read_fonts::FontData::new(data)).expect("failed to parse GPOS").to_owned_table()
}

pub fn kerning(table: &Gpos, script: Tag) -> Vec<u16> {
    for record in &table.script_list.script_records {
        if record.script_tag != script {
            continue;
        }
        let mut found = Vec::new();
        if let Some(default) = record.script.default_lang_sys.as_ref() {
            for index in &default.feature_indices {
                let feature = &table.feature_list.feature_records[*index as usize];
                if feature.feature_tag == Tag::new(b"kern") {
                    found.extend(feature.feature.lookup_list_indices.iter().copied());
                }
            }
        }
        return found;
    }
    Vec::new()
}

pub fn covered(table: &Gpos, indices: &[u16]) -> BTreeSet<u16> {
    let mut glyphs = BTreeSet::new();
    for index in indices {
        let Some(lookup) = table.lookup_list.lookups.get(*index as usize) else {
            continue;
        };
        if let PositionLookup::Pair(found) = &**lookup {
            for subtable in &found.subtables {
                match &**subtable {
                    PairPos::Format1(pair) => glyphs.extend(pair.coverage.iter().map(|glyph| glyph.to_u16())),
                    PairPos::Format2(pair) => glyphs.extend(pair.coverage.iter().map(|glyph| glyph.to_u16())),
                }
            }
        }
    }
    glyphs
}

#[test]
fn test_default_script_kerning_reaches_scripts_added_by_addons() {
    let font = merged();
    let table = table(&font);

    let glyphs = covered(&table, &kerning(&table, Tag::new(b"latn")));
    assert!(glyphs.contains(&1) && glyphs.contains(&3), "{:?}", glyphs);
}

#[test]
fn test_default_script_keeps_kerning_from_every_component() {
    let font = merged();
    let table = table(&font);

    let glyphs = covered(&table, &kerning(&table, Tag::new(b"DFLT")));
    assert!(glyphs.contains(&1), "{:?}", glyphs);
}
