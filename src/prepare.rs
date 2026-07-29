use std::collections::BTreeSet;

use read_fonts::{FontRead, FontRef, TableProvider};
use write_fonts::from_obj::FromTableRef;
use read_fonts::tables::glyf::CurvePoint;
use write_fonts::tables::glyf::{Anchor, Glyph};
use write_fonts::tables::gvar::{GlyphDelta, GlyphDeltas, GlyphVariations, Gvar, Tent};
use write_fonts::types::{F2Dot14, GlyphId, Tag};

use crate::design::{Axis, Mapping, Space};
use crate::font::{tags, Font, Metric};
use crate::harfbuzz;
use crate::scale::Scaler;

pub fn features(extra: &[&str], without: &[&str]) -> Vec<String> {
    let mut found: BTreeSet<&str> = Features::default.iter().copied().collect();
    found.extend(extra);
    for tag in without {
        found.remove(tag);
    }
    found.into_iter().map(str::to_string).collect()
}

pub struct Features;

#[allow(non_upper_case_globals)]
impl Features {
    pub const default: [&'static str; 67] = [
        "BUZZ", "Buzz", "HARF", "Harf", "abvf", "abvm", "abvs", "akhn", "blwf", "blwm", "blws", "calt", "ccmp", "cfar", "chws", "cjct",
        "clig", "cswh", "curs", "dist", "dnom", "fin2", "fin3", "fina", "frac", "half", "haln", "halt", "init", "isol", "jalt", "kern",
        "liga", "ljmo", "locl", "ltra", "ltrm", "mark", "med2", "medi", "mkmk", "mset", "nukt", "numr", "pref", "pres", "pstf", "psts",
        "rand", "rclt", "rkrf", "rlig", "rphf", "rtla", "rtlm", "rvrn", "stch", "tjmo", "valt", "vatu", "vchw", "vert", "vhal", "vjmo",
        "vkrn", "vpal", "vrt2",
    ];

