#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};

use kurbo::{Point, Vec2};
use read_fonts::tables::glyf::CurvePoint;
use read_fonts::{FontRef, TableProvider};
use write_fonts::tables::fvar::{AxisInstanceArrays, Fvar, VariationAxisRecord};
use write_fonts::tables::glyf::{Bbox, Glyph, SimpleGlyph};
use write_fonts::tables::head::Head;
use write_fonts::tables::hhea::Hhea;
use write_fonts::tables::maxp::Maxp;
use write_fonts::tables::name::{Name, NameRecord};
use write_fonts::tables::os2::{Os2, SelectionFlags};
use write_fonts::tables::post::Post;
use write_fonts::types::{Fixed, GlyphId, NameId, Tag, Version16Dot16};
use write_fonts::OffsetMarker;

use nercone_fonts::design::{interpolate, iup_delta, support_scalar, Space};
use nercone_fonts::font::{charmap, tags, Font, Metric, Points};
use nercone_fonts::naming;

pub struct Specimen {
    pub upem: u16,
    pub typo: (i16, i16, i16),
    pub line: (i16, i16, i16),
    pub window: (u16, u16),
    pub selection: u16,
    pub axes: Vec<(&'static str, f64, f64, f64)>,
}

impl Specimen {
    pub fn new() -> Specimen {
        Specimen {
            upem: 1000,
            typo: (800, -200, 0),
            line: (1000, -250, 0),
            window: (1000, 250),
            selection: 0x40,
            axes: Vec::new(),
        }
    }
}

impl Default for Specimen {
    fn default() -> Specimen {
        Specimen::new()
    }
}

pub fn square(upem: u16) -> SimpleGlyph {
    let side = (upem / 2) as i16;
    let contour: Vec<CurvePoint> = vec![
        CurvePoint::new(50, 0, true),
        CurvePoint::new(50, side, true),
        CurvePoint::new(side, side, true),
        CurvePoint::new(side, 0, true),
    ];
    SimpleGlyph {
        bbox: Bbox { x_min: 50, y_min: 0, x_max: side, y_max: side },
        contours: vec![contour.into()],
        instructions: Vec::new(),
        overlaps: false,
    }
}

pub fn build_font(glyphs: &[(u32, &str)], specimen: &Specimen) -> Font {
    let mut order: Vec<&str> = glyphs.iter().map(|(_, name)| *name).collect();
    order.sort();
    order.dedup();
    order.insert(0, ".notdef");

    let side = (specimen.upem / 2) as i16;
    let mut font = Font { tables: BTreeMap::new() };

    font.put(tags::HEAD, &Head {
        font_revision: Fixed::from_f64(1.0),
        units_per_em: specimen.upem,
        x_min: 50,
        y_min: 0,
        x_max: side,
        y_max: side,
        ..Default::default()
    });
    font.put(tags::MAXP, &Maxp {
        num_glyphs: order.len() as u16,
        max_points: Some(4),
        max_contours: Some(1),
        max_composite_points: Some(0),
        max_composite_contours: Some(0),
        max_zones: Some(2),
        max_twilight_points: Some(0),
        max_storage: Some(0),
        max_function_defs: Some(0),
        max_instruction_defs: Some(0),
        max_stack_elements: Some(0),
        max_size_of_instructions: Some(0),
        max_component_elements: Some(0),
        max_component_depth: Some(0),
    });
    font.put(tags::HHEA, &Hhea {
        ascender: specimen.line.0.into(),
        descender: specimen.line.1.into(),
        line_gap: specimen.line.2.into(),
        caret_slope_rise: 1,
        ..Default::default()
    });

    let outline = write_fonts::dump_table(&Glyph::Simple(square(specimen.upem))).expect("failed to serialize glyph");
    let outlines: Vec<Vec<u8>> = order.iter().map(|_| outline.clone()).collect();
    font.set_glyphs(&outlines);
    font.set_metrics(tags::HHEA, tags::HMTX, &vec![Metric { advance: specimen.upem / 2 + 100, bearing: 50 }; order.len()]);

    font.put(tags::OS2, &Os2 {
        s_typo_ascender: specimen.typo.0,
        s_typo_descender: specimen.typo.1,
        s_typo_line_gap: specimen.typo.2,
        us_win_ascent: specimen.window.0,
        us_win_descent: specimen.window.1,
        fs_selection: SelectionFlags::from_bits_truncate(specimen.selection),
        s_cap_height: Some(700),
        sx_height: Some(500),
        ul_code_page_range_1: Some(1),
        ul_code_page_range_2: Some(0),
        us_default_char: Some(0),
        us_break_char: Some(32),
        us_max_context: Some(0),
        ..Default::default()
    });
    font.put(tags::POST, &Post { version: Version16Dot16::VERSION_3_0, ..Default::default() });

    let mapping: BTreeMap<u32, u16> = glyphs
        .iter()
        .map(|(codepoint, name)| {
            let index = order.iter().position(|found| found == name).expect("glyph missing from order");
            (*codepoint, index as u16)
        })
        .collect();
    font.set(tags::CMAP, charmap(&mapping));

    let mut records = vec![
        NameRecord::new(naming::windows.0, naming::windows.1, naming::windows.2, NameId::new(1), OffsetMarker::new("Test".to_string())),
        NameRecord::new(naming::windows.0, naming::windows.1, naming::windows.2, NameId::new(2), OffsetMarker::new("Regular".to_string())),
    ];

    if !specimen.axes.is_empty() {
        let mut entries = Vec::new();
        for (index, (name, minimum, default, maximum)) in specimen.axes.iter().enumerate() {
            let tag = Tag::new(name.as_bytes().try_into().expect("tags are four bytes"));
            let identifier = NameId::new(256 + index as u16);
            records.push(NameRecord::new(naming::windows.0, naming::windows.1, naming::windows.2, identifier, OffsetMarker::new(naming::title(tag))));
            entries.push(VariationAxisRecord {
                axis_tag: tag,
                min_value: Fixed::from_f64(*minimum),
                default_value: Fixed::from_f64(*default),
                max_value: Fixed::from_f64(*maximum),
                flags: Default::default(),
                axis_name_id: identifier,
            });
        }
        font.put(tags::FVAR, &Fvar::new(AxisInstanceArrays::new(entries, Vec::new())));
    }

    let mut table = Name::new(records);
    table.name_record.sort();
    font.put(tags::NAME, &table);

    font
}

pub fn normalize(minimum: f64, default: f64, maximum: f64, value: f64) -> f64 {
    let value = minimum.max(maximum.min(value));
    if value < default {
        if default > minimum {
            return (value - default) / (default - minimum);
        }
        return 0.0;
    }
    if value > default {
        if maximum > default {
            return (value - default) / (maximum - default);
        }
        return 0.0;
    }
    0.0
}

pub fn location(font: &Font, position: &[(Tag, f64)]) -> HashMap<Tag, f64> {
    let fvar = font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar");
    let mappings = Space::mappings(font);

    let mut found = HashMap::new();
    for (index, entry) in fvar.axes().expect("failed to parse fvar axes").iter().enumerate() {
        let tag = entry.axis_tag();
        let value = position
            .iter()
            .find(|(wanted, _)| *wanted == tag)
            .map(|(_, value)| *value)
            .unwrap_or_else(|| entry.default_value().to_f64());
        let mut coordinate = normalize(entry.min_value().to_f64(), entry.default_value().to_f64(), entry.max_value().to_f64(), value);
        if let Some(pairs) = mappings.get(index) {
            if !pairs.is_empty() {
                coordinate = interpolate(coordinate, pairs);
            }
        }
        found.insert(tag, coordinate);
    }
    found
}

pub fn round(value: f64) -> i32 {
    (value + 0.5).floor() as i32
}

pub fn outline(font: &Font, glyph: u32, position: &[(Tag, f64)]) -> Vec<(i32, i32)> {
    let coordinates = location(font, position);

    let data = font.data();
    let reference = FontRef::new(&data).expect("failed to parse font");
    let glyf = reference.glyf().expect("missing glyf");
    let loca = reference.loca(None).expect("missing loca");
    let metrics = font.metrics(tags::HHEA, tags::HMTX);

    let identifier = GlyphId::new(glyph);
    let parsed = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph");
    let points = Points::of(parsed.as_ref(), &metrics[glyph as usize], None);
    let total = points.coordinates.len();
    let mut sample: Vec<Point> = points.coordinates.clone();

    let axes: Vec<Tag> = reference
        .fvar()
        .expect("missing fvar")
        .axes()
        .expect("failed to parse fvar axes")
        .iter()
        .map(|entry| entry.axis_tag())
        .collect();

    if let Ok(gvar) = reference.gvar() {
        if let Ok(Some(variations)) = gvar.glyph_variation_data(identifier) {
            for tuple in variations.tuples() {
                let peaks: Vec<f64> = tuple.peak().values().iter().map(|value| value.get().to_f32() as f64).collect();
                let (starts, ends): (Vec<f64>, Vec<f64>) = match (tuple.intermediate_start(), tuple.intermediate_end()) {
                    (Some(start), Some(end)) => (
                        start.values().iter().map(|value| value.get().to_f32() as f64).collect(),
                        end.values().iter().map(|value| value.get().to_f32() as f64).collect(),
                    ),
                    _ => (peaks.iter().map(|peak| peak.min(0.0)).collect(), peaks.iter().map(|peak| peak.max(0.0)).collect()),
                };
                let support: Vec<(Tag, (f64, f64, f64))> = axes
                    .iter()
                    .copied()
                    .zip(peaks.iter().zip(starts.iter().zip(&ends)).map(|(peak, (start, end))| (*start, *peak, *end)))
                    .collect();

                let scalar = support_scalar(&coordinates, &support);
                if scalar == 0.0 {
                    continue;
                }

                let mut deltas: Vec<Option<Vec2>> = vec![None; total];
                if tuple.has_deltas_for_all_points() {
                    for (index, delta) in tuple.deltas().enumerate() {
                        if index < total {
                            deltas[index] = Some(Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                        }
                    }
                } else {
                    for delta in tuple.deltas() {
                        let index = delta.position as usize;
                        if index < total {
                            deltas[index] = Some(Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                        }
                    }
                }

                let dense = iup_delta(&deltas, &points.coordinates, &points.ends);
                for (value, delta) in sample.iter_mut().zip(&dense) {
                    *value += *delta * scalar;
                }
            }
        }
    }

    let left = sample[total - 4];
    let right = sample[total - 3];
    let advance = round(right.x - left.x);
    let minimum = sample[..total - 4].iter().map(|point| point.x).fold(f64::MAX, f64::min);
    let bearing = round(minimum - left.x);

    let mut found = vec![(advance, bearing)];
    for point in &sample[..total - 4] {
        found.push((round(point.x), round(point.y)));
    }
    found
}

pub fn coordinates(font: &Font, glyph: u32) -> Vec<(i16, i16)> {
    let data = font.data();
    let reference = FontRef::new(&data).expect("failed to parse font");
    let glyf = reference.glyf().expect("missing glyf");
    let loca = reference.loca(None).expect("missing loca");

    match loca.get_glyf(GlyphId::new(glyph), &glyf).expect("failed to parse glyph") {
        Some(read_fonts::tables::glyf::Glyph::Simple(simple)) => simple.points().map(|point| (point.x, point.y)).collect(),
        _ => Vec::new(),
    }
}
