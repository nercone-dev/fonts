mod common;

use std::collections::{BTreeMap, BTreeSet};

use write_fonts::from_obj::ToOwnedTable;
use write_fonts::types::Tag;

use nercone_fonts::design::Axis;
use nercone_fonts::font::Font;
use nercone_fonts::models::{Family, License, Slope, Style, Typeface, Weight};
use nercone_fonts::naming::{Names, Notice};

use common::{build_font, Specimen};

#[allow(non_upper_case_globals)]
pub const ribbi: [&str; 4] = ["Regular", "Italic", "Bold", "Bold Italic"];

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

pub fn styled(weight: Option<Weight>, slope: Slope) -> Style {
    Style { weight, slope }
}

pub fn named(style: &Style) -> Font {
    let mut specimen = Specimen::new();
    if style.variable() {
        specimen.axes = vec![("opsz", 14.0, 14.0, 32.0), ("wght", 100.0, 400.0, 900.0)];
    }
    let mut font = build_font(&[(0x41, "A"), (0x56, "V")], &specimen);

    let axis = match style.variable() {
        true => Axis::new(100.0, 400.0, 900.0),
        false => Axis::new(style.value(), style.value(), style.value()),
    };
    Names::new(&family(), style, &axis, "1.0", "").apply(&mut font);
    font
}

pub fn read(font: &Font, identifier: u16) -> Option<String> {
    let table = font.read::<read_fonts::tables::name::Name>().expect("missing name");
    Notice::debug(&table, identifier)
}

#[test]
fn test_axis_names_match_their_tags() {
    let font = named(&styled(None, Slope::Upright));
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
    let font = named(&styled(None, Slope::Upright));
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
    let font = named(&styled(None, Slope::Upright));
    let stat: write_fonts::tables::stat::Stat = font.read::<read_fonts::tables::stat::Stat>().expect("missing STAT").to_owned_table();

    let tags: BTreeSet<Tag> = stat.design_axes.iter().map(|record| record.axis_tag).collect();
    for wanted in [Tag::new(b"opsz"), Tag::new(b"wght"), Tag::new(b"ital")] {
        assert!(tags.contains(&wanted), "{:?}", tags);
    }
}

#[test]
fn test_every_face_of_a_family_carries_its_own_names() {
    let expected: [(Style, &str, &str, &str, &str); 6] = [
        (styled(None, Slope::Upright),                 "Test Variable", "Regular",     "Test Variable",        "Test-Variable"),
        (styled(None, Slope::Italic),                  "Test Variable", "Italic",      "Test Variable Italic", "Test-VariableItalic"),
        (styled(Some(Weight::Regular), Slope::Upright), "Test",         "Regular",     "Test",                 "Test-Regular"),
        (styled(Some(Weight::Regular), Slope::Italic),  "Test",         "Italic",      "Test Italic",          "Test-RegularItalic"),
        (styled(Some(Weight::Bold), Slope::Upright),    "Test",         "Bold",        "Test Bold",            "Test-Bold"),
        (styled(Some(Weight::Bold), Slope::Italic),     "Test",         "Bold Italic", "Test Bold Italic",     "Test-BoldItalic"),
    ];

    for (style, name, variant, full, postscript) in expected {
        let font = named(&style);
        assert_eq!(read(&font, Names::FAMILY).as_deref(), Some(name), "{:?}", style);
        assert_eq!(read(&font, Names::VARIANT).as_deref(), Some(variant), "{:?}", style);
        assert_eq!(read(&font, Names::FULL).as_deref(), Some(full), "{:?}", style);
        assert_eq!(read(&font, Names::POSTSCRIPT).as_deref(), Some(postscript), "{:?}", style);
    }
}

#[test]
fn test_the_subfamily_is_always_one_of_the_four_ribbi_names() {
    for style in family().styles() {
        let font = named(&style);
        let value = read(&font, Names::VARIANT).expect("missing subfamily name");
        assert!(ribbi.contains(&value.as_str()), "{:?} -> {:?}", style, value);
    }
}

#[test]
fn test_the_unique_identifier_tells_every_face_apart() {
    let styles = family().styles();
    let mut found: BTreeSet<String> = BTreeSet::new();

    for style in &styles {
        let font = named(style);
        let value = read(&font, Names::IDENTIFIER).expect("missing unique identifier");
        let postscript = read(&font, Names::POSTSCRIPT).expect("missing postscript name");
        assert_eq!(value, format!("1.0;NRCN;{}", postscript), "{:?}", style);
        found.insert(value);
    }

    assert_eq!(found.len(), styles.len());
}

#[test]
fn test_typographic_names_appear_only_when_the_ribbi_names_lose_the_style() {
    for style in family().styles() {
        let font = named(&style);
        let name = read(&font, Names::TYPOGRAPHIC_FAMILY);
        let label = read(&font, Names::TYPOGRAPHIC_VARIANT);
        assert_eq!(name.is_some(), label.is_some(), "{:?}", style);
        assert_eq!(name, None, "{:?}", style);
    }
}