    pub const proportional: [&'static str; 11] = ["kern", "vkrn", "palt", "halt", "vpal", "vhal", "pwid", "twid", "qwid", "chws", "vchw"];
    pub const ligating: [&'static str; 7] = ["liga", "dlig", "clig", "hlig", "rlig", "calt", "rclt"];

    pub fn latin() -> Vec<String> {
        vec!["*".to_string()]
    }

    pub fn cjk() -> Vec<String> {
        features(&["fwid", "hwid", "pwid", "palt", "ruby"], &[])
    }

    pub fn symbols() -> Vec<String> {
        features(&[], &[])
    }
}

pub struct Tables;

#[allow(non_upper_case_globals)]
impl Tables {
    pub const defaults: [&'static str; 12] = ["DSIG", "EBDT", "EBLC", "EBSC", "Feat", "Glat", "Gloc", "JSTF", "LTSH", "PCLT", "Silf", "Sill"];

    pub const apple: [&'static str; 16] = ["morx", "mort", "feat", "prop", "kerx", "kern", "ankr", "bsln", "lcar", "opbd", "trak", "just", "Zapf", "acnt", "fdsc", "fmtx"];

    pub const private: [&'static str; 13] = ["DSIG", "PfEd", "FFTM", "TeX ", "Silf", "Glat", "Gloc", "Feat", "Sill", "gasp", "MVAR", "STAT", "cvar"];

    pub fn dropped() -> Vec<Tag> {
        let mut found: BTreeSet<&str> = Tables::defaults.iter().copied().collect();
        found.extend(Tables::apple);
        found.extend(Tables::private);
        found.into_iter().map(|tag| Tag::new(tag.as_bytes().try_into().expect("tags are four bytes"))).collect()
    }
}

pub struct Component {
    pub font: Font,
    pub name: String,
    pub codepoints: BTreeSet<u32>,
    pub features: Vec<String>,
}

impl Component {
    pub fn new(font: Font, name: &str, codepoints: Option<BTreeSet<u32>>, features: Option<Vec<String>>) -> Component {
        let codepoints = codepoints.unwrap_or_else(|| font.cmap().keys().copied().collect());
        Component {
            font,
            name: name.to_string(),
            codepoints,
            features: features.unwrap_or_else(Features::latin),
        }
    }

    pub fn load(data: &[u8], name: &str, features: Option<Vec<String>>) -> Component {
        Component::new(Font::new(data), name, None, features)
    }

    pub fn prepare(&mut self, axis: &Axis, upem: u16, scale: f64, retain: bool) -> &mut Component {
        self.subset();

        let current = self.font.upem();
        let target = (upem as f64 * scale).round_ties_even() as u16;
        if self.font.contains(tags::FVAR) && target != current {
            let factor = target as f64 / current as f64;
            crate::statics::Rebase::new(axis.minimum, axis.default, axis.maximum, factor).apply(&mut self.font);

            let scaler = Scaler::new(factor);
            scaler.headers(&mut self.font);
            scaler.profile(&mut self.font);
            scaler.post(&mut self.font);

            let mut head: write_fonts::tables::head::Head = write_fonts::from_obj::ToOwnedTable::to_owned_table(
                &self.font.read::<read_fonts::tables::head::Head>().expect("missing head"),
            );
            head.units_per_em = upem;
            self.font.put(tags::HEAD, &head);
        } else {
            self.rebase(axis, retain);
            self.scale(upem, scale);
        }
        self
    }

    pub fn space(&self) -> Option<Space> {
        Space::read(&self.font)
    }

    pub fn breakpoints(&self, space: &Space) -> Vec<f64> {
        Mapping::new(&self.font, space).breakpoints(&self.font)
    }

    pub fn retarget(&mut self, space: &Space, masters: &[f64]) {
        let mapping = Mapping::new(&self.font, space);
        mapping.apply(&mut self.font, masters);
    }

    pub fn subset(&mut self) {
        let face = harfbuzz::Face::new(&self.font.data());
        let input = harfbuzz::SubsetInput::new();

        input.unicodes(self.codepoints.iter().copied());
        if self.features == Features::latin() {
            input.all_layout_features();
        } else {
            let tags: Vec<Tag> = self.features.iter().map(|tag| Tag::new(tag.as_bytes().try_into().expect("tags are four bytes"))).collect();
            input.layout_features(&tags);
        }
        input.all_layout_scripts();
        input.drop_tables(&Tables::dropped());
        input.flags(harfbuzz::FLAGS_NO_HINTING | harfbuzz::FLAGS_NOTDEF_OUTLINE | harfbuzz::FLAGS_GLYPH_NAMES | harfbuzz::FLAGS_NO_PRUNE_UNICODE_RANGES);

        let data = input.subset(&face).expect("failed to subset");
        self.font = Font::new(&data);
        self.codepoints = self.font.cmap().keys().copied().collect();
    }

    pub fn explicit(&mut self) {
        use write_fonts::tables::gpos::{PairPos, PositionLookup, SinglePos, ValueRecord};

        let Some(data) = self.font.get(tags::GPOS) else {
            return;
        };
        let parsed = read_fonts::tables::gpos::Gpos::read(read_fonts::FontData::new(data)).expect("failed to parse GPOS");
        let mut owned: write_fonts::tables::gpos::Gpos = write_fonts::from_obj::ToOwnedTable::to_owned_table(&parsed);

        let fill = |record: &mut ValueRecord| {
            let missing = (record.x_placement_device.is_some() && record.x_placement.is_none())
                || (record.y_placement_device.is_some() && record.y_placement.is_none())
                || (record.x_advance_device.is_some() && record.x_advance.is_none())
                || (record.y_advance_device.is_some() && record.y_advance.is_none());
            if !missing {
                return;
            }

            let mut fresh = ValueRecord::new();
            fresh.x_placement = record.x_placement.or(if record.x_placement_device.is_some() { Some(0) } else { None });
            fresh.y_placement = record.y_placement.or(if record.y_placement_device.is_some() { Some(0) } else { None });
            fresh.x_advance = record.x_advance.or(if record.x_advance_device.is_some() { Some(0) } else { None });
            fresh.y_advance = record.y_advance.or(if record.y_advance_device.is_some() { Some(0) } else { None });
            fresh.x_placement_device = std::mem::take(&mut record.x_placement_device);
            fresh.y_placement_device = std::mem::take(&mut record.y_placement_device);
            fresh.x_advance_device = std::mem::take(&mut record.x_advance_device);
            fresh.y_advance_device = std::mem::take(&mut record.y_advance_device);
            *record = fresh;
        };

        let positions = |subtables: &mut Vec<write_fonts::OffsetMarker<SinglePos>>| {
            for subtable in subtables.iter_mut() {
                match &mut **subtable {
                    SinglePos::Format1(table) => fill(&mut table.value_record),
                    SinglePos::Format2(table) => table.value_records.iter_mut().for_each(&fill),
                }
            }
        };
        let pairs = |subtables: &mut Vec<write_fonts::OffsetMarker<PairPos>>| {
            for subtable in subtables.iter_mut() {
                match &mut **subtable {
                    PairPos::Format1(table) => {
                        for set in table.pair_sets.iter_mut() {
                            for record in set.pair_value_records.iter_mut() {
                                fill(&mut record.value_record1);
                                fill(&mut record.value_record2);
                            }
                        }
                    }
                    PairPos::Format2(table) => {
                        for first in table.class1_records.iter_mut() {
                            for second in first.class2_records.iter_mut() {
                                fill(&mut second.value_record1);
                                fill(&mut second.value_record2);
                            }
                        }
                    }
                }
            }
        };

        for lookup in owned.lookup_list.lookups.iter_mut() {
            match &mut **lookup {
                PositionLookup::Single(found) => positions(&mut found.subtables),
                PositionLookup::Pair(found) => pairs(&mut found.subtables),
                PositionLookup::Extension(found) => {
                    use write_fonts::tables::gpos::ExtensionSubtable;
                    for subtable in found.subtables.iter_mut() {
                        match &mut **subtable {
                            ExtensionSubtable::Single(extension) => match &mut *extension.extension {
                                SinglePos::Format1(table) => fill(&mut table.value_record),
                                SinglePos::Format2(table) => table.value_records.iter_mut().for_each(&fill),
                            },
                            ExtensionSubtable::Pair(extension) => match &mut *extension.extension {
                                PairPos::Format1(table) => {
                                    for set in table.pair_sets.iter_mut() {
                                        for record in set.pair_value_records.iter_mut() {
                                            fill(&mut record.value_record1);
                                            fill(&mut record.value_record2);
                                        }
                                    }
                                }
                                PairPos::Format2(table) => {
                                    for first in table.class1_records.iter_mut() {
                                        for second in first.class2_records.iter_mut() {
                                            fill(&mut second.value_record1);
                                            fill(&mut second.value_record2);
                                        }
                                    }
                                }
                            },
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        self.font.put(tags::GPOS, &owned);
    }

    pub fn rebase(&mut self, axis: &Axis, retain: bool) {
        let Some(fvar) = self.font.read::<read_fonts::tables::fvar::Fvar>() else {
            return;
        };
        let entries: Vec<(Tag, f64, f64, f64)> = fvar
            .axes()
            .expect("failed to parse fvar axes")
            .iter()
            .map(|entry| (entry.axis_tag(), entry.min_value().to_f64(), entry.default_value().to_f64(), entry.max_value().to_f64()))
            .collect();

        self.explicit();

        let face = harfbuzz::Face::new(&self.font.data());
        let input = harfbuzz::SubsetInput::new();
        input.all_glyphs();
        input.all_layout_features();
        input.all_layout_scripts();
        input.flags(harfbuzz::FLAGS_NOTDEF_OUTLINE | harfbuzz::FLAGS_GLYPH_NAMES | harfbuzz::FLAGS_NO_PRUNE_UNICODE_RANGES | harfbuzz::FLAGS_PASSTHROUGH_UNRECOGNIZED);

        let mut wanted = false;
        for (tag, minimum, default, maximum) in &entries {
            if *tag != Axis::tag() {
                if !retain {
                    input.pin_axis(&face, *tag, *default as f32);
                    wanted = true;
                }
                continue;
            }
            let low = minimum.max(axis.minimum);
            let high = maximum.min(axis.maximum);
            let pivot = minimum.max(axis.default).min(*maximum);
            input.axis_range(&face, *tag, low as f32, high as f32, pivot as f32);
            wanted = true;
        }

        if wanted {
            let data = input.subset(&face).expect("failed to rebase");
            self.font = Font::new(&data);
        }
    }

    pub fn scale(&mut self, upem: u16, factor: f64) {
        let current = self.font.upem();
        let target = (upem as f64 * factor).round_ties_even() as u16;

        if current != target {
            Scaler::new(target as f64 / current as f64).apply(&mut self.font);
        }

        let mut head: write_fonts::tables::head::Head = write_fonts::from_obj::ToOwnedTable::to_owned_table(
            &self.font.read::<read_fonts::tables::head::Head>().expect("missing head"),
        );
        head.units_per_em = upem;
        self.font.put(tags::HEAD, &head);
    }

    pub fn glyphs(&self) -> usize {
        self.font.glyph_count()
    }

    pub fn cmap(&self) -> std::collections::BTreeMap<u32, u16> {
        self.font.cmap()
    }

    pub fn monospace(&mut self, advance: u16) {
        let metrics = self.font.metrics(tags::HHEA, tags::HMTX);
        let count = metrics.len();

        let mut shifts = vec![0i32; count];
        let mut adjusted = Vec::with_capacity(count);
        for (index, metric) in metrics.iter().enumerate() {
            let (cells, shift) = if metric.advance > 0 {
                let cells = ((metric.advance as f64 / advance as f64).round_ties_even() as i32).max(1);
                (cells, (cells * advance as i32 - metric.advance as i32).div_euclid(2))
            } else {
                (1, 0)
            };
            shifts[index] = shift;
            adjusted.push(Metric { advance: (cells * advance as i32) as u16, bearing: (metric.bearing as i32 + shift) as i16 });
        }

        let data = self.font.data();
        let reference = FontRef::new(&data).expect("failed to parse font");
        let glyf = reference.glyf().expect("missing glyf");
        let loca = reference.loca(None).expect("missing loca");

        let mut glyphs = Vec::with_capacity(count);
        for index in 0..count {
            let identifier = GlyphId::new(index as u32);
            let Some(parsed) = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph") else {
                glyphs.push(Vec::new());
                continue;
            };
            let mut glyph = Glyph::from_table_ref(&parsed);
            match &mut glyph {
                Glyph::Composite(composite) => {
                    for component in composite.components_mut() {
                        let inner = component.glyph.to_u32() as usize;
                        if let Anchor::Offset { x, y: _ } = &mut component.anchor {
                            *x = (*x as i32 + shifts[index] - shifts.get(inner).copied().unwrap_or(0)) as i16;
                        }
                    }
                }
                Glyph::Simple(simple) => {
                    if shifts[index] != 0 {
                        for contour in simple.contours.iter_mut() {
                            let moved: Vec<CurvePoint> = contour.iter().map(|point| CurvePoint::new((point.x as i32 + shifts[index]) as i16, point.y, point.on_curve)).collect();
                            *contour = moved.into();
                        }
                        simple.bbox.x_min = (simple.bbox.x_min as i32 + shifts[index]) as i16;
                        simple.bbox.x_max = (simple.bbox.x_max as i32 + shifts[index]) as i16;
                    }
                }
                Glyph::Empty => {}
            }
            glyphs.push(if matches!(glyph, Glyph::Empty) { Vec::new() } else { write_fonts::dump_table(&glyph).expect("failed to serialize glyph") });
        }

        self.font.set_glyphs(&glyphs);
        self.font.set_metrics(tags::HHEA, tags::HMTX, &adjusted);

        self.freeze();
    }

    pub fn freeze(&mut self) {
        if !self.font.contains(tags::GVAR) {
            return;
        }

        let data = self.font.data();
        let reference = FontRef::new(&data).expect("failed to parse font");
        let gvar = reference.gvar().expect("missing gvar");
        let fvar = reference.fvar().expect("missing fvar");
        let axis_count = fvar.axis_count();

        let glyf = reference.glyf().expect("missing glyf");
        let loca = reference.loca(None).expect("missing loca");
        let horizontal = self.font.metrics(tags::HHEA, tags::HMTX);
        let vertical = if self.font.contains(tags::VMTX) { Some(self.font.metrics(tags::VHEA, tags::VMTX)) } else { None };

        let count = self.font.glyph_count();
        let mut rebuilt = Vec::with_capacity(count);
        for index in 0..count {
            let identifier = GlyphId::new(index as u32);
            let variations = match gvar.glyph_variation_data(identifier) {
                Ok(Some(found)) => found,
                _ => {
                    rebuilt.push(GlyphVariations::new(identifier, Vec::new()));
                    continue;
                }
            };

            let glyph = loca.get_glyf(identifier, &glyf).expect("failed to parse glyph");
            let geometry = crate::font::Points::of(glyph.as_ref(), &horizontal[index], vertical.as_ref().map(|found| &found[index]));
            let total = geometry.coordinates.len();

            let mut tuples = Vec::new();
            for tuple in variations.tuples() {
                let tents: Vec<Tent> = {
                    let peaks: Vec<F2Dot14> = tuple.peak().values().iter().map(|value| value.get()).collect();
                    match (tuple.intermediate_start(), tuple.intermediate_end()) {
                        (Some(start), Some(end)) => peaks
                            .iter()
                            .zip(start.values().iter().zip(end.values()))
                            .map(|(peak, (low, high))| Tent::new(*peak, Some((low.get(), high.get()))))
                            .collect(),
                        _ => peaks.iter().map(|peak| Tent::new(*peak, None)).collect(),
                    }
                };

                let mut deltas: Vec<Option<kurbo::Vec2>> = vec![None; total];
                if tuple.has_deltas_for_all_points() {
                    for (position, delta) in tuple.deltas().enumerate() {
                        if position < total {
                            deltas[position] = Some(kurbo::Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                        }
                    }
                } else {
                    for delta in tuple.deltas() {
                        let position = delta.position as usize;
                        if position < total {
                            deltas[position] = Some(kurbo::Vec2::new(delta.x_delta as f64, delta.y_delta as f64));
                        }
                    }
                }

                for position in [total.saturating_sub(4), total.saturating_sub(3)] {
                    if position < total {
                        deltas[position] = Some(kurbo::Vec2::ZERO);
                    }
                }

                let dense = crate::design::iup_delta(&deltas, &geometry.coordinates, &geometry.ends);
                let flattened: Vec<GlyphDelta> = deltas
                    .iter()
                    .zip(&dense)
                    .map(|(delta, inferred)| match delta {
                        Some(value) => GlyphDelta::required(value.x as i16, value.y as i16),
                        None => GlyphDelta::optional(inferred.x as i16, inferred.y as i16),
                    })
                    .collect();
                tuples.push(GlyphDeltas::new(tents, flattened));
            }

            rebuilt.push(GlyphVariations::new(identifier, tuples));
        }

        let table = Gvar::new(rebuilt, axis_count).expect("failed to build gvar");
        self.font.put(tags::GVAR, &table);

        self.font.remove(tags::HVAR);
    }
}
