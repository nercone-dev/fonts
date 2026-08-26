use std::collections::BTreeSet;

use read_fonts::tables::gpos::{Gpos, PositionSubtables};
use read_fonts::tables::gsub::{Gsub, SubstitutionSubtables};
use read_fonts::tables::layout::{ChainedSequenceContext, SequenceContext};

use crate::font::{tags, Font};

pub const UNICODE_RANGES: [(u32, u32, u32); 169] = [
    (0x0, 0x7F, 0),
    (0x80, 0xFF, 1),
    (0x100, 0x17F, 2),
    (0x180, 0x24F, 3),
    (0x250, 0x2AF, 4), (0x1D00, 0x1D7F, 4), (0x1D80, 0x1DBF, 4),
    (0x2B0, 0x2FF, 5), (0xA700, 0xA71F, 5),
    (0x300, 0x36F, 6), (0x1DC0, 0x1DFF, 6),
    (0x370, 0x3FF, 7),
    (0x2C80, 0x2CFF, 8),
    (0x400, 0x4FF, 9), (0x500, 0x52F, 9), (0x2DE0, 0x2DFF, 9), (0xA640, 0xA69F, 9),
    (0x530, 0x58F, 10),
    (0x590, 0x5FF, 11),
    (0xA500, 0xA63F, 12),
    (0x600, 0x6FF, 13), (0x750, 0x77F, 13),
    (0x7C0, 0x7FF, 14),
    (0x900, 0x97F, 15),
    (0x980, 0x9FF, 16),
    (0xA00, 0xA7F, 17),
    (0xA80, 0xAFF, 18),
    (0xB00, 0xB7F, 19),
    (0xB80, 0xBFF, 20),
    (0xC00, 0xC7F, 21),
    (0xC80, 0xCFF, 22),
    (0xD00, 0xD7F, 23),
    (0xE00, 0xE7F, 24),
    (0xE80, 0xEFF, 25),
    (0x10A0, 0x10FF, 26), (0x2D00, 0x2D2F, 26),
    (0x1B00, 0x1B7F, 27),
    (0x1100, 0x11FF, 28),
    (0x1E00, 0x1EFF, 29), (0x2C60, 0x2C7F, 29), (0xA720, 0xA7FF, 29),
    (0x1F00, 0x1FFF, 30),
    (0x2000, 0x206F, 31), (0x2E00, 0x2E7F, 31),
    (0x2070, 0x209F, 32),
    (0x20A0, 0x20CF, 33),
    (0x20D0, 0x20FF, 34),
    (0x2100, 0x214F, 35),
    (0x2150, 0x218F, 36),
    (0x2190, 0x21FF, 37), (0x27F0, 0x27FF, 37), (0x2900, 0x297F, 37), (0x2B00, 0x2BFF, 37),
    (0x2200, 0x22FF, 38), (0x2A00, 0x2AFF, 38), (0x27C0, 0x27EF, 38), (0x2980, 0x29FF, 38),
    (0x2300, 0x23FF, 39),
    (0x2400, 0x243F, 40),
    (0x2440, 0x245F, 41),
    (0x2460, 0x24FF, 42),
    (0x2500, 0x257F, 43),
    (0x2580, 0x259F, 44),
    (0x25A0, 0x25FF, 45),
    (0x2600, 0x26FF, 46),
    (0x2700, 0x27BF, 47),
    (0x3000, 0x303F, 48),
    (0x3040, 0x309F, 49),
    (0x30A0, 0x30FF, 50), (0x31F0, 0x31FF, 50),
    (0x3100, 0x312F, 51), (0x31A0, 0x31BF, 51),
    (0x3130, 0x318F, 52),
    (0xA840, 0xA87F, 53),
    (0x3200, 0x32FF, 54),
    (0x3300, 0x33FF, 55),
    (0xAC00, 0xD7AF, 56),
    (0xD800, 0xDFFF, 57),
    (0x10900, 0x1091F, 58),
    (0x4E00, 0x9FFF, 59), (0x2E80, 0x2EFF, 59), (0x2F00, 0x2FDF, 59), (0x2FF0, 0x2FFF, 59), (0x3400, 0x4DBF, 59), (0x20000, 0x2A6DF, 59), (0x3190, 0x319F, 59),
    (0xE000, 0xF8FF, 60),
    (0x31C0, 0x31EF, 61), (0xF900, 0xFAFF, 61), (0x2F800, 0x2FA1F, 61),
    (0xFB00, 0xFB4F, 62),
    (0xFB50, 0xFDFF, 63),
    (0xFE20, 0xFE2F, 64),
    (0xFE10, 0xFE1F, 65), (0xFE30, 0xFE4F, 65),
    (0xFE50, 0xFE6F, 66),
    (0xFE70, 0xFEFF, 67),
    (0xFF00, 0xFFEF, 68),
    (0xFFF0, 0xFFFF, 69),
    (0xF00, 0xFFF, 70),
    (0x700, 0x74F, 71),
    (0x780, 0x7BF, 72),
    (0xD80, 0xDFF, 73),
    (0x1000, 0x109F, 74),
    (0x1200, 0x137F, 75), (0x1380, 0x139F, 75), (0x2D80, 0x2DDF, 75),
    (0x13A0, 0x13FF, 76),
    (0x1400, 0x167F, 77),
    (0x1680, 0x169F, 78),
    (0x16A0, 0x16FF, 79),
    (0x1780, 0x17FF, 80), (0x19E0, 0x19FF, 80),
    (0x1800, 0x18AF, 81),
    (0x2800, 0x28FF, 82),
    (0xA000, 0xA48F, 83), (0xA490, 0xA4CF, 83),
    (0x1700, 0x171F, 84), (0x1720, 0x173F, 84), (0x1740, 0x175F, 84), (0x1760, 0x177F, 84),
    (0x10300, 0x1032F, 85),
    (0x10330, 0x1034F, 86),
    (0x10400, 0x1044F, 87),
    (0x1D000, 0x1D0FF, 88), (0x1D100, 0x1D1FF, 88), (0x1D200, 0x1D24F, 88),
    (0x1D400, 0x1D7FF, 89),
    (0xF0000, 0xFFFFD, 90), (0x100000, 0x10FFFD, 90),
    (0xFE00, 0xFE0F, 91), (0xE0100, 0xE01EF, 91),
    (0xE0000, 0xE007F, 92),
    (0x1900, 0x194F, 93),
    (0x1950, 0x197F, 94),
    (0x1980, 0x19DF, 95),
    (0x1A00, 0x1A1F, 96),
    (0x2C00, 0x2C5F, 97),
    (0x2D30, 0x2D7F, 98),
    (0x4DC0, 0x4DFF, 99),
    (0xA800, 0xA82F, 100),
    (0x10000, 0x1007F, 101), (0x10080, 0x100FF, 101), (0x10100, 0x1013F, 101),
    (0x10140, 0x1018F, 102),
    (0x10380, 0x1039F, 103),
    (0x103A0, 0x103DF, 104),
    (0x10450, 0x1047F, 105),
    (0x10480, 0x104AF, 106),
    (0x10800, 0x1083F, 107),
    (0x10A00, 0x10A5F, 108),
    (0x1D300, 0x1D35F, 109),
    (0x12000, 0x123FF, 110), (0x12400, 0x1247F, 110),
    (0x1D360, 0x1D37F, 111),
    (0x1B80, 0x1BBF, 112),
    (0x1C00, 0x1C4F, 113),
    (0x1C50, 0x1C7F, 114),
    (0xA880, 0xA8DF, 115),
    (0xA900, 0xA92F, 116),
    (0xA930, 0xA95F, 117),
    (0xAA00, 0xAA5F, 118),
    (0x10190, 0x101CF, 119),
    (0x101D0, 0x101FF, 120),
    (0x102A0, 0x102DF, 121), (0x10280, 0x1029F, 121), (0x10920, 0x1093F, 121),
    (0x1F030, 0x1F09F, 122), (0x1F000, 0x1F02F, 122),
];