#[test]
fn test_a_weight_outside_ribbi_moves_into_the_family_name() {
    for slope in Slope::all() {
        let style = styled(Some(Weight::SemiBold), slope);
        let font = named(&style);
        let label = match slope.italic() {
            true => "SemiBold Italic",
            false => "SemiBold",
        };

        assert_eq!(read(&font, Names::FAMILY).as_deref(), Some("Test SemiBold"), "{:?}", style);
        assert_eq!(read(&font, Names::VARIANT).as_deref(), Some(style.ribbi().as_str()), "{:?}", style);
        assert_eq!(read(&font, Names::TYPOGRAPHIC_FAMILY).as_deref(), Some("Test"), "{:?}", style);
        assert_eq!(read(&font, Names::TYPOGRAPHIC_VARIANT).as_deref(), Some(label), "{:?}", style);
    }
}

#[test]
fn test_the_full_name_joins_the_family_and_the_subfamily() {
    for style in family().styles() {
        let font = named(&style);
        let name = read(&font, Names::FAMILY).expect("missing family name");
        let variant = read(&font, Names::VARIANT).expect("missing subfamily name");
        let wanted = match variant.as_str() {
            "Regular" => name,
            _ => format!("{} {}", name, variant),
        };
        assert_eq!(read(&font, Names::FULL), Some(wanted), "{:?}", style);
    }
}

#[test]
fn test_named_instances_carry_a_postscript_name_of_their_own() {
    for slope in Slope::all() {
        let style = styled(None, slope);
        let font = named(&style);
        let fvar: write_fonts::tables::fvar::Fvar = font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar").to_owned_table();

        let mut names: BTreeSet<String> = BTreeSet::new();
        let mut labels: BTreeSet<String> = BTreeSet::new();
        for instance in &fvar.axis_instance_arrays.instances {
            let identifier = instance.post_script_name_id.expect("missing instance postscript name").to_u16();
            assert!(identifier == Names::POSTSCRIPT || (256..32768).contains(&identifier), "{}", identifier);

            let value = read(&font, identifier).expect("missing instance postscript name");
            assert!(value.chars().all(|entry| entry.is_ascii_alphanumeric() || entry == '-'), "{:?}", value);
            assert!(names.insert(value.clone()), "duplicate instance postscript name {:?}", value);

            let subfamily = instance.subfamily_name_id.to_u16();
            assert!(subfamily == Names::VARIANT || subfamily == Names::TYPOGRAPHIC_VARIANT || (256..32768).contains(&subfamily), "{}", subfamily);

            let label = read(&font, subfamily).expect("missing instance subfamily name");
            assert!(labels.insert(label.clone()), "duplicate instance subfamily name {:?}", label);
        }

        let count = fvar.axis_instance_arrays.instances.len();
        assert_eq!(count, Weight::all().len(), "{:?}", style);
        assert_eq!(names.len(), count, "{:?}", style);
        assert_eq!(labels.len(), count, "{:?}", style);
    }
}

#[test]
fn test_the_default_instance_takes_the_names_of_the_font() {
    for slope in Slope::all() {
        let style = styled(None, slope);
        let font = named(&style);
        let fvar: write_fonts::tables::fvar::Fvar = font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar").to_owned_table();

        let coordinates: Vec<write_fonts::types::Fixed> = fvar.axis_instance_arrays.axes.iter().map(|entry| entry.default_value).collect();
        let instance = fvar.axis_instance_arrays.instances.iter()
            .find(|instance| instance.coordinates == coordinates)
            .expect("missing default instance");

        assert_eq!(instance.post_script_name_id.map(|identifier| identifier.to_u16()), Some(Names::POSTSCRIPT), "{:?}", style);
        assert_eq!(read(&font, instance.subfamily_name_id.to_u16()), read(&font, Names::VARIANT), "{:?}", style);
    }
}

#[test]
fn test_variable_fonts_prefix_generated_instance_names() {
    for slope in Slope::all() {
        let style = styled(None, slope);
        let font = named(&style);
        let prefix = read(&font, Names::VARIATIONS).expect("missing variations prefix");

        assert!(prefix.chars().all(|entry| entry.is_ascii_alphanumeric()), "{:?}", prefix);
        assert!(prefix.len() <= 63, "{:?}", prefix);

        let postscript = read(&font, Names::POSTSCRIPT).expect("missing postscript name");
        assert_eq!(prefix, postscript.replace('-', ""), "{:?}", style);
    }
}

#[test]
fn test_a_font_without_axes_leaves_out_the_variations_prefix() {
    for slope in Slope::all() {
        for weight in [Weight::Regular, Weight::Bold] {
            let style = styled(Some(weight), slope);
            let font = named(&style);
            assert_eq!(read(&font, Names::VARIATIONS), None, "{:?}", style);
        }
    }
}

#[test]
fn test_a_single_weight_keeps_its_prefix_while_it_keeps_an_axis() {
    for slope in Slope::all() {
        let style = styled(Some(Weight::Regular), slope);
        let mut specimen = Specimen::new();
        specimen.axes = vec![("opsz", 14.0, 14.0, 32.0)];
        let mut font = build_font(&[(0x41, "A"), (0x56, "V")], &specimen);
        Names::new(&family(), &style, &Axis::new(400.0, 400.0, 400.0), "1.0", "").apply(&mut font);

        let prefix = read(&font, Names::VARIATIONS).expect("missing variations prefix");
        assert!(prefix.chars().all(|entry| entry.is_ascii_alphanumeric()), "{:?}", prefix);
        assert_eq!(prefix, read(&font, Names::POSTSCRIPT).expect("missing postscript name").replace('-', ""), "{:?}", style);
    }
}
