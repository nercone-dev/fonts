use std::collections::BTreeSet;

use nercone_fonts::constants::{mono, regions, sans, serif, Families, URLs};
use nercone_fonts::models::{Family, Slope};

pub fn typefaces() -> Vec<(&'static str, fn(&str, bool) -> Family)> {
    vec![("Sans", sans as fn(&str, bool) -> Family), ("Serif", serif), ("Mono", mono)]
}

pub fn symbols(family: &Family) -> String {
    let symbols = family.symbols.as_ref().unwrap_or_else(|| panic!("{} has no symbols", family.name));
    assert_eq!(symbols.sources.len(), 1, "{} needs exactly one symbol source", family.name);
    let source = &symbols.sources[0];
    assert_eq!(source.url, URLs::nerd_fonts, "{} takes its symbols from elsewhere", family.name);
    assert_eq!(source.slope, Slope::Upright, "{} has slanted symbols", family.name);
    let member = source.member.clone().unwrap_or_else(|| panic!("{} does not name a zip member", family.name));
    assert!(source.path.ends_with(&format!("/{}", member)), "{} stores {} at {}", family.name, member, source.path);
    member
}

#[test]
fn test_every_typeface_and_region_has_a_plain_and_a_nerd_fonts_family() {
    let all = Families::all();
    assert_eq!(all.len(), typefaces().len() * regions.len() * 2);

    for (typeface, _) in typefaces() {
        for region in regions {
            let found: Vec<&Family> = all.iter().filter(|family| family.typeface == typeface && family.region == region).collect();
            assert_eq!(found.len(), 2, "{} {} is not built in both variations", typeface, region);
            assert_eq!(found.iter().filter(|family| family.symbols.is_none()).count(), 1, "{} {} lacks a plain variation", typeface, region);
            assert_eq!(found.iter().filter(|family| family.symbols.is_some()).count(), 1, "{} {} lacks a Nerd Fonts variation", typeface, region);
        }
    }
}

#[test]
fn test_nerd_fonts_families_only_differ_by_their_suffix() {
    for (typeface, build) in typefaces() {
        for region in regions {
            let plain = build(region, false);
            let patched = build(region, true);

            assert!(plain.symbols.is_none(), "{} carries symbols without being asked", plain.name);
            assert_eq!(patched.name, format!("{} NF", plain.name));
            assert_eq!(patched.filename, format!("{}NF", plain.filename));
            assert_eq!(patched.latin, plain.latin, "{} uses another latin typeface", patched.name);
            assert_eq!(patched.cjk, plain.cjk, "{} uses other CJK typefaces", patched.name);
            assert_eq!(patched.license, plain.license, "{} is licensed differently", patched.name);
            assert_eq!(patched.typeface, typeface);
            assert_eq!(patched.region, region);
            assert_eq!(patched.monospace, plain.monospace, "{} changes its spacing", patched.name);
            assert_eq!(patched.styles(), plain.styles(), "{} offers other styles", patched.name);
        }
    }
}

#[test]
fn test_symbols_match_the_spacing_of_the_family() {
    for family in Families::all().iter().filter(|family| family.symbols.is_some()) {
        let wanted = if family.monospace { "SymbolsNerdFontMono-Regular.ttf" } else { "SymbolsNerdFont-Regular.ttf" };
        assert_eq!(symbols(family), wanted, "{} takes the wrong symbols", family.name);
    }
}

#[test]
fn test_monospace_is_reserved_for_mono() {
    for family in Families::all() {
        assert_eq!(family.monospace, family.typeface == "Mono", "{} is spaced unexpectedly", family.name);
    }
}

#[test]
fn test_names_and_filenames_are_unique() {
    let all = Families::all();
    let names: BTreeSet<&String> = all.iter().map(|family| &family.name).collect();
    let filenames: BTreeSet<&String> = all.iter().map(|family| &family.filename).collect();
    assert_eq!(names.len(), all.len());
    assert_eq!(filenames.len(), all.len());
}

#[test]
fn test_nerd_fonts_families_credit_the_symbols() {
    for family in Families::all() {
        let credited = family.credits().iter().any(|name| name == "Nerd Fonts");
        assert_eq!(credited, family.symbols.is_some(), "{} credits its symbols wrongly", family.name);
        assert_eq!(family.description().contains("Nerd Fonts"), family.symbols.is_some(), "{} describes its symbols wrongly", family.name);
    }
}

#[test]
fn test_collections_hold_every_family_once() {
    let collections = Families::collections();
    let mut collected: Vec<String> = collections.iter().flat_map(|(_, group)| group.iter().map(|family| family.filename.clone())).collect();
    let mut all: Vec<String> = Families::all().iter().map(|family| family.filename.clone()).collect();
    collected.sort();
    all.sort();
    assert_eq!(collected, all);

    for (name, group) in &collections {
        for family in group {
            assert!(family.filename.starts_with(name.as_str()), "{} does not belong in {}", family.filename, name);
        }
    }
}