pub struct Private;

#[allow(non_upper_case_globals)]
impl Private {
    pub const areas: [(u32, u32); 3] = [(0xE000, 0xF8FF), (0xF0000, 0xFFFFD), (0x100000, 0x10FFFD)];

    pub fn holds(codepoint: u32) -> bool {
        Private::areas.iter().any(|(low, high)| *low <= codepoint && codepoint <= *high)
    }

    pub fn of(codepoints: &BTreeSet<u32>) -> BTreeSet<u32> {
        codepoints.iter().copied().filter(|codepoint| Private::holds(*codepoint)).collect()
    }
}

pub struct Codepages;

#[allow(non_upper_case_globals)]
impl Codepages {
    pub const japanese: u32 = 17;
    pub const simplified: u32 = 18;
    pub const wansung: u32 = 19;
    pub const traditional: u32 = 20;
    pub const johab: u32 = 21;

    pub const cjk: [u32; 5] = [Codepages::japanese, Codepages::simplified, Codepages::wansung, Codepages::traditional, Codepages::johab];

    pub fn primary(region: &str) -> u32 {
        match region {
            "CJK" | "JP" => Codepages::japanese,
            "SC" => Codepages::simplified,
            "TC" => Codepages::traditional,
            "KR" => Codepages::wansung,
            _ => panic!("unsupported region: {}", region),
        }
    }

