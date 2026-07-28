use std::collections::BTreeMap;

use rayon::prelude::*;
use read_fonts::FontRead;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::gsub::{Gsub, SingleSubst, SingleSubstFormat2, SubstitutionLookup, SubstitutionLookupList};
use write_fonts::tables::layout::{
    ConditionFormat1, ConditionSet, CoverageTable, Feature, FeatureList, FeatureRecord, FeatureTableSubstitution, FeatureTableSubstitutionRecord,
    FeatureVariationRecord, FeatureVariations, LangSys, Lookup, Script, ScriptList, ScriptRecord,
};
use write_fonts::types::{F2Dot14, GlyphId16, Tag};

use crate::constants::{version, Paths};
use crate::design::{Axis, Space};
use crate::font::{capacity, tags, Font};
use crate::harfbuzz;
use crate::merge::Merger;
use crate::metrics::Metrics;
use crate::models::{Family, Format, Slope, Style, Typeface, Weight};
use crate::naming::{Names, Notice};
use crate::outlines::Outlines;
use crate::prepare::{Component, Features};
use crate::statics;

#[allow(non_upper_case_globals)]
pub const emphasis: f64 = 600.0;

pub struct Builder {
    pub family: Family,
    pub formats: Vec<Format>,
    pub directory: String,
    pub axis: Axis,
    pub notice: String,
}

impl Builder {
    pub fn new(family: Family, formats: Option<Vec<Format>>, directory: Option<&str>) -> Builder {
        Builder {
            family,
            formats: formats.unwrap_or_else(|| Format::all().to_vec()),
            directory: directory.unwrap_or(Paths::files).to_string(),
            axis: Axis::new(100.0, 400.0, 900.0),
            notice: String::new(),
        }
    }

    pub fn note(&self, message: &str) {
        println!("{}", message);
    }

    pub fn build(&mut self) -> Vec<String> {
        std::fs::create_dir_all(&self.directory).expect("failed to create output directory");

        let fonts: Vec<Font> = self.family.sources().iter().map(|source| Font::new(&source.read())).collect();
        let references: Vec<&Font> = fonts.iter().collect();
        self.axis = Axis::of(&references, 400.0);
        self.notice = Notice::of(&references);
        drop(fonts);

        Slope::all().into_par_iter().map(|slope| self.compile(slope)).flatten().collect()
    }

    pub fn compile(&self, slope: Slope) -> Vec<String> {
        self.note(&format!("{}: composing {}", self.family.name, format!("{:?}", slope).to_lowercase()));
        let (mut font, metrics, advance, space, substitutions) = self.compose(slope);

        let style = Style { weight: None, slope };
        self.finish(&mut font, &style, &metrics, advance, &space, &substitutions);
        font.finalize();
        let data = font.data();
        drop(font);

        let (_, statics) = rayon::join(
            || self.write(&style, &data),
            || {
                [Weight::Regular, Weight::Bold]
                    .into_par_iter()
                    .map(|weight| {
                        let style = Style { weight: Some(weight), slope };
                        self.note(&format!("{}: instancing {}", self.family.name, style.name()));
                        let mut fixed = self.instance(&data, weight.value() as f64);
                        self.finish(&mut fixed, &style, &metrics, advance, &space, &substitutions);
                        fixed.finalize();
                        (style, fixed.data())
                    })
                    .collect::<Vec<(Style, Vec<u8>)>>()
            },
        );

        statics.par_iter().for_each(|(style, data)| self.write(style, data));

        let mut written = self.paths(&style);
        for (style, _) in &statics {
            written.extend(self.paths(style));
        }
        written
    }

    pub fn instance(&self, data: &[u8], weight: f64) -> Font {
        let font = Font::new(data);
        let remaining = match font.read::<read_fonts::tables::fvar::Fvar>() {
            Some(fvar) => fvar.axes().expect("failed to parse fvar axes").iter().any(|entry| entry.axis_tag() != Axis::tag()),
            None => false,
        };

        if remaining {
            let mut font = font;
            statics::Pin::new(weight).apply(&mut font);
            font
        } else {
            Font::new(&Builder::pinned(data, &[(Axis::tag(), weight)]))
        }
    }

