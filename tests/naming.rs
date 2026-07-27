mod common;

use std::collections::{BTreeMap, BTreeSet};

use write_fonts::from_obj::ToOwnedTable;
use write_fonts::types::Tag;

use nercone_fonts::design::Axis;
use nercone_fonts::font::Font;
use nercone_fonts::models::{Family, License, Slope, Style, Typeface};
use nercone_fonts::naming::{Names, Notice};

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

pub fn named() -> Font {
    let mut specimen = Specimen::new();
    specimen.axes = vec![("opsz", 14.0, 14.0, 32.0), ("wght", 100.0, 400.0, 900.0)];
    let mut font = build_font(&[(0x41, "A"), (0x56, "V")], &specimen);

    let subject = family();
    let style = Style { weight: None, slope: Slope::Upright };
    let axis = Axis::new(100.0, 400.0, 900.0);
    Names::new(&subject, &style, &axis, "1.0", "").apply(&mut font);
    font
}

#[test]
fn test_axis_names_match_their_tags() {
    let font = named();
    let table = font.read::<read_fonts::tables::name::Name>().expect("missing name");
    let fvar: write_fonts::tables::fvar::Fvar = font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar").to_owned_table();

    let mut labels: BTreeMap<Tag, String> = BTreeMap::new();
    for entry in &fvar.axis_instance_arrays.axes {
        let value = Notice::debug(&table, entry.axis_name_id.to_u16()).expect("missing axis name");
        labels.insert(entry.axis_tag, value);
    }

    let expected: BTreeMap<Tag, String> = [
        (Tag::new(b"opsz"), "Optical Size".to_string()),
        (Tag::new(b"wght"), "Weight".to_string()),
    ].into_iter().collect();
    assert_eq!(labels, expected);
}

#[test]
fn test_instances_declare_a_coordinate_for_every_axis() {
    let font = named();
    let fvar: write_fonts::tables::fvar::Fvar = font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar").to_owned_table();

    let tags: Vec<Tag> = fvar.axis_instance_arrays.axes.iter().map(|entry| entry.axis_tag).collect();
    let position = tags.iter().position(|tag| *tag == Tag::new(b"opsz")).expect("missing opsz axis");

    for instance in &fvar.axis_instance_arrays.instances {
        assert_eq!(instance.coordinates.len(), tags.len(), "{:?}", instance);
        assert_eq!(instance.coordinates[position].to_f64(), 14.0, "{:?}", instance);
    }
}

#[test]
fn test_style_attributes_cover_every_axis() {
    let font = named();
    let stat: write_fonts::tables::stat::Stat = font.read::<read_fonts::tables::stat::Stat>().expect("missing STAT").to_owned_table();

    let tags: BTreeSet<Tag> = stat.design_axes.iter().map(|record| record.axis_tag).collect();
    for wanted in [Tag::new(b"opsz"), Tag::new(b"wght"), Tag::new(b"ital")] {
        assert!(tags.contains(&wanted), "{:?}", tags);
    }
}