    pub fn restrict(ranges: [u32; 2], region: &str) -> [u32; 2] {
        let primary = Codepages::primary(region);
        let mut words = ranges;
        for bit in Codepages::cjk {
            if bit != primary {
                words[(bit / 32) as usize] &= !(1 << (bit % 32));
            }
        }
        words
    }
}

pub fn unicode_ranges(codepoints: &BTreeSet<u32>) -> [u32; 4] {
    let mut ranges = UNICODE_RANGES;
    ranges.sort();

    let mut words = [0u32; 4];
    let mut set = |bit: u32| words[(bit / 32) as usize] |= 1 << (bit % 32);
    for &code in codepoints {
        let index = ranges.partition_point(|range| range.0 <= code);
        if index > 0 {
            let (_, stop, bit) = ranges[index - 1];
            if code <= stop {
                set(bit);
            }
        }
        if (0x10000..0x110000).contains(&code) {
            set(57);
        }
    }
    words
}

pub fn codepage_ranges(codepoints: &BTreeSet<u32>) -> [u32; 2] {
    let mut bits = BTreeSet::new();
    let has_ascii = (0x20..0x7E).all(|code| codepoints.contains(&code));
    let has_lineart = codepoints.contains(&('┤' as u32));
    let has_root = codepoints.contains(&('√' as u32));

    for &code in codepoints {
        if code == 'Þ' as u32 && has_ascii {
            bits.insert(0); // Latin 1
        } else if code == 'Ľ' as u32 && has_ascii {
            bits.insert(1); // Latin 2: Eastern Europe
            if has_lineart {
                bits.insert(58); // Latin 2
            }
        } else if code == 'Б' as u32 {
            bits.insert(2); // Cyrillic
            if codepoints.contains(&('Ѕ' as u32)) && has_lineart {
                bits.insert(57); // IBM Cyrillic
            }
            if codepoints.contains(&('╜' as u32)) && has_lineart {
                bits.insert(49); // MS-DOS Russian
            }
        } else if code == 'Ά' as u32 {
            bits.insert(3); // Greek
            if has_lineart && codepoints.contains(&('½' as u32)) {
                bits.insert(48); // IBM Greek
            }
            if has_lineart && has_root {
                bits.insert(60); // Greek, former 437 G
            }
        } else if code == 'İ' as u32 && has_ascii {
            bits.insert(4); // Turkish
            if has_lineart {
                bits.insert(56); // IBM turkish
            }
        } else if code == 'א' as u32 {
            bits.insert(5); // Hebrew
            if has_lineart && has_root {
                bits.insert(53); // Hebrew
            }
        } else if code == 'ر' as u32 {
            bits.insert(6); // Arabic
            if has_root {
                bits.insert(51); // Arabic
            }
            if has_lineart {
                bits.insert(61); // Arabic; ASMO 708
            }
        } else if code == 'ŗ' as u32 && has_ascii {
            bits.insert(7); // Windows Baltic
            if has_lineart {
                bits.insert(59); // MS-DOS Baltic
            }
        } else if code == '₫' as u32 && has_ascii {
            bits.insert(8); // Vietnamese
        } else if code == 'ๅ' as u32 {
            bits.insert(16); // Thai
        } else if code == 'エ' as u32 {
            bits.insert(17); // JIS/Japan
        } else if code == 'ㄅ' as u32 {
            bits.insert(18); // Chinese: Simplified
        } else if code == 'ㄱ' as u32 {
            bits.insert(19); // Korean wansung
        } else if code == '央' as u32 {
            bits.insert(20); // Chinese: Traditional
        } else if code == '곴' as u32 {
            bits.insert(21); // Korean Johab
        } else if code == '♥' as u32 && has_ascii {
            bits.insert(30); // OEM Character Set
        } else if code == 'þ' as u32 && has_ascii && has_lineart {
            bits.insert(54); // MS-DOS Icelandic
        } else if code == '╚' as u32 && has_ascii {
            bits.insert(62); // WE/Latin 1
            bits.insert(63); // US
        } else if has_ascii && has_lineart && has_root {
            if code == 'Å' as u32 {
                bits.insert(50); // MS-DOS Nordic
            } else if code == 'é' as u32 {
                bits.insert(52); // MS-DOS Canadian French
            } else if code == 'õ' as u32 {
                bits.insert(55); // MS-DOS Portuguese
            }
        }
    }

    if has_ascii && codepoints.contains(&('‰' as u32)) && codepoints.contains(&('∑' as u32)) {
        bits.insert(29); // Macintosh Character Set (US Roman)
    }
    if bits.is_empty() {
        bits.insert(0); // Latin 1, so that the font works in MS Word
    }

    let mut words = [0u32; 2];
    for bit in bits {
        words[(bit / 32) as usize] |= 1 << (bit % 32);
    }
    words
}