    pub fn pinned(data: &[u8], locations: &[(Tag, f64)]) -> Vec<u8> {
        let face = harfbuzz::Face::new(data);
        let input = harfbuzz::SubsetInput::new();
        input.all_glyphs();
        input.all_layout_features();
        input.all_layout_scripts();
        input.flags(harfbuzz::FLAGS_NOTDEF_OUTLINE | harfbuzz::FLAGS_GLYPH_NAMES | harfbuzz::FLAGS_NO_PRUNE_UNICODE_RANGES | harfbuzz::FLAGS_PASSTHROUGH_UNRECOGNIZED);
        for (tag, value) in locations {
            input.pin_axis(&face, *tag, *value as f32);
        }
        input.subset(&face).expect("failed to instance")
    }

    pub fn compose(&self, slope: Slope) -> (Font, Metrics, Option<u16>, Space, BTreeMap<u16, u16>) {
        let family = &self.family;

        let mut base = self.component(&family.latin, slope, Weight::Regular);
        let upem = base.font.upem();
        base.subset();
        base.rebase(&self.axis, true);
        base.scale(upem, 1.0);

        let advance = if family.monospace { Some(self.cell(&base)) } else { None };
        if let Some(advance) = advance {
            base.monospace(advance);
        }

        let mut claimed = base.codepoints.clone();
        let mut addons: Vec<Component> = Vec::new();
        let mut reference: Option<usize> = None;

        for typeface in &family.cjk {
            let mut component = self.component(typeface, slope, Weight::Regular);
            component.codepoints = component.codepoints.difference(&claimed).copied().collect();
            component.prepare(&self.axis, upem, 1.0, false);
            if let Some(advance) = advance {
                component.monospace(advance);
            }
            claimed.extend(component.codepoints.iter().copied());
            if reference.is_none() {
                reference = Some(addons.len());
            }
            addons.push(component);
        }

        if let Some(symbols) = &family.symbols {
            let mut component = self.component(symbols, slope, Weight::Regular);
            component.codepoints = component.codepoints.difference(&claimed).copied().collect();
            let ratio = self.ratio(&component, upem, advance);
            component.prepare(&self.axis, upem, ratio, false);
            if let Some(advance) = advance {
                component.monospace(advance);
            }
            claimed.extend(component.codepoints.iter().copied());
            addons.push(component);
        }

        let mut bold_cmaps: Option<(BTreeMap<u32, u16>, usize)> = None;
        if !family.latin.variable() {
            let mut bold = self.component(&family.latin, slope, Weight::Bold);
            bold.prepare(&self.axis, upem, 1.0, false);
            if let Some(advance) = advance {
                bold.monospace(advance);
            }
            let offset = base.font.glyph_count() + addons.iter().map(|addon| addon.font.glyph_count() - 1).sum::<usize>();
            bold_cmaps = Some((bold.cmap(), offset));
            addons.push(bold);
        }

        let mut components: Vec<&Component> = vec![&base];
        components.extend(addons.iter());
        let metrics = Metrics::of(&components, slope.italic());

        let substitutions = match &bold_cmaps {
            None => BTreeMap::new(),
            Some((heavy, offset)) => {
                let mut found = BTreeMap::new();
                for (code, glyph) in base.cmap() {
                    if let Some(target) = heavy.get(&code) {
                        if *target != 0 {
                            found.insert(glyph, target + *offset as u16 - 1);
                        }
                    }
                }
                found
            }
        };

        let space = self.space(reference.map(|index| &addons[index]).unwrap_or(&base));
        let masters = self.masters(&components, &space);
        self.note(&format!(
            "{}: {} masters at {}",
            self.family.name,
            masters.len(),
            masters.iter().map(|weight| (*weight as i64).to_string()).collect::<Vec<String>>().join(", ")
        ));
        drop(components);

        base.retarget(&space, &masters);
        for addon in &mut addons {
            addon.retarget(&space, &masters);
        }

        let total = base.font.glyph_count() + addons.iter().map(|addon| addon.font.glyph_count() - 1).sum::<usize>();
        if total > capacity {
            panic!("{} needs {} glyphs, more than the {} a font can hold", self.family.name, total, capacity);
        }

        let merger = Merger::new(base, addons, Space::new(Axis::new(space.axis.minimum, space.axis.default, space.axis.maximum), Some(space.segments.clone())), true);
        let mut font = merger.build();

        if !substitutions.is_empty() {
            self.emphasise(&mut font, &space, &substitutions);
        }

        (font, metrics, advance, space, substitutions)
    }

