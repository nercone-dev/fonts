mod common;

use read_fonts::FontRead;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::base::{Axis as BaseAxis, Base, BaseCoord, BaseScript, BaseScriptList, BaseScriptRecord, BaseTagList, BaseValues, MinMax};
use write_fonts::types::Tag;

use nercone_fonts::font::tags;
use nercone_fonts::scale::Scaler;

use common::{build_font, Specimen};

pub fn baselines(ideographic: i16, lowest: i16, highest: i16) -> Base {
    let tags = BaseTagList::new(vec![Tag::new(b"ideo"), Tag::new(b"romn")]);
    let values = BaseValues::new(0, vec![BaseCoord::format_1(ideographic), BaseCoord::format_1(0)]);
    let extremes = MinMax::new(Some(BaseCoord::format_1(lowest)), Some(BaseCoord::format_1(highest)), Vec::new());
    let script = BaseScript::new(Some(values), Some(extremes), Vec::new());
    let list = BaseScriptList::new(vec![BaseScriptRecord::new(Tag::new(b"DFLT"), script)]);
    Base::new(Some(BaseAxis::new(Some(tags), list)), None)
}

pub fn scaled(factor: f64) -> Base {
    let mut font = build_font(&[(0x41, "A")], &Specimen::new());
    font.put(tags::BASE, &baselines(-120, -288, 1160));

    Scaler::new(factor).apply(&mut font);

    let data = font.get(tags::BASE).expect("missing BASE");
    read_fonts::tables::base::Base::read(read_fonts::FontData::new(data)).expect("failed to parse BASE").to_owned_table()
}

pub fn coordinate(entry: &BaseCoord) -> i16 {
    match entry {
        BaseCoord::Format1(found) => found.coordinate,
        BaseCoord::Format2(found) => found.coordinate,
        BaseCoord::Format3(found) => found.coordinate,
    }
}

#[test]
fn test_baseline_coordinates_scale_with_the_em() {
    let table = scaled(2048.0 / 1000.0);
    let axis = table.horiz_axis.as_ref().expect("missing horizontal axis");
    let script = &axis.base_script_list.base_script_records[0].base_script;

    let values = script.base_values.as_ref().expect("missing baseline values");
    assert_eq!(coordinate(&values.base_coords[0]), -246);
    assert_eq!(coordinate(&values.base_coords[1]), 0);
}

#[test]
fn test_baseline_extremes_scale_with_the_em() {
    let table = scaled(2048.0 / 1000.0);
    let axis = table.horiz_axis.as_ref().expect("missing horizontal axis");
    let script = &axis.base_script_list.base_script_records[0].base_script;

    let extremes = script.default_min_max.as_ref().expect("missing baseline extremes");
    assert_eq!(coordinate(extremes.min_coord.as_ref().expect("missing lowest baseline")), -590);
    assert_eq!(coordinate(extremes.max_coord.as_ref().expect("missing highest baseline")), 2376);
}

#[test]
fn test_baselines_stay_put_when_the_em_does() {
    let table = scaled(1.0);
    let axis = table.horiz_axis.as_ref().expect("missing horizontal axis");
    let script = &axis.base_script_list.base_script_records[0].base_script;

    let values = script.base_values.as_ref().expect("missing baseline values");
    assert_eq!(coordinate(&values.base_coords[0]), -120);
}