pub fn max_context(font: &Font) -> u16 {
    let mut maximum = 0;
    if let Some(gsub) = font.read::<Gsub>() {
        if let Ok(list) = gsub.lookup_list() {
            for lookup in list.lookups().iter().flatten() {
                if let Ok(subtables) = lookup.subtables() {
                    maximum = substitution_context(maximum, &subtables);
                }
            }
        }
    }
    if let Some(gpos) = font.read::<Gpos>() {
        if let Ok(list) = gpos.lookup_list() {
            for lookup in list.lookups().iter().flatten() {
                if let Ok(subtables) = lookup.subtables() {
                    maximum = position_context(maximum, &subtables);
                }
            }
        }
    }
    maximum
}

pub fn substitution_context(mut maximum: u16, subtables: &SubstitutionSubtables) -> u16 {
    match subtables {
        SubstitutionSubtables::Single(tables) if !tables.is_empty() => maximum.max(1),
        SubstitutionSubtables::Multiple(tables) if !tables.is_empty() => maximum.max(1),
        SubstitutionSubtables::Alternate(tables) if !tables.is_empty() => maximum.max(1),
        SubstitutionSubtables::Ligature(tables) => {
            for table in tables.iter().flatten() {
                for set in table.ligature_sets().iter().flatten() {
                    for ligature in set.ligatures().iter().flatten() {
                        maximum = maximum.max(ligature.component_count());
                    }
                }
            }
            maximum
        }
        SubstitutionSubtables::Contextual(tables) => {
            tables.iter().flatten().fold(maximum, |found, table| sequence_context(found, &table))
        }
        SubstitutionSubtables::ChainContextual(tables) => {
            tables.iter().flatten().fold(maximum, |found, table| chained_context(found, &table))
        }
        SubstitutionSubtables::Reverse(tables) => {
            tables.iter().flatten().fold(maximum, |found, table| found.max(1 + table.lookahead_glyph_count()))
        }
        _ => maximum,
    }
}