    pub fn space(&self, reference: &Component) -> Space {
        let found = reference.space();
        Space::new(
            Axis::new(self.axis.minimum, self.axis.default, self.axis.maximum),
            found.map(|space| space.segments),
        )
    }

    pub fn masters(&self, components: &[&Component], space: &Space) -> Vec<f64> {
        let mut found: Vec<f64> = space.breakpoints();
        for component in components {
            found.extend(component.breakpoints(space));
        }

        let mut rounded: Vec<f64> = found
            .into_iter()
            .map(|weight| weight.round_ties_even())
            .filter(|weight| self.axis.minimum <= *weight && *weight <= self.axis.maximum)
            .collect();
        rounded.push(self.axis.default);
        rounded.sort_by(f64::total_cmp);
        rounded.dedup();
        rounded
    }

    pub fn component(&self, typeface: &Typeface, slope: Slope, weight: Weight) -> Component {
        let source = typeface.source(slope, Some(weight));
        Component::load(&source.read(), &typeface.name, Some(self.features(typeface)))
    }

    pub fn features(&self, typeface: &Typeface) -> Vec<String> {
        let default = if std::ptr::eq(typeface, &self.family.latin) {
            Features::latin()
        } else if self.family.symbols.as_ref().map(|symbols| std::ptr::eq(typeface, symbols)).unwrap_or(false) {
            Features::symbols()
        } else {
            Features::cjk()
        };

        if !self.family.monospace {
            return default;
        }

        let source = if default == Features::latin() { Features::cjk() } else { default };
        let dropped: Vec<&str> = Features::proportional.iter().chain(Features::ligating.iter()).copied().collect();
        source.into_iter().filter(|tag| !dropped.contains(&tag.as_str())).collect()
    }

    pub fn cell(&self, base: &Component) -> u16 {
        Builder::common(&base.font)
    }

    pub fn common(font: &Font) -> u16 {
        let metrics = font.metrics(tags::HHEA, tags::HMTX);
        let mut counts: BTreeMap<u16, (usize, usize)> = BTreeMap::new();
        for (index, metric) in metrics.iter().enumerate() {
            if metric.advance > 0 {
                let entry = counts.entry(metric.advance).or_insert((0, index));
                entry.0 += 1;
            }
        }
        counts
            .iter()
            .max_by(|a, b| a.1 .0.cmp(&b.1 .0).then(b.1 .1.cmp(&a.1 .1)))
            .map(|(advance, _)| *advance)
            .expect("no advance widths")
    }

    pub fn ratio(&self, component: &Component, upem: u16, advance: Option<u16>) -> f64 {
        let Some(advance) = advance else {
            return 1.0;
        };
        let common = Builder::common(&component.font);
        advance as f64 * component.font.upem() as f64 / (common as f64 * upem as f64)
    }