pub fn position_context(maximum: u16, subtables: &PositionSubtables) -> u16 {
    match subtables {
        PositionSubtables::Single(tables) if !tables.is_empty() => maximum.max(1),
        PositionSubtables::Pair(tables) if !tables.is_empty() => maximum.max(2),
        PositionSubtables::Contextual(tables) => {
            tables.iter().flatten().fold(maximum, |found, table| sequence_context(found, &table))
        }
        PositionSubtables::ChainContextual(tables) => {
            tables.iter().flatten().fold(maximum, |found, table| chained_context(found, &table))
        }
        _ => maximum,
    }
}

pub fn sequence_context(mut maximum: u16, context: &SequenceContext) -> u16 {
    match context {
        SequenceContext::Format1(table) => {
            for set in table.seq_rule_sets().iter().flatten().flatten() {
                for rule in set.seq_rules().iter().flatten() {
                    maximum = maximum.max(rule.glyph_count());
                }
            }
        }
        SequenceContext::Format2(table) => {
            for set in table.class_seq_rule_sets().iter().flatten().flatten() {
                for rule in set.class_seq_rules().iter().flatten() {
                    maximum = maximum.max(rule.glyph_count());
                }
            }
        }
        SequenceContext::Format3(table) => maximum = maximum.max(table.glyph_count()),
    }
    maximum
}

pub fn chained_context(mut maximum: u16, context: &ChainedSequenceContext) -> u16 {
    match context {
        ChainedSequenceContext::Format1(table) => {
            for set in table.chained_seq_rule_sets().iter().flatten().flatten() {
                for rule in set.chained_seq_rules().iter().flatten() {
                    maximum = maximum.max(rule.input_glyph_count() + rule.lookahead_glyph_count());
                }
            }
        }
        ChainedSequenceContext::Format2(table) => {
            for set in table.chained_class_seq_rule_sets().iter().flatten().flatten() {
                for rule in set.chained_class_seq_rules().iter().flatten() {
                    maximum = maximum.max(rule.input_glyph_count() + rule.lookahead_glyph_count());
                }
            }
        }
        ChainedSequenceContext::Format3(table) => {
            maximum = maximum.max(table.input_glyph_count() + table.lookahead_glyph_count());
        }
    }
    maximum
}