    pub fn emphasise(&self, font: &mut Font, space: &Space, substitutions: &BTreeMap<u16, u16>) {
        let mut table: Gsub = match font.get(tags::GSUB) {
            Some(data) => read_fonts::tables::gsub::Gsub::read(read_fonts::FontData::new(data)).expect("failed to parse GSUB").to_owned_table(),
            None => {
                let script = Script::new(Some(LangSys::new(Vec::new())), Vec::new());
                let scripts = ScriptList::new(vec![ScriptRecord::new(Tag::new(b"DFLT"), script)]);
                Gsub::new(scripts, FeatureList::new(Vec::new()), SubstitutionLookupList::new(Vec::new()))
            }
        };

        let coverage: CoverageTable = substitutions.keys().map(|glyph| GlyphId16::new(*glyph)).collect();
        let substitutes: Vec<GlyphId16> = substitutions.values().map(|glyph| GlyphId16::new(*glyph)).collect();
        let single = SingleSubst::Format2(SingleSubstFormat2::new(coverage, substitutes));
        table.lookup_list.lookups.push(write_fonts::OffsetMarker::new(SubstitutionLookup::Single(Lookup::new(Default::default(), vec![single]))));
        let added = (table.lookup_list.lookups.len() - 1) as u16;

        let mut indices: Vec<u16> = table
            .feature_list
            .feature_records
            .iter()
            .enumerate()
            .filter(|(_, record)| record.feature_tag == Tag::new(b"rvrn"))
            .map(|(index, _)| index as u16)
            .collect();

        if indices.is_empty() {
            table.feature_list.feature_records.push(FeatureRecord::new(Tag::new(b"rvrn"), Feature::new(None, Vec::new())));

            let mut order: Vec<usize> = (0..table.feature_list.feature_records.len()).collect();
            order.sort_by_key(|index| table.feature_list.feature_records[*index].feature_tag);
            let mut mapping = vec![0u16; order.len()];
            for (new, old) in order.iter().enumerate() {
                mapping[*old] = new as u16;
            }
            let records = std::mem::take(&mut table.feature_list.feature_records);
            let mut sorted: Vec<Option<FeatureRecord>> = records.into_iter().map(Some).collect();
            table.feature_list.feature_records = order.iter().map(|old| sorted[*old].take().unwrap()).collect();
            crate::layout::Visit::features(&mut table, &mut |index: &mut u16| *index = mapping[*index as usize]);

            let position = table
                .feature_list
                .feature_records
                .iter()
                .position(|record| record.feature_tag == Tag::new(b"rvrn"))
                .expect("rvrn feature missing") as u16;
            for record in table.script_list.script_records.iter_mut() {
                if record.script.default_lang_sys.as_ref().is_none() {
                    record.script.default_lang_sys = Some(LangSys::new(Vec::new())).into();
                }
                if let Some(language) = record.script.default_lang_sys.as_mut() {
                    language.feature_indices.push(position);
                }
                for language in record.script.lang_sys_records.iter_mut() {
                    language.lang_sys.feature_indices.push(position);
                }
            }
            indices.push(position);
        }

        let axis = {
            let fvar = font.read::<read_fonts::tables::fvar::Fvar>().expect("missing fvar");
            fvar.axes()
                .expect("failed to parse fvar axes")
                .iter()
                .position(|entry| entry.axis_tag() == Axis::tag())
                .expect("missing weight axis") as u16
        };

        let condition = ConditionFormat1::new(axis, F2Dot14::from_f32(space.normalize(emphasis) as f32), F2Dot14::from_f32(1.0));
        let mut records = Vec::new();
        for index in &indices {
            let mut lookups = vec![added];
            lookups.extend(table.feature_list.feature_records[*index as usize].feature.lookup_list_indices.iter().copied());
            records.push(FeatureTableSubstitutionRecord::new(*index, Feature::new(None, lookups)));
        }

        let variations = FeatureVariations::new(vec![FeatureVariationRecord::new(
            Some(ConditionSet::new(vec![condition.into()])),
            Some(FeatureTableSubstitution::new(records)),
        )]);
        table.feature_variations = Some(variations).into();

        font.put(tags::GSUB, &table);
    }

    pub fn finish(&self, font: &mut Font, style: &Style, metrics: &Metrics, advance: Option<u16>, space: &Space, substitutions: &BTreeMap<u16, u16>) {
        if !style.variable() {
            self.flatten(font, style, space, substitutions);
        }

        metrics.apply(font, &self.family, style, version.parse().expect("version is numeric"), advance);
        Names::new(&self.family, style, &self.axis, version, &self.notice).apply(font);

        if font.contains(tags::FVAR) && font.contains(tags::GVAR) {
            statics::hvar(font);
        }
    }

    pub fn flatten(&self, font: &mut Font, style: &Style, space: &Space, substitutions: &BTreeMap<u16, u16>) {
        if substitutions.is_empty() {
            return;
        }
        if space.normalize(style.value()) < space.normalize(emphasis) {
            return;
        }

        let assignments: BTreeMap<u32, u16> = font
            .cmap()
            .into_iter()
            .map(|(code, glyph)| (code, substitutions.get(&glyph).copied().unwrap_or(glyph)))
            .collect();

        font.set(tags::CMAP, crate::font::charmap(&assignments));
    }

    pub fn path(&self, style: &Style, format: Format) -> String {
        format!("{}/{}-{}.{}", self.directory, self.family.filename, style.name(), format.extension())
    }

    pub fn paths(&self, style: &Style) -> Vec<String> {
        self.formats.iter().map(|format| self.path(style, *format)).collect()
    }

    pub fn write(&self, style: &Style, data: &[u8]) {
        self.formats.par_iter().for_each(|format| {
            let path = self.path(style, *format);
            match format {
                Format::Ttf => std::fs::write(&path, data).expect("failed to write file"),
                Format::Otf => std::fs::write(&path, self.compact(data, style)).expect("failed to write file"),
                Format::Woff => std::fs::write(&path, Builder::woff(data)).expect("failed to write file"),
                Format::Woff2 => std::fs::write(&path, Builder::woff2(data)).expect("failed to write file"),
            }
            self.note(&format!("{}: wrote {} {}", self.family.name, style.name(), format.directory()));
        });
    }

    pub fn compact(&self, data: &[u8], style: &Style) -> Vec<u8> {
        if style.variable() {
            return data.to_vec();
        }

        let mut font = Font::new(data);
        if let Some(fvar) = font.read::<read_fonts::tables::fvar::Fvar>() {
            let locations: Vec<(Tag, f64)> = fvar
                .axes()
                .expect("failed to parse fvar axes")
                .iter()
                .map(|entry| (entry.axis_tag(), entry.default_value().to_f64()))
                .collect();
            font = Font::new(&Builder::pinned(data, &locations));
        }

        Outlines::compact(&mut font, &self.family, style, version);
        font.data()
    }

    pub fn checksum(data: &[u8]) -> u32 {
        let mut total: u32 = 0;
        for chunk in data.chunks(4) {
            let mut word = [0u8; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            total = total.wrapping_add(u32::from_be_bytes(word));
        }
        total
    }

    pub fn woff(data: &[u8]) -> Vec<u8> {
        let font = Font::new(data);
        let count = font.tables.len() as u16;

        let tables: Vec<(&Tag, &Vec<u8>)> = font.tables.iter().collect();
        let compressed: Vec<Vec<u8>> = tables
            .par_iter()
            .map(|(_, table)| {
                let mut compressed = Vec::new();
                let options = zopfli::Options::default();
                zopfli::compress(options, zopfli::Format::Zlib, &table[..], &mut compressed).expect("failed to compress table");
                if compressed.len() < table.len() {
                    compressed
                } else {
                    (*table).clone()
                }
            })
            .collect();

        let mut directory = Vec::new();
        let mut body = Vec::new();
        let mut sfnt_size = 12u32 + 16 * count as u32;

        for ((tag, table), stored) in tables.iter().zip(compressed) {
            directory.push((**tag, 44 + 20 * count as u32 + body.len() as u32, stored.len() as u32, table.len() as u32, Builder::checksum(table)));
            body.extend_from_slice(&stored);
            while body.len() % 4 != 0 {
                body.push(0);
            }
            sfnt_size += (table.len() as u32 + 3) & !3;
        }

        let mut output = Vec::new();
        output.extend_from_slice(b"wOFF");
        output.extend_from_slice(&0x00010000u32.to_be_bytes());
        output.extend_from_slice(&(44 + 20 * count as u32 + body.len() as u32).to_be_bytes());
        output.extend_from_slice(&count.to_be_bytes());
        output.extend_from_slice(&0u16.to_be_bytes());
        output.extend_from_slice(&sfnt_size.to_be_bytes());
        output.extend_from_slice(&0u16.to_be_bytes());
        output.extend_from_slice(&0u16.to_be_bytes());
        output.extend_from_slice(&0u32.to_be_bytes());
        output.extend_from_slice(&0u32.to_be_bytes());
        output.extend_from_slice(&0u32.to_be_bytes());
        output.extend_from_slice(&0u32.to_be_bytes());
        output.extend_from_slice(&0u32.to_be_bytes());

        for (tag, offset, stored, original, checksum) in &directory {
            output.extend_from_slice(&tag.to_be_bytes());
            output.extend_from_slice(&offset.to_be_bytes());
            output.extend_from_slice(&stored.to_be_bytes());
            output.extend_from_slice(&original.to_be_bytes());
            output.extend_from_slice(&checksum.to_be_bytes());
        }
        output.extend_from_slice(&body);
        output
    }

    pub fn woff2(data: &[u8]) -> Vec<u8> {
        ttf2woff2::encode(data, ttf2woff2::BrotliQuality::from(11u8)).expect("failed to compress woff2")
    }
}