pub fn average_width(font: &Font) -> i16 {
    if !font.contains(tags::HMTX) {
        return 0;
    }
    let widths: Vec<u64> = font
        .metrics(tags::HHEA, tags::HMTX)
        .iter()
        .map(|metric| metric.advance as u64)
        .filter(|width| *width > 0)
        .collect();
    if widths.is_empty() {
        return 0;
    }
    let average = widths.iter().sum::<u64>() as f64 / widths.len() as f64;
    (average + 0.5).floor() as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn load(path: &str) -> Font {
        Font::new(&std::fs::read(path).expect("missing test font"))
    }

    pub fn codepoints(font: &Font) -> BTreeSet<u32> {
        font.cmap().keys().copied().collect()
    }

    #[test]
    fn private_use_areas_follow_the_unicode_standard() {
        for codepoint in [0xE000, 0xE0B0, 0xF8FF, 0xF0000, 0xFFFFD, 0x100000, 0x10FFFD] {
            assert!(Private::holds(codepoint), "U+{:04X} is a private use codepoint", codepoint);
        }
        for codepoint in [0x41, 0x2665, 0xDFFF, 0xF900, 0xEFFFF, 0xFFFFE, 0x10FFFE] {
            assert!(!Private::holds(codepoint), "U+{:04X} is not a private use codepoint", codepoint);
        }
    }

    #[test]
    fn private_keeps_only_private_use_codepoints() {
        let codepoints: BTreeSet<u32> = [0x41, 0x2665, 0xE000, 0xE0B0, 0xF8FF, 0xF900, 0xF0000, 0x10FFFE].into_iter().collect();
        let found: BTreeSet<u32> = [0xE000, 0xE0B0, 0xF8FF, 0xF0000].into_iter().collect();
        assert_eq!(Private::of(&codepoints), found);
    }

    #[test]
    fn every_region_claims_a_single_east_asian_codepage() {
        let east: u32 = Codepages::cjk.iter().map(|bit| 1u32 << bit).sum();
        for (region, wanted) in [("CJK", Codepages::japanese), ("JP", Codepages::japanese), ("SC", Codepages::simplified), ("TC", Codepages::traditional), ("KR", Codepages::wansung)] {
            let found = Codepages::restrict([east | 0x0000019F, 0xFFFFFFFF], region);
            assert_eq!(found[0] & east, 1u32 << wanted, "{}", region);
            assert_eq!(found[0] & !east, 0x0000019F, "{}", region);
            assert_eq!(found[1], 0xFFFFFFFF, "{}", region);
        }
    }

    #[test]
    fn no_region_claims_a_codepage_the_font_does_not_carry() {
        for region in ["CJK", "JP", "SC", "TC", "KR"] {
            assert_eq!(Codepages::restrict([0, 0], region), [0, 0], "{}", region);
        }
    }

    #[test]
    #[should_panic(expected = "unsupported region")]
    fn an_unknown_region_is_rejected() {
        Codepages::restrict([0, 0], "Latin");
    }

    #[test]
    fn a_pan_east_asian_font_keeps_only_the_codepage_of_its_region() {
        let font = load("build/sources/noto/NotoSansJP.ttf");
        let codepoints = codepoints(&font);
        let ranges = codepage_ranges(&codepoints);

        for (region, wanted) in [("CJK", Codepages::japanese), ("JP", Codepages::japanese), ("SC", Codepages::simplified), ("TC", Codepages::traditional), ("KR", Codepages::wansung)] {
            let found = Codepages::restrict(ranges, region);
            let bits: Vec<u32> = Codepages::cjk.into_iter().filter(|bit| (found[0] >> bit) & 1 == 1).collect();
            assert_eq!(bits, vec![wanted], "{}", region);
        }
    }

    #[test]
    fn inter() {
        let font = load("build/sources/inter/InterVariable.ttf");
        let codepoints = codepoints(&font);
        assert_eq!(unicode_ranges(&codepoints), [0xE10002FF, 0x1200E5FF, 0x00000008, 0x00100000]);
        assert_eq!(codepage_ranges(&codepoints), [0x6000019F, 0x00000000]);
        assert_eq!(max_context(&font), 12);
        assert_eq!(average_width(&font), 1311);
    }

    #[test]
    fn noto() {
        let font = load("build/sources/noto/NotoSansJP.ttf");
        let codepoints = codepoints(&font);
        assert_eq!(unicode_ranges(&codepoints), [0xA00002FF, 0x6ADFFDFF, 0x00000016, 0x00000000]);
        assert_eq!(codepage_ranges(&codepoints), [0x601E0105, 0xC0D60000]);
        assert_eq!(max_context(&font), 6);
        assert_eq!(average_width(&font), 981);
    }

    #[test]
    fn meslo() {
        let font = load("build/sources/meslo/MesloLGS-Regular.ttf");
        let codepoints = codepoints(&font);
        assert_eq!(unicode_ranges(&codepoints), [0xE40002FF, 0x5000F9FB, 0x00000028, 0x00000000]);
        assert_eq!(codepage_ranges(&codepoints), [0x6000019F, 0xDFD70000]);
        assert_eq!(max_context(&font), 0);
        assert_eq!(average_width(&font), 1233);
    }
}
